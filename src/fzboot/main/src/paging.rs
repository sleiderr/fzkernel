use fzboot::mem::PhyAddr;
use fzboot::println;
use fzboot::x86::paging::{PageTable, PageTableEntry, PageTableFlags};

const PAGE_TABLE_PTR: *mut PageTable = 0x50_000 as *mut PageTable;
const PAGE_TABLE_LVL3: *mut PageTable = 0x51_000 as *mut PageTable;

pub(super) fn map_physical_mem() {
    let default_page_table: &mut PageTable = unsafe { &mut *PAGE_TABLE_PTR };
    let pdpt: &mut PageTable = unsafe { &mut *PAGE_TABLE_LVL3 };

    default_page_table
        .get_mut(0)
        .map_to_addr(
            PhyAddr::new(PAGE_TABLE_PTR as u64 + 0x1000),
            PageTableFlags::new().with_present(true).with_write(true),
        )
        .expect("failed to create level 3 pagetable");

    for i in 0..512 {
        pdpt.get_mut(i)
            .map_to_addr(
                PhyAddr::new(u64::from(i) * 4096 * 512 * 512),
                PageTableFlags::new()
                    .with_present(true)
                    .with_write(true)
                    .with_huge_page(true),
            )
            .unwrap();
    }
}
