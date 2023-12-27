//! ext4 extent-tree implementation
//!
//! Replaces the formerly used logical block map with indirect pointers.

use core::{cmp::Ordering, mem};

use alloc::vec::Vec;
use bytemuck::{bytes_of, cast, from_bytes, Pod, Zeroable};

use crate::{
    error,
    errors::{CanFail, IOError},
    fs::ext4::{
        crc32c_calc, dir::InodeNumber, inode::InodeGeneration, Ext4Fs, Ext4FsUuid, Inode,
        EXT4_FEATURE_INCOMPAT_EXTENTS,
    },
};

/// Internal ext4 extent tree representation.
#[derive(Default)]
pub(crate) struct ExtentTree {
    pub(crate) extents: Vec<Extent>,
    inode_id: InodeNumber,
    inode_gen: InodeGeneration,
}

impl core::fmt::Debug for ExtentTree {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for extent in &self.extents {
            f.write_fmt(format_args!(
                "({:?}-{:?}) -> {:?} \n",
                extent.ee_block,
                extent.ee_block + extent.ee_len,
                extent.start_blk()
            ))?;
        }

        Ok(())
    }
}

/// An extent block contains the data of the extent tree.
///
/// It begins with a header that contains information about the entries in the block.
/// If the block if a leaf block (its depth is == 0), the header is followed by [`Extent`] entries.
/// Otherwise, it is followed by index nodes ([`ExtentIdx`]).
///
/// Except for the first 4 extents contained in the inode (that do not follow this structure), an
/// extent block is checksummed, and that checksum is contained in the last 4 bytes of the block,
/// that would be left unused anyway.
///
/// The general structure of an extent block is therefore:
///
/// ┌─────────────┬────────────────────┬─────────────────────┬─────────────────────────────┐
/// │Extent header│ Index node /       │         ...         │          Extent tail        │
/// │             │ Extent (leaf node) │                     │    (checksum of the block)  │
/// └─────────────┴────────────────────┴─────────────────────┴─────────────────────────────┘
///
/// Extent blocks are directly loaded from disk when parsing an [`Inode`] extent tree.
///
/// # Checksum
///
/// The checksum of the extent block is :
/// ```
/// crc32c_calc(fs_uuid + inode_id + inode_gen + extent_blk)
/// ```
pub(crate) struct ExtentBlock(pub(crate) Vec<u8>);

impl ExtentBlock {
    /// Compares the checksum of the `ExtentBlock` loaded in memory to its on-disk value.
    pub(crate) fn validate_chksum(
        &self,
        fs_uuid: Ext4FsUuid,
        inode_id: InodeNumber,
        inode_gen: InodeGeneration,
    ) -> bool {
        let on_disk_chksum: ExtentBlockChksum =
            *from_bytes(&self.0[self.0.len() - 4..self.0.len()]);

        let mut chksum_bytes: Vec<u8> = alloc::vec![];
        chksum_bytes.extend_from_slice(bytes_of(&fs_uuid));
        chksum_bytes.extend_from_slice(bytes_of(&inode_id));
        chksum_bytes.extend_from_slice(bytes_of(&inode_gen));
        chksum_bytes.extend_from_slice(&self.0[..self.0.len() - 4]);

        let comp_chksum: ExtentBlockChksum = cast(crc32c_calc(&chksum_bytes));

        if comp_chksum != on_disk_chksum {
            error!(
                "ext4",
                "invalid extent block checksum (inode {:#x})",
                cast::<InodeNumber, u32>(inode_id)
            );

            return false;
        }

        true
    }

    /// Returns the [`ExtentHeader`] for this `ExtentBlock`
    ///
    /// Every block, whether it contains leaf nodes or index nodes, begins with an `ExtentHeader`.
    pub(crate) fn get_header(&self) -> ExtentHeader {
        *from_bytes(&self.0[..mem::size_of::<ExtentHeader>()])
    }

    /// Returns the raw bytes for the entry `entry` of the extent block.
    pub(crate) fn get_entry_bytes(&self, entry: u16) -> Option<ExtentBlkRawEntry> {
        let header = self.get_header();
        let entries = header.eh_entries;

        if cast::<u16, Ext4ExtentHeaderEntriesCount>(entry) >= entries {
            return None;
        }

        Some(ExtentBlkRawEntry(
            &self.0[(mem::size_of::<ExtentHeader>()
                + (usize::try_from(entry).unwrap()) * mem::size_of::<Extent>())
                ..mem::size_of::<ExtentHeader>()
                    + (1 + usize::try_from(entry).unwrap()) * mem::size_of::<Extent>()],
        ))
    }
}

/// Raw bytes for an extent block entry.
///
/// Can be consumed into an [`Extent`] or an [`ExtentIdx`], depending on what type the entry is
/// expected to have.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub(crate) struct ExtentBlkRawEntry<'en>(&'en [u8]);

impl<'en> ExtentBlkRawEntry<'en> {
    /// Consumes this raw extent block entry into an [`Extent`]
    pub(crate) fn as_extent(self) -> Extent {
        *from_bytes(self.0)
    }

    /// Consumed this raw extent block entry into an [`ExtentIdx`]
    fn as_extent_idx(self) -> ExtentIdx {
        *from_bytes(self.0)
    }
}

/// Checksum for an entire extent block.
///
/// Located on-disk in the last four bytes of any extent block, expect for the 4 extents located in
/// the inode which are already checksummed (as the entire inode structure is).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct ExtentBlockChksum(u32);

/// Extent-layer traversal routine.
fn traverse_extent_layer(
    fs: &Ext4Fs,
    ext_data: &ExtentBlock,
    extents: &mut Vec<Extent>,
    inode_id: InodeNumber,
    inode_gen: InodeGeneration,
) -> Option<()> {
    let header = ext_data.get_header();

    // this extent points directly to data blocks
    if header.is_leaf() {
        for entry in 0..cast::<Ext4ExtentHeaderEntriesCount, u16>(header.eh_entries) {
            let extent: Extent = ext_data.get_entry_bytes(entry)?.as_extent();

            extents.push(extent);
        }

        return Some(());
    }

    for entry in 0..cast::<Ext4ExtentHeaderEntriesCount, u16>(header.eh_entries) {
        let extent_idx: ExtentIdx = ext_data.get_entry_bytes(entry)?.as_extent_idx();

        let mut data = alloc::vec![0u8; usize::try_from(fs.superblock.blk_size()).ok()?];

        fs.__read_blk(extent_idx.leaf(), &mut data).ok()?;

        let extent_blk = ExtentBlock(data);
        extent_blk.validate_chksum(*from_bytes(&fs.superblock.s_uuid), inode_id, inode_gen);
        traverse_extent_layer(fs, &extent_blk, extents, inode_id, inode_gen);
    }

    Some(())
}

impl ExtentTree {
    /// Loads an entire extent tree associated with an [`Inode`] to memory.
    pub(crate) fn load_extent_tree(
        fs: &Ext4Fs,
        inode: &Inode,
        inode_id: InodeNumber,
    ) -> Option<Self> {
        if !fs
            .superblock
            .incompat_contains(EXT4_FEATURE_INCOMPAT_EXTENTS)
            | !inode.uses_extent_tree()
        {
            return None;
        };
        let mut extents: Vec<Extent> = alloc::vec![];
        let extent_blk = inode.i_block.as_extent_block();

        traverse_extent_layer(
            fs,
            &extent_blk,
            &mut extents,
            inode_id,
            cast(inode.i_generation),
        );
        extents.sort_unstable();

        Some(Self {
            extents,
            inode_id,
            inode_gen: cast(inode.i_generation),
        })
    }

    /// Returns the physical block address corresponding to a logical block for this [`Inode`].
    pub(crate) fn get_exact_blk_mapping(&self, blk_id: Ext4InodeRelBlkId) -> Option<Ext4RealBlkId> {
        let ext_id = self
            .extents
            .binary_search_by(|ext| {
                if ext.contains(blk_id) {
                    return Ordering::Equal;
                } else if ext.ee_block > blk_id {
                    return Ordering::Greater;
                }

                Ordering::Less
            })
            .ok()?;

        let extent = self.extents.get(ext_id)?;
        let offset_in_extent = blk_id - extent.ee_block;

        Some(extent.start_blk() + offset_in_extent)
    }
}

/// A physical block address (valid for direct reads from the disk).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct Ext4RealBlkId(u64);

impl From<Ext4RealBlkId> for usize {
    fn from(value: Ext4RealBlkId) -> Self {
        value.0.try_into().expect("invalid blk number")
    }
}
impl From<usize> for Ext4RealBlkId {
    fn from(value: usize) -> Self {
        Ext4RealBlkId(value.try_into().expect("invalid blk number"))
    }
}
impl core::ops::Add<Ext4ExtentLength> for Ext4RealBlkId {
    type Output = Ext4RealBlkId;

    fn add(self, rhs: Ext4ExtentLength) -> Self::Output {
        Self(self.0 + u64::from(rhs.0))
    }
}

impl core::ops::Add<u64> for Ext4RealBlkId {
    type Output = Ext4RealBlkId;

    fn add(self, rhs: u64) -> Self::Output {
        Ext4RealBlkId(self.0 + rhs)
    }
}

impl core::ops::Add<Ext4InodeRelBlkId> for Ext4RealBlkId {
    type Output = Ext4RealBlkId;

    fn add(self, rhs: Ext4InodeRelBlkId) -> Self::Output {
        Ext4RealBlkId(self.0 + rhs.0)
    }
}

/// A logical block address, relative to the beginning of this [`Inode`].
///
/// Must be translated to a [`Ext4RealBlkId`] in order to be used to read valid data from the
/// disk.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct Ext4InodeRelBlkId(u64);

impl core::ops::Add<u64> for Ext4InodeRelBlkId {
    type Output = Ext4InodeRelBlkId;

    fn add(self, rhs: u64) -> Self::Output {
        Ext4InodeRelBlkId(self.0 + rhs)
    }
}

impl core::ops::Sub<u64> for Ext4InodeRelBlkId {
    type Output = Self;

    fn sub(self, rhs: u64) -> Self::Output {
        Self(self.0 - rhs)
    }
}

impl core::ops::Sub<Ext4ExtentInitialBlock> for Ext4InodeRelBlkId {
    type Output = Self;

    fn sub(self, rhs: Ext4ExtentInitialBlock) -> Self::Output {
        Self(self.0 - u64::from(rhs.0))
    }
}

impl core::cmp::PartialEq<Ext4ExtentInitialBlock> for Ext4InodeRelBlkId {
    fn eq(&self, other: &Ext4ExtentInitialBlock) -> bool {
        self.0 == u64::from(other.0)
    }
}

impl core::cmp::PartialOrd<Ext4ExtentInitialBlock> for Ext4InodeRelBlkId {
    fn partial_cmp(&self, other: &Ext4ExtentInitialBlock) -> Option<core::cmp::Ordering> {
        Some(self.0.cmp(&u64::from(other.0)))
    }
}

/// A range bounded inclusively below and exclusively above between two logical block addresses
/// relative to an [`Inode`].
pub(crate) struct Ext4InodeRelBlkIdRange(
    pub(crate) Ext4InodeRelBlkId,
    pub(crate) Ext4InodeRelBlkId,
);

impl Iterator for Ext4InodeRelBlkIdRange {
    type Item = Ext4InodeRelBlkId;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0 < self.1 {
            self.0 = self.0 + 1;
            return Some(self.0 - 1);
        }

        None
    }
}

/// Magic number contained in an [`ExtentHeader`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
struct Ext4ExtentHeaderMagic(u16);

impl Ext4ExtentHeaderMagic {
    const VALID_EXT4_MAGIC: Self = Self(0xF30A);
}

/// Depth of the associated extent nodes in the extent tree.
///
/// If `== 0`, this extent points directly to data blocks (leaf nodes). Otherwise, it points to
/// other extent nodes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
struct Ext4ExtentHeaderDepth(u16);

impl Ext4ExtentHeaderDepth {
    pub(crate) const LEAF_DEPTH: Self = Self(0);

    /// Change the depth of the associated extent nodes.
    ///
    /// Must be at most 5.
    pub(crate) fn set_depth(&mut self, new_depth: u16) -> CanFail<IOError> {
        if new_depth > 5 {
            return Err(IOError::InvalidCommand);
        }

        self.0 = new_depth;

        Ok(())
    }
}

/// Generation of the extent tree.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
struct Ext4ExtentHeaderGeneration(u32);

/// Number of valid extent entries following the header.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
struct Ext4ExtentHeaderEntriesCount(u16);

/// Maximum number of valid extent entries following the header.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
struct Ext4ExtentHeaderEntriesMax(u16);

/// Header contained in each node of the `ext4` extent tree.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(C, packed)]
pub(crate) struct ExtentHeader {
    /// Magic number (should be `0xf30a`)
    eh_magic: Ext4ExtentHeaderMagic,

    /// Number of valid entries following the header
    eh_entries: Ext4ExtentHeaderEntriesCount,

    /// Maximum number of entries that could follow the header
    eh_max: Ext4ExtentHeaderEntriesMax,

    /// Depth of this node in the extent tree.
    ///
    /// If `eh_depth == 0`, this extent points to data blocks
    eh_depth: Ext4ExtentHeaderDepth,

    /// Generation of the tree
    eh_generation: Ext4ExtentHeaderGeneration,
}

impl ExtentHeader {
    /// Checks if this header corresponds to leaf nodes.
    pub(crate) fn is_leaf(&self) -> bool {
        let depth = self.eh_depth;
        depth == Ext4ExtentHeaderDepth::LEAF_DEPTH
    }

    /// Loads an `ExtentHeader` from raw bytes, and checks if it corresponds to a valid header.
    pub(crate) unsafe fn load(h_bytes: &[u8]) -> Option<Self> {
        let header: ExtentHeader = *from_bytes(h_bytes);

        let magic = header.eh_magic;
        if magic == Ext4ExtentHeaderMagic::VALID_EXT4_MAGIC {
            Some(header)
        } else {
            None
        }
    }
}

/// Number of blocks covered by a leaf node of the extent tree.
///
/// Covers at most 32768 blocks for an initialized extent, and 32767 blocks for an uninitialized
/// extent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(super) struct Ext4ExtentLength(u16);

impl Ext4ExtentLength {
    /// Checks if this extent is initialized
    pub(crate) fn is_initialized(self) -> bool {
        self.0 <= 32768
    }

    /// Returns the number of blocks covered by the associated extent, whether it is initialized or
    /// not.
    pub(crate) fn length(self) -> u16 {
        if self.is_initialized() {
            self.0
        } else {
            self.0 - 32768
        }
    }
}

/// First file block covered by an extent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(super) struct Ext4ExtentInitialBlock(u32);

impl core::cmp::PartialEq<Ext4InodeRelBlkId> for Ext4ExtentInitialBlock {
    fn eq(&self, other: &Ext4InodeRelBlkId) -> bool {
        u64::from(self.0) == other.0
    }
}

impl core::cmp::PartialOrd<Ext4InodeRelBlkId> for Ext4ExtentInitialBlock {
    fn partial_cmp(&self, other: &Ext4InodeRelBlkId) -> Option<core::cmp::Ordering> {
        Some(u64::from(self.0).cmp(&other.0))
    }
}

impl core::ops::Add<Ext4ExtentLength> for Ext4ExtentInitialBlock {
    type Output = Ext4ExtentInitialBlock;

    fn add(self, rhs: Ext4ExtentLength) -> Self::Output {
        Self(self.0 + u32::from(rhs.0))
    }
}

/// Lower 32-bits of the block number to which the extent points.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(super) struct Ext4ExtentPtrLo(u32);

/// Upper 16-bits of the block number to which the extent points.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(super) struct Ext4ExtentPtrHi(u16);

impl core::ops::Add<Ext4ExtentPtrHi> for Ext4ExtentPtrLo {
    type Output = Ext4RealBlkId;

    fn add(self, rhs: Ext4ExtentPtrHi) -> Self::Output {
        Ext4RealBlkId(u64::from(self.0) | (u64::from(rhs.0) << 32))
    }
}

/// Represents a leaf node of the extent tree.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(C)]
pub(crate) struct Extent {
    /// First file block number that this extent covers
    pub(super) ee_block: Ext4ExtentInitialBlock,

    /// Number of blocks covered by the extent.
    ///
    /// If `ee_len > 32768`, the extnt is uninitialized and the actual extent
    /// length is `ee_len - 32768`.
    pub(super) ee_len: Ext4ExtentLength,

    /// High 16-bits of the block number to which this extent points
    pub(super) ee_start_hi: Ext4ExtentPtrHi,

    /// Low 32-bits of the block number to which this extent points.
    pub(super) ee_start_lo: Ext4ExtentPtrLo,
}

impl Extent {
    pub(crate) fn start_blk(&self) -> Ext4RealBlkId {
        self.ee_start_lo + self.ee_start_hi
    }

    pub(crate) fn contains(&self, blk_id: Ext4InodeRelBlkId) -> bool {
        self.ee_block <= blk_id && self.ee_block + self.ee_len >= blk_id
    }
}

/// Lower 32-bits of the block number of the extent one level lower in the tree.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(super) struct Ext4ExtentLeafPtrLo(u32);

/// Upper 16-bits of the block number of the extent one level lower in the tree.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(super) struct Ext4ExtentLeafPtrHi(u16);

impl core::ops::Add<Ext4ExtentLeafPtrHi> for Ext4ExtentLeafPtrLo {
    type Output = u64;

    fn add(self, rhs: Ext4ExtentLeafPtrHi) -> Self::Output {
        u64::from(self.0) | (u64::from(rhs.0) << 32)
    }
}

/// Represents an internal node of the extent tree (an index node)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(C)]
struct ExtentIdx {
    /// This index node covers file blocks from `block` onward.
    ei_block: Ext4ExtentInitialBlock,

    /// Low 32-bits of the block number of the extent node that is the next level lower in the
    /// tree.
    ei_leaf_lo: Ext4ExtentLeafPtrLo,

    /// High 16-bits of the block number of the extent node that is the next level lower in the
    /// tree.
    ei_leaf_hi: Ext4ExtentLeafPtrHi,

    ei_unused: u16,
}

impl ExtentIdx {
    fn leaf(&self) -> u64 {
        self.ei_leaf_lo + self.ei_leaf_hi
    }
}
