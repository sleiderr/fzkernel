#![no_std]
#![no_main]

use core::panic::PanicInfo;
use flib::video_io::io::cprint_info;

#[link_section = ".startup"]
#[no_mangle]
pub fn loader() {

    cprint_info(b"Init phase");

}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {

    loop{}

}
