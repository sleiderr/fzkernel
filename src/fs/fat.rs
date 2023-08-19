//! FrozenBoot FAT filesystem support.

/// The `BiosParameterBlock` contains the FAT file-system metadata.
///
/// It is located on the first sector of the volume, which may be called the `boot sector`. The
/// implementation of the `BiosParameterBlock` depends on the FAT version (12, 16 or 32).
#[repr(C, packed)]
pub struct BiosParameterBlock {
    /// Jump instruction to boot code
    bs_jmpboot: [u8; 3],

    /// OEM Name Identifier
    bs_oemname: [u8; 8],

    /// Count of bytes per sector.
    bpb_byts_per_sec: u16,

    /// Number of sectors per allocation unit.
    ///
    /// This value must be a positive power of two
    bpb_sec_per_clus: u8,

    /// Number of reserved sectors in the reserved region of the volume starting at the first
    /// sector of the volume.
    ///
    /// This must not be null, but can be any non zero value
    bpb_rsvd_sec_cnt: u16,

    /// The count of File Allocation Tables (FAT) on the volume
    ///
    /// A value of 2 is recommended (but 1 is acceptable)
    bpb_num_fats: u8,

    /// Must be null
    reserved: u32,

    /// Media descriptor byte
    bpb_media: u8,

    /// Must be null
    reserved2: u16,

    /// Sectors per track for interrupt 13h
    bpb_sec_per_trk: u16,

    /// Number of heads for interrupt 13h
    num_heads: u16,

    /// Count of hidden sectors preceding the partition that contains the FAT volume.
    bpb_hidd_sec: u32,

    /// 32-bit total count of sectors on the volume.
    bpb_tot_sec_32: u32,

    /// FAT32 32-bit count of sectors occupied by one FAT
    bpb_fats_z32: u32,

    /// Flags
    bpb_ext_flags: u16,

    /// Version number.
    ///
    /// High byte is the major version, and the low byte is the minor version
    bpb_fs_ver: u16,

    /// Cluster number of the first cluster of the root directory
    bpb_root_clus: u32,

    /// Sector number of FSINFO structure in the reserved area of the FAT32 volume
    bpb_fs_info: u16,

    /// Indicates the sector number in the reserved area of a copy of the boot record.
    bpb_bk_boot_sec: u16,

    reserved3: [u8; 12],

    /// Interrupt 13h drive number.
    bs_drv_num: u8,

    reserved4: u8,

    /// Extended boot signature.
    ///
    /// Set to `0x29` if either of the following two fields are non-zero.
    bs_bootsig: u8,

    /// Volume serial number
    bs_vol_id: u32,

    /// Volume label
    bs_vol_lab: [u8; 11],

    /// Should be set to string `"FAT32   `
    bs_fil_sys_type: [u8; 8],
}
/// A `FatEntry` is an entry in the File Allocation Table (FAT).
#[repr(u32)]
pub enum FatEntry {
    /// Cluster is free
    Free,

    /// Cluster is allocated.
    ///
    /// The contained value is the cluster number of the next cluster.
    Allocated(usize),

    /// Bad cluster
    Defective,

    /// Cluster is allocated and is the final cluster of the file
    EOF,
}

/// `FSInfo` stands for File System Information.
///
/// It contains several information that helps optimizing file system implementation.
#[repr(C, packed)]
pub struct FSInfo {
    /// Lead signature
    ///
    /// Should be `0x41615252`
    fsi_leadsig: u32,

    /// Must be null
    reserved: [u8; 480],

    /// Additional signature.
    ///
    /// Should be `0x61417272`
    fsi_struc_sig: u32,

    /// Contains the last known free cluster count on the volume.
    ///
    /// Must be validated on volume mount
    fsi_free_count: u32,

    /// Contains the cluster number of the first available free cluster on the volume.
    ///
    /// The value 0xFFFFFFFF indicates the lack of information
    fsi_nxt_free: u32,

    /// Must be null
    reserved2: [u8; 12],

    /// Trailing signature
    ///
    /// Should be `0xAA550000`
    fsi_trail_sig: [u8; 4],
}

/// FAT Directory contents are a series of `DirectoryEntry`, which represents a contained file or a
/// sub-directory entry.
#[repr(C, packed)]
pub struct DirectoryEntry {
    /// Short file name (11 characters at most).
    ///
    /// It is composed of 2 parts:
    ///
    /// - the 8-character main part of the name
    /// - the 3-character extension
    dir_name: [u8; 11],

    /// File attributes
    dir_attr: u8,

    /// Must be null
    reserved: u8,

    /// Component of the file creation time
    ///
    /// Count of tenths of a second
    dir_crt_time_tenth: u8,

    /// Creation time with a granularity of 2 seconds.
    dir_crt_time: u16,

    /// Creation date
    dir_crt_date: u16,

    /// Last access date
    dir_lst_acc_date: u16,

    /// High 16-bits of first data cluster number for file/directory described by this entry
    dir_fst_clus_hi: u16,

    /// Last modification (write) time
    dir_wrt_time: u16,

    /// Last modification (write) date
    dir_wrt_date: u16,

    /// Low 16-bits of first data cluster number for file/directory described by this entry
    dir_fst_clus_lo: u16,

    /// 32-bit quantity containing the size bytes for the file/directory described by this entry
    dir_file_size: u32,
}

pub mod file_attr {
    //! Attribute values associated with a file or a sub-directory.

    /// The file cannot be modified.
    pub const ATTR_READ_ONLY: u8 = 0x01;

    /// The corresponding file or sub-directory must not be listed unless an explicit request is
    /// issued.
    pub const ATTR_HIDDEN: u8 = 0x02;

    /// The corresponding file is tagged as a component of the operating system. It must not be
    /// listed unless an explicit request is issued.
    pub const ATTR_SYSTEM: u8 = 0x04;

    /// The corresponding entry contains the volume label.
    pub const ATTR_VOLUME_ID: u8 = 0x08;

    /// The corresponding entry represents a directory.
    pub const ATTR_DIRECTORY: u8 = 0x10;

    /// This attribute must be set when the file is created, renamed or modified.
    ///
    /// For instance, it may be used by backup utilities to determine which files need to be backed
    /// up.
    pub const ATTR_ARCHIVE: u8 = 0x20;
}
