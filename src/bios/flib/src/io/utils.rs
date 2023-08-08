use core::arch::asm;

pub fn outb(port: u16, data: u8) {
    unsafe {
        asm!(
        "out {}, ah",
        in(reg_byte) port,
        in("ah") data
        )
    }
}

pub fn outw(port: u16, data: u16) {
    unsafe {
        asm!(
        "out {}, ax",
        in(reg_abcd) port,
        in("ax") data
        )
    }
}

pub fn outd(port: u16, data: u32) {
    unsafe {
        asm!(
        "out {0}, eax",
        in(reg) port,
        in("ax") data
        )
    }
}

pub fn io_wait() {
    unsafe { asm!("out 0x80, 0") }
}