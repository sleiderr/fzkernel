//! ext4 block group related structures.
//!
//! Block groups are a logical grouping of contiguous blocks on disk. Their size is equal to the number of bits in
//! one block (the [`BlockBitmap`] must fit in a single logical block).

use crate::errors::IOError;
use crate::fs::ext4::bitmap::{
    BlockBitmap, BlockBitmapChksumHi, BlockBitmapChksumLo, InodeBitmap, InodeBitmapChksumHi,
    InodeBitmapChksumLo,
};
use crate::fs::ext4::extent::{Ext4RealBlkId, Ext4RealBlkId32};
use crate::fs::ext4::inode::{InodeCount, InodeCount16};
use crate::fs::ext4::sb::{
    Ext4BlkCount, Ext4BlkCount16, Ext4FsUuid, Ext4Superblock, IncompatibleFeatureSet,
};
use crate::fs::ext4::{crc32c_calc, LockedExt4Fs, WeakLockedExt4Fs};
use crate::fs::IOResult;
use crate::time::{current_timestamp, UnixTimestamp};
use crate::{error, ext4_flag_field, ext4_uint_field_range};
use alloc::sync::Arc;
use alloc::vec::Vec;
use bytemuck::{bytes_of, cast, from_bytes, Pod, Zeroable};
use core::cmp::Ordering;
use core::mem;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::AtomicU32;
use hashbrown::HashMap;
use spin::RwLock;

/// A number representing a block group.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct BlockGroupNumber(u32);

impl BlockGroupNumber {
    /// First block group for a filesystem.
    pub(crate) const INITIAL_BLK_GRP: Self = Self(0);
}

impl PartialEq<u64> for BlockGroupNumber {
    fn eq(&self, other: &u64) -> bool {
        u64::from(self.0) == *other
    }
}

impl PartialOrd<u64> for BlockGroupNumber {
    fn partial_cmp(&self, other: &u64) -> Option<Ordering> {
        Some(u64::from(self.0).cmp(other))
    }
}

impl core::ops::Add<u32> for BlockGroupNumber {
    type Output = BlockGroupNumber;

    fn add(self, rhs: u32) -> Self::Output {
        Self(self.0.saturating_add(rhs))
    }
}

impl core::ops::Sub<u32> for BlockGroupNumber {
    type Output = BlockGroupNumber;

    fn sub(self, rhs: u32) -> Self::Output {
        Self(self.0.saturating_sub(rhs))
    }
}

impl core::ops::Mul<u64> for BlockGroupNumber {
    type Output = u64;

    fn mul(self, rhs: u64) -> Self::Output {
        u64::from(self.0) * rhs
    }
}

impl core::ops::Rem<u64> for BlockGroupNumber {
    type Output = u64;

    fn rem(self, rhs: u64) -> Self::Output {
        u64::from(self.0) % rhs
    }
}

ext4_uint_field_range!(
    BlockGroupNumberRange,
    BlockGroupNumber,
    "A range of block groups, between two block group identifiers ([`BlockGroupNumber`])."
);

ext4_flag_field!(GroupDescriptorFlags, u16, "");

impl GroupDescriptorFlags {
    /// Block group flag: Inode table and bitmap are not initialized
    pub(crate) const EXT4_BG_INODE_UNINIT: Self = Self(0x0001);

    /// Block group flag: Block bitmap is not initialized
    pub(crate) const EXT4_BG_BLOCK_UNINIT: Self = Self(0x0002);

    /// Block group flag: Inode table is zeroed
    pub(crate) const EXT4_BG_INODE_ZEROED: Self = Self(0x0004);
}

/// Checksum of the associated `GroupDescriptor` structure.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct GroupDescriptorChksum(u16);

impl GroupDescriptorChksum {
    /// Used to remove the checksum entry from an `Ext4GroupDescriptor` structure.
    pub(crate) const ERASE_CHKSUM: Self = Self(0);
}

#[derive(Clone, Debug)]
pub(crate) struct GroupDescriptor {
    pub(crate) group_number: BlockGroupNumber,
    pub(crate) block_bitmap: Option<BlockBitmap>,
    pub(crate) inode_bitmap: Option<InodeBitmap>,

    descriptor: Ext4GroupDescriptor,
    fs: LockedExt4Fs,
}

impl Deref for GroupDescriptor {
    type Target = Ext4GroupDescriptor;

    fn deref(&self) -> &Self::Target {
        &self.descriptor
    }
}

impl DerefMut for GroupDescriptor {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.descriptor
    }
}

impl GroupDescriptor {
    pub(crate) fn get_or_load_inode_bitmap(&mut self) -> &mut InodeBitmap {
        if self.inode_bitmap.is_some() {
            return self.inode_bitmap.as_mut().unwrap();
        }

        self.load_inode_bitmap();
        self.inode_bitmap.as_mut().unwrap()
    }

    pub(crate) fn get_or_load_blk_bitmap(&mut self) -> &mut BlockBitmap {
        if self.block_bitmap.is_some() {
            return self.block_bitmap.as_mut().unwrap();
        }

        self.load_blk_bitmap();
        self.block_bitmap.as_mut().unwrap()
    }
    /// Compares the checksum of the `GroupDescriptor` to its on-disk value.
    ///
    /// The checksum of an [`Ext4GroupDescriptor`] can be computed (after having set the checksum field to 0) using:
    ///
    /// ```
    /// crc32c_calc(fs_uuid + group_number + group_descriptor)
    /// ```
    pub(crate) fn validate_chksum(&self) -> bool {
        let fs = self.fs.read();
        let sb = fs.superblock.read();
        let comp_chksum = self.compute_chksum(sb.uuid);

        if comp_chksum != self.checksum {
            error!(
                "ext4",
                "invalid block group descriptor checksum (bg {:#X})",
                cast::<BlockGroupNumber, u32>(self.group_number)
            );

            return false;
        }

        true
    }

    /// Loads a `GroupDescriptor` from disk, from its identifier ([`BlockGroupNumber`]).
    pub(super) fn load_descriptor(
        id: BlockGroupNumber,
        locked_fs: &LockedExt4Fs,
    ) -> IOResult<Self> {
        let fs = locked_fs.read();
        let descriptor_fs_ptr = locked_fs.clone();
        let superblock = fs.superblock.read();

        if id >= superblock.bg_count() {
            return Err(IOError::InvalidCommand);
        }

        let descriptor_size = if superblock
            .feature_incompat
            .includes(IncompatibleFeatureSet::EXT4_FEATURE_INCOMPAT_64BIT)
        {
            64
        } else {
            32
        };

        let initial_blk_offset = if superblock.blk_size()
            == u64::try_from(mem::size_of::<Ext4Superblock>()).expect("invalid superblock size")
        {
            2
        } else {
            1
        };

        let descriptor_per_block = superblock.blk_size() / descriptor_size;
        let desc_blk_id = initial_blk_offset + (id * descriptor_size) / superblock.blk_size();
        let desc_idx_in_blk = id % descriptor_per_block;

        let mut desc_blk = fs.allocate_blk();
        fs.read_blk_from_device(Ext4RealBlkId::from(desc_blk_id), &mut desc_blk)?;

        let raw_bg_descriptor = &desc_blk[usize::try_from(desc_idx_in_blk * descriptor_size)
            .expect("invalid group descriptor")
            ..usize::try_from((desc_idx_in_blk + 1) * descriptor_size)
                .expect("invalid group descriptor")];

        let mut filled_descriptor = alloc::vec![0u8; mem::size_of::<Ext4GroupDescriptor>()];
        filled_descriptor[..raw_bg_descriptor.len()].copy_from_slice(raw_bg_descriptor);

        let ext4_descriptor: Ext4GroupDescriptor = *from_bytes(&filled_descriptor);

        let descriptor = Self {
            group_number: id,
            descriptor: ext4_descriptor,
            block_bitmap: None,
            inode_bitmap: None,
            fs: descriptor_fs_ptr,
        };

        Ok(descriptor)
    }

    /// Loads the [`BlockBitmap`] associated to this block group.
    ///
    /// It verifies its checksum, and initializes it if need be during the process.
    pub(crate) fn load_blk_bitmap(&mut self) {
        let fs = self.fs.read();
        let sb = fs.superblock.read();
        let mut blk_bitmap_buf = fs.allocate_blk();

        fs.read_blk_from_device(self.block_bitmap_blk_addr(), &mut blk_bitmap_buf)
            .unwrap();
        let bitmap = BlockBitmap::from_bytes(
            &blk_bitmap_buf
                [..usize::try_from(sb.blocks_per_group / 8).expect("invalid block bitmap size")],
        );
        let chksum = self.block_bitmap_csum_lo + self.block_bitmap_csum_hi;
        bitmap.validate_chksum(sb.uuid, cast(chksum));

        self.block_bitmap = Some(bitmap);
    }

    /// Returns the [`InodeBitmap`] associated to this block group.
    ///
    /// It verifies its checksum, and initializes it if need be during the process.
    pub(crate) fn load_inode_bitmap(&mut self) {
        let fs = self.fs.read();
        let sb = fs.superblock.read();
        let mut inode_bitmap_buf = fs.allocate_blk();

        fs.read_blk_from_device(self.inode_bitmap_blk_addr(), &mut inode_bitmap_buf)
            .unwrap();
        let bitmap = InodeBitmap::from_bytes(
            &inode_bitmap_buf
                [..usize::try_from(sb.inodes_per_group / 8).expect("invalid inode bitmap size")],
        );
        let chksum = self.inode_bitmap_csum_lo + self.inode_bitmap_csum_hi;
        bitmap.validate_chksum(sb.uuid, cast(chksum));

        self.inode_bitmap = Some(bitmap);
    }

    fn compute_chksum(&self, fs_uuid: Ext4FsUuid) -> GroupDescriptorChksum {
        let mut chksum_bytes: Vec<u8> = alloc::vec![];

        chksum_bytes.extend_from_slice(bytes_of(&fs_uuid));
        chksum_bytes.extend_from_slice(bytes_of(&self.group_number));

        let mut desc_no_chksum: Ext4GroupDescriptor = self.descriptor;
        desc_no_chksum.set_chksum(GroupDescriptorChksum::ERASE_CHKSUM);

        chksum_bytes.extend_from_slice(bytes_of(&desc_no_chksum));

        let comp_chksum: u16 = (crc32c_calc(&chksum_bytes) & 0xFFFF)
            .try_into()
            .expect("invalid group descriptor chksum");

        cast(comp_chksum)
    }
}

/// Each block group on the file system has a `GroupDescriptor` associated with it.
///
/// A `block group` is a logical grouping of contiguous block.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(C)]
pub(crate) struct Ext4GroupDescriptor {
    /// Lower 32-bit of location of block bitmap
    block_bitmap_lo: Ext4RealBlkId32,

    /// Lower 32-bit of location of inode bitmap
    inode_bitmap_lo: Ext4RealBlkId32,

    /// Lower 32-bit of location of inode table
    inode_table_lo: Ext4RealBlkId32,

    /// Lower 16-bit of free block count
    free_blocks_count_lo: Ext4BlkCount16,

    /// Lower 16-bit of free inode count
    free_inodes_count_lo: InodeCount16,

    /// Lower 16-bit of directory count
    used_dirs_count_lo: u16,

    /// Block group flags
    flags: GroupDescriptorFlags,

    /// Lower 32-bit of location of snapshot exclusion bitmap
    exclude_bitmap_lo: Ext4RealBlkId32,

    /// Lower 16-bit of the block bitmap checksum
    block_bitmap_csum_lo: BlockBitmapChksumLo,

    /// Lower 16-bit of the inode bitmap checksum
    inode_bitmap_csum_lo: InodeBitmapChksumLo,

    /// Lower 16-bit of unused inode count
    itable_unused_lo: InodeCount16,

    /// Group descriptor checksum
    checksum: GroupDescriptorChksum,

    /// High 32-bits of block bitmap
    block_bitmap_hi: Ext4RealBlkId32,

    /// High 32-bits of inode bitmap
    inode_bitmap_hi: Ext4RealBlkId32,

    /// High 32-bits of inode table
    inode_table_hi: Ext4RealBlkId32,

    /// High 16-bits of free blocks count
    free_blocks_count_hi: Ext4BlkCount16,

    /// High 16-bits of free inodes count
    free_inodes_count_hi: InodeCount16,

    /// High 16-bits of directory used count
    used_dirs_count_hi: u16,

    /// High 16-bits of unused inode count
    itable_unused_hi: InodeCount16,

    /// High 32-bits of location of snapshot exclusion bitmap
    exclude_bitmap_hi: Ext4RealBlkId32,

    /// High 16-bits of the block bitmap checksum
    block_bitmap_csum_hi: BlockBitmapChksumHi,

    /// High 16-bits of the inode bitmap checksum
    inode_bitmap_csum_hi: InodeBitmapChksumHi,
    reserved: u32,
}

impl Ext4GroupDescriptor {
    pub(crate) fn set_chksum(&mut self, chksum: GroupDescriptorChksum) {
        self.checksum = chksum;
    }

    /// Returns the logical block address of the [`BlockBitmap`] associated to this block group.
    pub(crate) fn block_bitmap_blk_addr(&self) -> Ext4RealBlkId {
        self.block_bitmap_lo.add_high_bits(self.block_bitmap_hi)
    }

    /// Returns the logical block address of the [`InodeBitmap`] associated to this block group.
    pub(crate) fn inode_bitmap_blk_addr(&self) -> Ext4RealBlkId {
        self.inode_bitmap_lo.add_high_bits(self.inode_bitmap_hi)
    }

    /// Returns the logical block address of the [`Inode`] table associated to this block group.
    pub(crate) fn inode_table_blk_addr(&self) -> Ext4RealBlkId {
        self.inode_table_lo.add_high_bits(self.inode_table_hi)
    }

    /// Returns the logical block address of the snapshot exclusion bitmap associated to this block group.
    pub(crate) fn snapshot_excl_bitmap_blk_addr(&self) -> Ext4RealBlkId {
        self.exclude_bitmap_lo.add_high_bits(self.exclude_bitmap_hi)
    }

    /// Returns the count of free blocks in this block group.
    pub(crate) fn free_blk_count(&self) -> Ext4BlkCount {
        self.free_blocks_count_lo
            .add_high_bits(self.free_blocks_count_hi)
            .into()
    }

    /// Returns the count of free [`Inode`] in this block group.
    pub(crate) fn free_inode_count(&self) -> InodeCount {
        self.free_inodes_count_lo
            .add_high_bits(self.free_inodes_count_hi)
    }

    /// Returns the count of [`Ext4Directory`] that belongs to this block group.
    pub(crate) fn directory_count(&self) -> u32 {
        u32::from(self.used_dirs_count_lo) | (u32::from(self.used_dirs_count_hi) << 16)
    }

    /// Returns the number of unused [`Inode`] entries in the inode table for this block group.
    pub(crate) fn unused_inodes_count(&self) -> InodeCount {
        self.itable_unused_lo.add_high_bits(self.itable_unused_hi)
    }
}

pub(super) type LockedGroupDescriptor = Arc<RwLock<GroupDescriptor>>;

#[derive(Debug)]
pub(super) struct GroupDescriptorCacheEntry {
    /// Strong pointer to the [`GroupDescriptor`] corresponding to the entry.
    pub(super) group_descriptor: LockedGroupDescriptor,

    /// Number of times this cache entry was accessed.
    pub(super) usage_count: AtomicU32,

    /// First time this entry was accessed.
    pub(super) first_access: UnixTimestamp,
}

#[derive(Debug)]
pub(super) struct GroupDescriptorCache {
    pub(super) descriptor_table: HashMap<BlockGroupNumber, GroupDescriptorCacheEntry>,

    pub(super) fs: WeakLockedExt4Fs,
}

impl GroupDescriptorCache {
    pub(super) fn load_cached_group_descriptor_or_insert(
        &mut self,
        bg_number: BlockGroupNumber,
    ) -> Option<LockedGroupDescriptor> {
        if let Some(bg_desc_cache_entry) = self.descriptor_table.get_mut(&bg_number) {
            bg_desc_cache_entry
                .usage_count
                .fetch_add(1, core::sync::atomic::Ordering::Relaxed);
            return Some(bg_desc_cache_entry.group_descriptor.clone());
        }

        let bg_desc = self.load_group_descriptor_from_raw(bg_number).ok()?;
        let bg_desc_cache_entry = GroupDescriptorCacheEntry {
            group_descriptor: bg_desc.clone(),
            usage_count: AtomicU32::default(),
            first_access: current_timestamp(),
        };

        self.descriptor_table.insert(bg_number, bg_desc_cache_entry);

        Some(bg_desc)
    }

    /// Flushes the entire cache (removes every entry), without deallocating the underlying physical memory.
    pub(super) fn flush_cache(&mut self) {
        self.descriptor_table.clear();
    }

    /// Flushes the entire cache (removes every entry), and deallocates the underlying physical memory.
    pub(super) fn flush_cache_and_deallocate(&mut self) {
        self.descriptor_table.clear();
        self.descriptor_table.shrink_to(0);
    }

    fn load_group_descriptor_from_raw(
        &self,
        bg_number: BlockGroupNumber,
    ) -> IOResult<LockedGroupDescriptor> {
        let bg_desc = GroupDescriptor::load_descriptor(
            bg_number,
            &self.fs.upgrade().ok_or(IOError::Unknown)?.clone(),
        )?;

        Ok(Arc::new(RwLock::new(bg_desc)))
    }
}

impl Deref for GroupDescriptorCache {
    type Target = HashMap<BlockGroupNumber, GroupDescriptorCacheEntry>;

    fn deref(&self) -> &Self::Target {
        &self.descriptor_table
    }
}

impl DerefMut for GroupDescriptorCache {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.descriptor_table
    }
}
