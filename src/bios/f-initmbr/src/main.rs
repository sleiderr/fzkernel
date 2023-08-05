#![no_std]
#![no_main]

use core::arch::asm;
use core::hint::unreachable_unchecked;
use core::panic::PanicInfo;

use flib::disk_io::disk::{drive_reset, edd_ext_check, AddressPacket};
use flib::video_io::io::{clear_screen, color, cprint_info};

pub fn main() {
    let loader_ptr = 0x200 as *const ();
    let loader: fn() -> ! = unsafe { core::mem::transmute(loader_ptr) };

    clear_screen();

    if !edd_ext_check() {
        return;
    }
    let stage2 = AddressPacket::new(127, 0x07e00, 0x1);
    stage2.disk_read(0x80);
    loader();
}

#[no_mangle]
#[link_section = ".startup"]
pub fn _start() -> ! {
    unsafe {
        asm!(
            "xor ax, ax",
            "mov ds, ax",
            "mov es, ax",
            "mov ss, ax",
            "mov fs, ax",
            "mov gs, ax",
            "cld",
            "mov sp, 0x7c00"
        );
        main();
        unreachable_unchecked();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
