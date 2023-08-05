#![no_std]
#![no_main]
#![feature(const_nonnull_new)]
#![feature(const_option)]

extern crate alloc;

use alloc::vec;
use conquer_once::spin::OnceCell;
use core::{arch::global_asm, fmt::Write, ptr, slice};
use core::{panic::PanicInfo, ptr::NonNull};
use flib::{error, hex_print, info, print};
use flib::{mem::bmalloc::heap::LockedBuddyAllocator, video_io::vesa::video_mode::ModeInfoBlock};
use flib::{
    println,
    video_io::{
        io::{clear_screen, color, cprint_info},
        vesa::{
            framebuffer::{LockedTextFrameBuffer, TextFrameBuffer},
            video_mode::VESA_MODE_BUFFER,
            TEXT_BUFFER,
        },
    },
};

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
    init_framebuffer();
    loop {}
}

pub fn init_framebuffer() {
    let vesamode_info_ptr = VESA_MODE_BUFFER as *mut ModeInfoBlock;
    let vesamode_info = unsafe { ptr::read(vesamode_info_ptr) };
    let mut framebuffer = TextFrameBuffer::from_vesamode_info(&vesamode_info);
    TEXT_BUFFER.init_once(|| LockedTextFrameBuffer::new(framebuffer));
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("fatal: {info}");
    loop {}
}
