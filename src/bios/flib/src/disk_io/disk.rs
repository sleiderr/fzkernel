use core::arch::asm;

pub fn edd_ext_check() -> bool {
    let mut cflag: u16 = 0x00;

    unsafe {
        asm!(
        "mov ah, 0x41",
        "mov bx, 0x55aa",
        "mov dl, 0x80",
        "int 0x13",
        "sbb {0}, {0}",
        out(reg_abcd) cflag
        );
    }

    if cflag == 0x00 {
        return true;
    }

    return false;
}

#[repr(C, packed)]
pub struct NativeReader {}

impl NativeReader {}
