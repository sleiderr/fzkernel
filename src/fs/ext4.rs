use core::mem::transmute;
use core::ptr::read_volatile;

use crate::io::disk::bios::AddressPacket;

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

/// The ext4 `Superblock` hold useful information about the filesystem's characteristics and
/// attributes (block count, sizes, required features, ...).
///
/// A copy of the partition's `Superblock` is kept in all groups, except if the `sparse_super`
/// feature is enabled, in which case it is only kept in groups whose group number is either 0 or a
/// power of 3, 5, 7.
#[repr(C, packed)]
pub struct Superblock {
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

    ///
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

    /// Inode for tracking orphan inodes
    s_orphan_file_inum: u32,

    s_reserved: [u32; 94],

    /// Checksum of the superblock: `crc32c(superblock)`
    s_checksum: u32,
}

impl Superblock {
    pub fn list_root(&self) {}

    pub fn load_block(&self, n: u32, partition: &Ext4Partition, buffer: u32) -> Result<(), ()> {
        let block_size_bytes = 2u32.pow((10 + self.s_log_block_size)) as u32;

        partition.read(n * block_size_bytes, block_size_bytes, buffer)
    }

    // Returns a reference to an Inode given its number (assuming default inode record size is 256 bytes)
    pub fn get_inode(&mut self, inode_nb: u32, partition: &Ext4Partition) -> &Inode {
        let block_group = (inode_nb - 1) / self.s_inodes_per_group;
        let index = (inode_nb - 1) % self.s_inodes_per_group;
        let block_size = 2u32.pow((10 + self.s_log_block_size)) as u32;
        let grp_descriptor_addr = block_size + 64 * block_group;

        partition.read(grp_descriptor_addr, 4096, 0x1500);

        let grp_descriptor_addr = (0x1500 + grp_descriptor_addr % 512) as *mut GroupDescriptor;
        let grp_desc: &GroupDescriptor32;
        grp_desc = unsafe { transmute(grp_descriptor_addr) };

        if self.s_inode_size == 0 {
            self.s_inode_size = 256
        }

        let inode_table_address = grp_desc.bg_inode_table * block_size;
        let inode_address = inode_table_address + (self.s_inode_size as u32) * index;

        partition.read(inode_address, 512, 0x1500);

        let inode: &Inode;
        let inode_addr = (0x1500 + inode_address % 512) as *mut Inode;

        inode = unsafe { transmute(inode_addr) };

        inode
    }
}

/// Each block group on the file system has a `GroupDescriptor` associated with it.
///
/// A `block group` is a logical grouping of contigous block.
pub enum GroupDescriptor {
    Size32(GroupDescriptor32),
    Size64(GroupDescriptor64),
}

/// 32-bit version of the [`GroupDescriptor`].
///
/// Used if [`EXT4_FEATURE_INCOMPAT_64BIT`] is clear.
#[repr(C, packed)]
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

/// 64-bit version of the [`GroupDescriptor`]
///
/// Used if [`EXT4_FEATURE_INCOMPAT_64BIT`] is set.
#[repr(C, packed)]
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

/// Block group flag: Inode table and bitmap are not initialized
pub const EXT4_BG_INODE_UNINIT: u16 = 0x0001;

/// Block group flag: Block bitmap is not initialized
pub const EXT4_BG_BLOCK_UNINIT: u16 = 0x0002;

/// Block group flag: Inode table is zeroed
pub const EXT4_BG_INODE_ZEROED: u16 = 0x0004;

#[repr(C, packed)]
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
    pub i_block: [u32; 15],

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

/// Header contained in each node of the `ext4` extent tree.
#[repr(C, packed)]
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

/// Represents a leaf node of the extent tree.
#[repr(C, packed)]
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
    /// This index node covers file blocks froù `block` onward.
    ei_block: u32,

    /// Low 32-bits of the block number of the extent node that is the next level lower in the
    /// tree.
    ei_leaf_lo: u32,

    /// High 16-bits of the block number of the extent node that is the next level lower in the
    /// tree.
    ei_leaf_hi: u16,

    ei_unused: u16,
}

impl Inode {
    // Returns true is this inode uses an extent tree
    pub fn uses_extent_tree(&self) -> bool {
        return self.i_flags == 0x80000;
    }

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
}

pub struct Ext4Partition {
    pub offset: u32,
    pub drive: u8,
}

impl Ext4Partition {
    #[inline(never)]
    pub fn read(&self, offset: u32, length: u32, buffer: u32) -> Result<(), ()> {
        let offset = (offset / 512 + self.offset) as u64;
        let address = AddressPacket::new(
            (length / 512) as u16,
            (buffer >> 16) as u16,
            (buffer & 0xffff) as u16,
            offset,
        );
        address.disk_read(self.drive);
        Ok(())
    }
}

#[repr(C, packed)]
struct LinkedDirectoryEntry {
    inode: u16,
    rec_len: u16,
    name_len: u8,
    file_type: u8,
    name: u32,
}
