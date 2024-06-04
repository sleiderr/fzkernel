use core::ptr;

use alloc::boxed::Box;
use fzboot::{
    boot::multiboot::mb_information::MultibootInformation,
    mem::PhyAddr32,
    video::vesa::video_mode::{ModeInfoBlock, VESA_MODE_BUFFER},
};

static BOOTLOADER_NAME: [u8; 11] = [
    b'F', b'r', b'o', b'z', b'e', b'n', b'B', b'o', b'o', b't', b'\0',
];

pub fn dump_multiboot_information_header() -> *mut u8 {
    let mut header = MultibootInformation::default();

    let vesamode_info_ptr = VESA_MODE_BUFFER as *mut ModeInfoBlock;
    let vesamode_info = unsafe { ptr::read(vesamode_info_ptr) };

    header.insert_framebuffer_info(vesamode_info);
    header.set_bootloader_name(PhyAddr32::new(
        u32::try_from(ptr::addr_of!(BOOTLOADER_NAME) as *const u8 as usize)
            .expect("invalid bootloader name string address"),
    ));

    Box::into_raw(Box::new(header)) as *mut u8
}
