//! File-system related code.
//!
//! Contains the implementations of various common file systems, such as `ext4`, `fat12`, `fat16`,
//! `fat32` as well as common utilities when working with files or directory.
//!
//! [`File`] and [`Directory`] are the most general representation of a file and a directory
//! respectively. They can be used independently of the file system of the partition we are
//! currently working with.
//!
//! Most of the utilities are designed to work with a `global_allocator`, to store files metadata,
//! but some low level primitives might not need one.

use core::{fmt::Debug, slice};

use alloc::{boxed::Box, string::String, vec::Vec};

use crate::errors::IOError;

pub mod ext4;
pub mod partitions;

pub type IOResult<T> = Result<T, IOError>;

/// Represents the current file-system on a [`Partition`]
#[derive(Clone, Debug)]
pub enum PartFS {
    Ext4(Box<ext4::Ext4FS>),
    Unknown,
}

/// A file-system independent file. This provides a basic set of functionalities when working with
/// files. That should be the only file-related type useful in most situations.
pub type File = Box<dyn FsFile>;

/// A file-system independent directory. This provides a basic set of functionalities when working
/// with directories. That should be the only directory-related type useful in most cases.
pub type Directory = Box<dyn FsDirectory<Item = DirEntry>>;

impl FsFile for Box<dyn FsFile> {
    fn read(&mut self, buf: &mut [u8]) -> IOResult<usize> {
        self.as_mut().read(buf)
    }

    fn seek(&mut self, pos: Seek) -> usize {
        self.as_mut().seek(pos)
    }

    fn size(&self) -> IOResult<usize> {
        self.as_ref().size()
    }

    fn truncate(&mut self, size: usize) -> IOResult<usize> {
        self.as_mut().truncate(size)
    }

    fn extend(&mut self, size: usize) -> IOResult<usize> {
        self.as_mut().extend(size)
    }
}

/// `Seek` provides a way to move the internal cursor of a file, or to retrieve the current
/// position of the cursor using `Seek::Current`.
pub enum Seek {
    /// Moves the cursor backwards of the provided number of bytes.
    Backward(usize),

    /// Does not move the cursor, used to retrieve the current position.
    Current,

    /// Moves the cursor forward of the provided number of bytes.
    Forward(usize),
}

/// `DirEntry` are returned when iterating over a [`Directory`].
///
/// They can either represent a [`File`], or a [`Directory`].
pub enum DirEntry {
    File(File),
    Directory(Directory),
}

/// A trait to represent a file-system independent directory.
///
/// This offers basic functionalities to work with directories.
pub trait FsDirectory: Iterator + Debug {
    /// Returns the directory's parent folder.
    ///
    /// # Errors
    ///
    /// Fails if the directory is the file system's root directory.
    fn parent(&self) -> IOResult<Directory>;

    /// Returns `true` if the directory is the file system's root directory.
    fn is_root_dir(&self) -> IOResult<bool>;

    /// Returns the size of the file, in bytes.
    ///
    /// # Errors
    ///
    /// In case of any I/O error, a generic error will be returned. An error may mean that the file
    /// is corrupted.
    fn size(&self) -> IOResult<usize>;
}

/// A trait to represent a file-system independent file.
///
/// This offers basic functionalities when working with files.
pub trait FsFile: Debug {
    /// Read some bytes from the file, and put them inside the specified buffer.
    /// Starts reading from the current position of the internal cursor.
    ///
    /// Returns how many bytes were read in case of success.
    /// If the return value is 0, it may mean the following:
    ///
    /// - The cursor reached EOF (End of File), and can no longer advance.
    ///
    /// - The buffer length is 0.
    ///
    /// # Errors
    ///
    /// In case of any I/O error, a generic error will be returned. It may be wise to retry reading
    /// in _some_ situations (such as in a real mode context, with reads being based on int 13h).
    fn read(&mut self, buf: &mut [u8]) -> IOResult<usize>;

    /// Changes the position of the file's internal cursor.
    ///
    /// Returns the new offset on the cursor, in bytes.
    ///
    /// The seek is performed using [`Seek`], the cursor can move forward ([`Seek::Forward`]) or
    /// backward ([`Seek::Backward`]).
    ///
    /// The current position of the cursor can be retrieved using [`Seek::Current`] which does not
    /// actually move the cursor, and just returns the current position.
    fn seek(&mut self, pos: Seek) -> usize;

    /// Returns the size of the file, in bytes.
    ///
    /// # Errors
    ///
    /// In case of any I/O error, a generic error will be returned. An error may mean that the file
    /// is corrupted.
    fn size(&self) -> IOResult<usize>;

    /// Truncates the file, changing the size of the underlying file to `size`.
    ///
    /// It may not update the position of the internal cursor, which may lie past the end of the
    /// file after truncating it.
    ///
    /// # Errors
    ///
    /// In case of any I/O error, a generic error will be returned.
    fn truncate(&mut self, size: usize) -> IOResult<usize>;

    /// Extends the file, changing the size of the underlying file to `size`.
    ///
    /// The newly added bytes will be set to 0s.
    ///
    /// # Errors
    ///
    /// In case of any I/O error, a generic error will be returned.
    fn extend(&mut self, size: usize) -> IOResult<usize>;

    /// Reads the whole file, and fill the provided buffer `buf`.
    ///
    /// # Safety
    ///
    /// This does not check that the buffer is large enough to contain the entire file.
    ///
    /// # Errors
    ///
    /// In case of any I/O error, a generic error will be returned. It may be wise to retry reading
    /// in _some_ situations (such as in a real mode context, with reads being based on int 13h).
    unsafe fn read_file_unchecked(&mut self, buf: &mut [u8]) -> IOResult<&[u8]> {
        self.reset_cursor();
        let buf_len = buf.len();
        let size = self.size()?;

        let bytes_read = self.read(core::slice::from_raw_parts_mut(
            (buf.as_ptr() as *mut u8).wrapping_add(buf_len),
            size,
        ))?;

        let extended_slice = core::slice::from_raw_parts(buf.as_ptr(), bytes_read + buf_len);

        Ok(extended_slice)
    }

    /// Reads the whole file, and appends the content to the provided `buf`, as bytes.
    ///
    /// # Errors
    ///
    /// In case of any I/O error, a generic error will be returned. It may be wise to retry reading
    /// in _some_ situations (such as in a real mode context, with reads being based on int 13h).
    #[cfg(feature = "alloc")]
    fn read_file(&mut self, buf: &mut Vec<u8>) -> IOResult<usize> {
        let size = self.size()?;
        let buf_len = buf.len();

        buf.try_reserve(size)
            .map_err(|e| IOError::Exception(Box::new(e)))?;
        unsafe {
            let extended_buf = self.read_file_unchecked(buf)?;
            buf.set_len(extended_buf.len());
        }

        Ok(buf.len() - buf_len)
    }

    /// Reads the whole file, and appends the content to the provided [`String`] in buf.
    ///
    /// # Safety
    ///
    /// The file bytes must all be valid UTF-8
    ///
    /// # Errors
    ///
    /// In case of any I/O error, a generic error will be returned. It may be wise to retry reading
    /// in _some_ situations (such as in a real mode context, with reads being based on int 13h).
    #[cfg(feature = "alloc")]
    unsafe fn read_file_as_string_unchecked(&mut self, buf: &mut String) -> IOResult<usize> {
        self.read_file(buf.as_mut_vec())
    }

    /// Reads the whole file, and appends the content to the provided [`String`] in buf.
    ///
    /// The `read_file_as_string_unchecked` variant does not check that the file is valid UTF-8.
    ///
    /// # Errors
    ///
    /// Returns an error if the file is not valid UTF-8.
    /// In case of any I/O error, a generic error will be returned. It may be wise to retry reading
    /// in _some_ situations (such as in a real mode context, with reads being based on int 13h).
    #[cfg(feature = "alloc")]
    fn read_file_as_string(&mut self, buf: &mut String) -> IOResult<usize> {
        let mut contents: Vec<u8> = Vec::new();

        let bytes_read = self.read_file(&mut contents)?;

        buf.push_str(
            core::str::from_utf8(contents.as_slice())
                .map_err(|e| IOError::Exception(Box::new(e)))?,
        );

        Ok(bytes_read)
    }

    /// Reset the internal cursor's position to the beginning of the file.
    fn reset_cursor(&mut self) {
        let curr_pos = self.seek(Seek::Current);

        self.seek(Seek::Backward(curr_pos));
    }

    /// Seeks to the provided offset, reads some bytes and fills the provided buffer.
    /// Returns how many bytes were read in case of success.
    /// If the return value is 0, it may mean the following:
    ///
    /// - The cursor reached EOF (End of File), and can no longer advance.
    ///
    /// - The buffer length is 0.
    ///
    /// # Errors
    ///
    /// In case of any I/O error, a generic error will be returned. It may be wise to retry reading
    /// in _some_ situations (such as in a real mode context, with reads being based on int 13h).
    fn seek_read(&mut self, buf: &mut [u8], offset: usize) -> IOResult<usize> {
        self.reset_cursor();

        self.seek(Seek::Forward(offset));
        self.read(buf)
    }

    /// Maps a file on to physical memory.
    ///
    /// Loads file's bytes to memory, starting at the address provided in `buf`.
    /// Returns a slice of the corresponding memory.
    ///
    /// # Safety
    ///
    /// This is highly unsafe. The caller must ensure that the memory was allocated prior to the
    /// call, or that the portion of memory that is being used is free and will not be used
    /// anywhere else.
    unsafe fn mmap(&mut self, buf: *mut u8) -> IOResult<&[u8]> {
        self.read_file_unchecked(slice::from_raw_parts_mut(buf, 0))
    }
}
