//! ext4 inode related structures.
//!
//! `Inode` (index node) are the base structure that holds data about file-system objects, such as files or
//! directories.

use core::fmt::{Display, Formatter};

use alloc::{format, string::String, vec::Vec};
use bytemuck::{bytes_of, cast, Pod, Zeroable};

use crate::fs::ext4::sb::Ext4FsUuid;
use crate::{
    error, ext4_uint_field_derive_display,
    fs::ext4::{crc32c_calc, extent::ExtentBlock},
    time::{DateTime, UnixTimestamp},
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub struct InodeCount(u32);

ext4_uint_field_derive_display!(InodeCount);

impl core::ops::Div<u32> for InodeCount {
    type Output = u32;

    fn div(self, rhs: u32) -> Self::Output {
        self.0 / rhs
    }
}

impl core::ops::Div<InodeCount> for u32 {
    type Output = u32;

    fn div(self, rhs: InodeCount) -> Self::Output {
        self / rhs.0
    }
}

impl core::ops::Rem<InodeCount> for u32 {
    type Output = u32;

    fn rem(self, rhs: InodeCount) -> Self::Output {
        self % rhs.0
    }
}

/// A number representing an inode.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub struct InodeNumber(u32);

impl InodeNumber {
    /// Inode 0 represents an unused directory entry
    pub const UNUSED_DIR_ENTRY: Self = Self(0);

    /// Inode 2 is reserved for the root directory of the file system.
    pub const ROOT_DIR: Self = Self(0x2);

    /// Inode 3 is reserved for the user quota
    pub const USER_QUOTA: Self = Self(0x3);

    /// Inode 4 is reserved for the group quota file
    pub const GROUP_QUOTA: Self = Self(0x4);

    /// Inode 5 is unused, but may have been intended for stage 2 bootloaders
    pub const BOOTLOADER: Self = Self(0x5);

    /// Inode 6 is unused, but may have been intended for the never implemented undeletion
    pub const UNDELETE: Self = Self(0x6);

    /// Inode 7 is the reserved group descriptors inode
    pub const RESIZE: Self = Self(0x7);

    /// Inode 8 is the ext4 journal
    pub const JOURNAL: Self = Self(0x8);

    /// Inode 9 is the exclude inode, for snapshots
    pub const EXCLUDE: Self = Self(0x9);

    /// Inode 10 is used for ext4 metadata replication in some non-upstream patches
    pub const REPLICA: Self = Self(0xA);
}

ext4_uint_field_derive_display!(InodeNumber);

impl core::ops::Sub<u32> for InodeNumber {
    type Output = u32;

    fn sub(self, rhs: u32) -> Self::Output {
        self.0.saturating_sub(rhs)
    }
}

impl From<InodeNumber> for u32 {
    fn from(value: InodeNumber) -> Self {
        value.0
    }
}

impl From<InodeNumber> for usize {
    fn from(value: InodeNumber) -> usize {
        value.0.try_into().expect("invalid inode number")
    }
}
impl From<usize> for InodeNumber {
    fn from(value: usize) -> Self {
        InodeNumber(value.try_into().expect("invalid inode number (not 32-bit)"))
    }
}

/// File mode / type representation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeFileMode(u16);

impl InodeFileMode {
    /// Empty file mode
    const EMPTY_MODE: Self = Self(0);

    /// Others may execute.
    pub(crate) const S_IXOTH: Self = Self(0x0001);

    /// Others may write.
    pub(crate) const S_IWOTH: Self = Self(0x0002);

    /// Others may read.
    pub(crate) const S_IROTH: Self = Self(0x0003);

    /// Group may execute.
    pub(crate) const S_IXGRP: Self = Self(0x0008);

    /// Group may write.
    pub(crate) const S_IWGRP: Self = Self(0x0010);

    /// Group may read.
    pub(crate) const S_IRGRP: Self = Self(0x0020);

    /// User may execute.
    pub(crate) const S_IXUSR: Self = Self(0x0040);

    /// User may write.
    pub(crate) const S_IWUSR: Self = Self(0x0080);

    /// User may read.
    pub(crate) const S_IRUSR: Self = Self(0x0100);

    /// Sticky bit.
    pub(crate) const S_ISVTX: Self = Self(0x0200);

    /// Set GID
    pub(crate) const S_ISGID: Self = Self(0x0400);

    /// Set UID
    pub(crate) const S_ISUID: Self = Self(0x0800);

    /// FIFO
    pub(crate) const S_IFIFO: Self = Self(0x1000);

    /// Character device
    pub(crate) const S_IFCHR: Self = Self(0x2000);

    /// Directory
    pub(crate) const S_IFDIR: Self = Self(0x4000);

    /// Block device
    pub(crate) const S_IFBLK: Self = Self(0x6000);

    /// Regular file
    pub(crate) const S_IFREG: Self = Self(0x8000);

    /// Symbolic link
    pub(crate) const S_IFLNK: Self = Self(0xA000);

    /// Socket
    pub(crate) const S_IFSOCK: Self = Self(0xC000);
}

macro_rules! symb_perm {
    ($self: ident, $str: tt,  $symbol: literal, $flag: expr) => {
        if $self.contains($flag) {
            $str.push($symbol);
        } else {
            $str.push('-');
        }
    };
}

impl Display for InodeFileMode {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let mut symbolic_str = String::new();

        symb_perm!(self, symbolic_str, 'r', InodeFileMode::S_IRUSR);
        symb_perm!(self, symbolic_str, 'w', InodeFileMode::S_IWUSR);
        symb_perm!(self, symbolic_str, 'x', InodeFileMode::S_IXUSR);
        symb_perm!(self, symbolic_str, 'r', InodeFileMode::S_IRGRP);
        symb_perm!(self, symbolic_str, 'w', InodeFileMode::S_IWGRP);
        symb_perm!(self, symbolic_str, 'x', InodeFileMode::S_IXGRP);
        symb_perm!(self, symbolic_str, 'r', InodeFileMode::S_IROTH);
        symb_perm!(self, symbolic_str, 'w', InodeFileMode::S_IWOTH);
        symb_perm!(self, symbolic_str, 'x', InodeFileMode::S_IXOTH);

        f.write_str(&symbolic_str)
    }
}

/// Type associated to a given [`Inode`].
#[allow(clippy::upper_case_acronyms)]
pub(crate) enum InodeType {
    Regular,
    Directory,
    FIFO,
    CharacterDevice,
    BlockDevice,
    SymbolicLink,
    Socket,
}

impl Display for InodeType {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let type_str = match self {
            InodeType::Regular => "File",
            InodeType::Directory => "Directory",
            InodeType::FIFO => "FIFO",
            InodeType::CharacterDevice => "CharacterDevice",
            InodeType::BlockDevice => "Block device",
            InodeType::SymbolicLink => "Symbolic Link",
            InodeType::Socket => "Socket",
        };

        f.write_str(type_str)
    }
}

impl From<InodeFileMode> for InodeType {
    fn from(value: InodeFileMode) -> Self {
        let file_type = InodeFileMode(value.0 & ((1 << 12) - 1));

        match file_type {
            InodeFileMode::S_IFSOCK => Self::Socket,
            InodeFileMode::S_IFLNK => Self::SymbolicLink,
            InodeFileMode::S_IFCHR => Self::CharacterDevice,
            InodeFileMode::S_IFBLK => Self::BlockDevice,
            InodeFileMode::S_IFIFO => Self::FIFO,
            InodeFileMode::S_IFDIR => Self::Directory,
            _ => Self::Regular,
        }
    }
}

impl core::ops::BitAnd for InodeFileMode {
    type Output = InodeFileMode;

    fn bitand(self, rhs: Self) -> Self::Output {
        InodeFileMode(self.0 & rhs.0)
    }
}

impl InodeFileMode {
    pub(crate) fn contains(self, mode: InodeFileMode) -> bool {
        self & mode != Self::EMPTY_MODE
    }
}

impl core::ops::BitOrAssign for InodeFileMode {
    fn bitor_assign(&mut self, rhs: Self) {
        // The upper bits of the `i_mode` field correpond to the file type, and these file types
        // are mutually exclusive. Therefore, we should only apply OR to the low order bits which
        // correspond to the file mode, as it does not make sense to do so for the high order bits.
        let file_mode_part_self = self.0 & ((1 << 12) - 1);
        let file_mode_part_rhs = rhs.0 & ((1 << 12) - 1);

        let final_file_mode = file_mode_part_rhs | file_mode_part_self;

        self.0 = final_file_mode | file_mode_part_self & !((1 << 12) - 1);
    }
}

impl core::ops::BitOr for InodeFileMode {
    type Output = InodeFileMode;

    fn bitor(self, rhs: Self) -> Self::Output {
        // The upper bits of the `i_mode` field correpond to the file type, and these file types
        // are mutually exclusive. Therefore, we should only apply OR to the low order bits which
        // correspond to the file mode, as it does not make sense to do so for the high order bits.
        let file_mode_part_self = self.0 & ((1 << 12) - 1);
        let file_mode_part_rhs = rhs.0 & ((1 << 12) - 1);

        let final_file_mode = file_mode_part_rhs | file_mode_part_self;

        InodeFileMode(final_file_mode | file_mode_part_self & !((1 << 12) - 1))
    }
}

/// Inode generation number / File number.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeGeneration(u32);

impl Display for InodeGeneration {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{}", self.0))
    }
}

/// Low 32-bits of the size in bytes of the associated `Inode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeSizeLo(u32);

/// High 32-bits of the size in bytes of the associated `Inode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeSizeHi(u32);

/// Size of the associated `Inode` in bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeSize(u64);

impl Display for InodeSize {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{}", self.0))
    }
}

impl From<InodeSize> for InodeSizeLo {
    fn from(value: InodeSize) -> Self {
        InodeSizeLo(u32::try_from(value.0 & 0xffff_ffff).expect("invalid conversion"))
    }
}

impl From<InodeSize> for InodeSizeHi {
    fn from(value: InodeSize) -> Self {
        InodeSizeHi(u32::try_from((value.0 >> 32) & 0xffff_ffff).expect("invalid conversion"))
    }
}

impl core::ops::Add<InodeSizeHi> for InodeSizeLo {
    type Output = InodeSize;

    fn add(self, rhs: InodeSizeHi) -> Self::Output {
        InodeSize(u64::from(self.0) | (u64::from(rhs.0) << 32))
    }
}

impl core::ops::Add<u64> for InodeSize {
    type Output = InodeSize;

    fn add(self, rhs: u64) -> Self::Output {
        InodeSize(self.0.saturating_add(rhs))
    }
}

impl core::ops::Sub<u64> for InodeSize {
    type Output = InodeSize;

    fn sub(self, rhs: u64) -> Self::Output {
        InodeSize(self.0.saturating_sub(rhs))
    }
}

/// Checksum of the associated `Inode` structure.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeChksum(u32);

impl Display for InodeChksum {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{}", self.0))
    }
}

impl InodeChksum {
    /// Used to remove the checksum entry from an `Inode` structure.
    pub(crate) const ERASE_CHKSUM: Self = Self(0);
}

impl From<InodeChksum> for InodeChksumLo {
    fn from(val: InodeChksum) -> Self {
        InodeChksumLo(u16::try_from(val.0 & 0xffff).expect("invalid conversion"))
    }
}

impl From<InodeChksum> for InodeChksumHi {
    fn from(val: InodeChksum) -> Self {
        InodeChksumHi(u16::try_from((val.0 >> 16) & 0xffff).expect("invalid conversion"))
    }
}

/// Low 16-bits of the checksum of the associated `Inode` structure.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeChksumLo(u16);

/// High 16-bits of the checksum of the associated `Inode` structure.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeChksumHi(u16);

impl core::ops::Add<InodeChksumHi> for InodeChksumLo {
    type Output = InodeChksum;

    fn add(self, rhs: InodeChksumHi) -> Self::Output {
        InodeChksum(u32::from(self.0) | (u32::from(rhs.0) << 16))
    }
}

/// Inode flags set in the associated `Inode` structure.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeFlags(u32);

impl Display for InodeFlags {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{}", self.0))
    }
}

impl InodeFlags {
    /// This file requires secure deletion. (not implemented)
    pub(crate) const EXT4_SECRM_FL: Self = Self(0x1);

    /// This file should be preserved. (not implemented)
    pub(crate) const EXT4_UNRM_FL: Self = Self(0x2);

    /// File is compressed
    pub(crate) const EXT4_COMPR_FL: Self = Self(0x4);

    /// All writes to the file must be synchronous
    pub(crate) const EXT4_SYNC_FL: Self = Self(0x8);

    /// File is immutable
    pub(crate) const EXT4_IMMUTABLE_FL: Self = Self(0x10);

    /// File can only be appended
    pub(crate) const EXT4_APPEND_FL: Self = Self(0x20);

    /// The `dump` utility should not dump this file.
    pub(crate) const EXT4_NODUMP_FL: Self = Self(0x40);

    /// Do not update access time
    pub(crate) const EXT4_NOATIME_FL: Self = Self(0x80);

    /// Dirty compressed file.
    pub(crate) const EXT4_DIRTY_FL: Self = Self(0x100);

    /// File has one or more compressed clusters.
    pub(crate) const EXT4_COMPRBLK_FL: Self = Self(0x200);

    /// Do not compress file.
    pub(crate) const EXT4_NOCOMPR_FL: Self = Self(0x400);

    /// Encrypted inode.
    pub(crate) const EXT4_ENCRYPT_FL: Self = Self(0x800);

    /// Directory has hashed indexes.
    pub(crate) const EXT4_INDEX_FL: Self = Self(0x1000);

    /// AFS magic directory
    pub(crate) const EXT4_IMAGIC_FL: Self = Self(0x2000);

    /// File data must always be written through the journal
    pub(crate) const EXT4_JOURNAL_DATA_FL: Self = Self(0x4000);

    /// File tail should not be merged.
    pub(crate) const EXT4_NOTAIL_FL: Self = Self(0x8000);

    /// All directory entry data should be written synchronously.
    pub(crate) const EXT4_DIRSYNC_FL: Self = Self(0x10000);

    /// Top of directory hierarchy.
    pub(crate) const EXT4_TOPDIR_FL: Self = Self(0x20000);

    /// Huge file.
    pub(crate) const EXT4_HUGE_FILE_FL: Self = Self(0x40000);

    /// Inode uses extents.
    pub(crate) const EXT4_EXTENTS_FL: Self = Self(0x80000);

    /// Verity protected file.
    pub(crate) const EXT4_VERITY_FL: Self = Self(0x10_0000);

    /// Inode stores a large extended attribute value in its data block.
    pub(crate) const EXT4_EA_INODE_FL: Self = Self(0x20_0000);

    /// This file has blocks allocated past `EOF`.
    pub(crate) const EXT4_EOFBLOCKS_FL: Self = Self(0x40_0000);

    /// Inode is a snapshot.
    pub(crate) const EXT4_SNAPFILE_FL: Self = Self(0x80_0000);

    /// Snapshot is being deleted.
    pub(crate) const EXT4_SNAPFILE_DELETED_FL: Self = Self(0x100_0000);

    /// Snapshot shrink has completed.
    pub(crate) const EXT4_SNAPFILE_SHRUNK_FL: Self = Self(0x200_0000);

    /// Inode has inline data.
    pub(crate) const EXT4_INLINE_DATA_FL: Self = Self(0x400_0000);

    /// Create children with the same project ID.
    pub(crate) const EXT4_PROJINHERIT_FL: Self = Self(0x800_0000);

    /// Reserved for `ext4` library.
    pub(crate) const EXT4_RESERVED_FL: Self = Self(0x8000_0000);
}

impl core::ops::BitOr for InodeFlags {
    type Output = InodeFlags;

    fn bitor(self, rhs: Self) -> Self::Output {
        InodeFlags(self.0 | rhs.0)
    }
}

impl core::ops::BitAnd for InodeFlags {
    type Output = InodeFlags;

    fn bitand(self, rhs: Self) -> Self::Output {
        InodeFlags(self.0 & rhs.0)
    }
}

/// Inode version.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeVersion(u32);

/// Block count for this `Inode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeBlkCount(u64);

impl Display for InodeBlkCount {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{}", self.0))
    }
}

/// Low 32-bits of the block count for this `Inode`
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeBlkCountLo(u32);

/// High 32-bits of the block count for this `Inode`
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeBlkCountHi(u16);

impl From<InodeBlkCount> for InodeBlkCountLo {
    fn from(value: InodeBlkCount) -> Self {
        InodeBlkCountLo(u32::try_from(value.0 & 0xffff_ffff).expect("invalid conversion"))
    }
}

impl From<InodeBlkCount> for InodeBlkCountHi {
    fn from(value: InodeBlkCount) -> Self {
        InodeBlkCountHi(u16::try_from((value.0 >> 32) & 0xffff).expect("invalid conversion"))
    }
}

impl core::ops::Add<InodeBlkCountHi> for InodeBlkCountLo {
    type Output = InodeBlkCount;

    fn add(self, rhs: InodeBlkCountHi) -> Self::Output {
        InodeBlkCount(u64::from(self.0) | (u64::from(rhs.0) << 32))
    }
}

/// Last access time of this `Inode`, in seconds since the `epoch`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeAccessTime(u32);

impl core::ops::Add<InodeAccessTimeExtraBits> for InodeAccessTime {
    type Output = UnixTimestamp;

    fn add(self, rhs: InodeAccessTimeExtraBits) -> Self::Output {
        UnixTimestamp::from(u64::from(self.0) | (u64::from(rhs.0) << 32))
    }
}
/// Last change time for this `Inode`, in seconds since the `epoch`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeChangeTime(u32);

impl core::ops::Add<InodeChangeTimeExtraBits> for InodeChangeTime {
    type Output = UnixTimestamp;

    fn add(self, rhs: InodeChangeTimeExtraBits) -> Self::Output {
        UnixTimestamp::from(u64::from(self.0) | (u64::from(rhs.0) << 32))
    }
}
/// Last modification time for this `Inode`, in seconds since the `epoch`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeModificationTime(u32);

impl core::ops::Add<InodeModificationTimeExtraBits> for InodeModificationTime {
    type Output = UnixTimestamp;

    fn add(self, rhs: InodeModificationTimeExtraBits) -> Self::Output {
        UnixTimestamp::from(u64::from(self.0) | (u64::from(rhs.0) << 32))
    }
}
/// Deletion time for this `Inode`, in seconds since the `epoch`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeDeletionTime(u32);

impl From<InodeDeletionTime> for UnixTimestamp {
    fn from(value: InodeDeletionTime) -> Self {
        Self::from(u64::from(value.0))
    }
}

/// Creation time for this `Inode`, in seconds since the `epoch`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeCreationTime(u32);

impl core::ops::Add<InodeCreationTimeExtraBits> for InodeCreationTime {
    type Output = UnixTimestamp;

    fn add(self, rhs: InodeCreationTimeExtraBits) -> Self::Output {
        UnixTimestamp::from(u64::from(self.0) | (u64::from(rhs.0) << 32))
    }
}

/// Group ID for this `Inode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeGroupId(u32);

impl Display for InodeGroupId {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{}", self.0))
    }
}

impl InodeGroupId {
    pub(crate) const SUPERUSER: Self = Self(0);
}

/// Low 16-bits of this `Inode` Group ID.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeGroupIdLo(u16);

/// High 16-bits of this `Inode` Group ID.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeGroupIdHi(u16);

impl core::ops::Add<InodeGroupIdHi> for InodeGroupIdLo {
    type Output = InodeGroupId;

    fn add(self, rhs: InodeGroupIdHi) -> Self::Output {
        InodeGroupId(u32::from(self.0) | (u32::from(rhs.0) << 16))
    }
}

/// Owner ID of this `Inode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeOwnerId(u32);

impl Display for InodeOwnerId {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{}", self.0))
    }
}

impl InodeOwnerId {
    pub(crate) const SUPERUSER: Self = Self(0);
}

/// Low 16-bits of this `Inode` Owner ID.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeOwnerIdLo(u16);

/// High 16-bits of this `Inode` Owner ID.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeOwnerIdHi(u16);

impl core::ops::Add<InodeOwnerIdHi> for InodeOwnerIdLo {
    type Output = InodeOwnerId;

    fn add(self, rhs: InodeOwnerIdHi) -> Self::Output {
        InodeOwnerId(u32::from(self.0) | (u32::from(rhs.0) << 16))
    }
}

/// Hard link count.
///
/// Usually, the maximum amount of hard links an `Inode` may have is 65000. That may be increased
/// if the `DIR_NLINK` feature is enabled for the filesystem.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeHardLinkCount(u16);

impl Display for InodeHardLinkCount {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{}", self.0))
    }
}

/// Used for file block indexing information.
///
/// May hold different structures depending on the situation.
///
/// # Symbolic Link
///
/// The target of the link will be stored in this field is the string is less than 60 bytes long
///
/// # Block Map
///
/// The `Inode` may use a block map to link file block numbers to logical block number on disk.
/// That block map consists in a 1 to 3 levels 1-1 map.
///
/// - 0 -> 11 : Direct map to file blocks 0 - 11
///
/// - 12: Indirect block
///
/// - 13: Double indirect block
///
/// - 14: Triple indirect block
///
/// # Extent Tree
///
/// Extent trees were introduced in `ext4`. Leaf nodes points directly to data blocks, and extent
/// index points to other index nodes or to data blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub(crate) struct InodeBlk([u8; 60]);

impl InodeBlk {
    pub(crate) fn as_extent_block(&self) -> ExtentBlock {
        ExtentBlock(self.0.to_vec())
    }
}

unsafe impl Pod for InodeBlk {}

unsafe impl Zeroable for InodeBlk {}

impl Default for InodeBlk {
    fn default() -> Self {
        Self([0; 60])
    }
}

/// Extended attributes block of the associated `Inode`.
///
/// Extended attributes are key / value pairs associated to an `Inode`.
/// ACLs may be implemented using extended attributes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeXattr(u64);

/// Low 32-bits of the extended attribute block.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeXattrLo(u32);

/// High 32-bits of the extended attribute block.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeXattrHi(u16);

impl core::ops::Add<InodeXattrHi> for InodeXattrLo {
    type Output = InodeXattr;

    fn add(self, rhs: InodeXattrHi) -> Self::Output {
        InodeXattr(u64::from(self.0) | (u64::from(rhs.0) << 32))
    }
}

/// Size of the `Inode` field beyond the original ext2 inode (`inode_size - 128`)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeExtraSize(u16);

impl InodeExtraSize {
    pub(crate) const NO_EXTRA_SIZE: Self = Self(0);
}

impl core::ops::Add<u16> for InodeExtraSize {
    type Output = u16;

    fn add(self, rhs: u16) -> Self::Output {
        self.0.saturating_add(rhs)
    }
}

/// Extra access time bits.
///
/// Extends the seconds since the epoch maximum value, and adds nanosecond timestamp accuracy
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeAccessTimeExtraBits(u32);

/// Extra changed time bits.
///
/// Extends the seconds since the epoch maximum value, and adds nanosecond timestamp accuracy
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeChangeTimeExtraBits(u32);

/// Extra modification time bits.
///
/// Extends the seconds since the epoch maximum value, and adds nanosecond timestamp accuracy
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeModificationTimeExtraBits(u32);

/// Extra creation time bits.
///
/// Extends the seconds since the epoch maximum value, and adds nanosecond timestamp accuracy
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeCreationTimeExtraBits(u32);

/// `Inode` project ID.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeProjectId(u32);

/// High 32-bits for version number.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InodeVersionHi(u32);

/// The `Inode` (index node) stores all metadata related to a file or a directory (permissions, blocks,
/// timestamps, ...).
///
/// Directories are direct file name to `Inode` maps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(C)]
pub(crate) struct Inode {
    /// File mode
    pub(crate) i_mode: InodeFileMode,

    /// Lower 16-bit of Owner UID
    pub(crate) i_uid: InodeOwnerIdLo,

    /// Lower 32-bits of size in bytes
    pub(crate) i_size_lo: InodeSizeLo,

    /// Last access time, in seconds since the epoch
    pub(crate) i_atime: InodeAccessTime,

    /// Last inode change time, in seconds since the epoch
    pub(crate) i_ctime: InodeChangeTime,

    /// Last data modification time, in seconds since the epoch
    pub(crate) i_mtime: InodeModificationTime,

    /// Deletion time, in seconds since the epoch
    pub(crate) i_dtime: InodeDeletionTime,

    /// Lower 16-bits of GID
    pub(crate) i_gid: InodeGroupIdLo,

    /// Hard link count
    ///
    /// The usual link limit is 65,000 hard links, but if [`EXT4_FEATURE_DIR_NLINK`] is set, `ext4`
    /// supports more than 64,998 subdirectories by setting this field to 1 to indicate that the
    /// number of hard links is not known.
    pub(crate) i_links_count: InodeHardLinkCount,

    /// Lower 32-bits of block count.
    pub(crate) i_blocks_lo: InodeBlkCountLo,

    /// Inode flags
    pub(crate) i_flags: InodeFlags,

    /// Inode version
    pub(crate) i_version: InodeVersion,

    /// Block map or extent tree
    pub(crate) i_block: InodeBlk,

    /// File version
    pub(crate) i_generation: InodeGeneration,

    /// Lower 32-bits of extended attribute block.
    pub(crate) i_file_acl_lo: InodeXattrLo,

    /// Upper 32-bits of file directory/size.
    pub(crate) i_size_hi: InodeSizeHi,

    /// Fragment address (outdated)
    i_faddr: u32,

    /// High 16-bits of the block count
    pub(crate) i_blocks_high: InodeBlkCountHi,

    /// High 16-bits of the extended attribute block
    pub(crate) i_file_acl_high: InodeXattrHi,

    /// High 16-bits of the Owner UID
    pub(crate) i_uid_high: InodeOwnerIdHi,

    /// High 16-bits of the GID
    pub(crate) i_gid_high: InodeGroupIdHi,

    /// Lower 16-bits of the inode checksum
    pub(crate) i_checksum_lo: InodeChksumLo,

    reserved: u16,

    /// Size of this inode - 128
    pub(crate) i_extra_isize: InodeExtraSize,

    /// Upper 16-bits of the inode checksum
    pub(crate) i_checksum_hi: InodeChksumHi,

    /// Extra change time bits
    pub(crate) i_ctime_extra: InodeChangeTimeExtraBits,

    /// Extra modification time bits
    pub(crate) i_mtime_extra: InodeModificationTimeExtraBits,

    /// Extra access time bits
    pub(crate) i_atime_extra: InodeAccessTimeExtraBits,

    /// File creation time, in seconds since the epoch
    pub(crate) i_crtime: InodeCreationTime,

    /// Extra file creation time bits.
    pub(crate) i_crtime_extra: InodeCreationTimeExtraBits,

    /// Upper 32-bits of version number
    pub(crate) i_version_hi: InodeVersionHi,

    /// Project ID
    pub(crate) i_projid: InodeProjectId,
}

impl Inode {
    /// Returns the type of this `Inode` (file, directory, ...)
    pub(crate) fn inode_type(&self) -> InodeType {
        InodeType::from(self.i_mode)
    }

    /// Returns this file deletion time in seconds since the epoch, if applicable.
    pub(crate) fn deletion_time(&self) -> UnixTimestamp {
        UnixTimestamp::from(self.i_dtime)
    }

    /// Returns the last time this file was changed, in seconds since the epoch.
    ///
    /// If the `Inode` structure is large enough, the signed seconds count encoded with 32 bits is
    /// extended with 2 extra bits, and 30 additional bits provide nanoseconds precision.
    pub(crate) fn change_time(&self) -> UnixTimestamp {
        self.i_ctime + self.i_ctime_extra
    }

    /// Returns the last time this file was accessed, in seconds since the epoch.
    ///
    /// If the `Inode` structure is large enough, the signed seconds count encoded with 32 bits is
    /// extended with 2 extra bits, and 30 additional bits provide nanoseconds precision.
    pub(crate) fn access_time(&self) -> UnixTimestamp {
        self.i_atime + self.i_atime_extra
    }

    /// Returns the last time this file was modified, in seconds since the epoch.
    ///
    /// If the `Inode` structure is large enough, the signed seconds count encoded with 32 bits is
    /// extended with 2 extra bits, and 30 additional bits provide nanoseconds precision.
    pub(crate) fn modification_time(&self) -> UnixTimestamp {
        self.i_mtime + self.i_mtime_extra
    }

    /// Returns the time at which this file was created, in seconds since the epoch.
    ///
    /// If the `Inode` structure is large enough, the signed seconds count encoded with 32 bits is
    /// extended with 2 extra bits, and 30 additional bits provide nanoseconds precision.
    pub(crate) fn creation_time(&self) -> UnixTimestamp {
        self.i_crtime + self.i_crtime_extra
    }

    /// Compares the checksum of the `Inode` to its on-disk value.
    ///
    /// The checksum of an `Inode` can be computed (after having set the checksum field to 0) using:
    ///
    /// ```
    /// crc32c_calc(fs_uuid + inode_id + inode_gen + inode_block)
    /// ```
    pub(crate) fn validate_chksum(&self, fs_uuid: Ext4FsUuid, inode_id: InodeNumber) -> bool {
        let on_disk_chksum = self.i_checksum_lo + self.i_checksum_hi;
        let comp_chksum = self.compute_chksum(fs_uuid, inode_id);

        let matching_chksum = if self.i_extra_isize == InodeExtraSize::NO_EXTRA_SIZE {
            let comp_chksum_lo: InodeChksumLo = comp_chksum.into();

            comp_chksum_lo == on_disk_chksum.into()
        } else {
            comp_chksum == on_disk_chksum
        };

        if !matching_chksum {
            error!(
                "ext4",
                "invalid inode checksum (inode {:#X})",
                cast::<InodeNumber, u32>(inode_id)
            );

            return false;
        }

        true
    }

    /// Sets the value of the checksum field for this `Inode`.
    ///
    /// May store only the low 16-bits of the checksum if this `Inode` does not use the
    /// `i_checksum_hi` field.
    pub(crate) fn set_chksum(&mut self, chksum: InodeChksum) {
        self.i_checksum_lo = chksum.into();
        if self.i_extra_isize != InodeExtraSize::NO_EXTRA_SIZE {
            self.i_checksum_hi = chksum.into();
        }
    }

    /// Returns the checksum value of this `Inode`.
    pub(crate) fn chksum(&self) -> InodeChksum {
        self.i_checksum_lo + self.i_checksum_hi
    }

    /// Updates the value of the checksum field for this `Inode`, based on the current value of the other
    /// fields.
    ///
    /// Useful before writing back the `Inode` to disk after having updated several of its field.
    pub(crate) fn update_chksum(&mut self, fs_uuid: Ext4FsUuid, inode_id: InodeNumber) {
        self.set_chksum(self.compute_chksum(fs_uuid, inode_id));
    }

    /// Returns the block count for this `Inode`.
    pub(crate) fn blk_count(&self) -> InodeBlkCount {
        self.i_blocks_lo + self.i_blocks_high
    }

    /// Sets the block count of this `Inode`.
    pub(crate) fn set_blk_count(&mut self, new_blk_count: InodeBlkCount) {
        self.i_blocks_lo = new_blk_count.into();
        self.i_blocks_high = new_blk_count.into();
    }

    /// Sets the size of this `Inode`, in bytes.
    pub(crate) fn set_size(&mut self, new_size: InodeSize) {
        self.i_size_lo = new_size.into();
        self.i_size_hi = new_size.into();
    }

    /// Returns the size of this inode, in bytes.
    pub(crate) fn size(&self) -> InodeSize {
        self.i_size_lo + self.i_size_hi
    }

    /// Returns the `Inode` generation number.
    pub(crate) fn generation(&self) -> InodeGeneration {
        self.i_generation
    }

    /// Checks if this `Inode` contains a subset of permissions in its file mode field, or if its
    /// file type matches a specific one.
    pub(crate) fn mode_contains(&self, mode: InodeFileMode) -> bool {
        let file_mode = self.i_mode;
        file_mode.contains(mode)
    }

    /// Checks if one or more `Inode` flags are set.
    pub(crate) fn has_flag(&self, flag: InodeFlags) -> bool {
        self.i_flags & flag != cast(0_u32)
    }

    /// Returns the Group ID to which this `Inode` belongs.
    pub(crate) fn gid(&self) -> InodeGroupId {
        self.i_gid + self.i_gid_high
    }

    /// Returns the Owner UID of this `Inode`.
    pub(crate) fn uid(&self) -> InodeOwnerId {
        self.i_uid + self.i_uid_high
    }

    /// Checks if this `Inode` uses an `extent tree`, or a `block map`
    pub(crate) fn uses_extent_tree(&self) -> bool {
        self.has_flag(InodeFlags::EXT4_EXTENTS_FL)
    }

    /// Returns the hard link count.
    ///
    /// The maximum hard link count is usually 65 000, but may be increased if the `DIR_NLINK`
    /// feature is enabled.
    pub(crate) fn links(&self) -> InodeHardLinkCount {
        self.i_links_count
    }

    fn compute_chksum(&self, fs_uuid: Ext4FsUuid, inode_id: InodeNumber) -> InodeChksum {
        let mut chksum_bytes: Vec<u8> = alloc::vec![];
        chksum_bytes.extend_from_slice(bytes_of(&fs_uuid));
        chksum_bytes.extend_from_slice(bytes_of(&inode_id));

        let inode_gen = self.i_generation;
        chksum_bytes.extend_from_slice(bytes_of(&inode_gen));

        let mut inode_no_chksum = *self;
        inode_no_chksum.set_chksum(InodeChksum::ERASE_CHKSUM);

        chksum_bytes.extend_from_slice(
            &bytes_of(&inode_no_chksum)[..usize::from(self.i_extra_isize + 0x80)],
        );

        cast(crc32c_calc(&chksum_bytes))
    }
}
#[allow(clippy::format_in_format_args)]
impl Display for Inode {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "Type : {:<9}    Blocks : {:<16}    Size : {} \n\
        Flags : {:<6}     Generation : {:<8}      Links : {} \n\
Permissions : {}      UID: {}       GID: {} \n\
Access: {} \n\
Modify: {} \n\
Change: {} \n\
Checksum: {}
        ",
            format!("{}", self.inode_type()),
            format!("{}", self.blk_count().0),
            self.size().0,
            format!("{:#x}", self.i_flags.0),
            format!("{:#x}", self.i_generation.0),
            self.links(),
            self.i_mode,
            self.uid(),
            self.gid(),
            DateTime::from(self.access_time()),
            DateTime::from(self.modification_time()),
            DateTime::from(self.change_time()),
            format!("{:#x}", self.chksum().0)
        ))
    }
}
