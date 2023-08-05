use core::arch::asm;
use core::fmt;
use core::{self, fmt::Write};

#[macro_export]
macro_rules! print {
    ($($t_elm:tt)*) => {
        $crate::video_io::io::__bios_print(format_args!($($t_elm)*))
    };
}

#[macro_export]
macro_rules! rinfo {
    ($msg: tt) => {
        $crate::video_io::io::__bios_print_str("\r\n[info] ");
        $crate::video_io::io::__bios_print_str($msg);
    };
}

#[macro_export]
macro_rules! rerror {
    ($msg: tt) => {
        $crate::video_io::io::__bios_print_str("\r\n[error] ");
        $crate::video_io::io::__bios_print_str($msg);
    };
}

#[macro_export]
macro_rules! hex_print {
    ($num: tt, $type: tt) => {
        let mut dsp_buffer = [0u8; 20];
        let bytes = $crate::numtoa::NumToA::numtoa($num, 16, &mut dsp_buffer);
        let mut dst_buffer = [0u8; 18];

        dst_buffer[17] = b'0';
        dst_buffer[16] = b'x';
        let mut cursor: u32 = 0;

        for (i, b) in bytes.iter().rev().enumerate() {
            cursor += 1;
            dst_buffer[i] = *b;
        }

        while (cursor < 16) {
            dst_buffer[cursor as usize] = b'0';
            cursor += 1;
        }

        dst_buffer.reverse();

        $crate::video_io::io::cprint_info(&dst_buffer);
    };
}

pub fn cprint_info(str: &[u8]) {
    for ch in str {
        __bios_printc(*ch);
    }
}

pub fn __bios_printc(ch: u8) {
    let reg: u16 = u16::from(ch) | 0x0e00;
    unsafe {
        asm!("push bx", "mov bx, 0", "int 0x10", "pop bx", in("ax") reg);
    }
}

pub fn __bios_print(args: fmt::Arguments) {
    let mut writer = Writer {};
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

struct Writer;
impl Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        __bios_printc(b'X');
        __bios_print_str(s);
        Ok(())
    }
}
