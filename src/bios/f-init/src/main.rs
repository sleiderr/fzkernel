#![no_std]
#![no_main]

mod pswitch;

use crate::pswitch::{a20::enable_a20, gdt::load_gdt};

use core::fmt::Write;
use core::panic::PanicInfo;
use flib::video_io::io::{clear_screen, color, cprint_info};
use flib::{error, hex_print, info, print};

#[no_mangle]
#[link_section = ".start"]
pub extern "C" fn _start() -> ! {
    loader();
}

pub fn loader() -> ! {
    info!("loading second stage (mem: 0x07C00)");
    info!("enabling A20 line");
    enable_a20();
    info!("A20 line enabled ");
    load_gdt();
    info!("switching to protected mode (x86)");

    let loader_ptr = 0x5e00 as *const ();
    let prot_entry: fn() -> ! = unsafe { core::mem::transmute(loader_ptr) };

    prot_entry();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!("Panic occured");
    loop {}
}
