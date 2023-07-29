#![no_std]
#![no_main]

mod pswitch;

use crate::pswitch::a20::enable_a20;

use core::panic::PanicInfo;
use core::fmt::Write;
use flib::video_io::io::{clear_screen, cprint_info, color};
use flib::mem::e820::memory_map;
use flib::{hex_print, info, error, print};

#[no_mangle]
#[link_section = ".start"]
pub extern "C" fn _start() -> ! {
    loader();
}

pub fn loader() -> ! {
    info!("loading second stage (mem: 0x07C00)");
    info!("enabling A20 line");
    enable_a20();
    memory_map();
    info!("A20 line enabled ");
    loop {}

}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!("Panic occured");
    loop{}
}
