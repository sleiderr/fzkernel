//! Internal data structures used by the `ELF` format.

use crate::mem::VirtAddr;

pub struct ElfHeader64 {
    /// Contains architecture independent information on how to decode the file's content.
    ident: ElfIdentification,

    /// Identifies the object file type.
    file_type: ElfFileType,

    /// Used to specify the required architecture for this file.
    arch: ElfMachineArch,

    /// Identifies the object file version.
    version: u32,

    /// Virtual address used when starting the process.
    entry: VirtAddr,

    /// Offset of the program header table (in bytes).
    prog_header_offset: u64,

    /// Offset of the section header table (in bytes).
    sect_header_offset: u64,

    /// Architecture-specific flags associated to this file.
    flags: u32,

    /// ELF Header size (in bytes).
    header_size: u16,

    /// Size of a single entry in the program header table (in bytes).
    prog_header_size: u16,

    /// Number of entries in the program header table.
    prog_header_entries_count: u16,

    /// Size of a section header (in bytes).
    sect_header_size: u16,

    /// Number of entries in the section header table.
    sect_header_entries_count: u16,

    /// Section header table index of the entry associated with the section name string table.
    sect_name_string_table_idx: u16,
}

pub struct ElfSectionHeader64 {
    name: u32,
    section_type: ElfSectionType,
    section_flags: ElfSectionFlags,
    addr: VirtAddr,
    offset: u64,
    size: u64,
    link: u32,
    info: u32,
    addr_align: u64,
    entry_size: u64,
}

pub struct ElfSymbolTableEntry64 {
    name: u32,
    info: ElfSymbolInfo,
    reserved: u8,
    section_table_idx: u16,
    symbol_value: VirtAddr,
    size: u64,
}

pub struct ElfSymbolInfo(u8);

pub struct ElfSectionFlags(u64);

pub struct ElfRelEntry {
    address: VirtAddr,
    info: ElfRelInfo,
}

pub struct ElfRelaEntry {
    offset: VirtAddr,
    info: ElfRelInfo,
    addend: u64,
}

pub struct ElfProgramHeaderEntry {
    seg_type: ElfSegmentType,
    flags: ElfSegmentFlags,
    offset: u64,
    virt_addr: VirtAddr,
    reserved: u64,
    file_seg_size: u64,
    mem_seg_size: u64,
    align: u64,
}

#[repr(u8)]
#[derive(Clone, Copy, Default)]
pub enum ElfSegmentType {
    Null = 0,
    Load = 1,
    Dynamic = 2,
    Interpreter = 3,
    Note = 4,
    ProgramHeaderTable = 6,

    #[default]
    Unknown = 0xFFFF,
}

pub struct ElfSegmentFlags(u32);

pub struct ElfRelInfo(u64);

#[repr(u32)]
#[derive(Clone, Copy, Default)]
pub enum ElfSectionType {
    Null = 0,
    ProgBits = 1,
    SymbolTable = 2,
    StringTable = 3,
    Rela = 4,
    SymbolHashTable = 5,
    DynamicLinkingTable = 6,
    NoteInformation = 7,
    NoBits = 8,
    Rel = 9,
    DynamicSymbolTable = 11,

    #[default]
    Unknown = 0xFFFF_FFFF,
}

pub struct ElfIdentification {
    /// Contains a magic number, used to identify `ELF` files.
    magic: [u8; 4],

    /// Identifies the file class (or capacity).
    class: ElfClass,

    /// Specifies the encoding of the processor-specific data in the object file.
    encoding: ElfDataEncoding,

    /// `ELF` header version number.
    version: u8,

    os_abi: ElfOsAbiIdent,
    abi_version: u8,
    reserved: [u8; 7],
}

#[repr(u8)]
pub enum ElfClass {
    None = 0,
    Class32 = 1,
    Class64 = 2,
}

#[repr(u8)]
pub enum ElfDataEncoding {
    None = 0,
    Lsb = 1,
    Msb = 2,
}

#[repr(u8)]
pub enum ElfOsAbiIdent {
    SysV = 0,
    HPUX = 1,
    Standalone = 0xFF,
}

#[repr(u16)]
pub enum ElfFileType {
    None = 0,
    Reloc = 1,
    Exec = 2,
    SharedObject = 3,
    Core = 4,
}

#[repr(u16)]
#[derive(Clone, Copy, Default)]
pub enum ElfMachineArch {
    None = 0,

    #[default]
    Unknown = 0xFFFF,
}
