#![no_std]
#![no_main]

mod pswitch;

use crate::pswitch::a20::enable_a20;

use core::panic::PanicInfo;
use core::fmt::Write;
use flib::video_io::io::{clear_screen, cprint_info, color};
use flib::video_io::writer::Writer;
use flib::print;

#[link_section = ".startup"]
#[no_mangle]
pub fn loader() {

    clear_screen();
    cprint_info(b"Loading second stage");
    enable_a20();

}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(s) = info.payload().downcast_ref::<&str>() {
        print!("{}", s);
    } else {
        cprint_info(b"panic occurred");
    }
    loop{}
}
