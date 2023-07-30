use core::{mem, ptr, slice, str};

use crate::{error, hex_print, info, video_io::io::cprint_info};

macro_rules! struct_type {
    ($name: tt, $val: tt) => {
        pub(crate) const $name: u8 = $val;
    };
}

macro_rules! get_str {
    () => {
        fn get_str(&self, num: u8) -> Option<&'static str> {
            if num == 0 {
                return None;
            }

            let mut str_count = 0;
            let mut last_str_start = self.data_base_addr;
            let mut last_len = 0;

            for i in 0..self.data_len {
                let curr_byte: u8 = unsafe { ptr::read((self.data_base_addr + i) as *const u8) };

                if curr_byte == 0 {
                    str_count += 1;
                    last_len = (self.data_base_addr + i) - (last_str_start + last_len);
                    if str_count == num {
                        let str_slice = unsafe {
                            slice::from_raw_parts(
                                (self.data_base_addr + i - last_len) as *mut u8,
                                last_len as usize,
                            )
                        };

                        let str = unsafe { str::from_utf8_unchecked(str_slice) };
                        return Some(str);
                    }
                    last_str_start = self.data_base_addr + i - last_len + 1;
                }
            }

            None
        }
    };
}

struct_type!(SM_BIOSINFO, 0);
struct_type!(SM_SYSINFO, 1);
struct_type!(SM_BASEBOARDINFO, 2);
struct_type!(SM_CHASSIS, 3);
struct_type!(SM_PROCINFO, 4);
struct_type!(SM_CACHEINFO, 7);

#[repr(C, packed)]
pub struct SMBIOSStructHeader {
    struct_type: u8,
    length: u8,
    handle: u16,
}

pub struct SMBIOSBiosInfo {
    data_base_addr: u32,
    data_len: u32,
    internal: InternalSMBIOSBiosInfo,
}

#[repr(C, packed)]
struct InternalSMBIOSBiosInfo {
    vendor: u8,
    version: u8,
    start_addr_segment: u16,
    release_date: u8,
    rom_size: u8,
    characteristics: u64,
    characteristics_ext: u16,
    major_release: u8,
    minor_release: u8,
    emb_ctrl_major: u8,
    emb_ctrl_minor: u8,
    ext_rom_size: u16,
}

pub struct SMBIOSSystemInfo {
    data_base_addr: u32,
    data_len: u32,
    internal: InternalSMBIOSSystemInfo,
}

impl SMBIOSBiosInfo {
    get_str!();
    pub fn get_vendor(&self) -> Option<&'static str> {
        self.get_str(self.internal.vendor)
    }
    pub fn get_version(&self) -> Option<&'static str> {
        self.get_str(self.internal.version)
    }
    pub fn get_release_date(&self) -> Option<&'static str> {
        self.get_str(self.internal.release_date)
    }
    pub fn get_rom_size(&self) -> u32 {
        ((self.internal.rom_size + 1) as u32) * 64
    }
    pub fn get_major_release(&self) -> Option<&'static str> {
        self.get_str(self.internal.major_release)
    }
    pub fn get_minor_release(&self) -> Option<&'static str> {
        self.get_str(self.internal.minor_release)
    }
}

#[repr(C, packed)]
struct InternalSMBIOSSystemInfo {
    manufacturer: u8,
    product_name: u8,
    version: u8,
    serial_number: u8,
    uuid: [u8; 16],
    wake_up_type: WakeupType,
    sku_number: u8,
    family: u8,
}

#[repr(u8)]
pub enum WakeupType {
    Reserved = 0,
    Other = 1,
    Unknown = 2,
    APMTimer = 3,
    ModemRing = 4,
    LanRemote = 5,
    PowerSwitch = 6,
    PCIPME = 7,
    ACPowerRestored = 8,
}

impl SMBIOSSystemInfo {
    get_str!();
    pub fn get_manufacturer(&self) -> Option<&'static str> {
        self.get_str(self.internal.manufacturer)
    }

    pub fn get_product_name(&self) -> Option<&'static str> {
        self.get_str(self.internal.product_name)
    }

    pub fn get_version(&self) -> Option<&'static str> {
        self.get_str(self.internal.version)
    }

    pub fn get_serial_number(&self) -> Option<&'static str> {
        self.get_str(self.internal.serial_number)
    }
}

#[repr(C, packed)]
pub struct SMBIOSEntryTable {
    pub anchor_string: [u8; 4],
    pub checksum: u8,
    pub length: u8,
    pub major_version: u8,
    pub minor_version: u8,
    pub max_structure_size: u16,
    pub entry_point_revision: u8,
    pub formatted_area: [u8; 5],
    pub anchor_string_int: [u8; 5],
    pub checksum_int: u8,
    pub struct_table_len: u16,
    pub struct_table_addr: u32,
    pub struct_num: u16,
    pub bcd_revision: u8,
}

macro_rules! struct_getter {
    ($func: tt, $main_name: tt, $code: tt, $internal: tt, $dbg: tt) => {
        pub fn $func(&self) -> Option<$main_name> {
            let mut curr_mem = self.struct_table_addr;
            let mut sys_info: Option<$main_name> = None;

            while (curr_mem - self.struct_table_addr) < (self.struct_table_len as u32) {
                let curr_struct_header: SMBIOSStructHeader;
                unsafe {
                    curr_struct_header = ptr::read(curr_mem as *mut SMBIOSStructHeader);
                }
                curr_mem += mem::size_of::<SMBIOSStructHeader>() as u32;

                match curr_struct_header.struct_type {
                    $code => {
                        let internal: $internal;

                        info!("found ");
                        cprint_info($dbg.as_bytes());
                        cprint_info(b" at ");
                        hex_print!(curr_mem, u32);

                        unsafe {
                            internal = ptr::read(curr_mem as *mut $internal);
                            let data_addr = curr_mem + mem::size_of::<$internal>() as u32;

                            curr_mem += (curr_struct_header.length
                                - mem::size_of::<SMBIOSStructHeader>() as u8)
                                as u32;
                            let mut curr_word: u16 = unsafe { ptr::read(curr_mem as *mut u16) };

                            while curr_word != 0 {
                                curr_word = unsafe { ptr::read_unaligned(curr_mem as *mut u16) };
                                curr_mem += mem::size_of::<u8>() as u32;
                            }
                            curr_mem += mem::size_of::<u8>() as u32;

                            return Some($main_name {
                                data_base_addr: data_addr,
                                data_len: (curr_mem - data_addr),
                                internal,
                            });
                        }
                    }
                    _ => {
                        curr_mem += (curr_struct_header.length
                            - mem::size_of::<SMBIOSStructHeader>() as u8)
                            as u32;
                        let mut curr_word: u16 = unsafe { ptr::read(curr_mem as *mut u16) };

                        while curr_word != 0 {
                            curr_word = unsafe { ptr::read_unaligned(curr_mem as *mut u16) };
                            curr_mem += mem::size_of::<u8>() as u32;
                        }
                        curr_mem += mem::size_of::<u8>() as u32;
                    }
                }
            }

            None
        }
    };
}

impl SMBIOSEntryTable {
    struct_getter!(
        get_system_information,
        SMBIOSSystemInfo,
        SM_SYSINFO,
        InternalSMBIOSSystemInfo,
        "SM_SYSINFO"
    );
    struct_getter!(
        get_bios_information,
        SMBIOSBiosInfo,
        SM_BIOSINFO,
        InternalSMBIOSBiosInfo,
        "SM_BIOSINFO"
    );
}

pub fn load_smbios_entry() -> Option<SMBIOSEntryTable> {
    let mem_addr = __locate_smbios_entry()?;
    let entry_table: SMBIOSEntryTable;

    unsafe { entry_table = ptr::read(mem_addr as *mut SMBIOSEntryTable) }

    info!("SMBIOS entry located at ");
    hex_print!(mem_addr, u32);

    Some(entry_table)
}

fn __locate_smbios_entry() -> Option<u32> {
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
