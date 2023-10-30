use crate::drivers::ahci::{
    ata_command::*,
    command::{AHCICommandHeader, AHCIPhysicalRegionDescriptor},
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

    pub fn load_identification(&mut self) {
        self.device_info = self.dispach_ata_identify(
            AHCI_CONTROLLER
                .get()
                .unwrap()
                .lock()
                .read_port_register(self.ahci_data.port),
        );
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

        let mut ahci_header = AHCICommandHeader::new_empty();
        ahci_header.set_prd_table_length(1);
        ahci_header.build_command_table(&identify_fis, &[0u8; 0], alloc::vec![prdt1]);

        port.send_and_wait_command(ahci_header);

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
