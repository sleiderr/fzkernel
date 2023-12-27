use crate::drivers::ahci::SATA_DRIVES;
use crate::errors::{CanFail, IOError};
use crate::fs::ext4::extent::{Ext4InodeRelBlkId, Ext4InodeRelBlkIdRange, ExtentTree};
use crate::fs::ext4::inode::{Inode, InodeFileMode, InodeFlags, InodeNumber, InodeSize};
use crate::fs::ext4::Ext4Fs;
use crate::fs::{FsFile, IOResult, PartFS, Seek};
use bytemuck::{cast, try_cast};
use core::slice;

/// Representation of a file in the `ext4` filesystem.
pub struct Ext4File {
    drive_id: usize,
    part_id: usize,
    inode: Inode,
    inode_id: InodeNumber,
    cursor: usize,
    extent_tree: Option<ExtentTree>,
}

impl core::fmt::Debug for Ext4File {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "ext4 file | inode = {}    flags = {:#x}    size = {}    mode = {:#x} \n    Extents: \n{:?}",
            self.inode_id,
            cast::<InodeFlags, u32>(self.inode.i_flags),
            cast::<InodeSize, u64>(self.inode.size()),
            cast::<InodeFileMode, u16>(self.inode.i_mode),
            self.extent_tree.as_ref().unwrap_or(&ExtentTree::default())        ))
    }
}

impl Ext4File {
    /// Loads a `Ext4File` from disk, from its [`InodeNumber`].
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

    /// Loads a `Ext4File` from disk, from its [`InodeNumber`] and the corresponding [`Inode`] structure.
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
        if !inode.mode_contains(InodeFileMode::S_IFREG) {
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
        let fs = &part.fs.clone();
        drop(drive);

        if let PartFS::Ext4(fs) = fs {
            let extent_tree = ExtentTree::load_extent_tree(fs, &inode, inode_id);
            Ok(Self {
                drive_id,
                part_id,
                inode,
                inode_id,
                cursor: 0,
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
        let last_blk: Ext4InodeRelBlkId =
            try_cast((offset + count) as u64 / fs.superblock.blk_size())
                .map_err(|_| IOError::Unknown)?;
        let last_blk_count = count % fs.superblock.blk_size() as usize;

        if let Some(ext_tree) = &self.extent_tree {
            let mut useful_extents = ext_tree.extents.iter().filter(|ext| {
                ext.block >= last_blk || ext.block + ext.len <= blk_offset_from_file_start
            });

            let mut curr_extent = useful_extents.next().unwrap();

            for i in Ext4InodeRelBlkIdRange(blk_offset_from_file_start, last_blk) {
                if (curr_extent.block + curr_extent.len) < i {
                    curr_extent = useful_extents.next().unwrap();
                }
                fs.__read_blk(
                    try_cast(curr_extent.start_blk() + i).map_err(|_| IOError::Unknown)?,
                    slice::from_raw_parts_mut(
                        buf.as_mut_ptr().offset(
                            (try_cast::<Ext4InodeRelBlkId, u64>(i).map_err(|_| IOError::Unknown)?
                                * fs.superblock.blk_size()) as isize,
                        ),
                        fs.superblock.blk_size() as usize,
                    ),
                )?;
            }

            if (curr_extent.block + curr_extent.len) < last_blk {
                curr_extent = useful_extents.next().unwrap();
            }

            let mut temp_buf = alloc::vec![0u8; fs.superblock.blk_size() as usize];
            fs.__read_blk(
                try_cast(curr_extent.start_blk() + last_blk).map_err(|_| IOError::Unknown)?,
                &mut temp_buf,
            )?;

            slice::from_raw_parts_mut(
                buf.as_mut_ptr().offset(
                    (try_cast::<Ext4InodeRelBlkId, u64>(last_blk).map_err(|_| IOError::Unknown)?
                        * fs.superblock.blk_size()) as isize,
                ),
                last_blk_count,
            )
            .copy_from_slice(&temp_buf[..last_blk_count]);
        }

        Ok(())
    }
}

impl FsFile for Ext4File {
    fn read(&mut self, buf: &mut [u8]) -> super::IOResult<usize> {
        let bytes_count = usize::min(buf.len(), self.size()? - self.cursor);

        unsafe { self.__read_bytes(self.cursor, bytes_count, buf)? };
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

    fn size(&self) -> super::IOResult<usize> {
        Ok(cast::<InodeSize, u64>(self.inode.size()) as usize)
    }

    fn truncate(&mut self, size: usize) -> super::IOResult<usize> {
        todo!()
    }

    fn extend(&mut self, size: usize) -> super::IOResult<usize> {
        todo!()
    }
}
