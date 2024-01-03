use bytemuck::{Pod, Zeroable};
use core::arch::asm;

pub mod acpi;
pub mod disk;
pub mod pic;
pub mod ps2;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub struct IOPort(u16);

impl IOPort {
    pub(crate) const IMCR_ADDR: Self = Self(0x22);

    pub(crate) const IMCR_DATA: Self = Self(0x23);
}

impl From<u16> for IOPort {
    fn from(value: u16) -> Self {
        Self(value)
    }
}

impl From<IOPort> for u16 {
    fn from(value: IOPort) -> Self {
        value.0
    }
}

pub fn outb(port: IOPort, data: u8) {
    unsafe {
        asm!(
        "out dx, al",
        in("dx") u16::from(port),
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
