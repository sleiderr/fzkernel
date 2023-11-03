use core::arch::asm;

use crate::io::{inb, outb};

pub fn send_data(data: u8) {
    outb(0x60, data);
}

pub fn read_ps2() -> u8 {
    inb(0x60)
}

pub fn send_ps2(cmd: u8) {
    outb(0x64, cmd);
}

pub fn input_wait(mut loops: u16) -> Result<(), ()> {
    while loops > 0 {
        let status_reg: u8;

        unsafe {
            asm!(
            "in al, 0x64",
            out("al") status_reg
            );
        }

        if (status_reg & 2) == 0 {
            return Ok(());
        }

        loops -= 1;
    }

    Err(())
}

pub fn output_wait(mut loops: u16) -> Result<(), ()> {
    while loops > 0 {
        let status_reg: u8;

        unsafe {
            asm!(
            "in al, 0x64",
            out("al") status_reg
            );
        }

        if (status_reg & 1) == 1 {
            return Ok(());
        }

        loops -= 1;
    }

    Err(())
}
