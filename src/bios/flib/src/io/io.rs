use core::arch::asm;

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

pub fn outb(data: u8, port: u32) {

    unsafe {
        asm!(
        "out dx, al",
        in("dx") port,
        in("al") data
        );
    }

}
