use core::{ptr, slice};

use crate::error;

pub fn locate_smbios_entry() -> Option<u32> {
    let mut mem: u32 = 0xF0000;

    while mem < 0x100000 {
        let entry: &[u8];
        unsafe {
            entry = slice::from_raw_parts(mem as *const u8, 4);
        }
        if entry == "_SM_".as_bytes() {
            let length: u8;
            let mut checksum: u8 = 0;

            unsafe {
                length = ptr::read((mem + 5) as *mut u8);
            }

            for i in 0..length {
                let c_byte: u8;
                unsafe {
                    c_byte = ptr::read((mem + i as u32) as *const u8);
                }
                checksum.wrapping_add(c_byte);
            }

            if checksum != 0 {
                error!("Invalid SMBIOS entry checksum");
                return None;
            }

            return Some(mem);
        }
        mem += 16;
    }

    None
}
