//! SATA-related utilities

use core::cell::UnsafeCell;
use core::sync::atomic::AtomicBool;

use alloc::vec::Vec;

use crate::drivers::generics::dev_disk::DiskDevice;
use crate::drivers::ide::ata_command::{
    ATA_EXECUTE_DEVICE_DIAGNOSTIC, ATA_IDENTIFY_DEVICE, ATA_READ_DMA, ATA_WRITE_DMA,
};
use crate::drivers::ide::ata_pio::{
    AtaError, AtaErrorCode, AtaIdentify, AtaIoRequest, AtaIoResult,
};
use crate::drivers::ide::AtaDeviceIdentifier;
use crate::{
    drivers::ahci::{
        command::{AHCIPhysicalRegionDescriptor, AHCITransaction},
        fis::RegisterHostDeviceFIS,
        port::HBAPort,
        AHCI_CONTROLLER, SATA_COMMAND_QUEUE,
    },
    errors::{CanFail, IOError},
    fs::partitions::{
        gpt::load_drive_gpt,
        mbr::{load_drive_mbr, PartitionType},
        Partition, PartitionMetadata, PartitionTable,
    },
    wait_for_or,
};

/// `SATADrive` is an interface to a physical drive attached to an [`AHCIController`].
///
/// It offers a convenient way to interact with the device, and other components that want to
/// interact with SATA drives should use this.
///
/// # Examples
///
/// After checking that a drive was available on a given AHCI port (let's use port 0), a
/// `SATADrive` can be derived from that port number.
///
/// ```
/// let drive = SATADrive::build_from_ahci(0);
/// ```
///
/// You can then use that interface to read sectors from the disk.
///
/// ```
/// let mut buffer = [0u8; 1024];
/// drive.read(0, 2, &mut buffer);
/// ```
pub struct AHCIDrive {
    pub id: AtaDeviceIdentifier,
    pub device_info: AtaIdentify,
    ahci_data: AHCIDriveInfo,
    partition_table: UnsafeCell<PartitionTable>,
    partitions: UnsafeCell<Vec<Partition>>,
}

unsafe impl Sync for AHCIDrive {}

#[derive(Debug)]
struct AHCIDriveInfo {
    port: u8,
}

impl DiskDevice for AHCIDrive {
    fn read(&self, start_lba: u64, sectors_count: u16) -> AtaIoRequest {
        let mut io_req = AtaIoRequest::new(AtomicBool::new(true));
        let mut data_buf: Vec<u8> = alloc::vec![];
        data_buf.resize(
            usize::from(sectors_count)
                * usize::try_from(self.logical_sector_size()).expect("invalid sector size"),
            0,
        );
        // TODO: Improve error codes, they need to be more explicit.
        let read_result = match self.read_to_buf(start_lba, sectors_count, &mut data_buf) {
            Ok(_) => crate::drivers::ide::ata_pio::AtaResult::Success,
            Err(e) => crate::drivers::ide::ata_pio::AtaResult::Error(AtaError {
                code: AtaErrorCode::CommandAbort,
                lba: 0,
            }),
        };

        let mut result = AtaIoResult {
            result: read_result,
            command: crate::drivers::ide::ata_command::AtaCommand::AtaReadSectors,
            data: Some(data_buf),
        };
        *io_req.inner.result.lock() = Some(result);

        io_req
    }

    fn write(&self, start_lba: u64, sectors_count: u16, data: Vec<u8>) -> AtaIoRequest {
        let mut io_req = AtaIoRequest::new(AtomicBool::new(true));

        let write_result = match self.write_from_buf(start_lba, sectors_count, &data) {
            Ok(_) => crate::drivers::ide::ata_pio::AtaResult::Success,
            Err(e) => crate::drivers::ide::ata_pio::AtaResult::Error(AtaError {
                code: AtaErrorCode::CommandAbort,
                lba: 0,
            }),
        };

        let mut result = AtaIoResult {
            result: write_result,
            command: crate::drivers::ide::ata_command::AtaCommand::AtaWriteSectors,
            data: None,
        };
        *io_req.inner.result.lock() = Some(result);

        io_req
    }

    fn partitions(&self) -> &Vec<Partition> {
        unsafe { &(*self.partitions.get()) }
    }

    fn identifier(&self) -> AtaDeviceIdentifier {
        self.id
    }

    fn max_sector(&self) -> usize {
        self.device_info.maximum_addressable_lba()
    }

    fn logical_sector_size(&self) -> u64 {
        self.device_info.logical_sector_size().into()
    }
}

impl AHCIDrive {
    pub fn build_from_ahci(port: u8, id: usize) -> Self {
        let ahci_data = AHCIDriveInfo { port };
        let mut drive = Self {
            id: AtaDeviceIdentifier::new(
                crate::drivers::generics::dev_disk::SataDeviceType::AHCI,
                id,
                0,
            ),
            device_info: AtaIdentify::from_bytes([0u16; 256]),
            ahci_data,
            partition_table: UnsafeCell::new(PartitionTable::Unknown),
            partitions: UnsafeCell::new(alloc::vec![]),
        };

        drive.load_identification();

        drive
    }

    /// Loads the partitions contained on this device, whether the partition scheme is _MBR_ or
    /// _GPT_.
    pub fn load_partition_table(&self) {
        let mbr = load_drive_mbr(self, 0);

        if mbr.is_pmbr() {
            let gpt = load_drive_gpt(self);

            unsafe {
                if let Some(gpt) = gpt {
                    *self.partitions.get() = gpt.get_partitions();
                    *self.partition_table.get() = PartitionTable::GPT(gpt);

                    for partition in &mut (*self.partitions.get()) {
                        partition.load_fs().unwrap();
                    }

                    return;
                }
            }
        }

        unsafe {
            *self.partitions.get() = mbr.get_partitions();
        }

        unsafe {
            for partition in (*self.partitions.get()).clone().iter() {
                if let PartitionMetadata::MBR(mut meta) = partition.metadata() {
                    // if this device uses _EPBR_, we traverse the linked list to find all partitions.
                    if matches!(meta.partition_type(), PartitionType::Extended)
                        || matches!(meta.partition_type(), PartitionType::ExtendedLBA)
                    {
                        while load_drive_mbr(self, meta.start_lba() as u64).get_partition_metadata()
                            [1]
                        .is_used()
                            || load_drive_mbr(self, meta.start_lba() as u64)
                                .get_partition_metadata()[0]
                                .is_used()
                        {
                            let partitions = load_drive_mbr(self, meta.start_lba() as u64)
                                .get_partition_metadata();

                            let mut ext_part = partitions[0];

                            ext_part.set_start_lba(ext_part.start_lba() + meta.start_lba());

                            (*self.partitions.get()).push(
                                Partition::from_metadata(
                                    0,
                                    self.id,
                                    PartitionMetadata::MBR(ext_part),
                                )
                                .unwrap(),
                            );
                            meta = partitions[1];
                        }
                    }
                }
            }

            *self.partition_table.get() = PartitionTable::MBR(mbr);
        }
    }

    /// Sends a `IDENTIFY DEVICE` command to the corresponding device.
    ///
    /// The device shall respond with a 512-bytes data block containing various information
    /// concerning itself.
    pub fn load_identification(&mut self) {
        self.device_info = AtaIdentify::from_bytes(
            self.dispach_ata_identify(
                AHCI_CONTROLLER
                    .get()
                    .unwrap()
                    .lock()
                    .read_port_register(self.ahci_data.port),
            ),
        );
    }

    /// Reads `sectors_count` sectors from this drive, starting at `start_lba`, into `buffer`.
    ///
    /// - Length of `buffer` must be larger than `sectors_count * sector_size`.
    ///
    /// - `start_lba` must be less than the `maximum_addressable_lba` for this drive.
    ///
    /// # Examples
    ///
    /// Read 2 sectors from a SATA drive into a buffer.
    ///
    /// ```
    /// let mut buffer = [0u8; 2096];
    /// get_sata_drive(0).lock().read(0, 4, &mut buffer);
    /// ```
    pub fn read_to_buf(
        &self,
        start_lba: u64,
        sectors_count: u16,
        buffer: &mut [u8],
    ) -> CanFail<IOError> {
        (sectors_count as usize * self.device_info.logical_sector_size() as usize <= buffer.len())
            .then_some(())
            .ok_or(IOError::InvalidCommand)?;
        (start_lba as usize + sectors_count as usize <= self.device_info.maximum_addressable_lba())
            .then_some(())
            .ok_or(IOError::InvalidCommand)?;

        let slot = unsafe { self.read_dma(start_lba, sectors_count, buffer.as_mut_ptr()) };

        wait_for_or!(
            !SATA_COMMAND_QUEUE.lock().contains_key(&(slot as u8)),
            10_000,
            return Err(IOError::IOTimeout)
        );

        Ok(())
    }

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
    pub fn write_from_buf(
        &self,
        start_lba: u64,
        sectors_count: u16,
        buffer: &[u8],
    ) -> CanFail<IOError> {
        (start_lba as usize + sectors_count as usize <= self.device_info.maximum_addressable_lba())
            .then_some(())
            .ok_or(IOError::InvalidCommand)?;

        let slot = unsafe { self.write_dma(start_lba, sectors_count, buffer.as_ptr()) };

        wait_for_or!(
            !SATA_COMMAND_QUEUE.lock().contains_key(&(slot as u8)),
            10_000,
            return Err(IOError::IOTimeout)
        );

        Ok(())
    }

    unsafe fn write_dma(&self, start_lba: u64, sectors_count: u16, buffer: *const u8) -> usize {
        let mut write_fis = RegisterHostDeviceFIS::new_empty();
        let sector_size = self.device_info.logical_sector_size();
        write_fis.set_command(ATA_WRITE_DMA);
        write_fis.set_device(1 << 6);
        write_fis.set_lba(start_lba);
        write_fis.set_count(sectors_count);
        write_fis.set_command_update_bit(true);

        let mut ahci_transaction = AHCITransaction::new();
        ahci_transaction.set_byte_size(
            (sectors_count as u32 * self.device_info.logical_sector_size()) as usize,
        );

        let mut prdtl = alloc::vec![];
        let prdt_count = (((sectors_count - 1) >> 4) + 1) as isize;

        for i in 0..prdt_count - 1 {
            let mut prdt = AHCIPhysicalRegionDescriptor::new_empty();

            prdt.set_base_address(buffer.offset(i * 16 * sector_size as isize) as *mut u8);
            prdt.set_data_bytes_count(16 * sector_size);
            prdt.set_interrupt_on_completion(true);

            prdtl.push(prdt);
        }

        let mut last_prdt = AHCIPhysicalRegionDescriptor::new_empty();
        last_prdt.set_base_address(
            buffer.offset((prdt_count - 1) * 16 * sector_size as isize) as *mut u8
        );
        last_prdt.set_data_bytes_count(
            (sectors_count as u32 * sector_size) - ((prdt_count - 1) as u32 * 16 * sector_size),
        );
        prdtl.push(last_prdt);

        ahci_transaction
            .header
            .build_command_table(&write_fis, &[0u8; 0], prdtl);

        let ahci = AHCI_CONTROLLER.get().unwrap().lock();
        let port = ahci.read_port_register(self.ahci_data.port);

        port.dispatch_command(ahci_transaction)
    }

    unsafe fn read_dma(&self, start_lba: u64, sectors_count: u16, buffer: *mut u8) -> usize {
        let mut read_fis = RegisterHostDeviceFIS::new_empty();
        let sector_size = self.device_info.logical_sector_size();
        read_fis.set_command(ATA_READ_DMA);
        read_fis.set_device(1 << 6);
        read_fis.set_lba(start_lba);
        read_fis.set_count(sectors_count);
        read_fis.set_command_update_bit(true);

        let mut ahci_transaction = AHCITransaction::new();
        ahci_transaction.set_byte_size(
            (sectors_count as u32 * self.device_info.logical_sector_size()) as usize,
        );

        let mut prdtl = alloc::vec![];
        let prdt_count = (((sectors_count - 1) >> 4) + 1) as isize;

        for i in 0..prdt_count - 1 {
            let mut prdt = AHCIPhysicalRegionDescriptor::new_empty();

            prdt.set_base_address(buffer.offset(i * 16 * sector_size as isize));
            prdt.set_data_bytes_count(16 * sector_size);
            prdt.set_interrupt_on_completion(true);

            prdtl.push(prdt);
        }

        let mut last_prdt = AHCIPhysicalRegionDescriptor::new_empty();
        last_prdt.set_base_address(buffer.offset((prdt_count - 1) * 16 * sector_size as isize));
        last_prdt.set_data_bytes_count(
            (sectors_count as u32 * sector_size) - ((prdt_count - 1) as u32 * 16 * sector_size),
        );
        prdtl.push(last_prdt);

        ahci_transaction
            .header
            .build_command_table(&read_fis, &[0u8; 0], prdtl);

        let ahci = AHCI_CONTROLLER.get().unwrap().lock();
        let port = ahci.read_port_register(self.ahci_data.port);

        port.dispatch_command(ahci_transaction)
    }

    fn internal_device_diagnostic(&mut self) {
        let mut diag_fis = RegisterHostDeviceFIS::new_empty();
        diag_fis.set_command(ATA_EXECUTE_DEVICE_DIAGNOSTIC);
        diag_fis.set_device(0);
        diag_fis.set_command_update_bit(true);

        let mut ahci_transaction = AHCITransaction::new();
        ahci_transaction
            .header
            .build_command_table(&diag_fis, &[0u8; 0], alloc::vec![]);

        let ahci = AHCI_CONTROLLER.get().unwrap().lock();

        let port = ahci.read_port_register(0);

        port.dispatch_command(ahci_transaction);
    }

    fn dispach_ata_identify(&mut self, port: &mut HBAPort) -> [u16; 256] {
        let mut identify_fis = RegisterHostDeviceFIS::new_empty();
        identify_fis.set_command(ATA_IDENTIFY_DEVICE);
        identify_fis.set_device(0);
        identify_fis.set_command_update_bit(true);

        let mut recv_buffer = [0u16; 256];

        let mut prdt1 = AHCIPhysicalRegionDescriptor::new_empty();
        prdt1.set_base_address(recv_buffer.as_mut_ptr() as *mut u8);
        prdt1.set_data_bytes_count(0x200);

        let mut ahci_transaction = AHCITransaction::new();
        ahci_transaction
            .header
            .build_command_table(&identify_fis, &[0u8; 0], alloc::vec![prdt1]);
        ahci_transaction.set_byte_size(0x200);

        port.dispatch_command(ahci_transaction);

        assert_eq!(
            port.read_received_fis().pio_setup().transfer_count(),
            0x200,
            "Invalid response from SATA device when issuing ATA IDENTIFY command"
        );

        assert_eq!(
            recv_buffer[0] & (1 << 15),
            0,
            "Invalid response from SATA device when issuing ATA IDENTIFY command"
        );

        recv_buffer
    }
}

pub enum SizeFormat {
    Bytes,
    Kilobytes,
    Megabytes,
    Gigabytes,
    Terabytes,
}

#[derive(Debug)]
pub enum ATAMediaRotationRate {
    NotReported,
    NonRotating,
    Rotating(usize),
}
