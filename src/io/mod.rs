use bytemuck::{Pod, Zeroable};
use core::arch::asm;
use core::ops::Add;

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

    pub(crate) const PRIM_ATA: Self = Self(0x1F0);

    pub(crate) const PRIM_ATA_CTRL: Self = Self(0x3F6);

    pub(crate) const SEC_ATA: Self = Self(0x170);

    pub(crate) const SEC_ATA_CTRL: Self = Self(0x376);
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

impl Add<u16> for IOPort {
    type Output = IOPort;

    fn add(self, rhs: u16) -> Self::Output {
        Self(self.0.saturating_add(rhs))
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

pub fn outw(port: IOPort, data: u16) {
    unsafe {
        asm!(
        "out dx, ax",
        in("dx") u16::from(port),
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

pub fn inb(port: IOPort) -> u8 {
    let data: u8;
    unsafe {
        asm!(
        "in al, dx",
        in("dx") u16::from(port),
        out("al") data
        );
    }
    data
}

pub fn inw(port: IOPort) -> u16 {
    let data: u16;
    unsafe {
        asm!(
        "in ax, dx",
        in("dx") u16::from(port),
        out("ax") data
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
