#![no_std]
#![no_main]

use core::panic::PanicInfo;

use fzboot::io::disk::bios::{edd_ext_check, AddressPacket};
use fzboot::video::io::clear_screen;

#[no_mangle]
#[link_section = ".startup"]
pub fn _start() -> ! {
    let loader_ptr = 0x200 as *const ();
    let loader: fn() -> ! = unsafe { core::mem::transmute(loader_ptr) };

    clear_screen();

    // Check if bios supports disk extensions.
    if !edd_ext_check(0x80) {
        // Not supported for now. We should fallback to int13h 0x2 to read from the disk,
        // but so far I did not manage to fit the code for that inside 512 bytes. That is
        // definitely possible though.
        loop {}
    }
    let stage2 = AddressPacket::new(127, 0x0, 0x7e00, 0x1);
    stage2.disk_read(0x80);
    let stage2 = AddressPacket::new(127, 0x17c0, 0x00, 128);
    stage2.disk_read(0x80);
    let stage2 = AddressPacket::new(127, 0x27a0, 0x00, 255);
    stage2.disk_read(0x80);
    let stage2 = AddressPacket::new(127, 0x3780, 0x00, 382);
    stage2.disk_read(0x80);
    loader();
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
