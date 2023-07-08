use core;
use core::arch::asm;
use core::fmt;
use core::fmt::Write;
use crate::video_io::writer::Writer;

#[macro_export]
macro_rules! print {
    ($($t_elm:tt)*) => {
        $crate::video_io::io::__bios_print(format_args!($($t_elm)*))
    };
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
        "mov ah, 0x00",
        "mov al, 0x03",
        "int 0x10"
        )
    }
}

pub fn cprint_info(str: &[u8]) {
    for ch in str {
        __bios_printc(*ch);
    }
}

pub fn __bios_print(args: fmt::Arguments) {
    let mut writer = Writer{};
    writer.write_fmt(args).unwrap();
}

#[inline(never)]
pub fn __bios_printc(ch: u8) {
    unsafe {
        asm! {
        "mov ah, 0x0e",
        "mov al, {}",
        "int 0x10",
        in(reg_byte) ch
        }
    }
}
