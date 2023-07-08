#![no_std]
#![no_main]

mod pswitch;

use core::panic::PanicInfo;
use core::fmt::Write;
use flib::video_io::io::{clear_screen, cprint_info, __bios_printc};
use flib::video_io::writer::Writer;
use flib::print;

#[link_section = ".startup"]
#[no_mangle]
pub fn loader() {

    clear_screen();
    let mut writer = Writer{};
    writeln!(writer, "test");
    cprint_info(b"Loading second stage");
    print!("test");

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
