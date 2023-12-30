use core::arch::asm;

pub mod acpi;
#[cfg(feature = "alloc")]
pub mod apic;
pub mod disk;
pub mod pic;
pub mod ps2;

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

pub fn outl(port: u16, data: u32) {
    unsafe { asm!("pusha") }
    unsafe {
        asm!(
        "out dx, eax",
        in("dx") port,
        in("eax") data
        )
    }
    unsafe { asm!("popa") }
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

pub fn inl(port: u16) -> u32 {
    let data: u32;
    unsafe {
        asm!(
        "in eax, dx",
        in("dx") port,
        out("eax") data
        );
    }

    data
}

#[inline(always)]
pub fn io_delay() {
    unsafe {
        asm!("xor al, al", "out 0x80, al");
    }
}
