#![no_std]
#![no_main]

mod pswitch;

use crate::pswitch::{a20::enable_a20, gdt::load_gdt};

use core::panic::PanicInfo;
use flib::mem::e820::memory_map;
use flib::video::vesa::vesa_mode_setup;
use flib::{rerror, rinfo};

#[no_mangle]
#[link_section = ".start"]
pub extern "C" fn _start() -> ! {
    loader();
}

pub fn loader() -> ! {
    flib::mem::zero_bss();
    rinfo!("loading second stage (mem: 0x07C00)");
    rinfo!("enabling A20 line");
    enable_a20();
    memory_map();
    rinfo!("A20 line enabled ");
    load_gdt();
    rinfo!("switching to protected mode (x86)");

    vesa_mode_setup(1440, 900);

    let loader_ptr = 0x5f00 as *const ();
    let prot_entry: fn() -> ! = unsafe { core::mem::transmute(loader_ptr) };

    prot_entry();
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    rerror!("Panic occured");
    loop {}
}
