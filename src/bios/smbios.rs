use core::{mem, ptr, slice, str};

use crate::{hex_print, rerror, rinfo, video::io::cprint_info};

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

macro_rules! str_field {
    ($self: ident, $func: tt, $field: expr) => {
        pub fn $func(&$self) -> Option<&'static str> {
            $self.get_str($field)
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

pub struct SMBIOSProcInfo {
    data_base_addr: u32,
    data_len: u32,
    internal: InternalSMBIOSProcInfo,
}

#[repr(C, packed)]
struct InternalSMBIOSProcInfo {
    socket_designation: u8,
    proc_type: ProcType,
    proc_family: u8,
    proc_manufacturer: u8,
    proc_id: u64,
    proc_version: u8,
    voltage: u8,
    pub external_clock: u16,
    pub max_speed: u16,
    pub curr_speed: u16,
    status: u8,
    proc_upgrade: u8,
    l1_cache_handle: u16,
    l2_cache_handle: u16,
    l3_cache_handle: u16,
    serial_number: u8,
    asset_tag: u8,
    part_number: u8,
    pub core_count: u8,
    pub core_enabled: u8,
    pub thread_count: u8,
    proc_characteristics: u16,
    proc_family_2: u16,
    pub core_count_2: u16,
    pub core_enabled_2: u16,
    pub thread_count_2: u16,
    pub thread_enabled: u16,
}

#[repr(u8)]
pub enum ProcType {
    Other = 1,
    Unknown = 2,
    CentralProc = 3,
    MathProc = 4,
    DSPProc = 5,
    VideoProc = 6,
}

impl SMBIOSProcInfo {
    get_str!();
    str_field!(self, get_asset_tag, self.internal.asset_tag);
    str_field!(self, get_serial_number, self.internal.serial_number);
    str_field!(self, get_proc_version, self.internal.proc_version);
    str_field!(self, get_proc_manufacturer, self.internal.proc_manufacturer);
    str_field!(self, get_socket_name, self.internal.socket_designation);
    str_field!(self, get_part_number, self.internal.part_number);
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
    str_field!(self, get_vendor, self.internal.vendor);
    str_field!(self, get_version, self.internal.version);
    str_field!(self, get_release_date, self.internal.release_date);
    str_field!(self, get_rom_size, self.internal.rom_size);
    str_field!(self, get_major_release, self.internal.major_release);
    str_field!(self, get_minor_release, self.internal.minor_release);
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
    str_field!(self, get_manufacturer, self.internal.manufacturer);
    str_field!(self, get_product_name, self.internal.product_name);
    str_field!(self, get_version, self.internal.version);
    str_field!(self, get_serial_number, self.internal.serial_number);
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

                        rinfo!("found ");
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
    struct_getter!(
        get_proc_information,
        SMBIOSProcInfo,
        SM_PROCINFO,
        InternalSMBIOSProcInfo,
        "SM_PROCINFO"
    );
}

pub fn load_smbios_entry() -> Option<SMBIOSEntryTable> {
    let mem_addr = __locate_smbios_entry()?;
    let entry_table: SMBIOSEntryTable;

    unsafe { entry_table = ptr::read(mem_addr as *mut SMBIOSEntryTable) }

    rinfo!("SMBIOS entry located at ");
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
                rerror!("Invalid SMBIOS entry checksum");
                return None;
            }

            return Some(mem);
        }
        mem += 16;
    }

    None
}
