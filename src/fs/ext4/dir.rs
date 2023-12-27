use core::slice;

use alloc::{string::String, vec::Vec};
use bytemuck::{cast, from_bytes, try_cast, Pod, Zeroable};

use crate::fs::ext4::file::Ext4File;
use crate::{
    drivers::ahci::SATA_DRIVES,
    errors::{CanFail, IOError},
    fs::{
        ext4::{
            extent::{Ext4InodeRelBlkId, Ext4InodeRelBlkIdRange},
            inode::{InodeFileMode, InodeNumber, InodeSize},
            Ext4Fs, ExtentTree, Inode,
        },
        IOResult, PartFS,
    },
};

/// Representation of a directory entry in the `ext4` filesystem.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ext4DirectoryEntry {
    drive_id: usize,
    part_id: usize,
    inode: InodeNumber,
    rec_len: u16,
    name_len: u8,

    /// File type associated to this entry (regular, directory, socket, ...)
    pub file_type: Option<Ext4DirectoryFileType>,

    /// Name associated to this entry
    pub name: Ext4Filename,
}

impl Ext4DirectoryEntry {
    /// Maximum ext4 directory entry size in bytes
    pub const MAX_ENTRY_SIZE: u16 = 263;

    /// Consumes this `Ext4DirectoryEntry` into a [`Ext4Directory`].
    ///
    /// The file type associated with the entry must be [`Ext4DirectoryFileType::DIRECTORY`].
    #[must_use]
    pub fn as_directory(&self) -> Option<Ext4Directory> {
        if let Some(file_type) = self.file_type {
            if file_type == Ext4DirectoryFileType::DIRECTORY {
                return Ext4Directory::from_inode_id(self.drive_id, self.part_id, self.inode).ok();
            }
        }

        None
    }

    /// Consumes this `Ext4DirectoryEntry` into a [`Ext4File`].
    ///
    /// The file type associated with the entry must be [`Ext4DirectoryFileType::REGULAR`].
    #[must_use]
    pub fn as_file(&self) -> Option<Ext4File> {
        if let Some(file_type) = self.file_type {
            if file_type == Ext4DirectoryFileType::REGULAR {
                return Ext4File::from_inode_id(self.drive_id, self.part_id, self.inode).ok();
            }
        }

        None
    }
}

/// File type code for a directory entry
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(C)]
pub struct Ext4DirectoryFileType(u8);

impl Ext4DirectoryFileType {
    pub const UNKNOWN: Self = Self(0);

    pub const REGULAR: Self = Self(0x1);

    pub const DIRECTORY: Self = Self(0x2);

    pub const CHAR_DEVICE: Self = Self(0x3);

    pub const BLOCK_DEVICE: Self = Self(0x4);

    pub const FIFO: Self = Self(0x5);

    pub const SOCKET: Self = Self(0x6);

    pub const SYMLINK: Self = Self(0x7);
}

/// File name associated to a [`Ext4DirectoryEntry`].
///
/// UTF-8 encoded string.
#[derive(Clone, Debug, Default, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub struct Ext4Filename(pub Vec<u8>);

impl Ext4Filename {
    /// Returns an [`Iterator`] over the characters of the file name.
    pub fn chars(&self) -> impl Iterator<Item = char> {
        self.0
            .clone()
            .into_iter()
            .filter(|&b| b != 0)
            .map(char::from)
    }
}

impl From<Ext4Filename> for String {
    fn from(value: Ext4Filename) -> Self {
        value.chars().collect::<String>()
    }
}

impl From<&str> for Ext4Filename {
    fn from(value: &str) -> Self {
        Self(value.chars().map(|ch| u8::try_from(ch).unwrap()).collect())
    }
}

/// Representation of a directory in the `ext4` filesystem.
#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct Ext4Directory {
    drive_id: usize,
    part_id: usize,
    inode: Inode,
    inode_id: InodeNumber,
    internal_cursor: usize,
    extent_tree: Option<ExtentTree>,
}

impl Iterator for Ext4Directory {
    type Item = Ext4DirectoryEntry;

    fn next(&mut self) -> Option<Self::Item> {
        let mut raw_entry = [0u8; Ext4DirectoryEntry::MAX_ENTRY_SIZE as usize];
        let count_to_read = u64::min(
            cast(self.inode.size() - self.internal_cursor.try_into().ok()?),
            u64::from(Ext4DirectoryEntry::MAX_ENTRY_SIZE),
        )
        .try_into()
        .ok()?;

        if count_to_read <= 8 {
            return None;
        }

        unsafe {
            self.__read_bytes(self.internal_cursor, count_to_read, &mut raw_entry)
                .ok()?;
        }

        let inode: InodeNumber = *from_bytes(&raw_entry[..4]);
        let rec_len = u16::from_le_bytes(raw_entry[4..6].try_into().ok()?);
        let name_len = raw_entry[6];
        let file_type: Option<Ext4DirectoryFileType> = Some(*from_bytes(&[raw_entry[7]]));
        let raw_name: Vec<u8> = raw_entry[8..8 + name_len as usize].to_vec();

        if inode == InodeNumber::UNUSED_DIR_ENTRY {
            return None;
        }

        let name = Ext4Filename(raw_name);

        self.internal_cursor = usize::min(
            self.internal_cursor + rec_len as usize,
            cast::<InodeSize, u64>(self.inode.size()) as usize,
        );

        Some(Ext4DirectoryEntry {
            inode,
            rec_len,
            name_len,
            file_type,
            name,
            drive_id: self.drive_id,
            part_id: self.part_id,
        })
    }
}

impl Ext4Directory {
    /// Search this directory for a given [`Ext4Filename`].
    ///
    /// Returns the corresponding entry if available.
    pub fn search(&mut self, name: Ext4Filename) -> Option<Ext4DirectoryEntry> {
        self.find(|entry| entry.name == name)
    }

    /// Loads a `Ext4Directory` from disk, from its [`InodeNumber`].
    ///
    /// # Errors
    ///
    /// May return any variant of [`IOError`] in case of a failure while attempting to read from disk, or because of
    /// an invalid disk descriptor.
    pub fn from_inode_id(drive_id: usize, part_id: usize, inode_id: InodeNumber) -> IOResult<Self> {
        let drive = SATA_DRIVES
            .get()
            .ok_or(IOError::InvalidDevice)?
            .get(drive_id)
            .ok_or(IOError::InvalidDevice)?
            .lock();
        let part = drive
            .partitions
            .get(part_id)
            .ok_or(IOError::InvalidDevice)?;
        let fs = &part.fs.clone();

        if let PartFS::Ext4(fs) = fs {
            drop(drive);
            let inode = fs.__read_inode(inode_id)?;

            return Self::from_inode(drive_id, part_id, inode, inode_id);
        }

        Err(IOError::Unknown)
    }

    /// Loads a `Ext4Directory` from disk, from its [`InodeNumber`] and the corresponding [`Inode`] structure.
    ///
    /// # Errors
    ///
    /// May return any variant of [`IOError`] in case of a failure while attempting to read from disk, or because of
    /// an invalid disk descriptor.
    pub(crate) fn from_inode(
        drive_id: usize,
        part_id: usize,
        inode: Inode,
        inode_id: InodeNumber,
    ) -> IOResult<Self> {
        if !inode.mode_contains(InodeFileMode::S_IFDIR) {
            return Err(IOError::InvalidCommand);
        }

        let drive = SATA_DRIVES
            .get()
            .ok_or(IOError::InvalidDevice)?
            .get(drive_id)
            .ok_or(IOError::InvalidDevice)?
            .lock();
        let part = drive
            .partitions
            .get(part_id)
            .ok_or(IOError::InvalidDevice)?;
        let fs = &part.fs;

        if let PartFS::Ext4(fs) = fs {
            let extent_tree = ExtentTree::load_extent_tree(fs, &inode, inode_id);

            Ok(Self {
                drive_id,
                part_id,
                inode,
                inode_id,
                internal_cursor: 0,
                extent_tree,
            })
        } else {
            Err(IOError::Unknown)
        }
    }

    fn __lock_fs(&self) -> IOResult<alloc::boxed::Box<Ext4Fs>> {
        let drive = SATA_DRIVES
            .get()
            .ok_or(IOError::InvalidDevice)?
            .get(self.drive_id)
            .ok_or(IOError::InvalidDevice)?
            .lock();
        let part = drive
            .partitions
            .get(self.part_id)
            .ok_or(IOError::InvalidDevice)?;
        let fs = &part.fs;

        if let PartFS::Ext4(fs) = fs {
            Ok(fs.clone())
        } else {
            Err(IOError::InvalidCommand)
        }
    }

    unsafe fn __read_bytes(&self, offset: usize, count: usize, buf: &mut [u8]) -> CanFail<IOError> {
        let fs = self.__lock_fs()?;

        let blk_offset_from_file_start: Ext4InodeRelBlkId =
            try_cast(offset as u64 / fs.superblock.blk_size()).map_err(|_| IOError::Unknown)?;
        let blk_offset_from_first_blk = offset % fs.superblock.blk_size() as usize;
        let last_blk: Ext4InodeRelBlkId =
            try_cast((offset + count) as u64 / fs.superblock.blk_size())
                .map_err(|_| IOError::Unknown)?;
        let last_blk_count = count % fs.superblock.blk_size() as usize;

        if let Some(ext_tree) = &self.extent_tree {
            let mut useful_extents = ext_tree.extents.iter().filter(|ext| {
                (cast(ext.block)..ext.block + ext.len).contains(&(blk_offset_from_file_start))
            });
            let mut curr_extent = useful_extents.next().unwrap();

            for i in Ext4InodeRelBlkIdRange(
                blk_offset_from_file_start,
                Ext4InodeRelBlkId::min(cast(0_u64), last_blk - 1),
            ) {
                if (curr_extent.block + curr_extent.len) < i {
                    curr_extent = useful_extents.next().unwrap();
                }
                fs.__read_blk(
                    try_cast(curr_extent.start_blk() + i).map_err(|_| IOError::Unknown)?,
                    slice::from_raw_parts_mut(
                        buf.as_mut_ptr().add(
                            (try_cast::<Ext4InodeRelBlkId, u64>(i)
                                .map_err(|_| IOError::Unknown)?
                                * fs.superblock.blk_size())
                            .try_into()
                            .expect("invalid inode number"),
                        ),
                        fs.superblock
                            .blk_size()
                            .try_into()
                            .expect("invalid fs block size"),
                    ),
                )?;
            }

            if (curr_extent.block + curr_extent.len) < last_blk {
                curr_extent = useful_extents.next().unwrap();
            }

            let mut temp_buf = alloc::vec![0u8; fs.superblock
                .blk_size().try_into().expect("invalid fs block size")];
            fs.__read_blk(
                try_cast(curr_extent.start_blk() + last_blk).map_err(|_| IOError::Unknown)?,
                &mut temp_buf,
            )?;

            if last_blk == cast::<u64, Ext4InodeRelBlkId>(0) {
                slice::from_raw_parts_mut(
                    buf.as_mut_ptr().add(
                        (try_cast::<Ext4InodeRelBlkId, u64>(last_blk)
                            .map_err(|_| IOError::Unknown)?
                            * fs.superblock.blk_size())
                        .try_into()
                        .expect("invalid inode number"),
                    ),
                    last_blk_count - blk_offset_from_first_blk,
                )
                .copy_from_slice(&temp_buf[blk_offset_from_first_blk..last_blk_count]);
            } else {
                slice::from_raw_parts_mut(
                    buf.as_mut_ptr().add(
                        (try_cast::<Ext4InodeRelBlkId, u64>(last_blk)
                            .map_err(|_| IOError::Unknown)?
                            * fs.superblock.blk_size())
                        .try_into()
                        .expect("invalid inode number"),
                    ),
                    last_blk_count,
                )
                .copy_from_slice(&temp_buf[..last_blk_count]);
            }
        }

        Ok(())
    }
}
