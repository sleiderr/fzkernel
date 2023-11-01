use alloc::{string::String, vec::Vec};

use crate::drivers::ahci::{
    ata_command::*,
    command::{AHCIPhysicalRegionDescriptor, AHCITransaction},
    fis::RegisterHostDeviceFIS,
    port::HBAPort,
    AHCI_CONTROLLER,
};

pub struct SATADrive {
    pub device_info: [u16; 256],
    ahci_data: AHCIDriveInfo,
}

struct AHCIDriveInfo {
    port: u8,
}

impl SATADrive {
    pub fn build_from_ahci(port: u8) -> Self {
        let ahci_data = AHCIDriveInfo { port };
        let mut drive = Self {
            device_info: [0u16; 256],
            ahci_data,
        };

        drive.load_identification();

        drive
    }

    /// Returns the `maximum queue depth` supported by the device.
    ///
    /// The queue depth includes all command for which acceptance has occured but not completion.
    /// Should be 0 if the `NCQ` feature set is not supported.
    pub fn queue_depth(&self) -> u8 {
        (self.device_info[75] & 0b11111) as u8 + 1
    }

    /// Returns the `Minimum Multiword DMA transfer cycle time per word`
    ///
    /// Defines, in nanoseconds, the minimum cycle time that the device supports when performing
    /// Multiword DMA transfer on a per word basis.
    ///
    /// Shall be set to `0x78` ns for `SATA` devices.
    pub fn minimum_multiword_dma_transfer_cycle_time_per_word(&self) -> u16 {
        self.device_info[65]
    }

    /// Returns the maximum number of logical sectors per `DRQ` data block that the device supports
    /// for `READ MULTIPLE`, `WRITE MULTIPLE`, ... commands.
    pub fn maximum_count_logical_sectors_per_drq(&self) -> u8 {
        (self.device_info[47] & 0xff) as u8
    }

    /// Indicates if:
    ///
    /// - the device has more than one logical sector per physical sector
    /// - the `Logical to Physical sector relationship` field is supported
    fn logical_physical_relationship_supported(&self) -> bool {
        self.device_info[106] & (1 << 13) != 0
    }

    /// Returns the size of a physical sector in number of logical sectors.
    fn logical_sectors_per_physical_sector(&self) -> u8 {
        1 << ((self.device_info[106] & (0b1111)) as u8)
    }

    /// Indicates the nominal media rotation rate of the device in rpm, if available.
    pub fn nominal_rotation_rate(&self) -> ATAMediaRotationRate {
        match self.device_info[217] {
            0x0001 => ATAMediaRotationRate::NonRotating,
            speed if (0x0401..0xFFFE).contains(&speed) => {
                ATAMediaRotationRate::Rotating(speed as usize)
            }
            _ => ATAMediaRotationRate::NotReported,
        }
    }

    /// Returns the physical device size, in bytes.
    pub fn device_size(&self, format: SizeFormat) -> u64 {
        let bytes_size = self.maximum_addressable_lba() as u64 * self.logical_sector_size() as u64;
        match format {
            SizeFormat::Bytes => bytes_size,
            SizeFormat::Kilobytes => bytes_size >> 10,
            SizeFormat::Megabytes => bytes_size >> 20,
            SizeFormat::Gigabytes => bytes_size >> 30,
            SizeFormat::Terabytes => bytes_size >> 40,
        }
    }

    /// Returns the number of bytes per logical sector.
    pub fn logical_sector_size(&self) -> u32 {
        // if the logical_sector_size bit is set, the sector size is higher than 512 bytes, and the
        // value is contained is the `Logical sector size` (117..118) field.
        let logical_sector_size_supported = self.device_info[106] & (1 << 12) != 0;

        if logical_sector_size_supported {
            return ((self.device_info[118] as u32) << 16) | (self.device_info[117] as u32);
        }

        0x200
    }

    /// Returns the maximum LBA in user accessible space.
    pub fn maximum_addressable_lba(&self) -> usize {
        let max_lba = ((self.device_info[61] as u32) << 16) | (self.device_info[60] as u32);

        if max_lba == 0x0fff_ffff && (self.device_info[69] & 0b1000) != 0 {
            // use extended number instead
            return (((self.device_info[233] as u64) << 48)
                | ((self.device_info[232] as u64) << 32)
                | ((self.device_info[231] as u64) << 16)
                | (self.device_info[230] as u64)) as usize;
        }

        max_lba as usize
    }

    pub fn media_serial_number(&self) -> String {
        let serial_words = &self.device_info[176..206];
        let mut serial_bytes: Vec<u8> = alloc::vec![];
        for word in serial_words {
            let word_lo = (word & 0xff) as u8;
            let word_hi = ((word >> 8) & 0xff) as u8;
            serial_bytes.push(word_hi);
            serial_bytes.push(word_lo);
        }

        unsafe { String::from_utf8_unchecked(serial_bytes) }
    }

    pub fn model_number(&self) -> String {
        let model_words = &self.device_info[27..47];
        let mut model_bytes: Vec<u8> = alloc::vec![];
        for word in model_words {
            let word_lo = (word & 0xff) as u8;
            let word_hi = ((word >> 8) & 0xff) as u8;
            model_bytes.push(word_hi);
            model_bytes.push(word_lo);
        }

        unsafe { String::from_utf8_unchecked(model_bytes) }
    }

    pub fn serial_number(&self) -> String {
        let serial_words = &self.device_info[10..20];
        let mut serial_bytes: Vec<u8> = alloc::vec![];
        for word in serial_words {
            let word_lo = (word & 0xff) as u8;
            let word_hi = ((word >> 8) & 0xff) as u8;
            serial_bytes.push(word_hi);
            serial_bytes.push(word_lo);
        }

        unsafe { String::from_utf8_unchecked(serial_bytes) }
    }

    pub fn firmware_revision(&self) -> String {
        let fw_words = &self.device_info[23..27];
        let mut fw_bytes: Vec<u8> = alloc::vec![];
        for word in fw_words {
            let word_lo = (word & 0xff) as u8;
            let word_hi = ((word >> 8) & 0xff) as u8;
            fw_bytes.push(word_hi);
            fw_bytes.push(word_lo);
        }

        unsafe { String::from_utf8_unchecked(fw_bytes) }
    }

    pub fn load_identification(&mut self) {
        self.device_info = self.dispach_ata_identify(
            AHCI_CONTROLLER
                .get()
                .unwrap()
                .lock()
                .read_port_register(self.ahci_data.port),
        );
    }

    pub unsafe fn read_dma(&mut self, start_lba: u64, sectors_count: u16, buffer: *mut u8) {
        let mut read_fis = RegisterHostDeviceFIS::new_empty();
        read_fis.set_command(ATA_READ_DMA);
        read_fis.set_device(1 << 6);
        read_fis.set_lba(start_lba);
        read_fis.set_count(sectors_count);
        read_fis.set_command_update_bit(true);

        let mut ahci_transaction = AHCITransaction::new();
        ahci_transaction
            .set_byte_size((sectors_count as u32 * self.logical_sector_size()) as usize);

        let mut prdtl = alloc::vec![];
        let prdt_count = (sectors_count >> 4) + 1;

        for i in 0..prdt_count {
            let mut prdt = AHCIPhysicalRegionDescriptor::new_empty();

            prdt.set_base_address(buffer.offset((i * 16 * 512) as isize));
            prdt.set_data_bytes_count(16 * 512);
            prdt.set_interrupt_on_completion(true);

            prdtl.push(prdt);
        }

        ahci_transaction
            .header
            .build_command_table(&read_fis, &[0u8; 0], prdtl);

        let ahci = AHCI_CONTROLLER.get().unwrap().lock();
        let port = ahci.read_port_register(0);

        port.dispatch_command(ahci_transaction);
    }

    pub fn internal_device_diagnostic(&mut self) {
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
            port.read_received_fis().pio_setup_fis().transfer_count(),
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
