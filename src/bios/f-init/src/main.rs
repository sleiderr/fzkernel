#![no_std]
#![no_main]

mod pswitch;

use crate::pswitch::a20::enable_a20;

use core::panic::PanicInfo;
use core::fmt::Write;
use flib::video_io::io::{clear_screen, cprint_info, color};
use flib::print;

#[no_mangle]
#[link_section = ".start"]
pub extern "C" fn _start() -> ! {
    loader();
}

pub fn loader() -> ! {
    print!("Je quitte VR !");
    loop {}

}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    print!("{}", info);
    loop{}
}
