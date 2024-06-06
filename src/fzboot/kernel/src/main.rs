#![feature(start)]
#![feature(const_nonnull_new)]
#![feature(const_option)]
#![no_std]
#![no_main]

use core::{arch::asm, fmt::Write, panic::PanicInfo, ptr::NonNull};

use fzboot::{
    boot::multiboot::mb_information,
    mem::bmalloc::heap::LockedBuddyAllocator,
    println,
    video::{self, vesa::text_buffer},
};

static mut DEFAULT_HEAP_ADDR: usize = 0x5000000;
const DEFAULT_HEAP_SIZE: usize = 0x1000000;

/// Minimum heap size: 16KiB
const MIN_HEAP_SIZE: usize = 0x4000;

/// Maximum heap size: 2GiB
const MAX_HEAP_SIZE: usize = 0x80000000;

/// Default stack size, if enough RAM is available: 8 MiB
const STACK_SIZE: usize = 0x800000;

#[global_allocator]
pub static BUDDY_ALLOCATOR: LockedBuddyAllocator<16> = LockedBuddyAllocator::new(
    NonNull::new(unsafe { DEFAULT_HEAP_ADDR as *mut u8 }).unwrap(),
    DEFAULT_HEAP_SIZE,
);

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let mut mb_information_ptr: u64 = 0;
    unsafe {
        asm!("", out("rcx") mb_information_ptr);
    }

    let mb_information: mb_information::MultibootInformation = unsafe {
        core::ptr::read(mb_information_ptr as *const mb_information::MultibootInformation)
    };
    _kmain(mb_information);
}

extern "C" fn _kmain(mb_information_header: mb_information::MultibootInformation) -> ! {
    video::vesa::init_text_buffer_from_multiboot(mb_information_header.framebuffer().unwrap());
    println!("Hello from the kernel !");

    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    loop {}
}
