//! Standard API to interact with disk devices, regardless of their physical specificities (IDE, AHCI).
//!
//! Disk devices are all assigned a unique identifier ([`AtaDeviceIdentifier`]) based on the physical
//! layer technology used, and a number unique across all devices that share the same technology.
//!
//! The `DiskDevice` trait specifies standard methods to interact with disk devices, however the actual
//! implementation of those method may depend on the physical controller to which the disk is linked.

use crate::drivers::ahci::ahci_devices;
use crate::drivers::ide::ata_pio::{ata_devices, AtaDevice, AtaIoRequest};
use crate::drivers::ide::AtaDeviceIdentifier;
use crate::fs::partitions::Partition;
use alloc::sync::Arc;
use alloc::vec::Vec;

/// Virtual structure that emulates the capacities of a standard physical device.
///
/// Forwards the various requests to an actual physical disk device, depending on the
/// technology used (`IDE`, `AHCI`).
///
/// Implements the [`DiskDevice`] trait, which specifies the standard methods through
/// which one should interact with a disk device.
pub struct SataDevice {
    identifier: AtaDeviceIdentifier,
    inner: Arc<dyn DiskDevice>,
}

/// Available physical devices types.
///
/// A [`SataDevice`] encapsulates one of these physical disk device.
#[derive(Clone, Copy, Debug)]
pub enum SataDeviceType {
    IDE,
    AHCI,
}

/// Returns a [`SataDevice`] structure encapsulating a physical disk device,
/// from its unique identifier ([`AtaDeviceIdentifier`]).
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

/// Returns an iterator over all availables [`SataDevice`] on the computer.
pub fn sata_drives() -> SataDeviceIterator {
    SataDeviceIterator::new()
}

/// Iterator over all [`SataDevice`] available on the computer.
pub struct SataDeviceIterator {
    identifiers: alloc::vec::IntoIter<AtaDeviceIdentifier>,
}

impl SataDeviceIterator {
    fn new() -> Self {
        let mut ata_devices_identifiers: Vec<AtaDeviceIdentifier> =
            ata_devices().read().keys().cloned().collect();

        let mut ahci_device_identifers: Vec<AtaDeviceIdentifier> =
            ahci_devices().read().keys().cloned().collect();

        ata_devices_identifiers.append(&mut ahci_device_identifers);

        Self {
            identifiers: ata_devices_identifiers.into_iter(),
        }
    }
}

impl Iterator for SataDeviceIterator {
    type Item = SataDevice;

    fn next(&mut self) -> Option<Self::Item> {
        get_sata_drive(self.identifiers.next()?)
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

    /// Returns a list of all partitions defined on the device.
    fn partitions(&self) -> &Vec<Partition>;

    /// Returns this device's unique identifier.
    fn identifier(&self) -> AtaDeviceIdentifier;

    /// Returns the maximum sector in user accessible space.
    fn max_sector(&self) -> usize;

    /// Returns the number of bytes per logical sector.
    fn logical_sector_size(&self) -> u64;
}
