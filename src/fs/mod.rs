use core::{fmt::Debug, slice};

#[cfg(feature = "alloc")]
use alloc::{string::String, vec::Vec};

pub mod partitions;

pub type IOResult<T> = Result<T, ()>;

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
    unsafe fn read_file_unchecked<T: AsRef<[u8]>>(&mut self, buf: &mut T) -> IOResult<&[u8]> {
        self.reset_cursor();
        let slice = buf.as_ref();
        let buf_len = slice.len();
        let size = self.size()?;

        let bytes_read = self.read(core::slice::from_raw_parts_mut(
            (slice.as_ptr() as *mut u8).wrapping_add(buf_len),
            size,
        ))?;

        let extended_slice = core::slice::from_raw_parts(slice.as_ptr(), bytes_read + buf_len);

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

        buf.try_reserve(size).map_err(|_| ())?;
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

        buf.push_str(core::str::from_utf8(contents.as_slice()).map_err(|_| ())?);

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
        let mut slice_buf = slice::from_raw_parts(buf, 0);

        self.read_file_unchecked(&mut slice_buf)
    }
}
