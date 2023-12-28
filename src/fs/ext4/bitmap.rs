use crate::error;
use crate::fs::ext4::crc32c_calc;
use crate::fs::ext4::extent::Ext4RealBlkId;
use crate::fs::ext4::inode::InodeNumber;
use crate::fs::ext4::sb::Ext4FsUuid;
use alloc::vec::Vec;
use bytemuck::{bytes_of, cast, Pod, Zeroable};
use core::ops::Range;
use vob::Vob;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(super) struct BlockBitmapChksum(u32);

pub(crate) struct BlockBitmap(pub(super) Vob);

impl BlockBitmap {
    /// Compares the checksum of the `BlockBitmap` to its on-disk value.
    ///
    /// The checksum of a `BlockBitmap` can be computed using:
    ///
    /// ```
    /// crc32_calc(fs_uuid + block_bitmap)
    /// ```
    pub(super) fn validate_chksum(
        &self,
        fs_uuid: Ext4FsUuid,
        on_disk_chksum: BlockBitmapChksum,
    ) -> bool {
        let mut chksum_bytes = alloc::vec![0u8; 0];
        chksum_bytes.extend_from_slice(bytes_of(&fs_uuid));

        self.0.get_storage().iter().for_each(|w| {
            let mut bitmap_bytes = w.to_le_bytes();
            bitmap_bytes.iter_mut().for_each(|b| *b = b.reverse_bits());

            chksum_bytes.extend_from_slice(&bitmap_bytes);
        });

        // we have to correct the length to remove additional bytes added by `Vob` using 32-bits aligned
        // storage instead of bytes aligned.
        let real_chksum_bytes_len = core::mem::size_of::<Ext4FsUuid>() + self.0.len() / 8;
        chksum_bytes.truncate(real_chksum_bytes_len);

        let comp_chksum: BlockBitmapChksum = cast(crc32c_calc(&chksum_bytes));

        if comp_chksum != on_disk_chksum {
            error!("ext4", "invalid inode bitmap checksum",);

            return false;
        }

        true
    }

    /// Converts a raw inode bitmap extracted from the filesystem to its in-memory representation, based on a [`Vob`].
    pub(crate) fn from_bytes(bitmap: &[u8]) -> Self {
        BlockBitmap(Vob::from_bytes(bitmap))
    }

    /// Checks if a given block, identified by its [`Ext4RealBlkId`] is marked in-use in this `BlockBitmap`.
    pub(crate) fn blk_in_use(&self, blk: Ext4RealBlkId) -> bool {
        self.0.get(blk.into()).unwrap_or(false)
    }

    /// Marks a given block, identified by its [`Ext4RealBlkId`] as in-use in this `BlockBitmap`.
    pub(crate) fn set_blk_in_use(&mut self, blk: Ext4RealBlkId) -> bool {
        self.0.set(blk.into(), true)
    }

    /// Frees a given block, identified by its [`Ext4RealBlkId`] in this `BlockBitmap`.
    pub(crate) fn free_blk(&mut self, blk: Ext4RealBlkId) -> bool {
        self.0.set(blk.into(), false)
    }

    /// Returns a [`Vec`] of all the available blocks in the given [`Ext4RealBlkId`] range.
    pub(crate) fn available_blks_in_range(
        &self,
        range: Range<Ext4RealBlkId>,
    ) -> Vec<Ext4RealBlkId> {
        let range_usize = usize::from(range.start)..usize::from(range.end);
        self.0
            .iter_unset_bits(range_usize)
            .map(Ext4RealBlkId::from)
            .collect()
    }

    /// Tries to find at most `count` available blocks (marked as free) in this `BlockBitmap`.
    pub(crate) fn get_some_available_blks(&self, count: u32) -> Vec<Ext4RealBlkId> {
        self.0
            .iter_unset_bits(..)
            .take(count.try_into().expect("invalid inode count"))
            .map(Ext4RealBlkId::from)
            .collect()
    }

    /// Marks a range of blocks, identified by their [`Ext4RealBlkId`] as in-use in this `BlockBitmap`.
    pub(crate) fn mark_blk_range_used(&mut self, range: Range<Ext4RealBlkId>) {
        let range_usize = usize::from(range.start)..usize::from(range.end);
        self.0.set_bit_range(range_usize);
    }

    /// Frees a range of blocks, identified by their [`Ext4RealBlkId`] in this `BlockBitmap`.
    pub(crate) fn free_blk_range(&mut self, range: Range<Ext4RealBlkId>) {
        let range_usize = usize::from(range.start)..usize::from(range.end);
        self.0.clear_bit_range(range_usize);
    }

    /// Frees multiple blocks, identified by their [`Ext4RealBlkId`] (supplied as a [`Vec`] in this `BlockBitmap`.
    pub(crate) fn free_some_blks(&mut self, blks: &Vec<Ext4RealBlkId>) {
        for blk in blks {
            self.free_blk(*blk);
        }
    }

    /// Returns the count of blocks marked as free in this `BlockBitmap`.
    pub(crate) fn count_free(&self) -> u32 {
        self.0
            .iter_unset_bits(..)
            .count()
            .try_into()
            .expect("invalid conversion")
    }
}

/// Checksum of the `InodeBitmap` structure.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(super) struct InodeBitmapChksum(u32);

/// The `InodeBitmap` is used by `ext4` to store whether the different [`Inode`] of a block group are in use or not.
///
/// Each bit in the bitmap represents the state of the corresponding `Inode` entry (in-use or free) for this block
/// group.
pub(crate) struct InodeBitmap(pub(super) Vob);

impl InodeBitmap {
    /// Compares the checksum of the `InodeBitmap` to its on-disk value.
    ///
    /// The checksum of a `InodeBitmap` can be computed using:
    ///
    /// ```
    /// crc32_calc(fs_uuid + inode_bitmap)
    /// ```
    pub(super) fn validate_chksum(
        &self,
        fs_uuid: Ext4FsUuid,
        on_disk_chksum: InodeBitmapChksum,
    ) -> bool {
        let mut chksum_bytes = alloc::vec![0u8; 0];
        chksum_bytes.extend_from_slice(bytes_of(&fs_uuid));

        self.0.get_storage().iter().for_each(|w| {
            let mut bitmap_bytes = w.to_le_bytes();
            bitmap_bytes.iter_mut().for_each(|b| *b = b.reverse_bits());

            chksum_bytes.extend_from_slice(&bitmap_bytes);
        });

        // we have to correct the length to remove additional bytes added by `Vob` using 32-bits aligned
        // storage instead of bytes aligned.
        let real_chksum_bytes_len = core::mem::size_of::<Ext4FsUuid>() + self.0.len() / 8;
        chksum_bytes.truncate(real_chksum_bytes_len);

        let comp_chksum: InodeBitmapChksum = cast(crc32c_calc(&chksum_bytes));

        if comp_chksum != on_disk_chksum {
            error!("ext4", "invalid inode bitmap checksum",);

            return false;
        }

        true
    }

    /// Converts a raw inode bitmap extracted from the filesystem to its in-memory representation, based on a [`Vob`].
    pub(crate) fn from_bytes(bitmap: &[u8]) -> Self {
        InodeBitmap(Vob::from_bytes(bitmap))
    }

    /// Checks if a given [`Inode`], identified by its [`InodeNumber`] is marked in-use in this `InodeBitmap`.
    pub(crate) fn inode_in_use(&self, inode: InodeNumber) -> bool {
        self.0.get(inode.into()).unwrap_or(false)
    }

    /// Marks a given [`Inode`], identified by its [`InodeNumber`] as in-use in this `InodeBitmap`.
    pub(crate) fn set_inode_in_use(&mut self, inode: InodeNumber) -> bool {
        self.0.set(inode.into(), true)
    }

    /// Frees a given [`Inode`], identified by its [`InodeNumber`] in this `InodeBitmap`.
    pub(crate) fn free_inode(&mut self, inode: InodeNumber) -> bool {
        self.0.set(inode.into(), false)
    }

    /// Returns a [`Vec`] of all the available [`Inode`] in the given [`InodeNumber`] range.
    pub(crate) fn available_inodes_in_range(&self, range: Range<InodeNumber>) -> Vec<InodeNumber> {
        let range_usize = usize::from(range.start)..usize::from(range.end);
        self.0
            .iter_unset_bits(range_usize)
            .map(InodeNumber::from)
            .collect()
    }

    /// Tries to find at most `count` available [`Inode`] (marked as free) in this `InodeBitmap`.
    pub(crate) fn get_some_available_inodes(&self, count: u32) -> Vec<InodeNumber> {
        self.0
            .iter_unset_bits(..)
            .take(count.try_into().expect("invalid inode count"))
            .map(InodeNumber::from)
            .collect()
    }

    /// Marks a range of [`Inode`], identified by their [`InodeNumber`] as in-use in this `InodeBitmap`.
    pub(crate) fn mark_inode_range_used(&mut self, range: Range<InodeNumber>) {
        let range_usize = usize::from(range.start)..usize::from(range.end);
        self.0.set_bit_range(range_usize);
    }

    /// Frees a range of [`Inode`], identified by their [`InodeNumber`] in this `InodeBitmap`.
    pub(crate) fn free_inode_range(&mut self, range: Range<InodeNumber>) {
        let range_usize = usize::from(range.start)..usize::from(range.end);
        self.0.clear_bit_range(range_usize);
    }

    /// Frees multiple [`Inode`], identified by their [`InodeNumber`] (supplied as a [`Vec`] in this `InodeBitmap`.
    pub(crate) fn free_some_inodes(&mut self, inodes: &Vec<InodeNumber>) {
        for inode in inodes {
            self.free_inode(*inode);
        }
    }

    /// Returns the count of [`Inode`] marked as free in this `InodeBitmap`.
    pub(crate) fn count_free(&self) -> u32 {
        self.0
            .iter_unset_bits(..)
            .count()
            .try_into()
            .expect("invalid conversion")
    }
}
