use crate::io::acpi::sdt::ACPISDTHeader;
use crate::println;
use crate::sdt_getter;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::Any;
use core::mem;
use core::mem::transmute;
use core::ptr::{addr_of, read, read_unaligned, read_volatile};

/// `MADT` implements an abstraction for Multiple APIC Description Table
#[repr(C, packed)]
pub struct MADT {
    pub header: ACPISDTHeader,
    pub lapic_address: u32,
    pub flags: u32,
}

pub struct MADTEntries {
    pub t0: Vec<MADTType0>,
    pub t1: Vec<MADTType1>,
    pub t2: Vec<MADTType2>,
    pub t3: Vec<MADTType3>,
    pub t4: Vec<MADTType4>,
    pub t5: Vec<MADTType5>,
    pub t9: Vec<MADTType9>,
}

impl MADTEntries {
    pub fn new() -> Self {
        Self {
            t0: Vec::new(),
            t1: Vec::new(),
            t2: Vec::new(),
            t3: Vec::new(),
            t4: Vec::new(),
            t5: Vec::new(),
            t9: Vec::new(),
        }
    }
}

impl MADT {
    sdt_getter!("APIC");
    /// From a given address, returns an [`MADTEntries`] instance combining every
    /// entries found in the table.
    pub fn parse_entries(&self, address: usize) -> MADTEntries {
        let mut entries = MADTEntries::new();
        let entries_start = address + mem::size_of::<MADT>();
        let length = self.header.length - (mem::size_of::<MADT>() as u32);
        let a = self.header.length;
        let mut i = entries_start;
        while i < (length as usize + entries_start) as usize {
            let header: Header = unsafe { read_volatile(i as *const Header) };
            let entry_address = i + mem::size_of::<Header>();
            match header.entry_type {
                0 => {
                    let entry: MADTType0 =
                        unsafe { read_volatile(entry_address as *const MADTType0) };
                    i += mem::size_of::<Header>() + mem::size_of::<MADTType0>();
                    entries.t0.push(entry);
                }
                1 => {
                    let entry: MADTType1 =
                        unsafe { read_volatile((entry_address) as *const MADTType1) };
                    i += mem::size_of::<Header>() + mem::size_of::<MADTType1>();
                    entries.t1.push(entry.clone());
                }
                2 => {
                    let entry: MADTType2 =
                        unsafe { read_volatile(entry_address as *const MADTType2) };
                    i += mem::size_of::<Header>() + mem::size_of::<MADTType2>();
                    entries.t2.push(entry);
                }
                3 => {
                    let entry: MADTType3 =
                        unsafe { read_volatile(entry_address as *const MADTType3) };
                    i += mem::size_of::<Header>() + mem::size_of::<MADTType3>();
                    entries.t3.push(entry);
                }
                4 => {
                    let entry: MADTType4 =
                        unsafe { read_volatile(entry_address as *const MADTType4) };
                    i += mem::size_of::<Header>() + mem::size_of::<MADTType4>();
                    entries.t4.push(entry);
                }
                5 => {
                    let entry: MADTType5 =
                        unsafe { read_volatile(entry_address as *const MADTType5) };
                    i += mem::size_of::<Header>() + mem::size_of::<MADTType5>();
                    entries.t5.push(entry);
                }
                9 => {
                    let entry: MADTType9 =
                        unsafe { read_volatile(entry_address as *const MADTType9) };
                    i += mem::size_of::<Header>() + mem::size_of::<MADTType9>();
                    entries.t9.push(entry);
                }
                _ => break,
            }
        }
        entries
    }
}

#[derive(Debug)]
#[repr(C, packed)]
pub struct MADTType0 {
    pub proc_id: u8,
    pub apic_id: u8,
    pub flags: u32,
}

#[derive(Debug, Clone)]
#[repr(C, packed)]
pub struct MADTType1 {
    pub io_apic_id: [u8; 1],
    pub _reserved: [u8; 1],
    pub io_apic_address: [u8; 4],
    pub gsib: [u8; 4],
}

#[derive(Debug)]
#[repr(C, packed)]
pub struct MADTType2 {
    pub bus_source: u8,
    pub irq_source: u8,
    pub gsi: u32,
    pub flags: u16,
}

#[repr(C)]
pub struct MADTType3 {
    pub nmi_source: u8,
    pub _reserved: u8,
    pub flags: u16,
    pub gsi: u32,
}

#[repr(C, packed)]
pub struct MADTType4 {
    pub acpi_proc_id: u8,
    pub flags: u16,
    pub lint: u8,
}

#[repr(C, packed)]
pub struct MADTType5 {
    pub _reserved: u16,
    pub lapic_overwrite_address: u64,
}

#[repr(C, packed)]
pub struct MADTType9 {
    pub _reserved: u16,
    pub x2apic_id: u32,
    pub flags: u32,
    pub acpi_id: u32,
}

#[repr(C, packed)]
pub struct Header {
    entry_type: u8,
    record_length: u8,
}
