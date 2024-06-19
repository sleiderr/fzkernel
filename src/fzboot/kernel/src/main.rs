#![feature(start)]
#![feature(const_nonnull_new)]
#![feature(const_option)]
#![feature(naked_functions)]
#![feature(panic_info_message)]
#![no_std]
#![no_main]

extern crate alloc;

use core::{arch::asm, panic::PanicInfo, ptr::NonNull};

use fzboot::{
    boot::multiboot::mb_information,
    exceptions::panic::panic_entry_no_exception,
    mem::bmalloc::heap::LockedBuddyAllocator,
    println,
    video::{self},
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
#[link_section = ".start"]
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
    if let Some(panic_msg) = info.message() {
        panic_entry_no_exception(panic_msg.as_str().unwrap());
    }

    if let Some(panic_msg) = info.payload().downcast_ref::<&str>() {
        panic_entry_no_exception(panic_msg);
    } else {
        panic_entry_no_exception("Unknown exception");
    }
}
