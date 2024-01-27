use crate::drivers::ide::ata_pio::AtaIoRequest;
use crate::errors::{CanFail, IOError};
use alloc::vec::Vec;

pub trait DiskDevice {
    /// Reads `sectors_count` sectors from this drive, starting at `start_lba`, into `buffer`.
    ///
    /// - Length of `buffer` must be larger than `sectors_count * sector_size`.
    ///
    /// - `start_lba` must be less than the `maximum_addressable_lba` for this drive.
    ///
    /// # Errors
    ///
    /// # Examples
    ///
    /// Read 2 sectors from a SATA drive into a buffer.
    ///
    /// ```
    /// let mut buffer = [0u8; 2096];
    /// get_sata_drive(0).lock().read(0, 4, &mut buffer);
    /// ```
    fn read(&self, start_lba: u64, sectors_count: u16) -> AtaIoRequest;
}
