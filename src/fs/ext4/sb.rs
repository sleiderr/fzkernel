//! ext4 Superblock related structures.
//!
//! The superblock store various information about the filesystem (supported features, block count, inode count, ...)
//!
//! Copies of the `Ext4Superblock` structure are kept in each block group, unless the `sparse_super` feature is set in
//! which case it is only kept in block groups whose number is 0, or a power of 3, 5 or 7.

#![allow(clippy::too_many_lines)]

use crate::error;
use crate::fs::ext4::block_grp::BlockGroupNumber;
use crate::fs::ext4::crc32c_calc;
use crate::fs::ext4::extent::{Ext4RealBlkId, Ext4RealBlkId32};
use crate::fs::ext4::inode::{InodeBlk, InodeCount, InodeNumber, InodeSizeHi, InodeSizeLo};
use crate::time::UnixTimestamp32;
use alloc::string::String;
use alloc::sync::Arc;
use bytemuck::{bytes_of, cast, Pod, Zeroable};
use core::cmp::Ordering;
use core::mem::transmute;
use core::ops::{Deref, DerefMut};
use spin::RwLock;

/// Derives the [`core::fmt::Display`] Trait for tuple structs containing a single field.
#[macro_export]
macro_rules! ext4_uint_field_derive_display {
    ($struct_name: tt) => {
        impl core::fmt::Display for $struct_name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_fmt(format_args!("{}", self.0))
            }
        }
    };
}

/// Derives a range type for custom types defined in `ext4` data structures.
#[macro_export]
macro_rules! ext4_uint_field_range {
    ($struct_name: tt, $field_name: tt, $desc: literal) => {
        #[derive(
            Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable,
        )]
        #[repr(C)]
        #[doc = $desc]
        pub(crate) struct $struct_name(pub(crate) $field_name, pub(crate) $field_name);

        impl Iterator for $struct_name {
            type Item = $field_name;

            fn next(&mut self) -> Option<Self::Item> {
                if self.0 < self.1 {
                    self.0 = self.0 + 1;
                    return Some(self.0 - 1);
                }

                None
            }
        }
    };
}

/// Defines a standard structure for flag-related fields in the `Ext4Superblock`.
#[macro_export]
macro_rules! ext4_flag_field {
    ($struct_name: tt, $size: ident, $desc: literal) => {
        #[derive(
            Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable,
        )]
        #[repr(transparent)]
        #[doc=$desc]
        pub(crate) struct $struct_name($size);

        impl core::ops::BitAnd for $struct_name {
            type Output = Self;

            fn bitand(self, rhs: Self) -> Self::Output {
                Self(self.0 & rhs.0)
            }
        }

        impl core::ops::BitOr for $struct_name {
            type Output = Self;

            fn bitor(self, rhs: Self) -> Self::Output {
                Self(self.0 | rhs.0)
            }
        }

        impl core::ops::BitXor for $struct_name {
            type Output = Self;

            fn bitxor(self, rhs: Self) -> Self::Output {
                Self(self.0 ^ rhs.0)
            }
        }
    };
}

ext4_flag_field!(
    CompatibleFeatureSet,
    u32,
    "Compatible feature set flags. \
The system may still read/write to this filesystem event if it doesn't understand \
/ implement all flags defined in the superblock."
);

impl CompatibleFeatureSet {
    /// Empty feature set
    pub(crate) const EMPTY_SET: Self = Self(0);

    /// Directory preallocation.
    pub(crate) const EXT4_FEATURE_COMPAT_DIR_PREALLOC: Self = Self(0x0001);

    /// Used by AFS to indicate inodes that are not linked into the directory namespace.
    pub(crate) const EXT4_FEATURE_COMPAT_IMAGIC_INODES: Self = Self(0x0002);

    /// Create a journal file to ensure file system consistency (even across dirty shutdowns).
    pub(crate) const EXT4_FEATURE_COMPAT_HAS_JOURNAL: Self = Self(0x0004);

    /// This feature enables the use of extended attributes.
    pub(crate) const EXT4_FEATURE_COMPAT_EXT_ATTR: Self = Self(0x0008);

    /// This feature indicates that space has been reserved so that the block group descriptor table
    /// can be extended while resizing a mounted file system.
    ///
    /// This features requires that the [`crate::fs::ext4::EXT4_FEATURE_R0_COMPAT_SPARSE_SUPER`] or
    /// [`crate::fs::ext4::EXT4_FEATURE_COMPAT_SPARSE_SUPER2`] feature be enabled.
    pub(crate) const EXT4_FEATURE_COMPAT_RESIZE_INODE: Self = Self(0x0010);

    /// Use hashed B-trees to speed up name lookup in large directories.
    pub(crate) const EXT4_FEATURE_COMPAT_DIR_INDEX: Self = Self(0x0020);

    /// This feature indicates that there will only be at most 2 backup superblocks and block group
    /// descriptors.
    pub(crate) const EXT4_FEATURE_COMPAT_SPARSE_SUPER2: Self = Self(0x0200);

    pub(crate) const EXT4_FEATURE_COMPAT_FAST_COMMIT: Self = Self(0x0400);

    /// Marks the file system's inode numbers and UUID as stable.
    ///
    /// This feature allows the use of specialized encryption settings that will make use of the inode
    /// numbers and UUID.
    pub(crate) const EXT4_FEATURE_COMPAT_STABLE_INODES: Self = Self(0x0800);

    pub(crate) const EXT4_FEATURE_COMPAT_ORPHAN_FILE: Self = Self(0x1000);

    /// Checks if this `CompatibleFeatureSet` is a subset of (included in) the `CompatibleFeatureSet`
    /// passed as argument.
    pub(crate) fn is_subset_of(self, features: CompatibleFeatureSet) -> bool {
        (self | features) ^ features == Self::EMPTY_SET
    }

    /// Extends this `CompatibleFeatureSet` with the flags of another `CompatibleFeatureSet` passed
    /// as argument.
    pub(crate) fn extend_from_set(&mut self, features: CompatibleFeatureSet) {
        self.0 |= features.0;
    }

    /// Checks if this `CompatibleFeatureSet` includes the `CompatibleFeatureSet` passed as argument.
    pub(crate) fn includes(self, features: Self) -> bool {
        features.is_subset_of(self)
    }
}

ext4_flag_field!(
    ReadOnlyCompatibleFeatureSet,
    u32,
    "Read-only compatible feature set flags. If the system does not understand one of the read-only
 compatible feature flags defined in the superblock, it may still mount the filesystem as
 read-only."
);

impl ReadOnlyCompatibleFeatureSet {
    /// Empty feature set
    pub(crate) const EMPTY_SET: Self = Self(0);

    /// This file system feature indicates that backup copies of the superblock are present only in a
    /// subset of block groups.
    pub(crate) const EXT4_FEATURE_R0_COMPAT_SPARSE_SUPER: Self = Self(0x0001);

    /// This feature flag is usually set when a file larger than 2 GB is created.
    pub(crate) const EXT4_FEATURE_R0_COMPAT_LARGE_FILE: Self = Self(0x0002);

    pub(crate) const EXT4_FEATURE_R0_COMPAT_BTREE_DIR: Self = Self(0x0004);

    /// This feature allows files to be larger than 2 TB in size.
    pub(crate) const EXT4_FEATURE_R0_COMPAT_HUGE_FILE: Self = Self(0x0008);

    /// Group descriptors have checksums.
    pub(crate) const EXT4_FEATURE_R0_COMPAT_GDT_CSUM: Self = Self(0x0010);

    /// This feature lifts the usual 65,000 hard links limit per inode.
    pub(crate) const EXT4_FEATURE_DIR_NLINK: Self = Self(0x0020);

    /// This feature reserves a specific amount of space in each node for extended metadata (ns
    /// timestamps, or file creation time).
    ///
    /// For this feature to be useful, the inode size must be 256 bytes in size or larger.
    pub(crate) const EXT4_FEATURE_R0_COMPAT_EXTRA_ISIZE: Self = Self(0x0040);

    /// This creates quota inodes and set them in the [`Superblock`].
    pub(crate) const EXT4_FEATURE_R0_COMPAT_QUOTA: Self = Self(0x0100);

    /// This feature enables clustered block allocation, so that the unit of allocation is a power of
    /// two number of blocks.
    ///
    /// Requires that the `EXT4_FEATURE_INCOMPAT_EXTENTS` feature be enabled.
    pub(crate) const EXT4_FEATURE_R0_COMPAT_BIGALLOC: Self = Self(0x0200);

    /// This feature enables metadata checksumming.
    ///
    /// This stores checksums for all of the file system metadata.
    pub(crate) const EXT4_FEATURE_R0_COMPAT_METADATA_CSUM: Self = Self(0x0400);

    /// Read-only file system image.
    pub(crate) const EXT4_FEATURE_R0_COMPAT_READONLY: Self = Self(0x1000);

    /// This feature provides project quota support.
    pub(crate) const EXT4_FEATURE_R0_COMPAT_PROJECT: Self = Self(0x2000);

    /// This feature enables support for verity protected files.
    ///
    /// Verity files are read-only, and their data is transparently verified against a Merkle tree
    /// hidden past the end of the file.
    /// This is most useful for authenticating important read-only files on read-write file systems.
    /// If the file system is read-only, using `dm-verity` to authenticate the entire block may provide
    /// much better security.
    pub(crate) const EXT4_FEATURE_R0_COMPAT_VERITY: Self = Self(0x8000);

    pub(crate) const EXT4_FEATURE_R0_COMPAT_ORPHAN_PRESENT: Self = Self(0x10000);

    /// Checks if this `CompatibleFeatureSet` is a subset of (included in) the `CompatibleFeatureSet`
    /// passed as argument.
    pub(crate) fn is_subset_of(self, features: ReadOnlyCompatibleFeatureSet) -> bool {
        (self | features) ^ features == Self::EMPTY_SET
    }

    /// Extends this `CompatibleFeatureSet` with the flags of another `CompatibleFeatureSet` passed
    /// as argument.
    pub(crate) fn extend_from_set(&mut self, features: ReadOnlyCompatibleFeatureSet) {
        self.0 |= features.0;
    }

    /// Checks if this `CompatibleFeatureSet` includes the `CompatibleFeatureSet` passed as argument.
    pub(crate) fn includes(self, features: Self) -> bool {
        features.is_subset_of(self)
    }
}

ext4_flag_field!(
    IncompatibleFeatureSet,
    u32,
    "Incompatible feature set flags. The system should not mount the filesystem if it does not
understand one of the incompatible feature flags defined in the superblock."
);

impl IncompatibleFeatureSet {
    /// Empty feature set
    pub(crate) const EMPTY_SET: Self = Self(0);

    /// Compression feature flag (not implemented)
    pub(crate) const EXT4_FEATURE_INCOMPAT_COMPRESSION: Self = Self(0x0001);

    /// Enables the storage of file type information in directory entries.
    pub(crate) const EXT4_FEATURE_INCOMPAT_FILETYPE: Self = Self(0x0002);

    /// File system needs journal recovery.
    pub(crate) const EXT4_FEATURE_INCOMPAT_RECOVER: Self = Self(0x0004);

    /// This feature is enabled on the [`Superblock`] found on an external journal device.
    pub(crate) const EXT4_FEATURE_INCOMPAT_JOURNAL_DEV: Self = Self(0x0008);

    /// This feature allows file systems to be resized on-line without explicitly needing to reserve
    /// space for growth in size of the block group descriptors.
    pub(crate) const EXT4_FEATURE_INCOMPAT_META_BG: Self = Self(0x0010);

    /// This feature allow the mapping of logical block numbers for a particular inode to physical
    /// blocks on the storage device to be stored using an extent tree, a more efficient data structure
    /// than the traditional indirect block scheme used by `ext2` and `ext3` filesystems.
    pub(crate) const EXT4_FEATURE_INCOMPAT_EXTENTS: Self = Self(0x0040);

    /// This feature allows for a file system size above 2^32 blocks.
    pub(crate) const EXT4_FEATURE_INCOMPAT_64BIT: Self = Self(0x0080);

    /// This feature provides Multiple mount protection (useful in shared environments).
    pub(crate) const EXT4_FEATURE_INCOMPAT_MMP: Self = Self(0x0100);

    /// This feature allow the per-block group metadata to be placed on the storage media.
    pub(crate) const EXT4_FEATURE_INCOMPAT_FLEX_BG: Self = Self(0x0200);

    /// This feature allow the value of each extended attribute to be placed in the data block of a
    /// separate inode if necessary, increasing the limit on the size and number of extended attributes
    /// per file.
    pub(crate) const EXT4_FEATURE_INCOMPAT_EA_INODE: Self = Self(0x0400);

    /// Data in directory entry.
    ///
    /// This allows additional data fields to be stored in each directory entry.
    pub(crate) const EXT4_FEATURE_INCOMPAT_DIRDATA: Self = Self(0x1000);

    /// This feature allows the file system to store the metadata checksum seed in the superblock,
    /// which allows the administrator to change the UUID of a file system while it is mounted.
    pub(crate) const EXT4_FEATURE_INCOMPAT_CSUM_SEED: Self = Self(0x2000);

    /// This feature increases the limit on the number of files per directory.
    pub(crate) const EXT4_FEATURE_INCOMPAT_LARGEDIR: Self = Self(0x4000);

    /// This feature allows data to be stored in the inode and extended attribute area.
    pub(crate) const EXT4_FEATURE_INCOMPAT_INLINE_DATA: Self = Self(0x8000);

    /// This feature enables support for file system level encryption of data block and file names. The
    /// inode metadata is _not_ encrypted
    pub(crate) const EXT4_FEATURE_INCOMPAT_ENCRYPT: Self = Self(0x10000);

    /// This feature provides file system level character encoding support for directories with the
    /// casefold (+F) flag enabled.
    pub(crate) const EXT4_FEATURE_INCOMPAT_CASEFOLD: Self = Self(0x20000);

    /// Checks if this `CompatibleFeatureSet` is a subset of (included in) the `CompatibleFeatureSet`
    /// passed as argument.
    pub(crate) fn is_subset_of(self, features: IncompatibleFeatureSet) -> bool {
        (self | features) ^ features == Self::EMPTY_SET
    }

    /// Extends this `CompatibleFeatureSet` with the flags of another `CompatibleFeatureSet` passed
    /// as argument.
    pub(crate) fn extend_from_set(&mut self, features: IncompatibleFeatureSet) {
        self.0 |= features.0;
    }

    /// Checks if this `CompatibleFeatureSet` includes the `CompatibleFeatureSet` passed as argument.
    pub(crate) fn includes(self, features: Self) -> bool {
        features.is_subset_of(self)
    }
}

ext4_flag_field!(Ext4SuperblockFlags, u32, "");

impl Ext4SuperblockFlags {
    /// Empty superblock flags set.
    pub(crate) const EMPTY_SET: Self = Self(0);

    /// Signed directory hash in use.
    pub(crate) const SIGNED_DIR_HASH: Self = Self(0x1);

    /// Unsigned directory hash in use.
    pub(crate) const UNSIGNED_DIR_HASH: Self = Self(0x2);

    /// To test development code.
    pub(crate) const DEV_TEST: Self = Self(0x4);

    /// Checks if these `Ext4SuperblockFlags` are a subset of (included in) the `Ext4SuperblockFlags`
    /// passed as argument.
    pub(crate) fn is_subset_of(self, features: Self) -> bool {
        (self | features) ^ features == Self::EMPTY_SET
    }

    /// Extends these `Ext4SuperblockFlags` with the options of another `Ext4SuperblockFlags` passed
    /// as argument.
    pub(crate) fn extend_from_set(&mut self, features: Self) {
        self.0 |= features.0;
    }

    /// Checks if these `Ext4SuperblockFlags` includes the `Ext4SuperblockFlags` passed as argument.
    pub(crate) fn is_flag_set(self, features: Self) -> bool {
        features.is_subset_of(self)
    }
}

ext4_flag_field!(Ext4SuperblockMountOptions, u32, "");

impl Ext4SuperblockMountOptions {
    /// Empty mount options set.
    pub(crate) const EMPTY_OPTS: Self = Self(0);

    /// Print debugging info upon mount.
    pub(crate) const EXT4_DEFM_DEBUG: Self = Self(0x1);

    /// New files take the GID of the containing directory, instead of the fsgid of the current process.
    pub(crate) const EXT4_DEFM_BSDGROUPS: Self = Self(0x2);

    /// Support userspace-provided xattr.
    pub(crate) const EXT4_DEFM_XATTR_USER: Self = Self(0x4);

    /// Support POSIX access control list.
    pub(crate) const EXT4_DEFM_ACL: Self = Self(0x8);

    /// Do not support 32-bit UIDs.
    pub(crate) const EXT_DEFM_UID16: Self = Self(0x10);

    /// All data and metadata are committed to the journal.
    pub(crate) const EXT4_DEFM_JMODE_DATA: Self = Self(0x20);

    /// All data are flushed to disk before metadata are committed to the journal.
    pub(crate) const EXT4_DEFM_JMODE_ORDERED: Self = Self(0x40);

    /// Data ordering is not preserved, data may be written after the metadata has been written.
    pub(crate) const EXT4_DEFM_JMODE_WBACK: Self = Self(0x60);

    /// Disable write flushes.
    pub(crate) const EXT4_DEFM_NOBARRIER: Self = Self(0x100);

    /// Track which blocks in a filesystem are metadata and therefore should not be used as data blocks.
    pub(crate) const EXT4_DEFM_BLOCK_VALIDITY: Self = Self(0x200);

    /// Enable `DISCARD` support, where the storage device is told about blocks becoming unused.
    pub(crate) const EXT4_DEFM_DISCARD: Self = Self(0x400);

    /// Disable delayed allocation.
    pub(crate) const EXT4_DEFM_NODEALLOC: Self = Self(0x800);

    /// Checks if these `Ext4SuperblockMountOptions` are a subset of (included in) the `Ext4SuperblockMountOptions`
    /// passed as argument.
    pub(crate) fn is_subset_of(self, features: Self) -> bool {
        (self | features) ^ features == Self::EMPTY_OPTS
    }

    /// Extends these `Ext4SuperblockMountOptions` with the options of another `Ext4SuperblockMountOptions` passed
    /// as argument.
    pub(crate) fn extend_from_set(&mut self, features: Self) {
        self.0 |= features.0;
    }

    /// Checks if these `Ext4SuperblockMountOptions` includes the `Ext4SuperblockMountOptions` passed as argument.
    pub(crate) fn is_option_set(self, features: Self) -> bool {
        features.is_subset_of(self)
    }
}

/// 8-bit encoded logical block count.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct Ext4BlkCount8(u8);

/// 16-bit encoded logical block count.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct Ext4BlkCount16(u16);

impl Ext4BlkCount16 {
    pub(crate) fn add_high_bits(self, high: Ext4BlkCount16) -> Ext4BlkCount32 {
        cast(u32::from(self.0) | (u32::from(high.0) << 16))
    }
}

/// 32-bit encoded logical block count.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct Ext4BlkCount32(u32);

impl core::ops::Div<u32> for Ext4BlkCount32 {
    type Output = u32;

    fn div(self, rhs: u32) -> Self::Output {
        self.0 / rhs
    }
}

impl core::ops::Div<Ext4BlkCount32> for u32 {
    type Output = u32;

    fn div(self, rhs: Ext4BlkCount32) -> Self::Output {
        self / rhs.0
    }
}

impl Ext4BlkCount32 {
    pub(crate) fn add_high_bits(self, high: Ext4BlkCount32) -> Ext4BlkCount {
        cast(u64::from(self.0) | (u64::from(high.0) << 32))
    }
}

/// 64-bit encoded logical block count.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct Ext4BlkCount(pub(super) u64);

ext4_uint_field_derive_display!(Ext4BlkCount);

impl PartialEq<Ext4RealBlkId> for Ext4BlkCount {
    fn eq(&self, other: &Ext4RealBlkId) -> bool {
        self.0 == cast(*other)
    }
}

impl PartialOrd<Ext4RealBlkId> for Ext4BlkCount {
    fn partial_cmp(&self, other: &Ext4RealBlkId) -> Option<Ordering> {
        Some(self.0.cmp(&cast(*other)))
    }
}

impl From<Ext4BlkCount32> for Ext4BlkCount {
    fn from(value: Ext4BlkCount32) -> Self {
        Self(u64::from(value.0))
    }
}

impl From<Ext4BlkCount16> for Ext4BlkCount {
    fn from(value: Ext4BlkCount16) -> Self {
        Self(u64::from(value.0))
    }
}

impl From<Ext4BlkCount8> for Ext4BlkCount {
    fn from(value: Ext4BlkCount8) -> Self {
        Self(u64::from(value.0))
    }
}

impl core::ops::Div for Ext4BlkCount {
    type Output = u64;

    fn div(self, rhs: Self) -> Self::Output {
        self.0 / rhs.0
    }
}

/// Magic number `Ext4Superblock` field.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct Ext4SuperblockMagic(u16);

impl Ext4SuperblockMagic {
    pub(crate) const MAGIC: Self = Self(0xEF53);

    pub(crate) fn is_valid(self) -> bool {
        self == Self::MAGIC
    }
}

/// Default hash algorithm to use for directory hashes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct Ext4HashAlgorithm(u8);

impl Ext4HashAlgorithm {
    pub(crate) const LEGACY: Self = Self(0);

    pub(crate) const HALF_MD4: Self = Self(0x1);

    pub(crate) const TEA: Self = Self(0x2);

    pub(crate) const LEGACY_UNSIGNED: Self = Self(0x3);

    pub(crate) const HALD_MD4_UNSIGNED: Self = Self(0x4);

    pub(crate) const TEA_UNSIGNED: Self = Self(0x5);
}

/// Behaviour to adopt when detecting errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct Ext4SuperblockErrorPolicy(u16);

impl Ext4SuperblockErrorPolicy {
    pub(crate) const CONTINUE: Self = Self(1);

    pub(crate) const REMOUNT_RO: Self = Self(2);

    pub(crate) const PANIC: Self = Self(3);
}

/// OS on which the filesystem was created.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct Ext4CreatorOS(u32);

impl Ext4CreatorOS {
    pub(crate) const LINUX: Self = Self(0);

    pub(crate) const HURD: Self = Self(1);

    pub(crate) const MASIX: Self = Self(2);

    pub(crate) const FREEBSD: Self = Self(3);

    pub(crate) const LITES: Self = Self(4);
}

/// Superblock's major revision level.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct Ext4SuperblockRevision(u32);

impl Ext4SuperblockRevision {
    /// Original format
    pub(crate) const ORIGINAL: Self = Self(0);

    /// v2 format with dynamic inode sizes
    pub(crate) const V2_FORMAT: Self = Self(1);
}

/// Encryption algorithm in-use (up to four at any time).
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct Ext4EncryptionAlgorithm(u8);

impl Ext4EncryptionAlgorithm {
    /// Invalid algorithm
    pub(crate) const ENCRYPTION_MODE_INVALID: Self = Self(0);

    /// 256-bit AES in XTS mode
    pub(crate) const ENCRYPTION_MODE_AES_256_XTS: Self = Self(1);

    /// 256-bit AES in GCM mode
    pub(crate) const ENCRYPTION_MODE_AES_256_GCM: Self = Self(2);

    /// 256-bit AES in CBC mode
    pub(crate) const ENCRYPTION_MODE_AES_256_CBC: Self = Self(3);
}

/// Filesystem state.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct Ext4SuperblockState(u16);

impl Ext4SuperblockState {
    pub(crate) const CLEANLY_UNMOUNTED: Self = Self(0x1);

    pub(crate) const ERRORS_DETECTED: Self = Self(0x2);

    pub(crate) const ORPHANS_IN_RECOVERY: Self = Self(0x4);
}

/// 128-bit UUID for the filesystem.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct Ext4FsUuid(u128);

/// CRC32 of the filesystem's UUID field, used while calculating the checksum of other
/// `ext4` structures: `crc32c_calc(fs_uuid)`
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct Ext4FsUuidCrc32(u32);

/// Defines a standard structure for string-based fields in the `Ext4Superblock`.
macro_rules! ext4_sb_string_field {
    ($struct_name: tt, $len: literal) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
        #[repr(transparent)]
        pub(crate) struct $struct_name([u8; $len]);

        impl $struct_name {
            /// Returns an [`Iterator`] over the characters of the field.
            pub(crate) fn chars(&self) -> impl Iterator<Item = char> {
                self.0
                    .clone()
                    .into_iter()
                    .filter(|&b| b != 0)
                    .map(char::from)
            }
        }

        impl From<$struct_name> for String {
            fn from(value: $struct_name) -> Self {
                value.chars().collect::<String>()
            }
        }
    };
}

ext4_sb_string_field!(Ext4SuperblockVolumeName, 16);
ext4_sb_string_field!(Ext4SuperblockLastMountedPath, 64);
ext4_sb_string_field!(Ext4SuperblockMountOptionsString, 64);

/// Metadata checksum algorithm type
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct Ext4ChksumAlgorithm(u8);

impl Ext4ChksumAlgorithm {
    /// crc32c algorithm (only valid value for that field).
    pub(crate) const CHKSUM_CRC32_C: Self = Self(0x1);
}

/// Checksum of the associated `Ext4Superblock` structure.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct Ext4SuperblockChksum(u32);

/// Smart pointer to a locked [`Superblock`].
///
/// Most `ext4` related data structures have their own copy of that pointer.
pub(super) type LockedSuperblock = Arc<RwLock<Superblock>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub(crate) struct Superblock {
    pub(crate) ext4_superblock: Ext4Superblock,
}

impl Deref for Superblock {
    type Target = Ext4Superblock;

    fn deref(&self) -> &Self::Target {
        &self.ext4_superblock
    }
}

impl DerefMut for Superblock {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ext4_superblock
    }
}

/// The ext4 `Superblock` hold useful information about the filesystem's characteristics and
/// attributes (block count, sizes, required features, ...).
///
/// A copy of the partition's `Ext4Superblock` is kept in all groups, except if the `sparse_super`
/// feature is enabled, in which case it is only kept in groups whose group number is either 0 or a
/// power of 3, 5, 7.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash)]
#[repr(align(0x8))]
pub(crate) struct Ext4Superblock {
    /// Inodes count
    pub(crate) inodes_count: InodeCount,

    /// Blocks count
    pub(crate) blocks_count: Ext4BlkCount32,

    /// Reserved blocks count
    pub(crate) r_blocks_count: Ext4BlkCount32,

    /// Free blocks count
    pub(crate) free_blocks_count: Ext4BlkCount32,

    /// Free inodes count
    pub(crate) free_inodes_count: InodeCount,

    /// First Data Block.
    ///
    /// Block number of the block containing the `Superblock`
    pub(crate) first_datablock: Ext4RealBlkId32,

    /// Block size.
    ///
    /// Defined as `log_2(block_size) - 10`
    pub(crate) log_block_size: u32,

    /// Allocation cluster size.
    ///
    /// Defined as `log_2(cluster_size) - 10`
    pub(crate) log_cluster_size: u32,

    /// Number of blocks in each group
    pub(crate) blocks_per_group: Ext4BlkCount32,

    /// Number of clusters in each group
    pub(crate) clusters_per_group: u32,

    /// Number of inodes in each group
    pub(crate) inodes_per_group: InodeCount,

    /// Last mount time
    pub(crate) mtime: UnixTimestamp32,

    /// Last write time
    pub(crate) wtime: UnixTimestamp32,

    /// Mount count (since last consistency check)
    pub(crate) mnt_count: u16,

    /// Number of mounts allowed before a consistency check is required
    pub(crate) max_mnt_count: u16,

    /// `ext4` magic signature: `0xef53`
    pub(crate) magic: Ext4SuperblockMagic,

    /// File system state
    pub(crate) state: Ext4SuperblockState,

    /// Behavior on error detection
    pub(crate) errors: Ext4SuperblockErrorPolicy,

    /// Minor revision level
    pub(crate) minor_rev_level: u16,

    /// Time of last consistency check
    pub(crate) lastcheck: UnixTimestamp32,

    /// Max time between successive consistency checks
    pub(crate) checkinterval: u32,

    /// Operating System ID from which the filesystem was created
    pub(crate) creator_os: Ext4CreatorOS,

    /// Major revision level
    pub(crate) rev_level: Ext4SuperblockRevision,

    /// Default user ID for reserved blocks
    pub(crate) def_resuid: u16,

    /// Default group ID for reserved blocks
    pub(crate) def_resgid: u16,

    /// First non-reserved inode in file system
    pub(crate) first_ino: InodeNumber,

    /// Size of each inode structure in bytes
    pub(crate) inode_size: u16,

    /// Block group number of this superblock
    pub(crate) block_group_nr: u16,

    /// Compatible feature set
    pub(crate) feature_compat: CompatibleFeatureSet,

    /// Incompatible feature set
    pub(crate) feature_incompat: IncompatibleFeatureSet,

    /// Read-only compatible feature set
    pub(crate) feature_ro_compat: ReadOnlyCompatibleFeatureSet,

    /// 128-bit UUID for volume
    pub(crate) uuid: Ext4FsUuid,

    /// Volume name
    pub(crate) volume_name: Ext4SuperblockVolumeName,

    /// Path volume was last mounted to
    pub(crate) last_mounted: Ext4SuperblockLastMountedPath,

    /// Compression algorithm used
    algo_bitmap: u32,

    /// Number of blocks to try to preallocate for files
    pub(crate) prealloc_blocks: Ext4BlkCount8,

    /// Number of block to preallocate for directories
    pub(crate) prealloc_dir_block: Ext4BlkCount8,

    pub(crate) reserved_gdt_blocks: Ext4BlkCount16,

    /// UUID of journal Superblock
    pub(crate) journal_uuid: u128,

    /// Inode number of journal file
    pub(crate) journal_inum: InodeNumber,

    /// Device number of journal file
    pub(crate) journal_dev: u32,

    /// Start of list of inodes to delete (orphan nodes)
    pub(crate) last_orphan: InodeNumber,

    /// HTREE hash seed
    pub(crate) hash_seed: [u32; 4],

    /// Default hash version to use
    pub(crate) def_hash_version: Ext4HashAlgorithm,

    pub(crate) jnl_backup_type: u8,

    /// Size of group descriptors (in bytes)
    pub(crate) desc_size: u16,

    /// Default mount options
    pub(crate) default_mount_options: Ext4SuperblockMountOptions,

    /// First metablock block group, if enabled
    pub(crate) first_meta_bg: BlockGroupNumber,

    /// File system creation time
    pub(crate) mkfs_time: UnixTimestamp32,

    /// Backup of the journal inode [`InodeBlk`]
    pub(crate) jnl_blocks_blk: InodeBlk,

    /// Backup of the journal inode [`InodeSizeHi']
    pub(crate) jnl_blocks_size_hi: InodeSizeHi,

    /// Backup of the journal inode [`InodeSizeLo']
    pub(crate) jnl_blocks_size_lo: InodeSizeLo,

    // Valid if the 64bit support is enabled `EXT4_FEATURE_INCOMPAT_64BIT`
    /// Blocks count high 32-bits
    pub(crate) blocks_count_hi: Ext4BlkCount32,

    /// Reserved blocks count high 32-bits
    pub(crate) r_blocks_count_hi: Ext4BlkCount32,

    /// Free blocks count high 32-bits
    pub(crate) free_blocks_count_hi: Ext4BlkCount32,

    /// Minimum inode size (in bytes)
    pub(crate) min_extra_isize: u16,

    /// Minimum inode reservation size (in bytes)
    pub(crate) want_extra_isize: u16,

    /// Miscellaneous flags
    pub(crate) flags: Ext4SuperblockFlags,

    /// Amount of logical blocks read of written per disk in a `RAID` array
    pub(crate) raid_stride: Ext4BlkCount16,

    /// Number of seconds to wait in Multi-mount prevention checking
    pub(crate) mmp_interval: u16,

    /// Block for Multi-mount protection
    pub(crate) mmp_block: Ext4RealBlkId,

    /// Amount of blocks to read or write before returning to the current disk in a RAID array
    /// (N * stride)
    pub(crate) raid_stripe_width: Ext4BlkCount32,

    /// `FLEX_BG` group size
    ///
    /// Defined as `log_2(groups_per_flex) - 10`
    pub(crate) log_groups_per_flex: u8,

    /// Metadata checksum algorithm used
    pub(crate) checksum_type: Ext4ChksumAlgorithm,

    /// Padding to next 32 bits
    reserved_pad: u16,

    /// Amount of KBs written
    pub(crate) kbytes_written: u64,

    /// Inode number of the active snapshot
    pub(crate) snapshot_inum: InodeNumber,

    /// Sequential ID of active snapshot
    pub(crate) snapshot_id: u32,

    /// Reserved blocks for active snapshot future use
    pub(crate) snapshot_r_blocks_count: Ext4BlkCount,

    /// Inode number of the head of the on-disk snapshot list
    pub(crate) snapshot_list: InodeNumber,

    /// Number of filesystem errors
    pub(crate) error_count: u32,

    /// First time an error occurred
    pub(crate) first_error_time: UnixTimestamp32,

    /// Inode number in the first error
    pub(crate) first_error_ino: InodeNumber,

    /// Block number in the first error
    pub(crate) first_error_block: Ext4RealBlkId,

    /// Function where the first error occurred
    first_error_func: [u8; 32],

    /// Line number where the first error occurred
    first_error_line: u32,

    /// Last time an error occurred
    pub(crate) last_error_time: UnixTimestamp32,

    /// Inode number of the last error
    pub(crate) last_error_ino: InodeNumber,

    /// Line number where the last error occurred
    last_error_line: u32,

    /// Block number in the last error
    pub(crate) last_error_block: Ext4RealBlkId,

    /// Function where the last error occurred
    last_error_func: [u8; 32],

    /// Mount options (C string)
    pub(crate) mount_opts: Ext4SuperblockMountOptionsString,

    /// Inode number for user quota file
    pub(crate) usr_quota_inum: InodeNumber,

    /// Inode number for group quota file
    pub(crate) grp_quota_inum: InodeNumber,

    /// Overhead block/clusters in file system
    pub(crate) overhead_blocks: Ext4BlkCount32,

    /// Block groups with backup `Superblock`s if the sparse superblock is set
    backup_bgs: [BlockGroupNumber; 2],

    /// Encryption algorithm used
    encrypt_algos: [Ext4EncryptionAlgorithm; 4],

    /// Salt used for `string2key` algorithm
    encrypt_pw_salt: [u8; 16],

    /// Location of the lost+found inode
    pub(crate) lpf_ino: InodeNumber,

    /// Inode for tracking project quota
    pub(crate) prj_quota_inum: InodeNumber,

    /// `crc32c(uuid)` if `csum_seed` is set
    pub(crate) checksum_seed: Ext4FsUuidCrc32,

    /// High 8-bits of the last written time field
    wtime_hi: u8,

    /// High 8-bits of the last mount time field
    mtime_hi: u8,

    /// High 8-bits of the filesystem creation time field
    mkfs_time_hi: u8,

    /// High 8-bits of the last consistency check time field
    lastcheck_hi: u8,

    /// High 8-bits of the first error time field
    first_error_time_hi: u8,

    /// High 8-bits of the last error time field
    last_error_time_hi: u8,

    /// Error code of the first error
    first_error_errcode: u8,

    /// Error code of the last error
    last_error_errcode: u8,

    /// Filename charset encoding
    encoding: u16,

    /// Filename charset encoding flags
    encoding_flags: u16,

    reserved: [u32; 95],

    /// Checksum of the superblock: `crc32c(superblock)`
    checksum: Ext4SuperblockChksum,
}

impl Ext4Superblock {
    /// Returns the [`BlockGroupNumber`] of the block group to which the given `Inode` belongs to.
    ///
    /// Does not check that the given [`InodeNumber`] is valid / in filesystem bounds.
    pub(super) fn get_inode_blk_group(&self, inode_id: InodeNumber) -> BlockGroupNumber {
        cast((inode_id - 1) / self.inodes_per_group)
    }

    /// Returns the position of the requested `Inode` on disk.
    ///
    /// The position is a tuple `(block_group_id, entry_block_offset_in_block_group, entry_byte_offset_in_block)`
    pub(super) fn get_inode_entry_pos(
        &self,
        inode_id: InodeNumber,
    ) -> (BlockGroupNumber, Ext4BlkCount, u64) {
        let inode_bg_idx = (inode_id - 1) % self.inodes_per_group;
        let inode_byte_idx = u64::from(inode_bg_idx) * u64::from(self.inode_size);

        let inode_blk_offset: u64 = inode_byte_idx / self.blk_size();

        let inode_bytes_idx_in_blk: u64 = inode_byte_idx % self.blk_size();

        (
            self.get_inode_blk_group(inode_id),
            cast(inode_blk_offset),
            inode_bytes_idx_in_blk,
        )
    }
    /// Compares the checksum of the `Ext4Superblock` to its on-disk value.
    ///
    /// The checksum of an `Ext4Superblock` can be computed (after having set the checksum field to 0) using:
    ///
    /// ```
    /// crc32c_calc(superblock)
    /// ```
    pub(crate) fn validate_chksum(&self) -> bool {
        let comp_chksum = self.compute_chksum();

        if comp_chksum != self.checksum {
            error!(
                    "ext4-fs",
                    "found ext4 filesystem with invalid superblock checksum (got {:#010x} expected {:#010x})",
                    comp_chksum.0,
                self.checksum.0
                );

            return false;
        }

        true
    }

    /// Updates the value of the checksum field for this `Ext4Superblock`, based on the current value of the other
    /// fields.
    ///
    /// Useful before writing back the `Ext4Superblock` to disk after having updated several of its field.
    pub(crate) fn update_chksum(&mut self) {
        self.checksum = self.compute_chksum();
    }

    /// Returns the number of Block Groups for this filesystem.
    pub(crate) fn bg_count(&self) -> BlockGroupNumber {
        cast::<u32, BlockGroupNumber>(
            (1 + self.blk_count() / self.blocks_per_group.into())
                .try_into()
                .expect("invalid block group count"),
        )
    }

    /// Returns the number of free blocks.
    pub(crate) fn free_blk_count(&self) -> Ext4BlkCount {
        if self
            .feature_incompat
            .includes(IncompatibleFeatureSet::EXT4_FEATURE_INCOMPAT_64BIT)
        {
            self.free_blocks_count
                .add_high_bits(self.free_blocks_count_hi)
        } else {
            self.free_blocks_count.into()
        }
    }

    /// Returns the total count of blocks.
    pub(crate) fn blk_count(&self) -> Ext4BlkCount {
        if self
            .feature_incompat
            .includes(IncompatibleFeatureSet::EXT4_FEATURE_INCOMPAT_64BIT)
        {
            self.blocks_count.add_high_bits(self.blocks_count_hi)
        } else {
            self.blocks_count.into()
        }
    }

    /// Returns the size of a block, in bytes.
    pub(crate) fn blk_size(&self) -> u64 {
        1024 << self.log_block_size
    }

    /// Checks whether this `ext4` filesystem uses the _Multi Mount Protection_ (`MMP`) feature.
    pub(crate) fn mmp_enabled(&self) -> bool {
        self.feature_incompat
            .includes(IncompatibleFeatureSet::EXT4_FEATURE_INCOMPAT_MMP)
    }

    fn compute_chksum(&self) -> Ext4SuperblockChksum {
        let sb_bytes = unsafe {
            core::slice::from_raw_parts(
                transmute::<*const Ext4Superblock, *const u8>(self),
                size_of::<Self>(),
            )
        };
        // we remove the checksum bytes from the calculation
        let sb_chk_bytes = &sb_bytes[..sb_bytes.len() - 4];

        cast(crc32c_calc(sb_chk_bytes))
    }
}
