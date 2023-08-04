#![no_std]
#![no_main]
#![feature(const_nonnull_new)]
#![feature(const_option)]

mod pswitch;

use crate::pswitch::{a20::enable_a20, gdt::load_gdt};

use core::{fmt::Write, mem, ptr};
use core::{panic::PanicInfo, ptr::NonNull};
use flib::mem::e820::memory_map;
use flib::video_io::io::{clear_screen, color, cprint_info};
use flib::video_io::vesa::vesa_mode_setup;
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
    memory_map();
    info!("A20 line enabled ");
    load_gdt();
    info!("switching to protected mode (x86)");

    vesa_mode_setup(1480, 900);

    let loader_ptr = 0x7e00 as *const ();
    let prot_entry: fn() -> ! = unsafe { core::mem::transmute(loader_ptr) };

    prot_entry();
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!("Panic occured");
    loop {}
}
