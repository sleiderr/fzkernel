#![no_std]
#![no_main]

use core::arch::asm;
use core::hint::unreachable_unchecked;
use core::panic::PanicInfo;

use flib::video_io::io::{color, cprint_info, clear_screen};
use flib::disk_io::disk::edd_ext_check;

#[link_section = ".boot"]
#[no_mangle]
pub static mut MAGIC_NUMBER: u16 = 0xaa55;

const INIT_TEXT: &[u8] = b"FrozenBoot";

pub fn main() {
    if edd_ext_check() {
        clear_screen();
        cprint_info(INIT_TEXT);
    }
}

#[no_mangle]
#[link_section = ".startup"]
pub fn _start() -> ! {
    unsafe {
        main();
        unreachable_unchecked();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
