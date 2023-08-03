#![no_std]
#![no_main]
#![feature(const_nonnull_new)]
#![feature(const_option)]

extern crate alloc;

use alloc::{format, vec};
use core::{arch::global_asm, fmt::Write};
use core::{panic::PanicInfo, ptr::NonNull};
use flib::mem::bmalloc::heap::LockedBuddyAllocator;
use flib::video_io::io::{clear_screen, color, cprint_info};
use flib::{error, hex_print, info, print};

global_asm!(include_str!("arch/x86/setup.S"));

const HEAP_ADDR: usize = 0x5000000;
const HEAP_SIZE: usize = 0x1000000;

#[global_allocator]
static BUDDY_ALLOCATOR: LockedBuddyAllocator<16> =
    LockedBuddyAllocator::new(NonNull::new(HEAP_ADDR as *mut u8).unwrap(), HEAP_SIZE);

#[no_mangle]
#[link_section = ".start"]
pub extern "C" fn _start() -> ! {
    boot_main();
}

pub fn boot_main() -> ! {
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!("Panic occured");
    loop {}
}
