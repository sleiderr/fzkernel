use core::{self, fmt::Write};
use core::arch::asm;
use core::fmt;

#[macro_export]
macro_rules! print {
    ($($t_elm:tt)*) => {
        $crate::video_io::io::__bios_print(format_args!($($t_elm)*))
    };
}

pub fn cprint_info(str: &[u8]) {
    for ch in str {
        __bios_printc(*ch);
    }
}


pub fn __bios_printc(ch: u8) {
    let reg : u16 = u16::from(ch) | 0x0e00;
    unsafe {
        asm!("push bx", "mov bx, 0", "int 0x10", "pop bx", in("ax") reg);
    }
}

pub fn __bios_print(args: fmt::Arguments) {
    let mut writer = Writer{};
}

pub fn __bios_print_str(s: &str) {
    for &ch in s.as_bytes() {
        __bios_printc(ch);
    }
}

pub fn color() {

    unsafe {
        asm!(
        "mov ah, 0x0b",
        "xor bh, bh",
        "mov bl, 0x01"
        )
    }
}

pub fn clear_screen() {
    unsafe {
        asm!(
        "push ax",
        "mov ah, 0x00",
        "mov al, 0x03",
        "int 0x10",
        "pop ax"
        )
    }
}

struct Writer;
impl Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        __bios_printc(b'X');
        __bios_print_str(s);
        Ok(())
    }
}
