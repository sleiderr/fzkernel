//! x86 Paging implementation.
//!
//! It takes care of mapping linear addresses to physical memory or I/O devices.
//! The x86 architecture supports various paging modes (_32-bit paging_, _PAE paging_, _4-level paging_,
//! _5-level paging_), that can be used in different contexts.

use crate::errors::BaseError;
use crate::mem::{MemoryAddress, PhyAddr, VirtAddr};

pub mod page_alloc;
pub mod page_table;

use conquer_once::spin::OnceCell;
use page_table::mapper::{PageTableMapper, PhysicalMemoryMapping};
use page_table::translate::PageAddressTranslator;
pub use page_table::{PageTable, PageTableFlags};
use spin::Mutex;

use super::msr::{Ia32ExtendedFeature, ModelSpecificRegister};
use super::registers::control::{ControlRegister, Cr0, Cr3, Cr4};

static VIRT_MEMORY_MAPPER: OnceCell<
    Mutex<PageTableMapper<PageAddressTranslator, PhysicalMemoryMapping>>,
> = OnceCell::uninit();

#[cfg(feature = "x86_64")]
pub unsafe fn init_global_mapper(page_table_address: PhyAddr) {
    use crate::kernel_syms::{KERNEL_CODE_MAPPING_BASE, KERNEL_PHYS_MAPPING_BASE};

    VIRT_MEMORY_MAPPER.init_once(|| {
        Mutex::new(PageTableMapper::new_from_raw(
            page_table_address.as_mut_ptr(),
            PhysicalMemoryMapping::KERNEL_DEFAULT_MAPPING,
        ))
    });

    // TODO: to be removed when better handling of stack ptr
    VIRT_MEMORY_MAPPER
        .get_unchecked()
        .lock()
        .map_physical_memory(
            PhyAddr::new(0x0),
            VirtAddr::NULL_PTR,
            PageTableFlags::new().with_write(true),
            0x400_000,
        );

    VIRT_MEMORY_MAPPER
        .get_unchecked()
        .lock()
        .map_physical_memory(
            PhyAddr::new(0x0),
            KERNEL_PHYS_MAPPING_BASE,
            PageTableFlags::new().with_write(true),
            0x200_000_000,
        );

    VIRT_MEMORY_MAPPER
        .get_unchecked()
        .lock()
        .map_physical_memory(
            PhyAddr::new(0x800_000),
            KERNEL_CODE_MAPPING_BASE,
            PageTableFlags::new().with_write(true),
            0x400_000,
        );

    Cr3::write(Cr3::new().set_page_table_addr(page_table_address).unwrap());
}

pub fn get_memory_mapper(
) -> &'static Mutex<PageTableMapper<PageAddressTranslator, PhysicalMemoryMapping>> {
    if let Some(mapper) = VIRT_MEMORY_MAPPER.get() {
        return mapper;
    } else {
        panic!("attempt to access virtual memory mappings before initialization")
    }
}

/// Represents a memory (or virtual) page.
///
/// It is a block of contiguous virtual memory, that is described and mapped to physical memory (through a _Page Frame_)
/// in the `Page Table`.
///
/// Multiple page size may be available depending on the `CPU`. The smallest page size available is `4 KB`, but can go
/// up to `1 GB` if the CPU supports such large pages.
#[derive(Clone, Copy, Debug)]
pub struct Page {
    pub(super) start: VirtAddr,
}

impl Page {
    pub(crate) fn new(start: VirtAddr) -> Self {
        Self { start }
    }
}

/// Represents a physical memory page.
///
/// It is a block of contiguous physical memory, that may be mapped to one or more virtual [`Page`] through paging.
///
/// The size of a `Frame` matches the one of the virtual [`Page`] it may be linked to, and therefore multiple frame
/// size may be available with proper CPU support.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    addr: PhyAddr,
}

impl Frame {
    pub(crate) fn new(start: PhyAddr) -> Self {
        Self { addr: start }
    }
}

#[cfg(not(feature = "x86_64"))]
/// Routines to enable paging at the pre-kernel init stage.
pub mod bootinit_paging {
    use crate::mem::{MemoryAddress, PhyAddr, PhyAddr32, VirtAddr};
    use crate::x86::int::disable_interrupts;
    use crate::x86::msr::{Ia32ExtendedFeature, ModelSpecificRegister};
    use crate::x86::paging::page_table::translate::{PageAddressTranslator, Translator};
    use crate::x86::paging::{PageTable, PageTableFlags};
    use crate::x86::registers::control::{ControlRegister, Cr0, Cr3, Cr4};
    use alloc::boxed::Box;

    /// Physical address of the layer 4 [`PageTable`] structure.
    pub const BOOT_PAGE_TABLE_ADDR: PhyAddr32 = PhyAddr32::new(0x20_000);

    /// Base virtual address for the physical memory mapping.
    pub const KERNEL_PHYS_MAPPING_BASE: VirtAddr = VirtAddr::new(0xFFFF_CF80_0000_0000);

    /// Base virtual address for the kernel code (`.text`) section.
    pub const KERNEL_CODE_MAPPING_BASE: VirtAddr = VirtAddr::new(0xFFFF_8C00_0000_0000);

    /// Pre-kernel load initialization of paging.
    ///
    /// Enables 64-bit level 4 paging if supported.
    /// Identity maps the physical memory, and also maps it to the virtual segment starting at [`KERNEL_PHYS_MAPPING_BASE`].
    /// Disables interrupts (the `IDT` has to be updated to support 64-bit).
    #[allow(clippy::missing_panics_doc)]
    pub fn init_paging() {
        identity_map_phys_level4(0, PhyAddr::new(0));
        identity_map_phys_level4(
            PageAddressTranslator::translate_address(KERNEL_PHYS_MAPPING_BASE).pml4_offset(),
            PhyAddr::new(0),
        );
        identity_map_phys_level4(
            PageAddressTranslator::translate_address(KERNEL_CODE_MAPPING_BASE).pml4_offset(),
            PhyAddr::new(0x800_000),
        );
        Cr3::write(
            Cr3::new()
                .set_page_table_addr(BOOT_PAGE_TABLE_ADDR)
                .unwrap(),
        );
        disable_interrupts();
        Cr4::write(Cr4::read().with_phys_addr_ext(true));
        Ia32ExtendedFeature::write(Ia32ExtendedFeature::read().unwrap().with_ia32e_enable(true));
        Cr0::write(Cr0::read().with_paging(true));
    }

    fn identity_map_phys_level4(entry_offset: u16, phy_offset: PhyAddr) {
        let default_page_table: &mut PageTable = unsafe { &mut *BOOT_PAGE_TABLE_ADDR.as_mut_ptr() };
        let pdpt: &mut PageTable = Box::leak(Box::default());

        default_page_table
            .get_mut(entry_offset)
            .map_to_addr(
                PhyAddr::new(
                    u64::try_from((pdpt as *mut PageTable).addr())
                        .expect("invalid pagetable address"),
                ),
                PageTableFlags::new().with_present(true).with_write(true),
            )
            .expect("failed to create level 3 pagetable");

        for i in 0..8 {
            let table_entry: &mut PageTable = Box::leak(Box::default());
            pdpt.get_mut(i)
                .map_to_addr(
                    PhyAddr::new(table_entry as *mut PageTable as u64),
                    PageTableFlags::new().with_present(true).with_write(true),
                )
                .expect("failed to create pagetable");
            for j in 0..512 {
                table_entry
                    .get_mut(j)
                    .map_to_addr(
                        phy_offset
                            + PhyAddr::new(
                                4096 * 512 * 512 * u64::from(i) + 4096 * 512 * u64::from(j),
                            ),
                        PageTableFlags::new()
                            .with_present(true)
                            .with_write(true)
                            .with_huge_page(true),
                    )
                    .expect("failed to create pagetable")
            }
        }
    }
}

/// Represents an error raised while attempting to map a [`Page`] to a [`Frame`].
#[derive(Clone, Copy, Debug)]
pub enum PageMappingError {
    /// The [`Page`] is not properly aligned.
    BadAlignment,
}

impl BaseError for PageMappingError {}

/// Represents an error raised while attempting to create a [`PageTable`] entry.
#[derive(Clone, Copy, Debug)]
pub enum PageTableCreationError {
    /// The frame allocator failed to allocate physical memory for the entry.
    AllocationError,

    /// The entry already exists, and the `huge page` flag is set.
    HugePage,
}

impl BaseError for PageTableCreationError {}
