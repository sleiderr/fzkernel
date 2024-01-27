#![feature(start)]
#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    _kmain();
}

extern "C" fn _kmain() -> ! {
    unsafe {
        asm!("mov rax, 0x7c");
    }

    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    loop {}
}
