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
    cprint_info(b"Loading second stage");
    cprint_info(b"\r\nAttempt to enable A20 line");
    enable_a20();
    cprint_info(b"\r\nA20 line enabled");
    loop {}

}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    print!("{}", info);
    loop{}
}
