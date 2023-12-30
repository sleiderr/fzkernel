//! `ext4` file-related structures
//!
//! Provides methods for loading and reading bytes from files, as defined by the `ext4` filesystem.
//! Serves as as interface between the `ext4` definition of a file and the abstract implementation in `FrozenBoot`

use crate::errors::{CanFail, IOError};
use crate::fs::ext4::extent::{Ext4InodeRelBlkId, Ext4InodeRelBlkIdRange, ExtentTree};
use crate::fs::ext4::inode::{
    InodeFileMode, InodeFlags, InodeNumber, InodeSize, LockedInode, LockedInodeStrongRef,
};
use crate::fs::ext4::LockedExt4Fs;
use crate::fs::{FsFile, IOResult, Seek};
use alloc::format;
use bytemuck::{cast, try_cast};
use core::slice;

/// Representation of a file in the `ext4` filesystem.
pub(crate) struct Ext4File {
    fs: LockedExt4Fs,
    inode: LockedInodeStrongRef,
    cursor: usize,
    extent_tree: Option<ExtentTree>,
}

impl core::fmt::Debug for Ext4File {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let inode = self.inode.read();
        f.write_fmt(format_args!(
            "ext4 file | inode = {}    flags = {:#x}    size = {}    mode = {:#x} \n    ",
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

/// Implements a method to read bytes associated to a `ext4` structure (such as a file, or a directory).
///
/// File reads and directory enumeration are based on this method.
#[macro_export]
macro_rules! ext4_fs_read_bytes {
    () => {
        unsafe fn ext4_read_bytes(
            &self,
            offset: usize,
            count: usize,
            buf: &mut [u8],
        ) -> CanFail<IOError> {
            let fs = self.fs.read();
            let sb = fs.superblock.read();

            let blk_offset_from_file_start: Ext4InodeRelBlkId =
                try_cast(u64::try_from(offset).expect("invalid byte offset") / sb.blk_size())
                    .map_err(|_| IOError::Unknown)?;
            let blk_offset_from_first_blk =
                offset % usize::try_from(sb.blk_size()).expect("invalid ext4fs block size");
            let last_blk: Ext4InodeRelBlkId = try_cast(
                u64::try_from(offset + count).expect("invalid byte offset") / sb.blk_size(),
            )
            .map_err(|_| IOError::Unknown)?;
            let last_blk_count =
                count % usize::try_from(sb.blk_size()).expect("invalid ext4fs block size");

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
                    fs.read_blk_from_device(
                        try_cast(curr_extent.start_blk() + i).map_err(|_| IOError::Unknown)?,
                        slice::from_raw_parts_mut(
                            buf.as_mut_ptr().add(
                                (try_cast::<Ext4InodeRelBlkId, u64>(i)
                                    .map_err(|_| IOError::Unknown)?
                                    * sb.blk_size())
                                .try_into()
                                .expect("invalid inode number"),
                            ),
                            sb.blk_size().try_into().expect("invalid fs block size"),
                        ),
                    )?;
                }

                if (curr_extent.block + curr_extent.len) < last_blk {
                    curr_extent = useful_extents.next().unwrap();
                }

                let mut temp_buf = fs.allocate_blk();
                fs.read_blk_from_device(
                    try_cast(curr_extent.start_blk() + last_blk).map_err(|_| IOError::Unknown)?,
                    &mut temp_buf,
                )?;

                if last_blk == cast::<u64, Ext4InodeRelBlkId>(0) {
                    slice::from_raw_parts_mut(
                        buf.as_mut_ptr().add(
                            (try_cast::<Ext4InodeRelBlkId, u64>(last_blk)
                                .map_err(|_| IOError::Unknown)?
                                * sb.blk_size())
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
                                * sb.blk_size())
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
    };
}

impl Ext4File {
    /// Loads a `Ext4File` from disk, from its [`InodeNumber`].
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

    /// Loads a `Ext4File` from disk, from its [`InodeNumber`] and the corresponding [`Ext4Inode`] structure.
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

        if !inode.mode_contains(InodeFileMode::S_IFREG) {
            return Err(IOError::Unknown);
        }

        drop(inode);

        let extent_tree = ExtentTree::load_extent_tree(locked_fs, inode_ptr.clone());

        Ok(Self {
            fs: inode_fs_ptr,
            inode: inode_ptr,
            cursor: 0,
            extent_tree,
        })
    }

    ext4_fs_read_bytes!();
}

impl FsFile for Ext4File {
    fn read(&mut self, buf: &mut [u8]) -> IOResult<usize> {
        let bytes_count = usize::min(buf.len(), self.size()? - self.cursor);

        unsafe { self.ext4_read_bytes(self.cursor, bytes_count, buf)? };
        self.seek(Seek::Forward(bytes_count));

        Ok(bytes_count)
    }

    fn seek(&mut self, pos: Seek) -> usize {
        match pos {
            Seek::Backward(count) => {
                self.cursor = self.cursor.saturating_sub(count);
            }
            Seek::Current => (),
            Seek::Forward(count) => {
                self.cursor = usize::min(self.cursor + count, self.size().unwrap());
            }
        }

        self.cursor
    }

    fn size(&self) -> IOResult<usize> {
        let inode = self.inode.read();
        Ok(usize::try_from(cast::<InodeSize, u64>(inode.size())).expect("invalid file size"))
    }

    fn truncate(&mut self, size: usize) -> IOResult<usize> {
        todo!()
    }

    fn extend(&mut self, size: usize) -> IOResult<usize> {
        todo!()
    }
}
