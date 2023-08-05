use core::{arch::asm, mem, ptr};

use bitfield::bitfield;

use crate::{hex_print, info, video_io::io::cprint_info};

pub const E820_MAP_ADDR: u16 = 0x4000;
pub static mut E820_MAP_LENGTH: u16 = 0;

pub struct E820MemoryMap {
    cursor: u16,
}

impl E820MemoryMap {
    pub fn new() -> Self {
        Self { cursor: 0 }
    }
}

impl Iterator for E820MemoryMap {
    type Item = AddressRangeDescriptor;

    fn next(&mut self) -> Option<Self::Item> {
        if unsafe { E820_MAP_LENGTH == 0 } {
            panic!();
        }

        if unsafe { E820_MAP_LENGTH <= self.cursor } {
            self.cursor = 0;
            return None;
        }

        let current_elem = (E820_MAP_ADDR + 24 * (self.cursor)) as *mut AddressRangeDescriptor;
        let ard: AddressRangeDescriptor = unsafe { ptr::read(current_elem) };

        self.cursor += 1;

        Some(ard)
    }
}

pub struct AddressRangeDescriptor {
    pub base_addr_low: u32,
    pub base_addr_high: u32,
    pub length_low: u32,
    pub length_high: u32,
    pub addr_type: E820MemType,
    pub extended_attributes: ExtendedAttributesARDS,
}

#[repr(u16)]
pub enum E820MemType {
    RAM = 1,
    RESERVED = 2,
    ACPI = 3,
    NVS = 4,
    UNUSABLE = 5,
    DISABLED = 6,
    PERSISTENT = 7,
    OEM = 12,
}

bitfield! {
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
        return Err(());
    }

    Ok(ebx)
}

fn e820_type_print(descriptor: &AddressRangeDescriptor) {
    match descriptor.addr_type {
        E820MemType::RAM => {
            cprint_info(b" (usable) ");
        }
        E820MemType::RESERVED => {
            cprint_info(b" (reserved) ");
        }
        E820MemType::ACPI => {
            cprint_info(b" (ACPI) ");
        }
        E820MemType::NVS => {
            cprint_info(b" (ACPI NVS) ");
        }
        E820MemType::UNUSABLE => {
            cprint_info(b" (unusable) ");
        }
        E820MemType::DISABLED => {
            cprint_info(b" (disabled) ");
        }
        E820MemType::PERSISTENT => {
            cprint_info(b" (persistent) ");
        }
        E820MemType::OEM => {
            cprint_info(b" (OEM) ");
        }
    }
}

pub fn memory_map() -> Result<(), ()> {
    let mut entry_count: u16 = 0;
    let mut ebx: u32 = 0;

    while let Ok(result) = __mem_entry_e820(ebx, E820_MAP_ADDR + entry_count * 24) {
        ebx = result;
        entry_count += 1;

        let ard = (E820_MAP_ADDR + (entry_count - 1) * 24) as *mut AddressRangeDescriptor;
        let descriptor: &AddressRangeDescriptor = unsafe { mem::transmute(ard) };

        let base_addr = (descriptor.base_addr_high << 16) + descriptor.base_addr_low;
        let length = (descriptor.length_high << 16) + descriptor.length_low;

        info!("memory: ");
        hex_print!(base_addr, u32);
        cprint_info(b" <-> ");
        hex_print!((base_addr + length - 1), u32);

        e820_type_print(descriptor);
    }

    unsafe { E820_MAP_LENGTH = entry_count };

    Ok(())
}
