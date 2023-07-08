#![no_std]
#![no_main]

use core::arch::asm;
use core::hint::unreachable_unchecked;
use core::panic::PanicInfo;

use flib::video_io::io::{color, cprint_info, clear_screen};
use flib::disk_io::disk::{AddressPacket, edd_ext_check, drive_reset};

#[link_section = ".boot"]
#[no_mangle]
pub static mut MAGIC_NUMBER: u16 = 0xaa55;


pub fn main() {

    let loader_ptr = 0x200 as *const ();
    let loader: fn() = unsafe { core::mem::transmute(loader_ptr) };

    cprint_info(b"tt");
    if !edd_ext_check() {
        return;
    }
    let stage2 = AddressPacket::new(63, 0x200 | (0x07C0 << 16), 0x1);
    match stage2.disk_read(0x80) {

        Ok(()) => {},
        Err(()) => { return; }

    };

    loader();

}

#[no_mangle]
#[link_section = ".startup"]
pub fn _start() -> ! {
    unsafe {
        main();
        unreachable_unchecked();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
