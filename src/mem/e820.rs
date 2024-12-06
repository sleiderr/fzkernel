use core::{arch::asm, ptr};

use bitfield::bitfield;

use crate::{errors::E820Error, hex_print, video::io::cprint_info};

pub const E820_MAP_ADDR: u32 = 0x4804;
pub static mut E820_MAP_LENGTH: u32 = 0;

#[cfg(feature = "alloc")]
/// Returns the list of memory entries returned by BIOS 0xE820 function.
pub fn e820_entries_bootloader() -> alloc::vec::Vec<AddressRangeDescriptor> {
    let map = E820MemoryMap::new(E820_MAP_ADDR as *mut u8);
    map.into_iter().collect()
}

#[derive(Debug)]
pub struct E820MemoryMap {
    base_addr: *mut u8,
    cursor: u32,
}

impl E820MemoryMap {
    pub fn new(base_addr: *mut u8) -> Self {
        Self {
            base_addr,
            cursor: 0,
        }
    }
}

impl Default for E820MemoryMap {
    fn default() -> Self {
        Self::new(E820_MAP_ADDR as *mut u8)
    }
}

impl Iterator for E820MemoryMap {
    type Item = AddressRangeDescriptor;

    fn next(&mut self) -> Option<Self::Item> {
        let map_len = unsafe { ptr::read(self.base_addr.sub(0x4) as *mut u32) };
        assert_ne!(map_len, 0);

        if map_len <= self.cursor {
            self.cursor = 0;
            return None;
        }

        let current_elem = unsafe {
            (self
                .base_addr
                .add(usize::try_from(24 * (self.cursor)).unwrap()))
                as *mut AddressRangeDescriptor
        };
        let ard: AddressRangeDescriptor = unsafe { ptr::read(current_elem) };

        self.cursor += 1;

        Some(ard)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AddressRangeDescriptor {
    pub base_addr_low: u32,
    pub base_addr_high: u32,
    pub length_low: u32,
    pub length_high: u32,
    pub addr_type: E820MemType,
    pub extended_attributes: ExtendedAttributesARDS,
}

impl AddressRangeDescriptor {
    /// Returns the length of this `AddressRangeDescriptor`, in bytes.
    pub fn length(&self) -> u64 {
        (self.length_high as u64) << 32 | (self.length_low as u64)
    }

    /// Returns a pointer to the base memory address of this `AddressRangeDescriptor`.
    pub fn base_addr(&self) -> *mut u8 {
        ((self.base_addr_high as u64) << 32 | (self.base_addr_low as u64)) as *mut u8
    }
}

impl Default for AddressRangeDescriptor {
    fn default() -> Self {
        Self {
            base_addr_low: 0,
            base_addr_high: 0,
            length_low: 0,
            length_high: 0,
            addr_type: E820MemType::RAM,
            extended_attributes: ExtendedAttributesARDS(0),
        }
    }
}

#[derive(Debug, Clone, Copy)]
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
    #[derive(Debug, Clone, Copy)]
    #[repr(packed)]
    pub struct ExtendedAttributesARDS(u8);
    u32;
    should_ignore, _: 1, 0;
    non_volatile, _: 1, 1;
}

#[cfg(not(feature = "x86_64"))]
fn __mem_entry_e820(mut ebx: u32, buffer: u32) -> Result<u32, E820Error> {
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
        "jnc 2f",
        "mov edx, 1",
        "2: mov eax, ebx",
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
        return Err(E820Error::new());
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

#[cfg(feature = "real")]
pub fn memory_map() {
    use crate::rinfo;

    let mut entry_count: u32 = 0;
    let mut ebx: u32 = 0;

    while let Ok(result) = __mem_entry_e820(ebx, E820_MAP_ADDR + entry_count * 24) {
        ebx = result;
        entry_count += 1;

        let ard = (E820_MAP_ADDR + (entry_count - 1) * 24) as *mut AddressRangeDescriptor;
        let descriptor: &AddressRangeDescriptor = unsafe { &*ard };

        let base_addr = (descriptor.base_addr_high << 16) + descriptor.base_addr_low;
        let length = (descriptor.length_high << 16) + descriptor.length_low;

        rinfo!("memory: ");
        hex_print!(base_addr, u32);
        cprint_info(b" <-> ");
        hex_print!((base_addr + length - 1), u32);

        e820_type_print(descriptor);
    }

    unsafe { ptr::write((E820_MAP_ADDR - 0x2) as *mut u32, entry_count) }
}
