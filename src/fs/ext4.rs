use core::{mem, ptr};

use alloc::{string::String, vec::Vec};

use crate::{
    drivers::ahci::get_sata_drive,
    error,
    errors::{CanFail, IOError},
    info,
};
use crate::{errors::MountError, fs::FsFile};

pub const EXT4_SIGNATURE: u16 = 0xEF53;

pub const EXT4_CHKSUM_TYPE_CRC32: u8 = 0x1;

/// Compression feature flag (not implemented)
pub const EXT4_FEATURE_INCOMPAT_COMPRESSION: u32 = 0x0001;

/// Directory preallocation.
pub const EXT4_FEATURE_COMPAT_DIR_PREALLOC: u32 = 0x0001;

/// Used by AFS to indicate inodes that are not linked into the directory namespace.
pub const EXT4_FEATURE_COMPAT_IMAGIC_INODES: u32 = 0x0002;

/// Create a journal file to ensure file system consistency (even across dirty shutdowns).
pub const EXT4_FEATURE_COMPAT_HAS_JOURNAL: u32 = 0x0004;

/// This feature enables the use of extended attributes.
pub const EXT4_FEATURE_COMPAT_EXT_ATTR: u32 = 0x0008;

/// This feature indicates that space has been reserved so that the block group descriptor table
/// can be extended while resizing a mounted file system.
///
/// This features requires that the [`EXT4_FEATURE_R0_COMPAT_SPARSE_SUPER`] or
/// [`EXT4_FEATURE_COMPAT_SPARSE_SUPER2`] feature be enabled.
pub const EXT4_FEATURE_COMPAT_RESIZE_INODE: u32 = 0x0010;

/// Use hashed B-trees to speed up name lookup in large directories.
pub const EXT4_FEATURE_COMPAT_DIR_INDEX: u32 = 0x0020;

/// This feature indicates that there will only be at most 2 backup superblocks and block group
/// descriptors.
pub const EXT4_FEATURE_COMPAT_SPARSE_SUPER2: u32 = 0x0200;

pub const EXT4_FEATURE_COMPAT_FAST_COMMIT: u32 = 0x0400;

/// Marks the file system's inode numbers and UUID as stable.
///
/// This feature allows the use of specialized encryption settings that will make use of the inode
/// numbers and UUID.
pub const EXT4_FEATURE_COMPAT_STABLE_INODES: u32 = 0x0800;

pub const EXT4_FEATURE_COMPAT_ORPHAN_FILE: u32 = 0x1000;

/// This file system feature indicates that backup copies of the superblock are present only in a
/// subset of block groups.
pub const EXT4_FEATURE_R0_COMPAT_SPARSE_SUPER: u32 = 0x0001;

/// This feature flag is usually set when a file larger than 2 GB is created.
pub const EXT4_FEATURE_R0_COMPAT_LARGE_FILE: u32 = 0x0002;

pub const EXT4_FEATURE_R0_COMPAT_BTREE_DIR: u32 = 0x0004;

/// This feature allows files to be larger than 2 TB in size.
pub const EXT4_FEATURE_R0_COMPAT_HUGE_FILE: u32 = 0x0008;

/// Group descriptors have checksums.
pub const EXT4_FEATURE_R0_COMPAT_GDT_CSUM: u32 = 0x0010;

/// This feature lifts the usual 65,000 hard links limit per inode.
pub const EXT4_FEATURE_DIR_NLINK: u32 = 0x0020;

/// This feature reserves a specific amount of space in each node for extended metadata (ns
/// timestamps, or file creation time).
///
/// For this feature to be useful, the inode size must be 256 bytes in size or larger.
pub const EXT4_FEATURE_R0_COMPAT_EXTRA_ISIZE: u32 = 0x0040;

/// This creates quota inodes and set them in the [`Superblock`].
pub const EXT4_FEATURE_R0_COMPAT_QUOTA: u32 = 0x0100;

/// This feature enables clustered block allocation, so that the unit of allocation is a power of
/// two number of blocks.
///
/// Requires that the `EXT4_FEATURE_INCOMPAT_EXTENTS` feature be enabled.
pub const EXT4_FEATURE_R0_COMPAT_BIGALLOC: u32 = 0x0200;

/// This feature enables metadata checksumming.
///
/// This stores checksums for all of the file system metadata.
pub const EXT4_FEATURE_R0_COMPAT_METADATA_CSUM: u32 = 0x0400;

/// Read-only file system image.
pub const EXT4_FEATURE_R0_COMPAT_READONLY: u32 = 0x1000;

/// This feature provides project quota support.
pub const EXT4_FEATURE_R0_COMPAT_PROJECT: u32 = 0x2000;

/// This feature enables support for verity protected files.
///
/// Verity files are read-only, and their data is transparently verified against a Merkle tree
/// hidden past the end of the file.
/// This is most useful for authenticating important read-only files on read-write file systems.
/// If the file system is read-only, using `dm-verity` to authenticate the entire block may provide
/// much better security.
pub const EXT4_FEATURE_R0_COMPAT_VERITY: u32 = 0x8000;

pub const EXT4_FEATURE_R0_COMPAT_ORPHAN_PRESENT: u32 = 0x10000;

/// Enables the storage of file type information in directory entries.
pub const EXT4_FEATURE_INCOMPAT_FILETYPE: u32 = 0x0002;

/// File system needs journal recovery.
pub const EXT4_FEATURE_INCOMPAT_RECOVER: u32 = 0x0004;

/// This feature is enabled on the [`Superblock`] found on an external journal device.
pub const EXT4_FEATURE_INCOMPAT_JOURNAL_DEV: u32 = 0x0008;

/// This feature allows file systems to be resized on-line without explicitly needing to reserve
/// space for growth in size of the block group descriptors.
pub const EXT4_FEATURE_INCOMPAT_META_BG: u32 = 0x0010;

/// This feature allow the mapping of logical block numbers for a particular inode to physical
/// blocks on the storage device to be stored using an extent tree, a more efficient data structure
/// than the traditional indirect block scheme used by `ext2` and `ext3` filesystems.
pub const EXT4_FEATURE_INCOMPAT_EXTENTS: u32 = 0x0040;

/// This feature allows for a file system size above 2^32 blocks.
pub const EXT4_FEATURE_INCOMPAT_64BIT: u32 = 0x0080;

/// This feature provides Multiple mount protection (useful in shared environments).
pub const EXT4_FEATURE_INCOMPAT_MMP: u32 = 0x0100;

/// This feature allow the per-block group metadata to be placed on the storage media.
pub const EXT4_FEATURE_INCOMPAT_FLEX_BG: u32 = 0x0200;

/// This feature allow the value of each extended attribute to be placed in the data block of a
/// separate inode if necessary, increasing the limit on the size and number of extended attributes
/// per file.
pub const EXT4_FEATURE_INCOMPAT_EA_INODE: u32 = 0x0400;

/// Data in directory entry.
///
/// This allows additional data fields to be stored in each directory entry.
pub const EXT4_FEATURE_INCOMPAT_DIRDATA: u32 = 0x1000;

/// This feature allows the file system to store the metadata checksum seed in the superblock,
/// which allows the administrator to change the UUID of a file system while it is mounted.
pub const EXT4_FEATURE_INCOMPAT_CSUM_SEED: u32 = 0x2000;

/// This feature increases the limit on the number of files per directory.
pub const EXT4_FEATURE_INCOMPAT_LARGEDIR: u32 = 0x4000;

/// This feature allows data to be stored in the inode and extended attribute area.
pub const EXT4_FEATURE_INCOMPAT_INLINE_DATA: u32 = 0x8000;

/// This feature enables support for file system level encryption of data block and file names. The
/// inode metadata is _not_ encrypted
pub const EXT4_FEATURE_INCOMPAT_ENCRYPT: u32 = 0x10000;

/// This feature provides file system level character encoding support for directories with the
/// casefold (+F) flag enabled.
pub const EXT4_FEATURE_INCOMPAT_CASEFOLD: u32 = 0x20000;

pub struct Ext4FS {
    drive_id: usize,
    partition_id: usize,
    superblock: Ext4Superblock,
    group_descriptors: Vec<GroupDescriptor>,
}

impl Ext4FS {
    pub fn mount(drive_id: usize, partition_id: usize) -> Result<Self, MountError> {
        let mut drive = get_sata_drive(drive_id).lock();
        let partition_data = drive
            .partitions
            .get(partition_id)
            .ok_or(MountError::Unknown)?;

        let sb_start_lba = (1024 / drive.logical_sector_size() as u64) + partition_data.start_lba();
        let sb_size_in_lba = mem::size_of::<Ext4Superblock>() as u32 / drive.logical_sector_size();

        let mut raw_sb = [0u8; mem::size_of::<Ext4Superblock>()];
        drive
            .read(sb_start_lba as u64, sb_size_in_lba as u16, &mut raw_sb)
            .map_err(|_| MountError::IOError)?;

        let sb = unsafe { ptr::read(raw_sb.as_ptr() as *mut Ext4Superblock) };

        if sb.s_checksum_type == EXT4_CHKSUM_TYPE_CRC32 {
            let sb_chcksum = crc32c_calc(&raw_sb[..mem::size_of::<Ext4Superblock>() - 4]);

            if sb_chcksum != sb.s_checksum {
                error!(
                    "ext4-fs",
                    "found ext4 filesystem with invalid superblock checksum (got {:#010x} expected {:#010x})",
                    sb_chcksum,
                    unsafe { ptr::read_unaligned(ptr::addr_of!(sb.s_checksum)) }
                );
                return Err(MountError::BadSuperblock);
            }
        }

        if sb.s_magic != EXT4_SIGNATURE {
            return Err(MountError::BadSuperblock);
        }

        info!(
            "ext4-fs",
            "mounted ext4 filesystem on drive {drive_id} partition {partition_id}"
        );

        info!(
            "ext4-fs",
            "label = {}    inodes_count = {}    blk_count = {}    mmp = {}    opts = {}",
            sb.label(),
            sb.inode_count(),
            sb.blk_count(),
            sb.mmp_enabled(),
            sb.mount_opts()
        );

        let mut fs = Self {
            drive_id,
            partition_id,
            superblock: sb,
            group_descriptors: alloc::vec![],
        };
        drop(drive);

        fs.load_group_descriptors()
            .map_err(|_| MountError::IOError)?;

        Ok(fs)
    }

    pub fn load_group_descriptors(&mut self) -> CanFail<IOError> {
        self.group_descriptors.clear();

        for i in 0..self.superblock.bg_count() {
            let gd = self.__read_bg_descriptor(i)?;
            self.group_descriptors.push(gd);
        }

        Ok(())
    }

    fn __read_blk(&self, blk_id: u64, buffer: &mut [u8]) -> CanFail<IOError> {
        if blk_id > self.superblock.blk_count() {
            return Err(IOError::InvalidCommand);
        }
        let mut drive = get_sata_drive(self.drive_id).lock();
        let partition_data = drive
            .partitions
            .get(self.partition_id)
            .ok_or(IOError::Unknown)?;

        let sectors_count = self.superblock.blk_size() / drive.logical_sector_size() as u64;
        let start_lba = partition_data.start_lba()
            + (blk_id * self.superblock.blk_size()) / drive.logical_sector_size() as u64;

        drive.read(start_lba, sectors_count as u16, buffer)
    }

    pub fn __read_bg_descriptor(&self, bg_id: u64) -> Result<GroupDescriptor, IOError> {
        assert!(bg_id < self.superblock.bg_count());

        let descriptor_size = if self.superblock.feat_64bit_support() {
            64
        } else {
            32
        };

        let initial_blk_offset =
            if self.superblock.blk_size() == mem::size_of::<Ext4Superblock>() as u64 {
                2
            } else {
                1
            };

        let descriptor_per_block = self.superblock.blk_size() / descriptor_size;
        let desc_blk_id =
            initial_blk_offset + (bg_id * descriptor_size) / self.superblock.blk_size();
        let desc_idx_in_blk = bg_id % descriptor_per_block;

        let mut blk = alloc::vec![0; self.superblock.blk_size() as usize];
        self.__read_blk(desc_blk_id, &mut blk)?;

        let raw_bg_descriptor = &blk[((desc_idx_in_blk * descriptor_size) as usize)
            ..(((desc_idx_in_blk + 1) * descriptor_size) as usize)];

        if self.superblock.feat_64bit_support() {
            Ok(GroupDescriptor::Size64(unsafe {
                ptr::read(raw_bg_descriptor.as_ptr() as *const GroupDescriptor64)
            }))
        } else {
            Ok(GroupDescriptor::Size32(unsafe {
                ptr::read(raw_bg_descriptor.as_ptr() as *const GroupDescriptor32)
            }))
        }
    }

    fn __read_inode(&self, inode_id: u64) -> Result<Inode, IOError> {
        let inode_blk_group = (inode_id - 1) / self.superblock.inodes_per_group() as u64;
        let inode_bg_idx = (inode_id - 1) % self.superblock.inodes_per_group() as u64;
        let inode_byte_idx = inode_bg_idx * self.superblock.s_inode_size as u64;

        let inode_blk_offset = inode_byte_idx / self.superblock.blk_size();
        let inode_bytes_idx_in_blk = inode_byte_idx % self.superblock.blk_size();

        let descriptor = self
            .group_descriptors
            .get(inode_blk_group as usize)
            .ok_or(IOError::Unknown)?;

        let mut raw_inode_blk = alloc::vec![0; self.superblock.blk_size() as usize];

        match descriptor {
            GroupDescriptor::Size32(desc) => self.__read_blk(
                inode_blk_offset + desc.inode_table_blk_addr() as u64,
                &mut raw_inode_blk,
            ),
            GroupDescriptor::Size64(desc) => self.__read_blk(
                inode_blk_offset + desc.inode_table_blk_addr(),
                &mut raw_inode_blk,
            ),
        }?;

        let raw_inode = &raw_inode_blk[(inode_bytes_idx_in_blk as usize)
            ..(inode_bytes_idx_in_blk + mem::size_of::<Inode>() as u64) as usize];

        unsafe { Ok(ptr::read(raw_inode.as_ptr() as *const Inode)) }
    }
}

/// The ext4 `Superblock` hold useful information about the filesystem's characteristics and
/// attributes (block count, sizes, required features, ...).
///
/// A copy of the partition's `Superblock` is kept in all groups, except if the `sparse_super`
/// feature is enabled, in which case it is only kept in groups whose group number is either 0 or a
/// power of 3, 5, 7.
#[derive(Debug)]
#[repr(C, packed)]
pub struct Ext4Superblock {
    /// Inodes count
    pub s_inodes_count: u32,

    /// Blocks count
    pub s_blocks_count: u32,

    /// Reserved blocks count
    pub s_r_blocks_count: u32,

    /// Free blocks count
    pub s_free_blocks_count: u32,

    /// Free inodes count
    pub s_free_inodes_count: u32,

    /// First Data Block.
    ///
    /// Block number of the block containing the `Superblock`
    pub s_first_datablock: u32,

    /// Block size.
    ///
    /// Defined as `log_2(block_size) - 10`
    pub s_log_block_size: u32,

    /// Allocation cluster size.
    ///
    /// Defined as `log_2(cluster_size) - 10`
    pub s_log_cluster_size: u32,

    /// Number of blocks in each group
    pub s_blocks_per_group: u32,

    /// Number of clusters in each group
    pub s_clusters_per_group: u32,

    /// Number of inodes in each group
    pub s_inodes_per_group: u32,

    /// Last mount time
    pub s_mtime: u32,

    /// Last write time
    pub s_wtime: u32,

    /// Mount count (since last consistency check)
    pub s_mnt_count: u16,

    /// Number of mounts allowed before a consistency check is required
    pub s_max_mnt_count: u16,

    /// `ext4` magic signature: `0xef53`
    pub s_magic: u16,

    /// File system state
    pub s_state: u16,

    /// Behavior on error detection
    pub s_errors: u16,

    /// Minor revision level
    pub s_minor_rev_level: u16,

    /// Time of last consistency check
    pub s_lastcheck: u32,

    /// Max time between successive consistency checks
    pub s_checkinterval: u32,

    /// Operating System ID from which the filesystem was created
    pub s_creator_os: u32,

    /// Major revision level
    pub s_rev_level: u32,

    /// Default user ID for reserved blocks
    pub s_def_resuid: u16,

    /// Default group ID for reserved blocks
    pub s_def_resgid: u16,

    /// First non-reserved inode in file system
    pub s_first_ino: u32,

    /// Size of each inode structure in bytes
    pub s_inode_size: u16,

    /// Block group number of this superblock
    pub s_block_group_nr: u16,

    /// Compatible feature set
    pub s_feature_compat: u32,

    /// Incompatible feature set
    pub s_feature_incompat: u32,

    /// Read-only compatible feature set
    pub s_feature_ro_compat: u32,

    /// 128-bit UUID for volume
    pub s_uuid: [u8; 16],

    /// Volume name
    pub s_volume_name: [u8; 16],

    /// Path volume was last mounted to
    pub s_last_mounted: [u8; 64],

    /// Compression algorithm used
    pub s_algo_bitmap: u32,

    /// Number of blocks to try to preallocate for files
    pub s_prealloc_blocks: u8,

    /// Number of block to preallocate for directories
    pub s_prealloc_dir_block: u8,

    pub s_reserved_gdt_blocks: u16,

    /// UUID of journal Superblock
    pub s_journal_uuid: [u8; 16],

    /// Inode number of journal file
    pub s_journal_inum: u32,

    /// Device number of journal file
    pub s_journal_dev: u32,

    /// Start of list of inodes to delete (orphan nodes)
    pub s_last_orphan: u32,

    /// HTREE hash seed
    pub s_hash_seed: [u32; 4],

    /// Default hash version to use
    pub s_def_hash_version: u8,

    pub s_jnl_backup_type: u8,
    pub s_desc_size: u16,

    /// Default mount options
    pub s_default_mount_options: u32,

    /// First metablock block group, if enabled
    pub s_first_meta_bg: u32,

    /// File system creation time
    pub s_mkfs_time: u32,

    /// Backup of the journal inode
    pub s_jnl_blocks: [u32; 17],

    // Valid if the 64bit support is enabled `EXT4_FEATURE_INCOMPAT_64BIT`
    /// Blocks count high 32-bits
    pub s_blocks_count_hi: u32,

    /// Reserved blocks count high 32-bits
    pub s_r_blocks_count_hi: u32,

    /// Free blocks count high 32-bits
    pub s_free_blocks_count_hi: u32,

    /// Minimum inode size
    pub s_min_extra_isize: u16,

    /// Minimum inode reservation size
    pub s_want_extra_isize: u16,

    /// Miscellaneous flags
    pub s_flags: u32,

    /// Amount of logical blocks read of written per disk in a `RAID` array
    pub s_raid_stride: u16,

    /// Number of seconds to wait in Multi-mount prevention checking
    pub s_mmp_interval: u16,

    /// Block for Multi-mount protection
    pub s_mmp_block: u64,

    /// Amount of blocks to read or write before returning to the current disk in a RAID array
    /// (N * stride)
    pub s_raid_stripe_width: u32,

    /// `FLEX_BG` group size
    ///
    /// Defined as `log_2(groups_per_flex) - 10`
    pub s_log_groups_per_flex: u8,

    /// Metadata checksum algorithm used
    pub s_checksum_type: u8,

    /// Padding to next 32 bits
    s_reserved_pad: u16,

    /// Amount of KBs written
    pub s_kbytes_written: u64,

    /// Inode number of the active snapshot
    pub s_snapshot_inum: u32,

    /// Sequential ID of active snapshot
    pub s_snapshot_id: u32,

    /// Reserved blocks for active snapshot future use
    pub s_snapshot_r_blocks_count: u64,

    /// Inode number of the head of the on-disk snapshot list
    s_snapshot_list: u32,

    /// Number of filesystem errors
    s_error_count: u32,

    /// First time an error occured
    s_first_error_time: u32,

    /// Inode number in the first error
    s_first_error_ino: u32,

    /// Block number in the first error
    s_first_error_block: u64,

    /// Function where the first error occured
    s_first_error_func: [u8; 32],

    /// Line number where the first error occured
    s_first_error_line: u32,

    /// Last time an error occured
    s_last_error_time: u32,

    /// Inode number of the last error
    s_last_error_ino: u32,

    /// Line number where the last error occured
    s_last_error_line: u32,

    /// Block number in the last error
    s_last_error_block: u64,

    /// Function where the last error occured
    s_last_error_func: [u8; 32],

    /// Mount options (C string)
    s_mount_opts: [u8; 64],

    /// Inode number for user quota file
    s_usr_quota_inum: u32,

    /// Inode number for group quota file
    s_grp_quota_inum: u32,

    /// Overhead block/clusters in file system
    s_overhead_blocks: u32,

    /// Block groups with backup `Superblock`s if the sparse superblock is set
    s_backup_bgs: [u32; 2],

    /// Encryption algorithm used
    s_encrypt_algos: [u8; 4],

    /// Salt used for `string2key` algorithm
    s_encrypt_pw_salt: [u8; 16],

    /// Location of the lost+found inode
    s_lpf_ino: u32,

    /// Inode for tracking project quota
    s_prj_quota_inum: u32,

    /// `crc32c(uuid)` if `csum_seed` is set
    s_checksum_seed: u32,

    /// High 8-bits of the last written time field
    s_wtime_hi: u8,

    /// High 8-bits of the last mount time field
    s_mtime_hi: u8,

    /// High 8-bits of the filesystem creation time field
    s_mkfs_time_hi: u8,

    /// High 8-bits of the last consistency check time field
    s_lastcheck_hi: u8,

    /// High 8-bits of the first error time field
    s_first_error_time_hi: u8,

    /// High 8-bits of the last error time field
    s_last_error_time_hi: u8,

    /// Error code of the first error
    s_first_error_errcode: u8,

    /// Error code of the last error
    s_last_error_errcode: u8,

    /// Filename charset encoding
    s_encoding: u16,

    /// Filename charset encoding flags
    s_encoding_flags: u16,

    s_reserved: [u32; 95],

    /// Checksum of the superblock: `crc32c(superblock)`
    s_checksum: u32,
}

impl Ext4Superblock {
    /// Checks if this `ext4` filesystem uses 64 bit features.
    pub fn feat_64bit_support(&self) -> bool {
        self.s_feature_incompat & EXT4_FEATURE_INCOMPAT_64BIT != 0
    }

    /// Checks is a feature part of the incompatible set is available for this `Ext4FS`.
    pub fn incompat_contains(&self, flag: u32) -> bool {
        self.s_feature_incompat & flag != 0
    }

    /// Returns the number of Block Groups for this filesystem.
    pub fn bg_count(&self) -> u64 {
        1 + self.blk_count() / self.blocks_per_group() as u64
    }

    /// Returns a C-style string describing this filesystem's mount options.
    pub fn mount_opts(&self) -> String {
        let opts_bytes: Vec<u8> = self
            .s_mount_opts
            .into_iter()
            .take_while(|&ch| ch != 0 && ch.is_ascii())
            .collect();

        String::from_utf8(opts_bytes).unwrap_or_else(|_| String::from(""))
    }

    /// Returns the number of blocks per block group.
    pub fn blocks_per_group(&self) -> u32 {
        self.s_blocks_per_group
    }

    /// Returns the number of inodes per block group.
    pub fn inodes_per_group(&self) -> u32 {
        self.s_inodes_per_group
    }

    /// Returns the total count of inodes.
    pub fn inode_count(&self) -> u32 {
        self.s_inodes_count
    }

    /// Returns the number of free inodes.
    pub fn free_inodes_count(&self) -> u32 {
        self.s_free_inodes_count
    }

    /// Returns the number of free blocks.
    pub fn free_blk_count(&self) -> u64 {
        if self.feat_64bit_support() {
            (self.s_free_blocks_count as u64) | ((self.s_free_blocks_count_hi as u64) << 32)
        } else {
            self.s_free_blocks_count as u64
        }
    }

    /// Returns the total count of blocks.
    pub fn blk_count(&self) -> u64 {
        if self.feat_64bit_support() {
            (self.s_blocks_count as u64) | ((self.s_blocks_count_hi as u64) << 32)
        } else {
            self.s_blocks_count as u64
        }
    }

    /// Returns the size of a block, in bytes.
    pub fn blk_size(&self) -> u64 {
        1024 << self.s_log_block_size
    }

    /// Checks if this `ext4` filesystem uses the _Multi Mount Protection_ (`MMP`) feature.
    pub fn mmp_enabled(&self) -> bool {
        self.s_feature_incompat & EXT4_FEATURE_INCOMPAT_MMP != 0
    }

    /// Returns this filesystem's label.
    pub fn label(&self) -> String {
        let label_bytes: Vec<u8> = self
            .s_volume_name
            .into_iter()
            .take_while(|&ch| ch != 0 && ch.is_ascii())
            .collect();

        String::from_utf8(label_bytes).unwrap_or_else(|_| {
            error!("ext4-fs", "invalid volume label");
            String::from("")
        })
    }
}

/// Each block group on the file system has a `GroupDescriptor` associated with it.
///
/// A `block group` is a logical grouping of contiguous block.
#[derive(Debug)]
pub enum GroupDescriptor {
    Size32(GroupDescriptor32),
    Size64(GroupDescriptor64),
}

/// 32-bit version of the [`GroupDescriptor`].
///
/// Used if [`EXT4_FEATURE_INCOMPAT_64BIT`] is clear.
#[repr(C, packed)]
#[derive(Debug)]
pub struct GroupDescriptor32 {
    /// 32-bit location of block bitmap
    bg_block_bitmap: u32,

    /// 32-bit location of inode bitmap
    bg_inode_bitmap: u32,

    /// 32-bit location of inode table
    bg_inode_table: u32,

    /// 16-bit free block count
    bg_free_blocks_count: u16,

    /// 16-bit free inode count
    bg_free_inodes_count: u16,

    /// 16-bit directory count
    bg_used_dirs_count: u16,

    /// Block group flags
    bg_flags: u16,

    /// 32-bit location of snapshot exclusion bitmap
    bg_exclude_bitmap_lo: u32,

    /// 16-bit of the block bitmap checksum
    bg_block_bitmap_csum_lo: u16,

    /// 16-bit of the inode bitmap checksum
    bg_inode_bitmap_csum_lo: u16,

    /// 16-bit unused inode count
    bg_itable_unused_lo: u16,

    /// Group descriptor checksum
    bg_checksum: u16,
}

impl GroupDescriptor32 {
    pub fn inode_table_blk_addr(&self) -> u32 {
        self.bg_inode_table
    }
}

/// 64-bit version of the [`GroupDescriptor`]
///
/// Used if [`EXT4_FEATURE_INCOMPAT_64BIT`] is set.
#[repr(C, packed)]
#[derive(Debug)]
pub struct GroupDescriptor64 {
    /// Lower 32-bit of location of block bitmap
    bg_block_bitmap_lo: u32,

    /// Lower 32-bit of location of inode bitmap
    bg_inode_bitmap_lo: u32,

    /// Lower 32-bit of location of inode table
    bg_inode_table_lo: u32,

    /// Lower 16-bit of free block count
    bg_free_blocks_count_lo: u16,

    /// Lower 16-bit of free inode count
    bg_free_inodes_count_lo: u16,

    /// Lower 16-bit of directory count
    bg_used_dirs_count_lo: u16,

    /// Block group flags
    bg_flags: u16,

    /// Lower 32-bit of location of snapshot exclusion bitmap
    bg_exclude_bitmap_lo: u32,

    /// Lower 16-bit of the block bitmap checksum
    bg_block_bitmap_csum_lo: u16,

    /// Lower 16-bit of the inode bitmap checksum
    bg_inode_bitmap_csum_lo: u16,

    /// Lower 16-bit of unused inode count
    bg_itable_unused_lo: u16,

    /// Group descriptor checksum
    bg_checksum: u16,

    /// High 32-bits of block bitmap
    bg_block_bitmap_hi: u32,

    /// High 32-bits of inode bitmap
    bg_inode_bitmap_hi: u32,

    /// High 32-bits of inode table
    bg_inode_table_hi: u32,

    /// High 16-bits of free blocks count
    bg_free_blocks_count_hi: u16,

    /// High 16-bits of free inodes count
    bg_free_inodes_count_hi: u16,

    /// High 16-bits of directory used count
    bg_used_dirs_count_hi: u16,

    /// High 16-bits of unused inode count
    bg_itable_unused_hi: u16,

    /// High 32-bits of location of snapshot exclusion bitmap
    bg_exclude_bitmap_hi: u32,

    /// High 16-bits of the block bitmap checksum
    bg_block_bitmap_csum_hi: u16,

    /// High 16_bits of the inode bitmap checksum
    bg_inode_bitmap_csum_hi: u16,
    reserved: u32,
}

impl GroupDescriptor64 {
    pub fn block_bitmap_blk_addr(&self) -> u64 {
        self.bg_block_bitmap_lo as u64 | ((self.bg_block_bitmap_hi as u64) << 32)
    }

    pub fn inode_bitmap_blk_addr(&self) -> u64 {
        self.bg_inode_bitmap_lo as u64 | ((self.bg_inode_bitmap_hi as u64) << 32)
    }

    pub fn inode_table_blk_addr(&self) -> u64 {
        self.bg_inode_table_lo as u64 | ((self.bg_inode_table_hi as u64) << 32)
    }

    pub fn snapshot_excl_bitmap_blk_addr(&self) -> u64 {
        self.bg_exclude_bitmap_lo as u64 | ((self.bg_exclude_bitmap_hi as u64) << 32)
    }

    pub fn free_blk_count(&self) -> u32 {
        self.bg_free_blocks_count_lo as u32 | ((self.bg_free_blocks_count_hi as u32) << 16)
    }

    pub fn free_inode_count(&self) -> u32 {
        self.bg_free_inodes_count_lo as u32 | ((self.bg_free_inodes_count_hi as u32) << 16)
    }

    pub fn directory_count(&self) -> u32 {
        self.bg_used_dirs_count_lo as u32 | ((self.bg_used_dirs_count_hi as u32) << 16)
    }

    pub fn unused_inodes_count(&self) -> u32 {
        self.bg_itable_unused_lo as u32 | ((self.bg_itable_unused_hi as u32) << 16)
    }
}

/// Block group flag: Inode table and bitmap are not initialized
pub const EXT4_BG_INODE_UNINIT: u16 = 0x0001;

/// Block group flag: Block bitmap is not initialized
pub const EXT4_BG_BLOCK_UNINIT: u16 = 0x0002;

/// Block group flag: Inode table is zeroed
pub const EXT4_BG_INODE_ZEROED: u16 = 0x0004;

#[repr(C, packed)]
#[derive(Debug)]
pub struct Inode {
    /// File mode
    pub i_mode: u16,

    /// Lower 16-bit of Owner UID
    i_uid: u16,

    /// Lower 32-bits of size in bytes
    i_size: u32,

    /// Last access time, in seconds since the epoch
    i_atime: u32,

    /// Last inode change time, in seconds since the epoch
    i_ctime: u32,

    /// Last data modification time, in seconds since the epoch
    i_mtime: u32,

    /// Deletion time, in seconds since the epoch
    i_dtime: u32,

    /// Lower 16-bits of GID
    i_gid: u16,

    /// Hard link count
    ///
    /// The usual link limit is 65,000 hard links, but if [`EXT4_FEATURE_DIR_NLINK`] is set, `ext4`
    /// supports more than 64,998 subdirectories by setting this field to 1 to indicate that the
    /// number of hard links is not known.
    i_links_count: u16,

    /// Lower 32-bits of block count.
    pub i_blocks_lo: u32,

    /// Inode flags
    i_flags: u32,

    /// Inode version
    i_version: u32,

    /// Block map or extent tree
    pub i_block: [u8; 60],

    /// File version
    i_generation: u32,

    /// Lower 32-bits of extended attribute block.
    i_file_acl_lo: u32,

    /// Upper 32-bits of file directory/size.
    i_dir_acl: u32,

    /// Fragment address (outdated)
    i_faddr: u32,

    /// High 16-bits of the block count
    i_blocks_high: u16,

    /// High 16-bits of the extended attribute block
    i_file_acl_high: u16,

    /// High 16-bits of the Owner UID
    i_uid_high: u16,

    /// High 16-bits of the GID
    i_gid_high: u16,

    /// Lower 16-bits of the inode checksum
    i_checksum_lo: u16,

    reserved: u16,

    /// Size of this inode - 128
    i_extra_isize: u16,

    /// Upper 16-bits of the inode checksum
    i_checksum_hi: u16,

    /// Extra change time bits
    i_ctime_extra: u32,

    /// Extra modification time bits
    i_mtime_extra: u32,

    /// Extra access time bits
    i_atime_extra: u32,

    /// File creation time, in seconds since the epoch
    i_crtime: u32,

    /// Extra file creation time bits.
    i_crtime_extra: u32,

    /// Upper 32-bits of version number
    i_version_hi: u32,

    /// Project ID
    i_projid: u32,
}

pub mod inode_mode {
    //! [`Inode`] field `i_mode` value is a combination of these flags.

    /// Others may execute.
    pub const S_IXOTH: u16 = 0x0001;

    /// Others may write.
    pub const S_IWOTH: u16 = 0x0002;

    /// Others may read.
    pub const S_IROTH: u16 = 0x0004;

    /// Group may execute.
    pub const S_IXGRP: u16 = 0x0008;

    /// Group may write.
    pub const S_IWGRP: u16 = 0x0010;

    /// Group may read.
    pub const S_IRGRP: u16 = 0x0020;

    /// User may execute.
    pub const S_IXUSR: u16 = 0x0040;

    /// User may write.
    pub const S_IWUSR: u16 = 0x0080;

    /// User may read.
    pub const S_IRUSR: u16 = 0x0100;

    /// Sticky bit.
    pub const S_ISVTX: u16 = 0x0200;

    /// Set GID
    pub const S_ISGID: u16 = 0x0400;

    /// Set UID
    pub const S_ISUID: u16 = 0x0800;

    /// FIFO
    pub const S_IFIFO: u16 = 0x1000;

    /// Character device
    pub const S_IFCHR: u16 = 0x2000;

    /// Directory
    pub const S_IFDIR: u16 = 0x4000;

    /// Block device
    pub const S_IFBLK: u16 = 0x6000;

    /// Regular file
    pub const S_IFREG: u16 = 0x8000;

    /// Symbolic link
    pub const S_IFLNK: u16 = 0xA000;

    /// Socket
    pub const S_IFSOCK: u16 = 0xC000;
}

pub mod inode_flags {
    //! [`Inode`] field `i_value` is a combination of these flags.

    /// This file requires secure deletion. (not implemented)
    pub const EXT4_SECRM_FL: u32 = 0x1;

    /// This file should be preserved. (not implemented)
    pub const EXT4_UNRM_FL: u32 = 0x2;

    /// File is compressed
    pub const EXT4_COMPR_FL: u32 = 0x4;

    /// All writes to the file must be synchronous
    pub const EXT4_SYNC_FL: u32 = 0x8;

    /// File is immutable
    pub const EXT4_IMMUTABLE_FL: u32 = 0x10;

    /// File can only be appended
    pub const EXT4_APPEND_FL: u32 = 0x20;

    /// The `dump` utility should not dump this file.
    pub const EXT4_NODUMP_FL: u32 = 0x40;

    /// Do not update access time
    pub const EXT4_NOATIME_FL: u32 = 0x80;

    /// Dirty compressed file.
    pub const EXT4_DIRTY_FL: u32 = 0x100;

    /// File has one or more compressed clusters.
    pub const EXT4_COMPRBLK_FL: u32 = 0x200;

    /// Do not compress file.
    pub const EXT4_NOCOMPR_FL: u32 = 0x400;

    /// Encrypted inode.
    pub const EXT4_ENCRYPT_FL: u32 = 0x800;

    /// Directory has hashed indexes.
    pub const EXT4_INDEX_FL: u32 = 0x1000;

    /// AFS magic directory
    pub const EXT4_IMAGIC_FL: u32 = 0x2000;

    /// File data must always be written through the journal
    pub const EXT4_JOURNAL_DATA_FL: u32 = 0x4000;

    /// File tail should not be merged.
    pub const EXT4_NOTAIL_FL: u32 = 0x8000;

    /// All directory entry data should be written synchronously.
    pub const EXT4_DIRSYNC_FL: u32 = 0x10000;

    /// Top of directory hierarchy.
    pub const EXT4_TOPDIR_FL: u32 = 0x20000;

    /// Huge file.
    pub const EXT4_HUGE_FILE_FL: u32 = 0x40000;

    /// Inode uses extents.
    pub const EXT4_EXTENTS_FL: u32 = 0x80000;

    /// Verity protected file.
    pub const EXT4_VERITY_FL: u32 = 0x100000;

    /// Inode stores a large extended attribute value in its data block.
    pub const EXT4_EA_INODE_FL: u32 = 0x200000;

    /// This file has blocks allocated past `EOF`.
    pub const EXT4_EOFBLOCKS_FL: u32 = 0x400000;

    /// Inode is a snapshot.
    pub const EXT4_SNAPFILE_FL: u32 = 0x800000;

    /// Snapshot is being deleted.
    pub const EXT4_SNAPFILE_DELETED_FL: u32 = 0x1000000;

    /// Snapshot shrink has completed.
    pub const EXT4_SNAPFILE_SHRUNK_FL: u32 = 0x2000000;

    /// Inode has inline data.
    pub const EXT4_INLINE_DATA_FL: u32 = 0x4000000;

    /// Create children with the same project ID.
    pub const EXT4_PROJINHERIT_FL: u32 = 0x8000000;

    /// Reserved for `ext4` library.
    pub const EXT4_RESERVED_FL: u32 = 0x80000000;
}

impl Inode {
    pub fn mode_contains(&self, flag: u16) -> bool {
        self.i_mode & flag != 0
    }
}

pub struct BlockMap {
    i_entries: [u32; 15],
}

impl BlockMap {
    pub fn load_blk_map(inode: &Inode) -> Option<Self> {
        let mut i_entries = [0u32; 15];

        for (i, entry) in i_entries.iter_mut().enumerate() {
            *entry = u32::from_le_bytes(inode.i_block[i..i + 4].try_into().ok()?);
        }

        Some(Self { i_entries })
    }

    pub fn read_file_blk(&self, blk_id: u32) -> u32 {
        if blk_id < 11 {
            return self.i_entries[blk_id as usize];
        }

        0
    }
}

pub const EXT4_EXTENTH_MAGIC: u16 = 0xF30A;

pub struct ExtentTree {
    extents: Vec<Extent>,
}

fn traverse_extent_layer(fs: &Ext4FS, ext_data: &[u8], extents: &mut Vec<Extent>) -> Option<()> {
    let header = unsafe { ExtentHeader::load(ext_data.as_ptr() as *const ExtentHeader) }?;

    // this extent points directly to data blocks
    if header.eh_depth == 0 {
        for entry in 0..header.eh_entries {
            let raw_entry = &ext_data[(mem::size_of::<ExtentHeader>()
                + (entry as usize) * mem::size_of::<Extent>())
                ..mem::size_of::<ExtentHeader>() + (1 + entry as usize) * mem::size_of::<Extent>()];

            let extent = Extent {
                ee_block: u32::from_le_bytes(raw_entry[0..4].try_into().ok()?),
                ee_len: u16::from_le_bytes(raw_entry[4..6].try_into().ok()?),
                ee_start_hi: u16::from_le_bytes(raw_entry[6..8].try_into().ok()?),
                ee_start_lo: u32::from_le_bytes(raw_entry[8..12].try_into().ok()?),
            };

            extents.push(extent);
        }

        return Some(());
    }

    for entry in 0..header.eh_entries {
        let raw_entry = &ext_data[(mem::size_of::<ExtentHeader>()
            + (entry as usize) * mem::size_of::<ExtentIdx>())
            ..mem::size_of::<ExtentHeader>() + (1 + entry as usize) * mem::size_of::<ExtentIdx>()];

        let extent_idx = ExtentIdx {
            ei_block: u32::from_le_bytes(raw_entry[0..4].try_into().ok()?),
            ei_leaf_lo: u32::from_le_bytes(raw_entry[4..8].try_into().ok()?),
            ei_leaf_hi: u16::from_le_bytes(raw_entry[8..10].try_into().ok()?),
            ei_unused: u16::from_le_bytes(raw_entry[10..12].try_into().ok()?),
        };

        let mut data = alloc::vec![0u8; fs.superblock.blk_size() as usize];

        fs.__read_blk(extent_idx.leaf(), &mut data).ok()?;
        traverse_extent_layer(fs, &data, extents);
    }

    Some(())
}

impl ExtentTree {
    pub fn load_extent_tree(fs: &Ext4FS, inode: &Inode) -> Option<Self> {
        if !fs
            .superblock
            .incompat_contains(EXT4_FEATURE_INCOMPAT_EXTENTS)
            | !inode.contains_flag(inode_flags::EXT4_EXTENTS_FL)
        {
            return None;
        };
        let mut extents: Vec<Extent> = alloc::vec![];

        traverse_extent_layer(fs, &inode.i_block, &mut extents);

        Some(Self { extents })
    }
}

/// Header contained in each node of the `ext4` extent tree.
#[derive(Debug)]
#[repr(C)]
struct ExtentHeader {
    /// Magic number (should be `0xf30a`)
    eh_magic: u16,

    /// Number of valid entries following the header
    eh_entries: u16,

    /// Maximum number of entries that could follow the header
    eh_max: u16,

    /// Depth of this node in the extent tree.
    ///
    /// If `eh_depth == 0`, this extent points to data blocks
    eh_depth: u16,

    /// Generation of the tree
    eh_generation: u32,
}

impl ExtentHeader {
    pub unsafe fn load(exth_ref: *const ExtentHeader) -> Option<Self> {
        let header = ptr::read_unaligned(exth_ref);
        if header.eh_magic != EXT4_EXTENTH_MAGIC {
            None
        } else {
            Some(header)
        }
    }
}

/// Represents a leaf node of the extent tree.
#[derive(Debug)]
#[repr(C)]
struct Extent {
    /// First file block number that this extent covers
    ee_block: u32,

    /// Number of blocks covered by the extent.
    ///
    /// If `ee_len > 32768`, the extent is uninitialized and the actual extent
    /// length is `ee_len - 32768`.
    ee_len: u16,

    /// High 16-bits of the block number to which this extent points
    ee_start_hi: u16,

    /// Low 32-bits of the block number to which this extent points.
    ee_start_lo: u32,
}

/// Represents an internal node of the extent tree (an index node)
#[repr(C, packed)]
struct ExtentIdx {
    /// This index node covers file blocks from `block` onward.
    ei_block: u32,

    /// Low 32-bits of the block number of the extent node that is the next level lower in the
    /// tree.
    ei_leaf_lo: u32,

    /// High 16-bits of the block number of the extent node that is the next level lower in the
    /// tree.
    ei_leaf_hi: u16,

    ei_unused: u16,
}

impl ExtentIdx {
    fn leaf(&self) -> u64 {
        (self.ei_leaf_lo as u64) | ((self.ei_leaf_hi as u64) << 32)
    }
}

impl Inode {
    pub fn contains_flag(&self, flag: u32) -> bool {
        self.i_flags & flag != 0
    }

    pub fn uses_extent_tree(&self) -> bool {
        self.contains_flag(inode_flags::EXT4_EXTENTS_FL)
    }

    pub fn size(&self) -> u64 {
        self.i_size as u64 | ((self.i_dir_acl as u64) << 32)
    }

    /*
    // Get block number of the nth data block.
    pub fn get_nth_data_block(&self, block_size: u32, n: u32, partition: &Ext4Partition) -> u64 {
        let q = self.i_size as i32 / block_size as i32;
        let r = self.i_size as i32 % block_size as i32;
        if !((n <= q as u32) | ((n == (q as u32 + 1)) & (r > 0))) {
            return 0;
        }
        if self.uses_extent_tree() {
            return self.get_nth_data_block_extent(block_size, n, partition);
        } else {
            return 0;
        }
    }

    // Get block number of the nth data block, considering this inode uses an extent tree structure
    pub fn get_nth_data_block_extent(
        &self,
        block_size: u32,
        n: u32,
        partition: &Ext4Partition,
    ) -> u64 {
        let mem_offset = self as *const Inode as u32 + 0x28;
        self.explore_next_layer(mem_offset, n, block_size, partition)
    }

    // Explore next layer, used by get_nth_data_block_extent
    pub fn explore_next_layer(
        &self,
        mem_offset: u32,
        n: u32,
        block_size: u32,
        partition: &Ext4Partition,
    ) -> u64 {
        let header_address = mem_offset as *const ExtentHeader;
        let header: &ExtentHeader;
        header = unsafe { transmute(header_address) };
        let leaf_number = header.eh_entries;
        // Handle leaves
        if header.eh_depth == 0 {
            for i in 0..leaf_number {
                let extent_addr = (mem_offset + 12 * (i + 1) as u32) as *const Extent;
                let extent: &Extent;
                extent = unsafe { transmute(extent_addr) };
                let len = {
                    if extent.ee_len <= 32768 {
                        extent.ee_len
                    } else {
                        extent.ee_len - 32768
                    }
                };
                if extent.ee_block + len as u32 >= n {
                    // Return address of n-th block
                    return (extent.ee_start_lo as u64 + ((extent.ee_start_hi as u64) << 32))
                        + (n - extent.ee_block) as u64;
                }
            }
            return 0;
        } else {
            // Offset + block_size in memory and iterate over entries recursively
            for i in 0..leaf_number {
                let extent_idx: &ExtentIdx;
                let extent_idx_addr = (mem_offset + 12 * (i + 1) as u32) as *const ExtentIdx;
                extent_idx = unsafe { transmute(extent_idx_addr) };
                let next_block_address =
                    extent_idx.ei_leaf_lo as u64 + (extent_idx.ei_leaf_hi as u64) << 2;
                // let a = AddressPacket::new((block_size / 512) as u16, mem_offset + block_size, (partition_offset + next_block_address / 512) as u64);
                let _ = partition.read(
                    next_block_address as u32,
                    block_size,
                    mem_offset + block_size,
                );
                return self.explore_next_layer(mem_offset + block_size, n, block_size, partition);
            }
            return 0;
        }
    }

    // Parse an inode as a directory, using at most 2 * block_size of memory
    pub fn parse_as_directory(&self, offset: u32, part: &Ext4Partition, block_size: u32) {
        let mut current_block = 0;
        let first_block = self.get_nth_data_block_extent(block_size, current_block, part);
        //debug!(first_block);

        let result = part.read((first_block as u32) * block_size, block_size, offset);
        let mut parser = offset;
        let mut inode = unsafe { read_volatile(offset as *const u32) };
        // The end is defined by a 0x00 inode pointer
        while inode != 0x00 {
            let mut flag_reset = false;
            let mut begin = parser;
            parser += 4;
            let rec_len = unsafe { read_volatile(parser as *const u16) };
            // Test if we need to load the next block
            if rec_len as u32 + parser - offset >= block_size {
                let next_block =
                    self.get_nth_data_block_extent(block_size, current_block + 1, part);
                let result = part.read(
                    (next_block as u32) * block_size,
                    block_size,
                    offset + block_size,
                );
                flag_reset = true;
            }
            parser += 2;
            let name_len = unsafe { read_volatile(parser as *const u8) };
            parser += 1;
            let type_flag = unsafe { read_volatile(parser as *const u8) };
            parser += 1;
            for i in 0..name_len {
                let char = unsafe { read_volatile(parser as *const u8) };
                parser += 1;
            }
            parser = begin + rec_len as u32;

            inode = unsafe { read_volatile(parser as *const u32) };

            // If reset flag is set, return to offset for the next block
            if flag_reset {
                let next_block =
                    self.get_nth_data_block_extent(block_size, current_block + 1, part);
                let result = part.read(
                    (next_block as u32) * block_size,
                    block_size,
                    offset + block_size,
                );
                parser = parser % block_size + offset;
                current_block += 1;
            }
        }
    }

    // Copy n block of file to a specific offset. This is risky as this function will overwrite everything it can.
    pub fn block_copy(&self, offset: u32, part: &Ext4Partition, block_size: u32, n: u32) {
        {
            let mut current_block = 0;
            let mut next_block = self.get_nth_data_block(block_size, current_block, part);
            let result = part.read((next_block as u32) * block_size, block_size, offset);
            while current_block < n {
                current_block += 1;
                next_block = self.get_nth_data_block(block_size, current_block, part);
                let result = part.read(
                    (next_block as u32) * block_size,
                    block_size,
                    offset + current_block * block_size,
                );
            }
        }
    }

    // Show content of a file using at most 1 block_size space in memory
    pub fn read_as_file(&self, offset: u32, part: &Ext4Partition, block_size: u32, n: u32) -> u8 {
        let mut total_byte = 0u32;
        let mut byte = 0u32;
        let mut current_block = 0;
        let mut next_block = self.get_nth_data_block(block_size, current_block, part);
        let result = part.read((next_block as u32) * block_size, block_size, offset);

        while total_byte < n {
            let char = unsafe { read_volatile((byte + offset) as *const u8) };
            if (byte % block_size == 0) & (byte != 0) {
                current_block += 1;
                next_block = self.get_nth_data_block(block_size, current_block, part);
                let result = part.read((next_block as u32) * block_size, block_size, offset);
                match result {
                    Err(_) => return 1,
                    Ok(_) => (),
                }
                byte = 0;
            } else {
                byte += 1;
            }
            total_byte += 1;
        }
        0
    }

    // Search in an inode considered as a directory, using at most 2 * block_size of memory
    pub fn search(
        &self,
        offset: u32,
        part: &Ext4Partition,
        block_size: u32,
        file_type: u8,
        name: &str,
    ) -> u32 {
        let name = name.as_bytes();
        let mut current_block = 0;
        let first_block = self.get_nth_data_block_extent(block_size, current_block, part);
        let result = part.read((first_block as u32) * block_size, block_size, offset);
        let mut parser = offset;
        let mut inode = unsafe { read_volatile(offset as *const u32) };
        // The end is defined by a 0x00 inode pointer
        while inode != 0x00 {
            let mut flag_reset = false;
            let mut begin = parser;
            parser += 4;

            let rec_len = unsafe { read_volatile(parser as *const u16) };

            // Test if we need to load the next block
            if rec_len as u32 + parser - offset >= block_size {
                let next_block =
                    self.get_nth_data_block_extent(block_size, current_block + 1, part);
                let result = part.read(
                    (next_block as u32) * block_size,
                    block_size,
                    offset + block_size,
                );
                flag_reset = true;
            }
            parser += 2;
            let name_len = unsafe { read_volatile(parser as *const u8) };
            parser += 1;
            let type_flag = unsafe { read_volatile(parser as *const u8) };

            let research_size = name.len();

            parser += 1;

            let mut same = 0;
            for i in 0..name_len {
                let char = unsafe { read_volatile(parser as *const u8) };
                if i < research_size as u8 {
                    if char == name[i as usize] {
                        same += 1;
                        parser += 1;
                    } else {
                        parser += 1;
                    }
                } else {
                    parser += 1
                }
            }

            parser = begin + rec_len as u32;

            if same != name.len() as u32 {
            } else if type_flag == file_type {
                return inode;
            }

            inode = unsafe { read_volatile(parser as *const u32) };

            // If reset flag is set, return to offset for the next block
            if flag_reset {
                let next_block =
                    self.get_nth_data_block_extent(block_size, current_block + 1, part);
                let result = part.read(
                    (next_block as u32) * block_size,
                    block_size,
                    offset + block_size,
                );
                parser = parser % block_size + offset;
                current_block += 1;
            }
        }

        return 0;
    }

    // Get block number of the nth data block, considering this inode uses a direct/indirect block addressing system
    pub fn get_nth_data_block_basic(
        &self,
        n: u32,
        offset: u32,
        block_size: u32,
        partition: &Ext4Partition,
    ) -> u32 {
        if n > 12 {
            let (path, depth) = self.get_path_recursive(n - 12, 1, block_size);
            let initial_block = self.i_block[(11 + depth) as usize];
            partition.read(initial_block * block_size, block_size, offset);
            let mut next_block_nb = 0u32;
            for step in path {
                if step != 0 {
                    next_block_nb = unsafe { read_volatile((offset + step * 4) as *const u32) };
                    partition.read(next_block_nb * block_size, block_size, offset);
                } else {
                    return next_block_nb;
                }
            }
            return 0;
        } else {
            return self.i_block[n as usize];
        }
    }

    pub fn get_path_recursive(&self, mut n: u32, depth: usize, block_size: u32) -> ([u32; 4], u8) {
        // Compute the number of bytes contained
        let address_per_block = (block_size / 4);
        let block_count = address_per_block.pow(depth as u32);

        // Check if we have to go to the next stage
        if n > block_count {
            return self.get_path_recursive(n - block_count, depth + 1, block_size);
        } else {
            let mut path = [0u32; 4];
            let mut i = 0;
            while n > block_count {
                path[i] = (n / (address_per_block).pow((depth - i - 1) as u32)) as u32;
                n = n % (address_per_block).pow((depth - i - 1) as u32);
                i += 1;
            }
            return (path, depth as u8);
        }
    }

    */
}

#[repr(C, packed)]
struct LinkedDirectoryEntry {
    inode: u16,
    rec_len: u16,
    name_len: u8,
    file_type: u8,
    name: u32,
}

#[derive(Debug)]
pub struct Ext4File {
    inode: Inode,
}

impl Ext4File {
    pub fn from_inode(inode: Inode) -> Result<Self, IOError> {
        if !inode.mode_contains(inode_mode::S_IFREG) {
            return Err(IOError::Unknown);
        }
        Ok(Self { inode })
    }
}

impl FsFile for Ext4File {
    fn read(&mut self, buf: &mut [u8]) -> super::IOResult<usize> {
        todo!()
    }

    fn seek(&mut self, pos: super::Seek) -> usize {
        todo!()
    }

    fn size(&self) -> super::IOResult<usize> {
        Ok(self.inode.size() as usize)
    }

    fn truncate(&mut self, size: usize) -> super::IOResult<usize> {
        todo!()
    }

    fn extend(&mut self, size: usize) -> super::IOResult<usize> {
        todo!()
    }
}

/*****************************************************************/
/*                                                               */
/* CRC LOOKUP TABLE                                              */
/* ================                                              */
/* The following CRC lookup table was generated automagically    */
/* by the Rocksoft^tm Model CRC Algorithm Table Generation       */
/* Program V1.0 using the following model parameters:            */
/*                                                               */
/*    Width   : 4 bytes.                                         */
/*    Poly    : 0x1EDC6F41L                                      */
/*    Reverse : TRUE.                                            */
/*                                                               */
/* For more information on the Rocksoft^tm Model CRC Algorithm,  */
/* see the document titled "A Painless Guide to CRC Error        */
/* Detection Algorithms" by Ross Williams                        */
/* (ross@guest.adelaide.edu.au.). This document is likely to be  */
/* in the FTP archive "ftp.adelaide.edu.au/pub/rocksoft".        */
/*                                                               */
/*****************************************************************/

const CRC32C_LO_TABLE: [u32; 256] = [
    0x00000000, 0xF26B8303, 0xE13B70F7, 0x1350F3F4, 0xC79A971F, 0x35F1141C, 0x26A1E7E8, 0xD4CA64EB,
    0x8AD958CF, 0x78B2DBCC, 0x6BE22838, 0x9989AB3B, 0x4D43CFD0, 0xBF284CD3, 0xAC78BF27, 0x5E133C24,
    0x105EC76F, 0xE235446C, 0xF165B798, 0x030E349B, 0xD7C45070, 0x25AFD373, 0x36FF2087, 0xC494A384,
    0x9A879FA0, 0x68EC1CA3, 0x7BBCEF57, 0x89D76C54, 0x5D1D08BF, 0xAF768BBC, 0xBC267848, 0x4E4DFB4B,
    0x20BD8EDE, 0xD2D60DDD, 0xC186FE29, 0x33ED7D2A, 0xE72719C1, 0x154C9AC2, 0x061C6936, 0xF477EA35,
    0xAA64D611, 0x580F5512, 0x4B5FA6E6, 0xB93425E5, 0x6DFE410E, 0x9F95C20D, 0x8CC531F9, 0x7EAEB2FA,
    0x30E349B1, 0xC288CAB2, 0xD1D83946, 0x23B3BA45, 0xF779DEAE, 0x05125DAD, 0x1642AE59, 0xE4292D5A,
    0xBA3A117E, 0x4851927D, 0x5B016189, 0xA96AE28A, 0x7DA08661, 0x8FCB0562, 0x9C9BF696, 0x6EF07595,
    0x417B1DBC, 0xB3109EBF, 0xA0406D4B, 0x522BEE48, 0x86E18AA3, 0x748A09A0, 0x67DAFA54, 0x95B17957,
    0xCBA24573, 0x39C9C670, 0x2A993584, 0xD8F2B687, 0x0C38D26C, 0xFE53516F, 0xED03A29B, 0x1F682198,
    0x5125DAD3, 0xA34E59D0, 0xB01EAA24, 0x42752927, 0x96BF4DCC, 0x64D4CECF, 0x77843D3B, 0x85EFBE38,
    0xDBFC821C, 0x2997011F, 0x3AC7F2EB, 0xC8AC71E8, 0x1C661503, 0xEE0D9600, 0xFD5D65F4, 0x0F36E6F7,
    0x61C69362, 0x93AD1061, 0x80FDE395, 0x72966096, 0xA65C047D, 0x5437877E, 0x4767748A, 0xB50CF789,
    0xEB1FCBAD, 0x197448AE, 0x0A24BB5A, 0xF84F3859, 0x2C855CB2, 0xDEEEDFB1, 0xCDBE2C45, 0x3FD5AF46,
    0x7198540D, 0x83F3D70E, 0x90A324FA, 0x62C8A7F9, 0xB602C312, 0x44694011, 0x5739B3E5, 0xA55230E6,
    0xFB410CC2, 0x092A8FC1, 0x1A7A7C35, 0xE811FF36, 0x3CDB9BDD, 0xCEB018DE, 0xDDE0EB2A, 0x2F8B6829,
    0x82F63B78, 0x709DB87B, 0x63CD4B8F, 0x91A6C88C, 0x456CAC67, 0xB7072F64, 0xA457DC90, 0x563C5F93,
    0x082F63B7, 0xFA44E0B4, 0xE9141340, 0x1B7F9043, 0xCFB5F4A8, 0x3DDE77AB, 0x2E8E845F, 0xDCE5075C,
    0x92A8FC17, 0x60C37F14, 0x73938CE0, 0x81F80FE3, 0x55326B08, 0xA759E80B, 0xB4091BFF, 0x466298FC,
    0x1871A4D8, 0xEA1A27DB, 0xF94AD42F, 0x0B21572C, 0xDFEB33C7, 0x2D80B0C4, 0x3ED04330, 0xCCBBC033,
    0xA24BB5A6, 0x502036A5, 0x4370C551, 0xB11B4652, 0x65D122B9, 0x97BAA1BA, 0x84EA524E, 0x7681D14D,
    0x2892ED69, 0xDAF96E6A, 0xC9A99D9E, 0x3BC21E9D, 0xEF087A76, 0x1D63F975, 0x0E330A81, 0xFC588982,
    0xB21572C9, 0x407EF1CA, 0x532E023E, 0xA145813D, 0x758FE5D6, 0x87E466D5, 0x94B49521, 0x66DF1622,
    0x38CC2A06, 0xCAA7A905, 0xD9F75AF1, 0x2B9CD9F2, 0xFF56BD19, 0x0D3D3E1A, 0x1E6DCDEE, 0xEC064EED,
    0xC38D26C4, 0x31E6A5C7, 0x22B65633, 0xD0DDD530, 0x0417B1DB, 0xF67C32D8, 0xE52CC12C, 0x1747422F,
    0x49547E0B, 0xBB3FFD08, 0xA86F0EFC, 0x5A048DFF, 0x8ECEE914, 0x7CA56A17, 0x6FF599E3, 0x9D9E1AE0,
    0xD3D3E1AB, 0x21B862A8, 0x32E8915C, 0xC083125F, 0x144976B4, 0xE622F5B7, 0xF5720643, 0x07198540,
    0x590AB964, 0xAB613A67, 0xB831C993, 0x4A5A4A90, 0x9E902E7B, 0x6CFBAD78, 0x7FAB5E8C, 0x8DC0DD8F,
    0xE330A81A, 0x115B2B19, 0x020BD8ED, 0xF0605BEE, 0x24AA3F05, 0xD6C1BC06, 0xC5914FF2, 0x37FACCF1,
    0x69E9F0D5, 0x9B8273D6, 0x88D28022, 0x7AB90321, 0xAE7367CA, 0x5C18E4C9, 0x4F48173D, 0xBD23943E,
    0xF36E6F75, 0x0105EC76, 0x12551F82, 0xE03E9C81, 0x34F4F86A, 0xC69F7B69, 0xD5CF889D, 0x27A40B9E,
    0x79B737BA, 0x8BDCB4B9, 0x988C474D, 0x6AE7C44E, 0xBE2DA0A5, 0x4C4623A6, 0x5F16D052, 0xAD7D5351,
];

fn crc32c_calc(buf: &[u8]) -> u32 {
    let mut crc = 0xFFFFFFFF;

    for &b in buf {
        crc = CRC32C_LO_TABLE[((crc ^ b as u32) & 0xff) as usize] ^ (crc >> 8);
    }

    crc
}