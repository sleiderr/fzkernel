use crate::drivers::ahci::ahci_devices;
use crate::drivers::ide::ata_pio::{ata_devices, AtaIoRequest};
use crate::drivers::ide::AtaDeviceIdentifier;
use crate::fs::partitions::Partition;
use alloc::sync::Arc;
use alloc::vec::Vec;

pub struct SataDevice {
    identifier: AtaDeviceIdentifier,
    inner: Arc<dyn DiskDevice>,
}

#[derive(Clone, Copy, Debug)]
pub enum SataDeviceType {
    IDE,
    AHCI,
}

pub fn get_sata_drive(id: AtaDeviceIdentifier) -> Option<SataDevice> {
    match id.disk_type {
        SataDeviceType::IDE => Some(SataDevice {
            identifier: id.clone(),
            inner: ata_devices().read().get(&id)?.clone(),
        }),
        SataDeviceType::AHCI => Some(SataDevice {
            identifier: id.clone(),
            inner: ahci_devices().read().get(&id)?.clone(),
        }),
    }
}

impl DiskDevice for SataDevice {
    fn read(&self, start_lba: u64, sectors_count: u16) -> AtaIoRequest {
        self.inner.read(start_lba, sectors_count)
    }

    fn write(&self, start_lba: u64, sectors_count: u16, data: Vec<u8>) -> AtaIoRequest {
        self.inner.write(start_lba, sectors_count, data)
    }

    fn partitions(&self) -> &Vec<Partition> {
        self.inner.partitions()
    }

    fn identifier(&self) -> AtaDeviceIdentifier {
        self.identifier
    }

    fn max_sector(&self) -> usize {
        self.inner.max_sector()
    }

    fn logical_sector_size(&self) -> u64 {
        self.inner.logical_sector_size()
    }
}

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

    /// Writes `sectors_count` sectors from the buffer to the drive, starting at `start_lba`.
    ///
    /// - Length of `buffer` must be larger than `sectors_count * sector_size`.
    ///
    /// - `start_lba` must be less than the `maximum_addressable_lba` for this drive
    ///
    /// # Examples
    ///
    /// Write 2 sectors from a buffer into a `SATA` drive.
    ///
    /// ```
    /// let buffer = [1u8; 2096];
    /// get_sata_drive(0).lock().write(0, 4, &buffer);
    /// ```
    fn write(&self, start_lba: u64, sectors_count: u16, data: Vec<u8>) -> AtaIoRequest;

    fn partitions(&self) -> &Vec<Partition>;

    fn identifier(&self) -> AtaDeviceIdentifier;

    fn max_sector(&self) -> usize;

    fn logical_sector_size(&self) -> u64;
}
