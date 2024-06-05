#![feature(start)]
#![no_std]
#![no_main]

use core::{arch::asm, panic::PanicInfo};

#[no_mangle]
pub extern "C" fn _start(multiboot_information_ptr: *mut u8) -> ! {
    _kmain();
}

extern "C" fn _kmain() -> ! {
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    loop {}
}
