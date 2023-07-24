use core;
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

pub fn write(x: u16, y: u16, color: u8) {
    unsafe {
        asm!(
        "pusha",
        "mov ah, 0x0c",
        "mov bh, 0x00",
        "int 0x10",
        "popa",
        in("dx") y,
        in("cx") x,
        in("al") color
        )
    }
}

pub fn __bios_printc(ch: u8) {
    let reg: u16 = u16::from(ch) | 0x0e00;
    unsafe {
        asm!("push bx", "mov bx, 0", "int 0x10", "pop bx", in("ax") reg);
    }
}

pub fn switch_graphic() -> () {
    unsafe { asm!("pusha", "mov ah, 0x00", "mov al, 0x12", "int 0x10", "popa") }
}

pub fn __bios_print(args: fmt::Arguments) {
    unsafe {
        __bios_print_str(args.as_str().unwrap());
    }
}

pub fn __bios_print_str(s: &str) {
    for &ch in s.as_bytes() {
        __bios_printc(ch);
    }
}

pub fn color() {
    unsafe { asm!("mov ah, 0x0b", "xor bh, bh", "mov bl, 0x01") }
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
