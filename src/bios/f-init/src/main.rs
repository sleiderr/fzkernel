#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[link_section = ".startup"]
#[no_mangle]
pub fn loader() {



}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {

    loop{}

}
