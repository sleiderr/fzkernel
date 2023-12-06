//! Memory related utilities module.

use core::ptr;

use conquer_once::spin::OnceCell;

pub mod bmalloc;
pub mod e820;
pub mod gdt;

pub static MEM_STRUCTURE: OnceCell<MemoryStructure> = OnceCell::uninit();

pub struct MemoryStructure {
    pub heap_addr: usize,
    pub heap_size: usize,
}

/// Zeroise the .bss segment when entering the program.
///
/// Uses two external symbols `_bss_start` and `_bss_end` that must
/// be added at link time and that points to the start and the end
/// of the .bss section respectively.
pub fn zero_bss() {
    extern "C" {
        static _bss_start: u8;
        static _bss_end: u8;
    }

    unsafe {
        let bss_start = &_bss_start as *const u8 as u32;
        let bss_end = &_bss_end as *const u8 as u32;
        let bss_len = bss_end - bss_start;

        ptr::write_bytes(bss_start as *mut u8, 0, bss_len as usize);
    }
}
