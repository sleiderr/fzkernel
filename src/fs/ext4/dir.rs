//! `ext4` directory-related structures
//!
//! Provides methods for loading and parsing directories, as defined by the `ext4` filesystem.
//! Serves as as interface between the `ext4` definition of a directory and the abstract implementation in `FrozenBoot`

use core::slice;

use alloc::boxed::Box;
use alloc::{format, string::String, vec::Vec};
use bytemuck::{cast, from_bytes, try_cast, Pod, Zeroable};

use crate::fs::ext4::file::Ext4File;
use crate::fs::ext4::inode::{InodeFlags, InodeType, LockedInode, LockedInodeStrongRef};
use crate::fs::ext4::LockedExt4Fs;
use crate::fs::{DirEntry, Directory, FsDirectory};
use crate::{
    errors::{CanFail, IOError},
    ext4_fs_read_bytes,
    fs::{
        ext4::{
            extent::{Ext4InodeRelBlkId, Ext4InodeRelBlkIdRange},
            inode::{InodeFileMode, InodeNumber, InodeSize},
            ExtentTree,
        },
        IOResult,
    },
};

/// Representation of a directory entry in the `ext4` filesystem.
#[derive(Clone)]
pub(crate) struct Ext4DirectoryEntry {
    fs: LockedExt4Fs,
    rec_len: u16,
    name_len: u8,

    /// File type associated to this entry (regular, directory, socket, ...)
    pub(crate) file_type: Option<Ext4DirectoryFileType>,

    /// Name associated to this entry
    pub(crate) name: Ext4Filename,

    pub(crate) inode_number: InodeNumber,
}

impl Ext4DirectoryEntry {
    /// Maximum ext4 directory entry size in bytes
    pub(crate) const MAX_ENTRY_SIZE: usize = 263;

    /// Consumes this `Ext4DirectoryEntry` into a [`Ext4Directory`].
    ///
    /// The file type associated with the entry must be [`Ext4DirectoryFileType::DIRECTORY`].
    #[must_use]
    pub(crate) fn as_directory(&self) -> Option<Ext4Directory> {
        if let Some(file_type) = self.file_type {
            if file_type == Ext4DirectoryFileType::DIRECTORY {
                let fs = self.fs.read();
                let inode = fs.get_inode(self.inode_number)?;
                drop(fs);
                return Ext4Directory::from_inode(self.fs.clone(), &inode).ok();
            }
        }

        None
    }

    /// Consumes this `Ext4DirectoryEntry` into a [`Ext4File`].
    ///
    /// The file type associated with the entry must be [`Ext4DirectoryFileType::REGULAR`].
    #[must_use]
    pub(crate) fn as_file(&self) -> Option<Ext4File> {
        if let Some(file_type) = self.file_type {
            if file_type == Ext4DirectoryFileType::REGULAR {
                let fs = self.fs.read();
                let inode = fs.get_inode(self.inode_number)?;
                drop(fs);
                return Ext4File::from_inode(self.fs.clone(), &inode).ok();
            }
        }

        None
    }
}

/// File type code for a directory entry
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(C)]
pub(crate) struct Ext4DirectoryFileType(u8);

impl Ext4DirectoryFileType {
    pub(crate) const UNKNOWN: Self = Self(0);

    pub(crate) const REGULAR: Self = Self(0x1);

    pub(crate) const DIRECTORY: Self = Self(0x2);

    pub(crate) const CHAR_DEVICE: Self = Self(0x3);

    pub(crate) const BLOCK_DEVICE: Self = Self(0x4);

    pub(crate) const FIFO: Self = Self(0x5);

    pub(crate) const SOCKET: Self = Self(0x6);

    pub(crate) const SYMLINK: Self = Self(0x7);
}

impl TryInto<DirEntry> for Ext4DirectoryEntry {
    type Error = IOError;

    fn try_into(self) -> Result<DirEntry, Self::Error> {
        if let Some(file_type) = self.file_type {
            if file_type == Ext4DirectoryFileType::REGULAR {
                Ok(DirEntry::File(Box::new(self.as_file().unwrap())))
            } else if file_type == Ext4DirectoryFileType::DIRECTORY {
                Ok(DirEntry::Directory(Box::new(self.as_directory().unwrap())))
            } else {
                return Err(IOError::Unknown);
            }
        } else {
            // The directory entries do not contain a `file_type` field, so we must load the inode to check the type
            let fs = self.fs.read();
            let inode_ref = fs
                .get_inode(self.inode_number)
                .ok_or(IOError::Unknown)?
                .upgrade()
                .ok_or(IOError::Unknown)?;
            let inode = inode_ref.read();

            match inode.inode_type() {
                InodeType::Regular => {
                    drop(inode);
                    drop(fs);
                    Ok(DirEntry::File(Box::new(self.as_file().unwrap())))
                }
                InodeType::Directory => {
                    drop(inode);
                    drop(fs);
                    Ok(DirEntry::Directory(Box::new(self.as_directory().unwrap())))
                }
                _ => Err(IOError::Unknown),
            }
        }
    }
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
#[derive(Clone)]
pub(crate) struct Ext4Directory {
    inode: LockedInodeStrongRef,
    fs: LockedExt4Fs,
    internal_cursor: usize,
    extent_tree: Option<ExtentTree>,
}

impl core::fmt::Debug for Ext4Directory {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let inode = self.inode.read();
        f.write_fmt(format_args!(
            "ext4 directory | inode = {}    flags = {:#x}    size = {}    mode = {:#x} \n    ",
            inode.number,
            cast::<InodeFlags, u32>(inode.i_flags),
            cast::<InodeSize, u64>(inode.size()),
            cast::<InodeFileMode, u16>(inode.i_mode)
        ))?;

        if let Some(extent_tree) = &self.extent_tree {
            f.write_str(&format!("Extents: \n{extent_tree:?}"))?;
        }

        Ok(())
    }
}

impl Iterator for Ext4Directory {
    type Item = Ext4DirectoryEntry;

    fn next(&mut self) -> Option<Self::Item> {
        let inode = self.inode.read();
        let mut raw_entry = [0u8; Ext4DirectoryEntry::MAX_ENTRY_SIZE];
        let count_to_read = u64::min(
            cast(inode.size() - self.internal_cursor.try_into().ok()?),
            u64::try_from(Ext4DirectoryEntry::MAX_ENTRY_SIZE)
                .expect("invalid directory entry size"),
        )
        .try_into()
        .ok()?;

        if count_to_read <= 8 {
            self.internal_cursor = 0;
            return None;
        }

        unsafe {
            self.ext4_read_bytes(self.internal_cursor, count_to_read, &mut raw_entry)
                .ok()?;
        }

        let inode_number: InodeNumber = *from_bytes(&raw_entry[..4]);
        let rec_len = u16::from_le_bytes(raw_entry[4..6].try_into().ok()?);
        let name_len = raw_entry[6];
        let file_type: Option<Ext4DirectoryFileType> = Some(*from_bytes(&[raw_entry[7]]));
        let raw_name: Vec<u8> = raw_entry[8..8 + usize::from(name_len)].to_vec();

        if inode_number == InodeNumber::UNUSED_DIR_ENTRY {
            self.internal_cursor = 0;
            return None;
        }

        let name = Ext4Filename(raw_name);

        self.internal_cursor = usize::min(
            self.internal_cursor + usize::from(rec_len),
            usize::try_from(cast::<InodeSize, u64>(inode.size())).expect("invalid inode size"),
        );

        Some(Ext4DirectoryEntry {
            fs: self.fs.clone(),
            rec_len,
            name_len,
            file_type,
            name,
            inode_number,
        })
    }
}

impl FsDirectory for Ext4Directory {
    fn parent(&mut self) -> Option<Directory> {
        Some(Box::new(self.search("..".into())?.as_directory()?))
    }

    fn is_root_dir(&self) -> IOResult<bool> {
        let inode = self.inode.read();
        Ok(inode.number == InodeNumber::ROOT_DIR)
    }

    fn size(&self) -> IOResult<usize> {
        let inode = self.inode.read();
        Ok(usize::try_from(cast::<InodeSize, u64>(inode.size())).expect("invalid file size"))
    }
}

impl Ext4Directory {
    /// Search this directory for a given [`Ext4Filename`].
    ///
    /// Returns the corresponding entry if available.
    #[allow(clippy::needless_pass_by_value)]
    pub(crate) fn search(&mut self, name: Ext4Filename) -> Option<Ext4DirectoryEntry> {
        self.find(|entry| entry.name == name)
    }

    /// Loads a `Ext4Directory` from disk, from its [`InodeNumber`].
    ///
    /// # Errors
    ///
    /// May return any variant of [`IOError`] in case of a failure while attempting to read from disk, or because of
    /// an invalid disk descriptor.
    pub(crate) fn from_inode_id(locked_fs: LockedExt4Fs, inode_id: InodeNumber) -> IOResult<Self> {
        let fs = locked_fs.read();
        let inode = fs.get_inode(inode_id).ok_or(IOError::Unknown)?;
        drop(fs);

        Self::from_inode(locked_fs, &inode)
    }

    /// Loads a `Ext4Directory` from disk, from its [`InodeNumber`].
    ///
    /// # Errors
    ///
    /// May return any variant of [`IOError`] in case of a failure while attempting to read from disk, or because of
    /// an invalid disk descriptor.
    pub(crate) fn from_inode(
        locked_fs: LockedExt4Fs,
        locked_inode: &LockedInode,
    ) -> IOResult<Self> {
        let inode_fs_ptr = locked_fs.clone();
        let inode_ptr = locked_inode.upgrade().ok_or(IOError::Unknown)?;
        let inode = inode_ptr.read();

        if !inode.mode_contains(InodeFileMode::S_IFDIR) {
            return Err(IOError::Unknown);
        }

        let extent_tree = ExtentTree::load_extent_tree(locked_fs, inode_ptr.clone());

        Ok(Self {
            inode: locked_inode.upgrade().ok_or(IOError::Unknown)?,
            fs: inode_fs_ptr,
            internal_cursor: 0,
            extent_tree,
        })
    }

    ext4_fs_read_bytes!();
}
