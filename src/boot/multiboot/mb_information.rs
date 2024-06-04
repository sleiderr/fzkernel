//! Multiboot information structure related code.
//!
//! The bootloader can use this structure to communicate basic information to the operating system. It may
//! be placed anywhere in memory, and the operating system should be careful not to overwrite it before
//! readint it.

use alloc::string::String;
use bytemuck::{Pod, Zeroable};

use crate::{
    mem::{
        e820::{E820_MAP_ADDR, E820_MAP_LENGTH},
        MemoryAddress, PhyAddr, PhyAddr32,
    },
    video::vesa::video_mode::ModeInfoBlock,
};

/// Multiboot information structure.
///
/// Used by the bootloader to communicate basic information to the operating system, before handing out
/// control of the system.
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct MultibootInformation {
    /// Indicates the presence and validity of other fields in the information structure.
    flags: MultibootInformationFlags,

    /// Indicates the amount of lower memory, in kilobytes.
    ///
    /// The lower memory starts at address 0, and the maximum value possible is 640Kb.
    mem_lower: u32,

    /// Indicates the amount of upper memory, in kilobytes.
    ///
    /// It usually is the address of the first upper memory hole, minus 1 megabyte.
    mem_upper: u32,

    /// Indicates which _BIOS_ disk device the OS image was loaded from.
    boot_device: MultibootBootDevice,

    /// Contains the physical address of the command line to be passed to the kernel.
    ///
    /// The command line has to be a C-style zero terminated string.
    cmdline: PhyAddr32,

    /// Indicates the number of boot modules loaded along the kernel image.
    mods_count: u32,

    /// Contains the physical address of the first module structure, describing the boot
    /// modules loaded along with the kernel image.
    mods_addr: PhyAddr32,

    /// Contains either information about the symbol table associated with a kernel image, or
    /// about the section header from an ELF kernel.
    ///
    /// The actual value contained in that field depend on the value of the mutually exclusive bits
    /// [`MultibootInformationFlags::SYMS_TABLE_VALID`] and [`MultibootInformationFlags::SHDR_TABLE_VALID`].
    syms: MultibootTableField,

    /// Specifies the length of a buffer containing a _BIOS_-provided memory map.
    mmap_length: u32,

    /// Specifies the address of a buffer containing a _BIOS_-provided memory map.
    mmap_addr: PhyAddr32,

    /// Specifies the size of the `drives` structure.
    ///
    /// That structure contains information about the drives available on the system, both _BIOS_
    /// and drive specific.
    drives_length: u32,

    /// Specifies the memory address of the `drives` structure.
    ///
    /// That structure contains information about the drives available on the system, both _BIOS_
    /// and drive specific.
    drives_addr: PhyAddr32,

    /// Indicates the address of the _ROM_ configuration table returned by the `GET CONFIGURATION`
    /// _BIOS_ call.
    config_table: PhyAddr32,

    /// Contains the memory address of the name of a bootloader booting the kernel.
    boot_loader_name: PhyAddr32,

    /// Contains the memory address of an `APM` (_Advanced Power Management_) table.
    apm_table: PhyAddr32,

    vbe: VbeMultibootInformation,
    framebuffer: FramebufferMultibootInformation,
}

impl MultibootInformation {
    pub fn set_bootloader_name(&mut self, str_address: PhyAddr32) {
        self.flags |= MultibootInformationFlags::BOOTLOADER_NAME_VALID;
        self.boot_loader_name = str_address;
    }

    pub fn get_bootloader_name(self) -> Option<String> {
        if !self
            .flags
            .contains(MultibootInformationFlags::BOOTLOADER_NAME_VALID)
        {
            return None;
        }

        let mut bootloader_name = String::new();
        let mut curr_addr = self.boot_loader_name.as_ptr::<u8>();

        unsafe {
            loop {
                let curr_byte = core::ptr::read(curr_addr);
                if curr_byte == 0 {
                    break;
                }
                bootloader_name.push(char::from(curr_byte));
                curr_addr = curr_addr.add(1);
            }
        }

        Some(bootloader_name)
    }

    pub fn insert_framebuffer_info(&mut self, mode_info_block: ModeInfoBlock) {
        self.flags |= MultibootInformationFlags::FRAMEBUFFER_VALID;

        self.framebuffer = FramebufferMultibootInformation {
            addr: u64::from(mode_info_block.framebuffer).into(),
            pitch: u32::from(
                mode_info_block.bytes_per_scanline / u16::from(mode_info_block.bits_per_pixel >> 3),
            ),
            width: u32::from(mode_info_block.width),
            height: u32::from(mode_info_block.height),
            bpp: mode_info_block.bits_per_pixel,
            framebuffer_type: 1,
            red_field_pos: mode_info_block.red_field_pos,
            red_mask_size: mode_info_block.red_mask_s,
            green_field_pos: mode_info_block.green_field_pos,
            green_mask_size: mode_info_block.green_mask_s,
            blue_field_pos: mode_info_block.blue_field_pos,
            blue_mask_size: mode_info_block.blue_mask_s,
        };
    }
}

impl Default for MultibootInformation {
    fn default() -> Self {
        Self {
            flags: MultibootInformationFlags::NO_FLAGS | MultibootInformationFlags::MMAP_VALID,
            mem_lower: 0,
            mem_upper: 0,
            boot_device: MultibootBootDevice::default(),
            cmdline: PhyAddr32::new(0),
            mods_count: 0,
            mods_addr: PhyAddr32::new(0),
            syms: MultibootTableField {
                syms: MultibootSymbolTable::default(),
            },
            mmap_length: unsafe { E820_MAP_LENGTH },
            mmap_addr: PhyAddr32::new(E820_MAP_ADDR),
            drives_length: 0,
            drives_addr: PhyAddr32::new(0),
            config_table: PhyAddr32::new(0),
            boot_loader_name: PhyAddr32::new(0),
            apm_table: PhyAddr32::new(0),
            vbe: VbeMultibootInformation::default(),
            framebuffer: FramebufferMultibootInformation::default(),
        }
    }
}

/// Indicates which fields in the Multiboot information structure (['MultibootInformation'])
/// are available and usable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Pod, Zeroable)]
#[repr(C, packed)]
pub struct MultibootInformationFlags(u32);

impl MultibootInformationFlags {
    pub const NO_FLAGS: Self = Self(0);

    /// All `mem_*` fields of the information structure are valid if this bit is set.
    pub const MEM_FIELD_VALID: Self = Self(1 << 0);

    /// The `boot_device` field of the information structure is valid if this bit is set.
    pub const BOOT_DEVICE_VALID: Self = Self(1 << 1);

    /// The `cmdline` field of the information structure is valid if this bit is set.
    pub const CMDLINE_VALID: Self = Self(1 << 2);

    /// All `mods_` fields of the information structure are valid if this bit is set.
    pub const MODS_VALID: Self = Self(1 << 3);

    /// The `syms` field of the information structure indicates where the symbol table from
    /// a kernel image can be found.
    ///
    /// This bit and [`SHDR_TABLE_VALID`] are mutually exclusive.
    pub const SYMS_TABLE_VALID: Self = Self(1 << 4);

    /// The `syms` field of the information structure indicates  wwhere the section header
    /// from an ELF kernel can be found.
    ///
    /// This bit and [`SYMS_TABLE_VALID`] are mutually exclusive.
    pub const SHDR_TABLE_VALID: Self = Self(1 << 5);

    /// All `mmap_*` fields of the information structure are valid if this bit is set.
    pub const MMAP_VALID: Self = Self(1 << 6);

    /// All `drives_*` field of the information structure are valid if this bit is set.
    pub const DRIVES_VALID: Self = Self(1 << 7);

    /// The `config_table` field of the information structure is valid if this bit is set.
    pub const CONFIG_TABLE_VALID: Self = Self(1 << 8);

    /// The `boot_loader_name` field of the information structure is valid if this bit is set.
    pub const BOOTLOADER_NAME_VALID: Self = Self(1 << 9);

    /// The `apm_table` field of the information structure is valid if this bit is set.
    pub const APM_TABLE_VALID: Self = Self(1 << 10);

    /// The VBE table fields of the information structure is valid if this bit is set.
    pub const VBE_TABLE_VALID: Self = Self(1 << 11);

    /// The Framebuffer fields of the information structure is valid if this bit is set.
    pub const FRAMEBUFFER_VALID: Self = Self(1 << 12);

    pub fn contains(self, mode: Self) -> bool {
        self & mode != Self::NO_FLAGS
    }
}

impl core::ops::BitOr for MultibootInformationFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(rhs.0 | self.0)
    }
}

impl core::ops::BitAnd for MultibootInformationFlags {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl core::ops::BitOrAssign for MultibootInformationFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[repr(C)]
#[derive(Clone, Copy, Zeroable)]
pub union MultibootTableField {
    syms: MultibootSymbolTable,
    section_header: MultibootSectionHeaderTable,
}

unsafe impl Pod for MultibootTableField {}

impl core::fmt::Debug for MultibootTableField {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("table field")
    }
}

/// Contains information about the symbol table associated with a kernel image.
///
/// Contains the memory address of an array of `a.out` format _nlist_ structure,
/// followed by the array itself and then the size of an array of C-style zero
/// terminated strings, and finally the actual array of strings.
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
#[repr(C)]
pub struct MultibootSymbolTable {
    /// Size of the array of `a.out` format _nlist_ structures.
    tabsize: u32,

    /// Size of the C-style strings array.
    strsize: u32,

    /// Memory address of the symbol table information structure.
    addr: PhyAddr32,
    reserved: u32,
}

/// Contains information about the section header from an ELF kernel.
///
/// They correspond to the `shdr_*` entries in the ELF specification in
/// the program header.
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct MultibootSectionHeaderTable {
    num: u32,
    size: u32,
    addr: PhyAddr32,
    shndx: u32,
}

#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct VbeMultibootInformation {
    control_info: u32,
    mode_info: u32,
    mode: u16,
    interface_seg: u16,
    interface_off: u16,
    interface_len: u16,
}

#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct FramebufferMultibootInformation {
    addr: PhyAddr,
    pitch: u32,
    width: u32,
    height: u32,
    bpp: u8,
    framebuffer_type: u8,
    red_field_pos: u8,
    red_mask_size: u8,
    green_field_pos: u8,
    green_mask_size: u8,
    blue_field_pos: u8,
    blue_mask_size: u8,
}

/// Contains information about the disk device from which the OS image was loaded.
///
/// Part of the Multiboot information header.
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
#[repr(C, packed)]
pub struct MultibootBootDevice {
    /// Contains the _BIOS_ drive number, as defined by the _INT 13h_ disk interface.
    drive: u8,

    /// Specifies the top-level partition number.
    top_level_part: u8,

    /// Specifies a sub partition in the top-level partition.
    ///
    /// If unused, should be set to `OxFF`.
    sub_part: u8,

    /// Specifies a sub partition in the sub partition defined in the
    /// `sub_part` field.
    ///
    /// If unused, should be set to `0xFF`.
    sub_sub_part: u8,
}
