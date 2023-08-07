use core::arch::asm;

pub mod disk;
pub mod ps2;

#[inline]
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

#[inline]
pub fn outb(port: u32, data: u8) {
    unsafe {
        asm!(
        "out dx, al",
        in("dx") port,
        in("al") data
        );
    }
}
