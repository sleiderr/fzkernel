use crate::mem::{MemoryAddress, PhyAddr, VirtAddr};
use crate::x86::paging::page_table::translate::Translator;
use crate::x86::paging::page_table::{PageTable, PageTableEntry, PageTableFlags};
use crate::x86::paging::{Frame, Page, PageTableCreationError};
use alloc::boxed::Box;
use core::marker::PhantomData;
use core::mem::ManuallyDrop;
use core::ptr;

#[derive(Debug)]
pub struct PhysicalMemoryMapping {
    offset: VirtAddr,
}

#[allow(overflowing_literals)]
impl PhysicalMemoryMapping {
    pub(crate) const DEFAULT_OFFSET: VirtAddr = VirtAddr::new(0xffff_1000_0000_0000);
    pub(crate) const DEFAULT_MAX_SIZE: usize = 0x0100_0000_0000;

    pub(crate) const IDENTITY: Self = Self {
        offset: VirtAddr::new(0),
    };

    pub fn new(offset: VirtAddr) -> Self {
        Self { offset }
    }
}

impl MemoryMapping for PhysicalMemoryMapping {
    fn convert(&self, phys: PhyAddr) -> VirtAddr {
        self.offset + VirtAddr::new(u64::from(phys))
    }
}

pub trait MemoryMapping {
    fn convert(&self, phys: PhyAddr) -> VirtAddr;
}

pub(crate) struct PageTableMapper<T: Translator, M: MemoryMapping> {
    pml4: ManuallyDrop<Box<PageTable>>,
    phys_mapping: M,
    translator: PhantomData<T>,
}

impl<T: Translator, M: MemoryMapping> PageTableMapper<T, M> {
    pub(crate) unsafe fn new_from_raw(table_ptr: *mut PageTable, mapping: M) -> Self {
        Self {
            pml4: ManuallyDrop::new(Box::from_raw(table_ptr)),
            phys_mapping: mapping,
            translator: PhantomData,
        }
    }

    fn map_1gb_page(
        &mut self,
        page: Page,
        frame: Frame,
        flags: PageTableFlags,
        parent_flags: PageTableFlags,
    ) -> Result<(), PageTableMappingError> {
        let translated_addr = T::translate_address(page.start);
        let pml4 = &mut self.pml4;
        let pdpte =
            Self::get_or_create_entry(pml4.get_mut(translated_addr.pml4_offset()), parent_flags)
                .map_err(|_| PageTableMappingError::TableCreationError)?;

        if pdpte.get_mut(translated_addr.pdpte_offset()).used() {
            return Err(PageTableMappingError::AlreadyMapped);
        }
        pdpte
            .get_mut(translated_addr.pdpte_offset())
            .map_to_frame(frame, flags | PageTableFlags::new().with_huge_page(true));

        Ok(())
    }

    pub(crate) fn map_2mb_page(
        &mut self,
        page: Page,
        frame: Frame,
        flags: PageTableFlags,
        parent_flags: PageTableFlags,
    ) -> Result<(), PageTableMappingError> {
        let translated_addr = T::translate_address(page.start);
        let pml4 = &mut self.pml4;
        let pdpte =
            Self::get_or_create_entry(pml4.get_mut(translated_addr.pml4_offset()), parent_flags)
                .map_err(|_| PageTableMappingError::TableCreationError)?;
        let pde =
            Self::get_or_create_entry(pdpte.get_mut(translated_addr.pdpte_offset()), parent_flags)
                .map_err(|_| PageTableMappingError::TableCreationError)?;

        if pde.get_mut(translated_addr.pde_offset()).used() {
            return Err(PageTableMappingError::AlreadyMapped);
        }
        pde.get_mut(translated_addr.pde_offset())
            .map_to_frame(frame, flags | PageTableFlags::new().with_huge_page(true));

        Ok(())
    }

    pub(crate) fn map_4kb_page(
        &mut self,
        page: Page,
        frame: Frame,
        flags: PageTableFlags,
        parent_flags: PageTableFlags,
    ) -> Result<(), PageTableMappingError> {
        let translated_addr = T::translate_address(page.start);
        let pml4 = &mut self.pml4;
        let pdpte =
            Self::get_or_create_entry(pml4.get_mut(translated_addr.pml4_offset()), parent_flags)
                .map_err(|_| PageTableMappingError::TableCreationError)?;
        let pde =
            Self::get_or_create_entry(pdpte.get_mut(translated_addr.pdpte_offset()), parent_flags)
                .map_err(|_| PageTableMappingError::TableCreationError)?;
        let pte =
            Self::get_or_create_entry(pde.get_mut(translated_addr.pde_offset()), parent_flags)
                .map_err(|_| PageTableMappingError::TableCreationError)?;

        if pte.get_mut(translated_addr.pte_offset()).used() {
            return Err(PageTableMappingError::AlreadyMapped);
        }
        pte.get_mut(translated_addr.pte_offset())
            .map_to_frame(frame, flags);

        Ok(())
    }

    fn get_or_create_entry(
        entry: &mut PageTableEntry,
        flags: PageTableFlags,
    ) -> Result<&mut PageTable, PageTableCreationError> {
        if !entry.used() {
            let table = Box::new(PageTable::default());
            let table_ptr = ptr::from_mut(Box::leak(table));
            entry.map_to_addr(PhyAddr::from(table_ptr), flags);
        } else {
            entry.set_flags(flags);
        }

        if entry.flags().huge_page() {
            return Err(PageTableCreationError::HugePage);
        }

        PageTableMapper::<T, M>::get_next_table(entry).map_err(|_| PageTableCreationError::HugePage)
    }

    fn get_next_table(entry: &mut PageTableEntry) -> Result<&mut PageTable, NextPageTableFailed> {
        let table_ptr = entry.frame().addr.as_mut_ptr();

        Ok(unsafe { &mut *table_ptr })
    }
}

#[derive(Debug)]
pub enum NextPageTableFailed {}

#[derive(Debug)]
pub enum PageTableMappingError {
    AlreadyMapped,
    TableCreationError,
}
