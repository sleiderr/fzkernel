#![no_std]
#![no_main]
#![feature(const_nonnull_new)]
#![feature(const_option)]

extern crate alloc;

mod pswitch;

use crate::pswitch::{a20::enable_a20, gdt::load_gdt};

use alloc::vec::{self, Vec};
use core::{fmt::Write, mem};
use core::{panic::PanicInfo, ptr::NonNull};
use flib::mem::{bmalloc::heap::LockedBuddyAllocator, e820::memory_map};
use flib::video_io::io::{clear_screen, color, cprint_info};
use flib::{error, hex_print, info, print};

const HEAP_ADDR: usize = 0x11000;
const HEAP_SIZE: usize = 0x1000;

#[global_allocator]
static BUDDY_ALLOCATOR: LockedBuddyAllocator<4> =
    LockedBuddyAllocator::new(NonNull::new(HEAP_ADDR as *mut u8).unwrap(), HEAP_SIZE);

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

    //hex_print!((vbe_lock.vbe_version), u32);
    //hex_print!((mem::size_of::<VbeInfoBlock>()), u32);
    //hex_print!(0xc, u32);

    info!("switching to protected mode (x86)");

    let loader_ptr = 0x3e00 as *const ();
    let prot_entry: fn() -> ! = unsafe { core::mem::transmute(loader_ptr) };

    prot_entry();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!("Panic occured");
    loop {}
}
