#![no_std]
#![no_main]

use core::panic::PanicInfo;
use core::{arch::global_asm, fmt::Write};
use flib::video_io::io::{clear_screen, color, cprint_info};
use flib::{error, hex_print, info, print};

global_asm!(include_str!("arch/x86/setup.S"));

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
