use core::arch::asm;

pub fn edd_ext_check() -> bool {
    let mut cflag: u16;

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

pub fn drive_reset(driver_number: u8) {

    unsafe {
        asm!(
        "xor ah, ah",
        "int 0x13",
        in("dl") driver_number
        )
    }

}

#[repr(C, packed)]
pub struct AddressPacket {

    size: u8,
    zero: u8,
    sectors_count: u16,
    buffer: u32,
    s_lba: u64

}

impl AddressPacket {

    pub fn new(sectors_count: u16, buffer: u32, s_lba: u64) -> Self {

        AddressPacket{
            size: 0x10,
            zero: 0x00,
            sectors_count,
            buffer,
            s_lba
        }

    }

    pub fn disk_read(&self, drive_number: u8) -> Result<(), ()> {
        let result: u8;
        let dap_addr: *const AddressPacket = self;

        unsafe {
            asm!(
                "mov ah, 0x42",
                "mov si, cx",
                "xor bx, bx",
                "mov ds, bx",
                "int 0x13",
                in("dl") drive_number,
                in("cx") dap_addr,
                out("ah") result
            )
        }

        if result == 0x00 {
            return Ok(())
        }

        return Err(())
    }





}
