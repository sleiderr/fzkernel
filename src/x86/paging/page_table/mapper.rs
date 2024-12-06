use crate::kernel_syms::PAGE_SIZE;
use crate::mem::{MemoryAddress, PhyAddr, VirtAddr};
use crate::x86::paging::page_alloc::frame_alloc::alloc_page;
use crate::x86::paging::page_table::translate::Translator;
use crate::x86::paging::page_table::{PageTable, PageTableEntry, PageTableFlags};
use crate::x86::paging::{Frame, Page, PageTableCreationError};
use alloc::boxed::Box;
use core::arch::asm;
use core::marker::PhantomData;
use core::mem::ManuallyDrop;

#[derive(Clone, Copy, Debug)]
pub struct PhysicalMemoryMapping {
    offset: VirtAddr,
}

#[allow(overflowing_literals)]
impl PhysicalMemoryMapping {
    pub(crate) const DEFAULT_OFFSET: VirtAddr = VirtAddr::new(0xFFFF_CF80_0000_0000);
    pub(crate) const DEFAULT_MAX_SIZE: usize = 0x0100_0000_0000;

    pub(crate) const IDENTITY: Self = Self {
        offset: VirtAddr::new(0),
    };

    pub const KERNEL_DEFAULT_MAPPING: Self = Self {
        offset: Self::DEFAULT_OFFSET,
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

pub trait MemoryMapping: Copy + Clone {
    fn convert(&self, phys: PhyAddr) -> VirtAddr;
}

pub struct PageTableMapper<T: Translator, M: MemoryMapping> {
    pub(in crate::x86::paging) pml4: ManuallyDrop<Box<PageTable>>,
    phys_mapping: M,
    translator: PhantomData<T>,
}

impl<T: Translator, M: MemoryMapping> PageTableMapper<T, M> {
    pub(crate) unsafe fn new_from_raw(table_ptr: PhyAddr, mapping: M) -> Self {
        Self {
            pml4: ManuallyDrop::new(Box::from_raw(mapping.convert(table_ptr).as_mut_ptr())),
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
        let pdpte = Self::get_or_create_entry(
            self.phys_mapping,
            pml4.get_mut(translated_addr.pml4_offset()),
            parent_flags,
        )
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
        let pdpte = Self::get_or_create_entry(
            self.phys_mapping,
            pml4.get_mut(translated_addr.pml4_offset()),
            parent_flags,
        )
        .map_err(|_| PageTableMappingError::TableCreationError)?;
        let pde = Self::get_or_create_entry(
            self.phys_mapping,
            pdpte.get_mut(translated_addr.pdpte_offset()),
            parent_flags,
        )
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
        let pdpte = Self::get_or_create_entry(
            self.phys_mapping,
            pml4.get_mut(translated_addr.pml4_offset()),
            parent_flags,
        )
        .map_err(|_| PageTableMappingError::TableCreationError)?;
        let pde = Self::get_or_create_entry(
            self.phys_mapping,
            pdpte.get_mut(translated_addr.pdpte_offset()),
            parent_flags,
        )
        .map_err(|_| PageTableMappingError::TableCreationError)?;
        let pte = Self::get_or_create_entry(
            self.phys_mapping,
            pde.get_mut(translated_addr.pde_offset()),
            parent_flags,
        )
        .map_err(|_| PageTableMappingError::TableCreationError)?;

        if pte.get_mut(translated_addr.pte_offset()).used() {
            return Err(PageTableMappingError::AlreadyMapped);
        }
        pte.get_mut(translated_addr.pte_offset())
            .map_to_frame(frame, flags);

        Ok(())
    }

    fn get_or_create_entry(
        mapping: M,
        entry: &mut PageTableEntry,
        flags: PageTableFlags,
    ) -> Result<&mut PageTable, PageTableCreationError> {
        if !entry.used() {
            let table_addr =
                alloc_page(PAGE_SIZE).map_err(|_| PageTableCreationError::AllocationError)?;
            unsafe {
                *mapping.convert(table_addr.start).as_mut_ptr::<PageTable>() = PageTable::default()
            }
            entry.map_to_addr(table_addr.start, flags);
        } else {
            entry.set_flags(flags);
        }

        if entry.flags().huge_page() {
            return Err(PageTableCreationError::HugePage);
        }

        PageTableMapper::<T, M>::get_next_table(mapping, entry)
            .map_err(|_| PageTableCreationError::HugePage)
    }

    fn get_next_table(
        mapping: M,
        entry: &mut PageTableEntry,
    ) -> Result<&mut PageTable, NextPageTableFailed> {
        let table_ptr = mapping.convert(entry.frame().addr).as_mut_ptr();

        Ok(unsafe { &mut *table_ptr })
    }

    pub(crate) unsafe fn map_physical_memory(
        &mut self,
        phys_base: PhyAddr,
        virt_base: VirtAddr,
        flags: PageTableFlags,
        parent_flags: PageTableFlags,
        len: usize,
    ) {
        let table = self.pml4.as_mut();

        let phys_memory_base_translation = T::translate_address(virt_base);

        let last_phys_memory_addr_translation = T::translate_address(virt_base + len);

        for map_entry_id in phys_memory_base_translation.pml4_offset()
            ..last_phys_memory_addr_translation.pml4_offset() + 1
        {
            let curr_phys_offset_map_level =
                u64::from(map_entry_id - phys_memory_base_translation.pml4_offset())
                    * 0x1000
                    * 0x200
                    * 0x200
                    * 0x200;
            let mut idx_range = 0..0x200;

            if map_entry_id == phys_memory_base_translation.pml4_offset() {
                idx_range.start = phys_memory_base_translation.pdpte_offset();
            }

            if map_entry_id == last_phys_memory_addr_translation.pml4_offset() {
                idx_range.end = last_phys_memory_addr_translation.pdpte_offset() + 1;
            }

            let page_table_dir = if table.get_mut(map_entry_id).used() {
                PageTableMapper::<T, M>::get_next_table(
                    self.phys_mapping,
                    table.get_mut(map_entry_id),
                )
                .unwrap()
            } else {
                let page_table_dir_addr = alloc_page(PAGE_SIZE).unwrap();
                *self
                    .phys_mapping
                    .convert(page_table_dir_addr.start)
                    .as_mut_ptr::<PageTable>() = PageTable::default();
                table
                    .get_mut(phys_memory_base_translation.pml4_offset())
                    .map_to_addr(page_table_dir_addr.start, parent_flags.with_present(true));

                &mut *self
                    .phys_mapping
                    .convert(page_table_dir_addr.start)
                    .as_mut_ptr::<PageTable>()
            };

            let idx_range_start = idx_range.start;

            for directory_ptr_table_entry_id in idx_range {
                let curr_phys_offset_dir_ptr_level =
                    u64::from(directory_ptr_table_entry_id - idx_range_start)
                        * 0x1000
                        * 0x200
                        * 0x200;
                let directory_ptr_table_entry =
                    if page_table_dir.get_mut(directory_ptr_table_entry_id).used() {
                        PageTableMapper::<T, M>::get_next_table(
                            self.phys_mapping,
                            page_table_dir.get_mut(directory_ptr_table_entry_id),
                        )
                        .unwrap()
                    } else {
                        let directory_ptr_table_entry_addr = alloc_page(PAGE_SIZE).unwrap();
                        page_table_dir
                            .get_mut(directory_ptr_table_entry_id)
                            .map_to_addr(
                                directory_ptr_table_entry_addr.start,
                                parent_flags.with_present(true),
                            );
                        *self
                            .phys_mapping
                            .convert(directory_ptr_table_entry_addr.start)
                            .as_mut_ptr::<PageTable>() = PageTable::default();

                        &mut *self
                            .phys_mapping
                            .convert(directory_ptr_table_entry_addr.start)
                            .as_mut_ptr::<PageTable>()
                    };

                let mut directory_idx_range = 0..0x200;

                if map_entry_id == phys_memory_base_translation.pml4_offset()
                    && directory_ptr_table_entry_id == phys_memory_base_translation.pdpte_offset()
                {
                    directory_idx_range.start = phys_memory_base_translation.pde_offset();
                }

                if map_entry_id == last_phys_memory_addr_translation.pml4_offset()
                    && directory_ptr_table_entry_id
                        == last_phys_memory_addr_translation.pdpte_offset()
                {
                    directory_idx_range.end = last_phys_memory_addr_translation.pde_offset() + 1;
                }

                let directory_idx_range_start = directory_idx_range.start;

                for directory_entry_id in directory_idx_range {
                    let curr_phys_offset_dir_level =
                        u64::from(directory_entry_id - directory_idx_range_start) * 0x1000 * 0x200;
                    let mut table_entry_range = 0..0x200;

                    if map_entry_id == phys_memory_base_translation.pml4_offset()
                        && directory_ptr_table_entry_id
                            == phys_memory_base_translation.pdpte_offset()
                        && directory_entry_id == phys_memory_base_translation.pde_offset()
                    {
                        table_entry_range.start = phys_memory_base_translation.pte_offset();
                    }

                    if map_entry_id == last_phys_memory_addr_translation.pml4_offset()
                        && directory_ptr_table_entry_id
                            == last_phys_memory_addr_translation.pdpte_offset()
                        && directory_entry_id == last_phys_memory_addr_translation.pde_offset()
                    {
                        table_entry_range.end = last_phys_memory_addr_translation.pte_offset() + 1;
                    }

                    // We can use a huge page if we would have to update all entries.
                    if table_entry_range.start == 0 && table_entry_range.end == 0x200 {
                        directory_ptr_table_entry
                            .get_mut(directory_entry_id)
                            .map_to_addr(
                                phys_base
                                    + PhyAddr::new(
                                        curr_phys_offset_map_level
                                            + curr_phys_offset_dir_ptr_level
                                            + curr_phys_offset_dir_level,
                                    ),
                                flags.with_present(true).with_huge_page(true),
                            );
                    } else {
                        let directory_entry =
                            if directory_ptr_table_entry.get_mut(directory_entry_id).used() {
                                PageTableMapper::<T, M>::get_next_table(
                                    self.phys_mapping,
                                    directory_ptr_table_entry.get_mut(directory_entry_id),
                                )
                                .unwrap()
                            } else {
                                let directory_entry_addr = alloc_page(PAGE_SIZE).unwrap();
                                directory_ptr_table_entry
                                    .get_mut(directory_entry_id)
                                    .map_to_addr(
                                        directory_entry_addr.start,
                                        parent_flags.with_present(true),
                                    );

                                *self
                                    .phys_mapping
                                    .convert(directory_entry_addr.start)
                                    .as_mut_ptr::<PageTable>() = PageTable::default();

                                &mut *self
                                    .phys_mapping
                                    .convert(directory_entry_addr.start)
                                    .as_mut_ptr::<PageTable>()
                            };
                        let table_entry_range_start = table_entry_range.start;

                        for table_entry_id in table_entry_range {
                            let curr_phys_offset_page_level =
                                u64::from(table_entry_id - table_entry_range_start) * 0x1000;
                            directory_entry.get_mut(table_entry_id).map_to_addr(
                                phys_base
                                    + PhyAddr::new(
                                        curr_phys_offset_map_level
                                            + curr_phys_offset_dir_ptr_level
                                            + curr_phys_offset_dir_level
                                            + curr_phys_offset_page_level,
                                    ),
                                flags.with_present(true),
                            );
                        }
                    }
                }
            }
        }
    }

    pub unsafe fn unmap_physical_memory(&mut self, virt_base: VirtAddr, len: usize) {
        let table = self.pml4.as_mut();

        let phys_memory_base_translation = T::translate_address(virt_base);

        let last_phys_memory_addr_translation = T::translate_address(virt_base + len);

        for map_entry_id in phys_memory_base_translation.pml4_offset()
            ..last_phys_memory_addr_translation.pml4_offset() + 1
        {
            let curr_phys_offset_map_level =
                u64::from(map_entry_id - phys_memory_base_translation.pml4_offset())
                    * 0x1000
                    * 0x200
                    * 0x200
                    * 0x200;
            let mut idx_range = 0..0x200;

            if map_entry_id == phys_memory_base_translation.pml4_offset() {
                idx_range.start = phys_memory_base_translation.pdpte_offset();
            }

            if map_entry_id == last_phys_memory_addr_translation.pml4_offset() {
                idx_range.end = last_phys_memory_addr_translation.pdpte_offset() + 1;
            }

            let page_table_dir = if table.get_mut(map_entry_id).used() {
                PageTableMapper::<T, M>::get_next_table(
                    self.phys_mapping,
                    table.get_mut(map_entry_id),
                )
                .unwrap()
            } else {
                return;
            };

            let idx_range_start = idx_range.start;

            for directory_ptr_table_entry_id in idx_range {
                let curr_phys_offset_dir_ptr_level =
                    u64::from(directory_ptr_table_entry_id - idx_range_start)
                        * 0x1000
                        * 0x200
                        * 0x200;
                let directory_ptr_table_entry =
                    if page_table_dir.get_mut(directory_ptr_table_entry_id).used() {
                        PageTableMapper::<T, M>::get_next_table(
                            self.phys_mapping,
                            page_table_dir.get_mut(directory_ptr_table_entry_id),
                        )
                        .unwrap()
                    } else {
                        continue;
                    };

                let mut directory_idx_range = 0..0x200;

                if map_entry_id == phys_memory_base_translation.pml4_offset()
                    && directory_ptr_table_entry_id == phys_memory_base_translation.pdpte_offset()
                {
                    directory_idx_range.start = phys_memory_base_translation.pde_offset();
                }

                if map_entry_id == last_phys_memory_addr_translation.pml4_offset()
                    && directory_ptr_table_entry_id
                        == last_phys_memory_addr_translation.pdpte_offset()
                {
                    directory_idx_range.end = last_phys_memory_addr_translation.pde_offset() + 1;
                }

                let directory_idx_range_start = directory_idx_range.start;

                for directory_entry_id in directory_idx_range {
                    let curr_phys_offset_dir_level =
                        u64::from(directory_entry_id - directory_idx_range_start) * 0x1000 * 0x200;
                    let mut table_entry_range = 0..0x200;

                    if map_entry_id == phys_memory_base_translation.pml4_offset()
                        && directory_ptr_table_entry_id
                            == phys_memory_base_translation.pdpte_offset()
                        && directory_entry_id == phys_memory_base_translation.pde_offset()
                    {
                        table_entry_range.start = phys_memory_base_translation.pte_offset();
                    }

                    if map_entry_id == last_phys_memory_addr_translation.pml4_offset()
                        && directory_ptr_table_entry_id
                            == last_phys_memory_addr_translation.pdpte_offset()
                        && directory_entry_id == last_phys_memory_addr_translation.pde_offset()
                    {
                        table_entry_range.end = last_phys_memory_addr_translation.pte_offset() + 1;
                    }

                    let directory_entry = directory_ptr_table_entry.get_mut(directory_entry_id);
                    if directory_entry.used() && directory_entry.flags().huge_page() {
                        *directory_entry = PageTableEntry::EMPTY_ENTRY;
                        invalidate_tlb_entry(
                            virt_base
                                + VirtAddr::new(
                                    curr_phys_offset_map_level
                                        + curr_phys_offset_dir_ptr_level
                                        + curr_phys_offset_dir_level,
                                ),
                        )
                    } else {
                        let directory_entry =
                            if directory_ptr_table_entry.get_mut(directory_entry_id).used() {
                                PageTableMapper::<T, M>::get_next_table(
                                    self.phys_mapping,
                                    directory_ptr_table_entry.get_mut(directory_entry_id),
                                )
                                .unwrap()
                            } else {
                                continue;
                            };
                        let table_entry_range_start = table_entry_range.start;

                        for table_entry_id in table_entry_range {
                            let curr_phys_offset_page_level =
                                u64::from(table_entry_id - table_entry_range_start) * 0x1000;
                            *directory_entry.get_mut(table_entry_id) = PageTableEntry::EMPTY_ENTRY;

                            invalidate_tlb_entry(
                                virt_base
                                    + VirtAddr::new(
                                        curr_phys_offset_map_level
                                            + curr_phys_offset_dir_ptr_level
                                            + curr_phys_offset_dir_level
                                            + curr_phys_offset_page_level,
                                    ),
                            )
                        }
                    }
                }
            }
        }
    }
}

fn invalidate_tlb_entry(mem: VirtAddr) {
    let mem_ptr = mem.as_mut_ptr::<u8>();
    unsafe { asm!("invlpg [{}]", in(reg) mem_ptr) }
}

#[derive(Debug)]
pub enum NextPageTableFailed {}

#[derive(Debug)]
pub enum PageTableMappingError {
    AlreadyMapped,
    TableCreationError,
}
