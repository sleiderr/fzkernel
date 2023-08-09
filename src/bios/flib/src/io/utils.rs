use core::arch::asm;

pub fn outb(port: u16, data: u8) {
    unsafe {
        asm!(
        "out dx, al",
        in("dx") port,
        in("al") data
        )
    }
}

pub fn outw(port: u16, data: u16) {
    unsafe {
        asm!(
        "out dx, ax",
        in("dx") port,
        in("ax") data
        )
    }
}

pub fn outd(port: u16, data: u32) {
    unsafe {
        asm!("pusha")
    }
    unsafe {
        asm!(
        "out dx, eax",
        in("dx") port,
        in("eax") data
        )
    }
    unsafe {
        asm!("popa")
    }
}

pub fn io_wait() {
    unsafe { asm!(
    "out dx, al",
    in("dx") 0x80,
    in("al") 0 as u8
    ) }
}

pub fn inb(port: u32) -> u8 {
    let data: u8;
    unsafe {
        asm!(
        "in al, dx",
        in("dx") port,
        out("al") data
        );
    }
    data
}

