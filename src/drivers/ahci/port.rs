use super::fis::{DMASetupFIS, PIOSetupFIS, RegisterDeviceHostFIS, SetDeviceBitsFIS};
use crate::{drivers::ahci::command::AHCICommandHeader, hba_reg_field};

const HBA_PORT_TIMEOUT: u64 = 5000;

pub const SATA_ATA_SIG: u32 = 0x101;
pub const SATA_ATAPI_SIG: u32 = 0xEB140101;
pub const SATA_SEMB_SIG: u32 = 0xC33C0101;
pub const SATA_PM_SIG: u32 = 0x96690101;

#[repr(packed)]
pub struct HBAPortReceivedFIS {
    pub dma_setup: DMASetupFIS,
    pub padding1: u32,
    pub pio_setup: PIOSetupFIS,
    pub padding2: [u32; 3],
    pub d2h_register: RegisterDeviceHostFIS,
    pub padding3: u32,
    pub set_device_bits: SetDeviceBitsFIS,
    pub unknown: [u8; 64],
}

impl HBAPortReceivedFIS {
    pub fn pio_setup_fis(&self) -> PIOSetupFIS {
        unsafe { core::ptr::read_unaligned(core::ptr::addr_of!(self.pio_setup)) }
    }
}

#[derive(Debug)]
pub struct HBAPort {
    /// Command List Base Address
    pub clb: u32,

    /// Command List Base Address Upper 32-bits
    pub clbu: u32,

    /// FIS Base Address
    pub fb: u32,

    /// FIS Base Address Upper 32-bits
    pub fbu: u32,

    /// Interrupt Status
    pub is: u32,

    /// Interrupt Enable
    pub ie: u32,

    /// Command and Status
    pub cmd: u32,

    pub reserved: u32,

    /// Task File data
    pub tfd: u32,

    /// Signature
    pub sig: u32,

    /// Serial ATA Status
    pub ssts: u32,

    /// Serial ATA Control
    pub sctl: u32,

    /// Serial ATA Error
    pub serr: u32,

    /// Serial ATA Active
    pub sact: u32,

    /// Command Issue
    pub ci: u32,

    /// Serial ATA Notification
    pub sntf: u32,

    /// FIS-based Switching Control
    pub fbs: u32,

    /// Device Sleep
    pub devslp: u32,
}

impl HBAPort {
    pub fn read_received_fis(&self) -> &HBAPortReceivedFIS {
        unsafe { &*(self.port_fis_base_address() as *const HBAPortReceivedFIS) }
    }

    pub fn send_and_wait_command(&mut self, cmd: AHCICommandHeader) {
        let cmd_slot = self.find_command_slot();
        self.update_command_list_entry(cmd_slot, cmd);

        self.port_command_set_issued(cmd_slot as u8);
    }

    fn find_command_slot(&self) -> usize {
        if let Some(slot) = (0..32)
            .map(|i| self.get_command_list_entry(i))
            .position(|s| s.command_fis_length() == 0)
        {
            return slot;
        };

        panic!("AHCI Timeout when trying to obtain a command slot");
    }

    fn command_list(&self) -> &[AHCICommandHeader; 32] {
        unsafe { &mut *(self.port_cmdlist_base_address() as *mut [AHCICommandHeader; 32]) }
    }

    fn command_list_mut(&mut self) -> &mut [AHCICommandHeader; 32] {
        unsafe { &mut *(self.port_cmdlist_base_address() as *mut [AHCICommandHeader; 32]) }
    }

    pub fn update_command_list_entry(&mut self, id: usize, new_entry: AHCICommandHeader) {
        unsafe {
            core::ptr::write_volatile(
                &mut self.command_list_mut()[id] as *mut AHCICommandHeader,
                new_entry,
            )
        }
    }

    pub fn get_command_list_entry(&self, id: usize) -> AHCICommandHeader {
        unsafe { core::ptr::read_volatile(&self.command_list()[id] as *const AHCICommandHeader) }
    }

    pub fn port_cmdlist_base_address(&self) -> *mut u8 {
        let clbu = unsafe { core::ptr::read_volatile(&self.clbu as *const u32) };
        let clb = unsafe { core::ptr::read_volatile(&self.clb as *const u32) };
        (((clbu as u64) << 32) | (clb as u64)) as *mut u8
    }

    pub fn port_fis_base_address(&self) -> *mut u8 {
        let fbu = unsafe { core::ptr::read_volatile(&self.fbu as *const u32) };
        let fb = unsafe { core::ptr::read_volatile(&self.fb as *const u32) };
        (((fbu as u64) << 32) | (fb as u64)) as *mut u8
    }

    pub fn port_icc_read(&self) -> AHCIInterfaceState {
        let cmd = unsafe { core::ptr::read_volatile(&self.cmd as *const u32) };
        (((cmd >> 28) & 0xf) as u8).into()
    }

    pub fn port_icc_write(&mut self, cmd: AHCIInterfaceState) {
        unsafe {
            let cmd_reg = core::ptr::read_volatile(&self.cmd as *const u32);
            core::ptr::write_volatile(
                &mut self.cmd as *mut u32,
                (cmd_reg & !(0xf << 28)) | ((Into::<u8>::into(cmd) as u32) << 28),
            );
        }
    }

    pub fn port_current_command_slot(&self) -> u8 {
        let cmd = unsafe { core::ptr::read_volatile(&self.cmd as *const u32) };
        (((cmd) >> 7) & 0x1f) as u8
    }

    pub fn port_device_sig(&self) -> u32 {
        unsafe { core::ptr::read_volatile(&self.sig as *const u32) }
    }

    pub fn port_interface_state(&self) -> AHCIInterfaceState {
        let ssts = unsafe { core::ptr::read_volatile(&self.ssts as *const u32) };
        (((ssts >> 8) & 0xf) as u8).into()
    }

    pub fn port_interface_speed(&self) -> AHCIInterfaceSpeed {
        let ssts = unsafe { core::ptr::read_volatile(&self.ssts as *const u32) };
        (((ssts >> 4) & 0xf) as u8).into()
    }

    pub fn port_interface_device_detection(&self) -> AHCIDeviceDetection {
        let ssts = unsafe { core::ptr::read_volatile(&self.ssts as *const u32) };
        ((ssts & 0xf) as u8).into()
    }

    pub fn port_tag_status(&self, tag: u8) -> bool {
        let sact = unsafe { core::ptr::read_volatile(&self.sact as *const u32) };
        (sact & (1 << tag)) != 0
    }

    pub fn port_tag_set_outstanding(&mut self, tag: u8) {
        self.sact = (self.sact & !(1 << tag)) | (1 << tag);
    }

    pub fn port_tag_clear_outstanding(&mut self, tag: u8) {
        self.sact &= !(1 << tag);
    }

    pub fn port_command_set_issued(&mut self, tag: u8) {
        unsafe {
            let ci = core::ptr::read_volatile(&self.ci as *const u32);
            core::ptr::write_volatile(&mut self.ci as *mut u32, (ci & !(1 << tag)) | (1 << tag));
        }
    }

    pub fn port_command_clear_issued(&mut self, tag: u8) {
        unsafe {
            let ci = core::ptr::read_volatile(&self.ci as *const u32);
            core::ptr::write_volatile(&mut self.ci as *mut u32, ci & !(1 << tag));
        }
    }

    pub fn port_pm_notification_received(&self, port: u8) -> bool {
        let sntf = unsafe { core::ptr::read_volatile(&self.sntf as *const u32) };
        sntf & (1 << port) != 0
    }

    pub fn port_pm_notification_clear(&mut self, port: u8) {
        unsafe {
            let sntf = core::ptr::read_volatile(&self.sntf as *const u32);
            core::ptr::write_volatile(
                &mut self.sntf as *mut u32,
                (sntf & !(1 << port)) | (1 << port),
            );
        }
    }

    hba_reg_field!(
        PORT_IS_CPDS,
        31,
        "Cold Port Detect Status",
        is,
        port_cold_detect,
        port_clear_cold_detect
    );

    hba_reg_field!(
        PORT_IS_TFES,
        30,
        "Task File Error Status",
        is,
        port_task_file_err,
        port_clear_task_file_err
    );

    hba_reg_field!(
        PORT_IS_HBFS,
        29,
        "Host Bus Fatal Error Status",
        is,
        port_host_bus_fatal,
        port_clear_host_bus_fatal
    );

    hba_reg_field!(
        PORT_IS_HBDS,
        28,
        "Host Bus Data Error Status",
        is,
        port_host_bus_data_error,
        port_clear_host_bus_data_error
    );

    hba_reg_field!(
        PORT_IS_IFS,
        27,
        "Interface Fatal Error Status",
        is,
        port_interface_error,
        port_clear_interface_error
    );

    hba_reg_field!(
        PORT_IS_INFS,
        26,
        "Interface Non-fatal Error Status",
        is,
        port_nonfatal_interface_error,
        port_clear_nonfatal_interfcae_error
    );

    hba_reg_field!(
        PORT_IS_OFS,
        24,
        "Overflow Status",
        is,
        port_overflow,
        port_clear_overflow
    );

    hba_reg_field!(
        PORT_IS_IPMS,
        23,
        "Incorrect Port Multiplier Status",
        is,
        port_incorrect_multiplier,
        port_clear_incorrect_multiplier
    );

    hba_reg_field!(
        PORT_IS_PRCS,
        22,
        "PhyRdy Change Status",
        is,
        port_phyrdy_changed
    );

    hba_reg_field!(
        PORT_IS_DMPS,
        7,
        "Device Mechanical Presence Status",
        is,
        port_mech_presence_switch,
        port_clear_mech_presence_switch
    );

    hba_reg_field!(
        PORT_IS_PCS,
        6,
        "Port Connect Change Status",
        is,
        port_connect_change
    );

    hba_reg_field!(
        PORT_IS_DPS,
        5,
        "Descriptor Processed",
        is,
        port_descriptor_processed,
        port_clear_descriptor_processed
    );

    hba_reg_field!(
        PORT_IS_UFS,
        4,
        "Unknown FIS Interrupt",
        is,
        port_unknown_fis_interrupt
    );

    hba_reg_field!(
        PORT_IS_SDBS,
        3,
        "Set Device Bits Interrupt",
        is,
        port_set_device_bits_recv,
        port_clear_set_device_bits_recv
    );

    hba_reg_field!(
        PORT_IS_DSS,
        2,
        "DMA Setup FIS Interrupt",
        is,
        port_dma_setup_recv,
        port_clear_dma_setup_recv
    );

    hba_reg_field!(
        PORT_IS_PSS,
        1,
        "PIO Setup FIS Interrupt",
        is,
        port_pio_setup_recv,
        port_clear_pio_setup_recv
    );

    hba_reg_field!(
        PORT_IS_DHRS,
        0,
        "Device to Host Register FIS Interrupt",
        is,
        port_d2h_register_recv,
        port_clear_d2h_register_recv
    );

    hba_reg_field!(
        PORT_IE_CPDE,
        31,
        "Cold Presence Detect Enable",
        ie,
        port_cold_presence_detect,
        port_enable_cold_presence_detect
    );

    hba_reg_field!(
        PORT_IE_TFEE,
        30,
        "Task File Error Enable",
        ie,
        port_task_file_error_enabled,
        port_enable_task_file_error
    );

    hba_reg_field!(
        PORT_IE_HBFE,
        29,
        "Host Bus Fatal Error Enable",
        ie,
        port_host_bus_fatal_err_enabled,
        port_enable_host_bus_fatal_err
    );

    hba_reg_field!(
        PORT_IE_HBDE,
        28,
        "Host Bus Data Error Enable",
        ie,
        port_host_bus_data_err_enabled,
        port_enable_host_bus_data_err
    );

    hba_reg_field!(
        PORT_IE_IFE,
        27,
        "Interface Fatal Error Enable",
        ie,
        port_interface_fatal_err_enabled,
        port_enable_interface_fatal_err
    );

    hba_reg_field!(
        PORT_IE_INFE,
        26,
        "Interface Non-fatal Error Enable",
        ie,
        port_interface_nonfatal_err_enabled,
        port_enable_interface_nonfatal_err
    );

    hba_reg_field!(
        PORT_IE_OFE,
        24,
        "Overflow Enable",
        ie,
        port_overflow_enabled,
        port_enable_overflow
    );

    hba_reg_field!(
        PORT_IE_IPME,
        23,
        "Incorrect Port Multiplier Enable",
        ie,
        port_incorrect_multiplier_enabled,
        port_enable_incorrect_multiplier
    );

    hba_reg_field!(
        PORT_IE_PRCE,
        22,
        "PhyRdy Change Interrupt Enable",
        ie,
        port_phyrdy_change_interrupt_enabled,
        port_enable_phyrdy_change_interrupt
    );

    hba_reg_field!(
        PORT_IE_DMPE,
        7,
        "Device Mechanical Presence Enable",
        ie,
        port_device_mechanical_presence_enabled,
        port_enable_device_mechanical_presence
    );

    hba_reg_field!(
        PORT_IE_PCE,
        6,
        "Port Change Interrupt Enable",
        ie,
        port_change_interrupt_enabled,
        port_enable_change_interrupt
    );

    hba_reg_field!(
        PORT_IE_DPE,
        5,
        "Descriptor Processed Interrupt Enable",
        ie,
        port_descriptor_processed_interrupt_enabled,
        port_enable_descriptor_processed_interrupt
    );

    hba_reg_field!(
        PORT_IE_UFE,
        4,
        "Unknown FIS Interrupt Enable",
        ie,
        port_unknown_fis_interrupt_enabled,
        port_enable_unknown_fis_interrupt
    );

    hba_reg_field!(
        PORT_IE_SDBE,
        3,
        "Set Device Bits FIS Interrupt Enable",
        ie,
        port_set_device_bits_interrupt_enabled,
        port_enable_set_device_bits
    );

    hba_reg_field!(
        PORT_IE_DSE,
        2,
        "DMA Setup FIS Interrupt Enable",
        ie,
        port_dma_setup_interrupt_enabled,
        port_enable_dma_setup_interrupt
    );

    hba_reg_field!(
        PORT_IE_PSE,
        1,
        "PIO Setup FIS Interrupt Enable",
        ie,
        port_pio_setup_interrupt_enabled,
        port_enable_pio_setup_interrupt
    );

    hba_reg_field!(
        PORT_IE_DHRE,
        0,
        "Device to Host Register FIS Interrupt Enable",
        ie,
        port_d2h_register_interrupt_enabled,
        port_enable_d2h_register_interrupt
    );

    hba_reg_field!(
        PORT_CMD_ASP,
        27,
        "Aggressive Slumber / Partial",
        cmd,
        port_cmd_asp,
        port_cmd_set_asp
    );

    hba_reg_field!(
        PORT_CMD_ALPE,
        26,
        "Aggressive Link Power Management Enable",
        cmd,
        port_alp_enabled,
        port_enable_alp
    );

    hba_reg_field!(
        PORT_CMD_DLAE,
        25,
        "Drive LED on ATAPI Enable",
        cmd,
        port_led_drive_enabled,
        port_enable_led_drive
    );

    hba_reg_field!(
        PORT_CMD_ATAPI,
        24,
        "Device is ATAPI",
        cmd,
        port_connected_device_is_atapi,
        port_set_connected_is_atapi
    );

    hba_reg_field!(
        PORT_CMD_APSTE,
        23,
        "Automatic Partial to Slumber Transitions Enabled",
        cmd,
        port_auto_partial2slumber,
        port_enble_auto_partial2slumber
    );

    hba_reg_field!(
        PORT_CMD_FBSCP,
        22,
        "FIS-based Switching Capable Port",
        cmd,
        port_fis_based_switching_capable
    );

    hba_reg_field!(
        PORT_CMD_ESP,
        21,
        "External SATA Port",
        cmd,
        port_external_sata
    );

    hba_reg_field!(
        PORT_CMD_CPD,
        20,
        "Cold Presence Detection",
        cmd,
        port_cold_presence_detection_support
    );

    hba_reg_field!(
        PORT_CMD_MPSP,
        19,
        "Mechanical Presence Switch Attached to Port",
        cmd,
        port_mechanical_presence_switch_support
    );

    hba_reg_field!(
        PORT_CMD_HPCP,
        18,
        "Hot Plug Capable Port",
        cmd,
        port_hot_plug_capable
    );

    hba_reg_field!(
        PORT_CMD_PMA,
        17,
        "Port Multiplier Attached",
        cmd,
        port_multiplier_attached,
        port_set_multiplier_attached
    );

    hba_reg_field!(
        PORT_CMD_CPS,
        16,
        "Cold Presence State",
        cmd,
        port_device_cold_presence_detected
    );

    hba_reg_field!(
        PORT_CMD_CR,
        15,
        "Command List Running",
        cmd,
        port_command_list_dma_engine_running
    );

    hba_reg_field!(
        PORT_CMD_FR,
        14,
        "FIS Receive Running",
        cmd,
        port_fis_receive_dma_engine_running
    );

    hba_reg_field!(
        PORT_CMD_MPSS,
        13,
        "Mechanical Presence Switch State",
        cmd,
        port_mechanical_switch_state
    );

    hba_reg_field!(
        PORT_CMD_FRE,
        4,
        "FIS Receive Enable",
        cmd,
        port_fis_receive_enabled,
        port_enable_fis_receive
    );

    hba_reg_field!(
        PORT_CMD_CLO,
        3,
        "Command List Override",
        cmd,
        port_command_list_override,
        port_set_command_list_override
    );

    hba_reg_field!(
        PORT_CMD_POD,
        2,
        "Power On Device",
        cmd,
        port_power_on_device,
        port_set_power_on_device
    );

    hba_reg_field!(
        PORT_CMD_SUD,
        1,
        "Spin-Up Device",
        cmd,
        port_spin_up_device,
        port_set_spin_up_device
    );

    hba_reg_field!(PORT_CMD_ST, 0, "Start", cmd, port_start, port_set_start);
}

#[derive(Debug)]
pub enum AHCIDeviceDetection {
    NoDevice,
    DeviceDetectedNoPhysicalCom,
    DeviceDetectedPhysicalCom,
    PhysicalOffline,
}

impl From<u8> for AHCIDeviceDetection {
    fn from(value: u8) -> Self {
        match value {
            1 => AHCIDeviceDetection::DeviceDetectedNoPhysicalCom,
            3 => AHCIDeviceDetection::DeviceDetectedPhysicalCom,
            4 => AHCIDeviceDetection::PhysicalOffline,
            _ => AHCIDeviceDetection::NoDevice,
        }
    }
}

#[derive(Debug)]
pub enum AHCIInterfaceSpeed {
    NotPresent,
    Gen1,
    Gen2,
    Gen3,
}

impl From<AHCIInterfaceSpeed> for u8 {
    fn from(value: AHCIInterfaceSpeed) -> Self {
        match value {
            AHCIInterfaceSpeed::NotPresent => 0,
            AHCIInterfaceSpeed::Gen1 => 1,
            AHCIInterfaceSpeed::Gen2 => 2,
            AHCIInterfaceSpeed::Gen3 => 3,
        }
    }
}

impl From<u8> for AHCIInterfaceSpeed {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Gen1,
            2 => Self::Gen2,
            3 => Self::Gen3,
            _ => Self::NotPresent,
        }
    }
}

#[derive(Debug)]
pub enum AHCIInterfaceState {
    DevSleep,
    Slumber,
    Partial,
    Active,
    Idle,
}

impl From<AHCIInterfaceState> for u8 {
    fn from(value: AHCIInterfaceState) -> Self {
        match value {
            AHCIInterfaceState::DevSleep => 0x8,
            AHCIInterfaceState::Slumber => 0x6,
            AHCIInterfaceState::Partial => 0x2,
            AHCIInterfaceState::Active => 0x1,
            AHCIInterfaceState::Idle => 0,
        }
    }
}

impl From<u8> for AHCIInterfaceState {
    fn from(value: u8) -> Self {
        match value {
            0x1 => AHCIInterfaceState::Active,
            0x2 => AHCIInterfaceState::Partial,
            0x6 => AHCIInterfaceState::Slumber,
            0x8 => AHCIInterfaceState::DevSleep,
            _ => AHCIInterfaceState::Idle,
        }
    }
}
