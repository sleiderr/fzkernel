use core::{arch::asm, mem};

use bitfield::bitfield;
use numtoa::NumToA;

use crate::{video_io::io::cprint_info, print};

pub struct AddressRangeDescriptor {

    pub base_addr_low: u32,
    pub base_addr_high: u32,
    pub length_low: u32,
    pub length_high: u32,
    pub addr_type: u32,
    pub extended_attributes: ExtendedAttributesARDS

}

bitfield!{
    #[repr(packed)]
    pub struct ExtendedAttributesARDS(u8);
    u32;
    should_ignore, _: 1, 0;
    non_volatile, _: 1, 1;
}

fn __mem_entry_e820(mut ebx: u32, buffer: u16) -> Result<u32, ()> {

    let cf: u32;

    unsafe {
        asm!(
        "pushf",
        "push es",
        "push di",
        "push ecx",
        "mov di, ax",
        "xor ax, ax",
        "mov es, ax",
        "mov edx, 0x534D4150",
        "mov eax, 0xe820",
        "mov ecx, 24",
        "int 0x15",
        "xor edx, edx",
        "jnc 1f",
        "mov edx, 1",
        "1: mov eax, ebx",
        "pop ecx",
        "pop di",
        "pop es",
        "popf",
        in("ebx") ebx,
        in("ax") buffer,
        lateout("eax") ebx,
        out("edx") cf,
        )
    }

    if cf == 1 || ebx == 0 {
        return Err(())
    }

    Ok(ebx)

}

pub fn dbg_memory_map(buffer: u16) -> Result<(), ()> {

    let mut entry_count: u16 = 0;
    let mut ebx: u32 = 0;

    while let Ok(result) = __mem_entry_e820(ebx, buffer + entry_count * 24) {
        ebx = result;
        entry_count += 1;

        let ard = (buffer + (entry_count - 1) * 24) as *mut AddressRangeDescriptor;
        let descriptor: &AddressRangeDescriptor = unsafe {
            mem::transmute(ard)
        };

        let base_addr = (descriptor.base_addr_high << 16) + descriptor.base_addr_low ;
        cprint_info(b"\r\nMemory address: ");
        let mut dsp_buffer = [0u8; 20];

        cprint_info(base_addr.numtoa(16, &mut dsp_buffer))
    }

    Ok(())

}
