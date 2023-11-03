//! AHCI driver for `FrozenBoot`.

use alloc::{boxed::Box, collections::BTreeMap, vec::Vec};
use conquer_once::spin::OnceCell;

use crate::{
    drivers::{
        ahci::{
            command::{AHCICommandHeader, AHCITransaction},
            device::SATADrive,
            port::{AHCIDeviceDetection, HBAPort, HBAPortReceivedFIS, SATA_ATA_SIG},
        },
        pci::{
            device::{MappedRegister, PCIDevice, PCIMappedMemory},
            DeviceClass, PCI_DEVICES,
        },
    },
    error, info, wait_for, wait_for_or,
};

pub mod device;

mod ata_command;
mod command;
mod fis;
mod port;

/// Offset of the `Generic Host Control` register in the HBA Memory (in bytes).
pub const GHC_BOFFSET: isize = 0x00;

/// Offset of the ports registers in the HBA Memory (in bytes).
pub const PORT_REG_OFFSET: isize = 0x100;

/// Global internal `AHCI Controller` interface, usable after PCI enumeration if such a controller is
/// available on the system.
pub static AHCI_CONTROLLER: OnceCell<spin::Mutex<AHCIController>> = OnceCell::uninit();

/// List of available `SATA` drives on the system, usable after [`AHCIController`] initialization.
pub static SATA_DRIVES: OnceCell<Vec<spin::Mutex<SATADrive>>> = OnceCell::uninit();

/// Global `SATA` commands queue. Contains all commands sent to the [`AHCIController`] awaiting
/// completion.
pub static SATA_COMMAND_QUEUE: spin::Mutex<BTreeMap<u8, AHCITransaction>> =
    spin::Mutex::new(BTreeMap::new());

/// Returns a locked [`SATADrive`] given its internal identifier.
pub fn get_sata_drive(id: usize) -> &'static spin::Mutex<SATADrive> {
    &SATA_DRIVES.get().unwrap()[id]
}

/// Returns the list of [`SATADrive`] identifiers.
pub fn sata_drive_ids() -> core::ops::Range<usize> {
    0..SATA_DRIVES.get().unwrap().len()
}

/// Initialize the [`AHCIController`] into a minimal working state.
///
/// Performs a firmware initialization phase, and then a system software phase.
/// Enumerates the available SATA devices, and sets up the corresponding ports on the HBA, as well
/// as the device itself.
pub fn ahci_init() {
    let mut pci_dev_opt = PCI_DEVICES
        .get()
        .unwrap()
        .get_by_class(DeviceClass::SATAControllerAHCI);

    let pci_dev = match pci_dev_opt.get_mut(0) {
        Some(dev) => dev,
        None => return,
    };

    pci_dev.set_memory_space_access(true).unwrap();
    pci_dev.set_interrupt_disable(false).unwrap();
    pci_dev.set_bus_master(true).unwrap();

    AHCI_CONTROLLER.init_once(|| {
        spin::Mutex::new(unsafe { AHCIController::try_from_pci_device(pci_dev).unwrap() })
    });
    let mut ahci_ctrl = unsafe { AHCI_CONTROLLER.get().unwrap_unchecked().lock() };

    // Performs BIOS/OS Handoff is available.
    if ahci_ctrl.read_ghc().hba_cap_bios_os_handoff() {
        ahci_ctrl.read_ghc().hba_request_ownership(true);
        wait_for!(!ahci_ctrl.read_ghc().hba_bohc_bos(), 1);
    }

    // Performs a HBA hard reset.
    ahci_ctrl.reset();
    wait_for!(ahci_ctrl.read_ghc().hba_ghc_rst(), 50);
    ahci_ctrl.enable();

    // Setup each implemented port.
    ahci_ctrl
        .read_ghc()
        .ports_implemented()
        .iter()
        .map(|&i| ahci_ctrl.read_port_register(i))
        .for_each(|port| {
            port.port_set_start(false);
            port.port_enable_fis_receive(false);
            wait_for_or!(
                !(port.port_start()
                    || port.port_command_list_dma_engine_running()
                    || port.port_fis_receive_dma_engine_running()),
                50,
                return
            );
            // Allocate memory for received FIS and for the command list.
            let fis_receive = Box::new(HBAPortReceivedFIS::new());
            port.port_set_fis_base_address(Box::into_raw(fis_receive) as *mut u8);

            let command_list = Box::new([AHCICommandHeader::new_empty(); 32]);
            port.port_set_cmdlist_base_address(Box::into_raw(command_list) as *mut u8);

            port.port_enable_fis_receive(true);

            if ahci_ctrl.read_ghc().hba_cap_ss_support() {
                port.port_spin_up_device(true);
            }

            wait_for_or!(
                matches!(
                    port.port_interface_device_detection(),
                    AHCIDeviceDetection::DeviceDetectedPhysicalCom,
                ),
                1,
                return
            );

            port.serr = 0xffffffff;
            wait_for_or!(!(port.device_busy() || port.device_drq()), 50, return);

            // clear interrupts before enabling them.
            port.is = 0;
            port.ie = 0xffffffff;
            if matches!(
                port.port_interface_device_detection(),
                AHCIDeviceDetection::DeviceDetectedPhysicalCom
            ) {
                port.port_set_start(true);
            }
        });

    ahci_ctrl.read_ghc().set_hba_ghc_interrupt_enable(true);
    info!("ahci", "initializing AHCI controller");
    info!(
        "ahci",
        "version = {}.{}    ports_count = {}    cmd_slots = {}",
        ahci_ctrl.read_ghc().ahci_major_version(),
        ahci_ctrl.read_ghc().ahci_minor_version(),
        ahci_ctrl.read_ghc().hba_number_ports(),
        ahci_ctrl.read_ghc().hba_number_cmd_slots(),
    );
    unsafe {
        AHCI_CONTROLLER.get_unchecked().force_unlock();
        ahci_ctrl.load_sata_drives();
    }
}

/// AHCI controller related IRQs entry point.
pub fn irq_entry() {
    unsafe { AHCI_CONTROLLER.get_unchecked().force_unlock() };
    let ahci_ctrl = AHCI_CONTROLLER.get().unwrap().lock();

    for i in 0..32 {
        if ahci_ctrl.read_ghc().port_has_interrupt_pending(i) {
            let port = ahci_ctrl.read_port_register(i);
            unsafe {
                SATA_COMMAND_QUEUE.force_unlock();
            }
            let mut commands = SATA_COMMAND_QUEUE.lock();
            let commands_completed: Vec<u8> = commands
                .keys()
                .copied()
                .filter(|&i| !port.port_command_is_issued(i))
                .collect();
            for command_id in &commands_completed {
                let transaction = unsafe { commands.get(command_id).unwrap_unchecked() };
                info!(
                    "ahci",
                    "task completed ({:?} bytes transferred)",
                    transaction.byte_size()
                );
                commands.remove(command_id);
            }

            if port.tfd_error() != 0 {
                error!(
                    "ahci",
                    "tfd error on port {i}    code = {}    cmd = {}",
                    port.tfd_error(),
                    port.port_current_command_slot()
                );
            }
            port.clear_interrupts();
        }
    }

    ahci_ctrl.read_ghc().reset_pending_interrupts();
}

/// Internal representation of an `AHCI Controller` (_Advanced Host Controller Interface_).
///
/// Follows Intel's _AHCI Specifications 1.3.1_
/// The `AHCI controller` (or HBA, Host bus adapter) provides a standard interface to access SATA
/// devices using PCI-related methods (memory-mapped registers).
pub struct AHCIController {
    hba_mem: PCIMappedMemory<'static>,
}

impl AHCIController {
    /// Loads the `AHCIController` from a [`PCIDevice`] structure.
    ///
    /// The HBA memory is located in the `BAR` register 5.
    ///
    /// # Safety
    ///
    /// The `device` must be a valid AHCI Controller, with the `BAR` 5 being a memory-mapped
    /// register.
    pub unsafe fn try_from_pci_device(device: &PCIDevice<'static>) -> Option<Self> {
        let hba_reg = &device.registers[5];

        if let MappedRegister::Memory(hba_mem) = hba_reg {
            let hba_mem = hba_mem.copy_ref();

            return Some(Self { hba_mem });
        }

        None
    }

    /// Initializes the [`SATADrive`] that are attached to the [`AHCIController`].
    ///
    /// Fills the [`SATA_DRIVES`] vector of devices.
    pub fn load_sata_drives(&mut self) {
        let mut drives = alloc::vec![];
        for port in self.read_ghc().ports_implemented() {
            let port_reg = self.read_port_register(port);
            if let AHCIDeviceDetection::DeviceDetectedPhysicalCom =
                port_reg.port_interface_device_detection()
            {
                if port_reg.port_device_sig() == SATA_ATA_SIG {
                    info!(
                        "ahci",
                        "found SATA device (id = {}    port = {})",
                        drives.len(),
                        port
                    );
                    let drive = spin::Mutex::new(SATADrive::build_from_ahci(port, drives.len()));
                    drives.push(drive);
                }
            }
        }
        SATA_DRIVES.init_once(|| drives);

        let drives = SATA_DRIVES.get().unwrap();

        for drive in drives {
            drive.lock().load_partition_table();
        }
    }

    /// Returns a `mutable reference` to the `Generic Host Control` section of the HBA
    /// controller memory.
    ///
    /// Loads the [`HBAGenericHostControl`] structure directly from the HBA memory-mapped registers, and thus
    /// should be considered as `MMIO`.
    pub fn read_ghc(&self) -> &mut HBAGenericHostControl {
        unsafe {
            &mut *(self.hba_mem.as_ptr().byte_offset(GHC_BOFFSET) as *mut HBAGenericHostControl)
        }
    }

    /// Returns a `mutable reference` to a [`HBAPort`], given its port id.
    ///
    /// Loads the [`HBAPort`] structure directly from the HBA memory-mapped registers, and thus
    /// should be considered as `MMIO`.
    pub fn read_port_register(&self, port: u8) -> &mut HBAPort {
        assert!(port < 32);
        unsafe {
            &mut *(self
                .hba_mem
                .as_ptr()
                .byte_offset(PORT_REG_OFFSET + (port as isize) * 0x80)
                as *mut HBAPort)
        }
    }

    /// Performs a HBA reset on the `AHCIController`.
    ///
    /// It performs the following actions:
    ///
    /// - Resets all HBA state machine variables to their reset values.
    ///
    /// - Resets `GHC.AE`, `GHC.IE` and the `IS` register to their reset values.
    ///
    /// - Clears `GHC.HR` to 0 after reset completion
    ///
    /// Transitions to `H:WaitForAhciEnable` state afterwards.
    pub fn reset(&mut self) {
        self.read_ghc().perform_hba_ghc_rst(true);
    }

    /// Enables AHCI support. Used after a controller reset.
    ///
    /// Transitions to `H:Idle` state afterwards.
    pub fn enable(&mut self) {
        self.read_ghc().set_hba_ghc_ahci_enable(true);
    }
}

/// AHCI device's Generic Host Control register.
///
/// Contains registers that apply to the entire HBA.
#[derive(Debug)]
pub struct HBAGenericHostControl {
    /// HBA Capabilities
    pub cap: u32,

    /// GHC - Global HBA Control
    pub ghc: u32,

    /// IS - Interrupt Status Register
    pub isr: u32,

    /// PI - Ports Implemented
    pub pi: u32,

    /// VS - AHCI Version
    pub vs: u32,

    /// Command Completion Coalescing Control
    pub ccc_ctl: u32,

    /// Command Completion Coalescing Ports
    pub ccc_ports: u32,

    /// Encoslure Management Location
    pub em_loc: u32,

    /// Enclosure Management Control
    pub em_ctl: u32,

    /// HBA Capabilities Extended
    pub cap2: u32,

    /// BIOS/OS Handoff Control and Status
    pub bohc: u32,
}

#[macro_export]
macro_rules! hba_reg_field {
    ($name: tt, $offset: literal, $desc: tt, $field: tt, $getter: tt, $setter: tt) => {
        #[doc = $desc]
        pub(super) const $name: u32 = $offset;

        #[doc = $desc]
        pub fn $getter(&self) -> bool {
            unsafe {
                core::ptr::read_volatile(&self.$field as *const u32) & (1 << Self::$name) != 0
            }
        }

        #[doc = $desc]
        pub fn $setter(&mut self, new_state: bool) {
            let field = unsafe { core::ptr::read_volatile(&self.$field as *const u32) };
            let new_field = if new_state {
                field | (1 << Self::$name)
            } else {
                field & (!(1 << Self::$name))
            };
            unsafe { core::ptr::write_volatile(&mut self.$field as *mut u32, new_field) }
        }
    };
    ($name: tt, $offset: literal, $desc: tt, $field: tt, $getter: tt) => {
        #[doc = $desc]
        pub(super) const $name: u32 = $offset;

        #[doc = $desc]
        pub fn $getter(&self) -> bool {
            unsafe {
                core::ptr::read_volatile(&self.$field as *const u32) & (1 << Self::$name) != 0
            }
        }
    };
    ($name: tt, $offset: literal, $desc: tt) => {
        #[doc = $desc]
        pub(super) const $name: u32 = $offset;
    };
}

impl HBAGenericHostControl {
    /// Number of Ports.
    pub fn hba_number_ports(&self) -> u8 {
        (1 + (unsafe { core::ptr::read_volatile(&self.cap as *const u32) } & 0b11111)) as u8
    }
    /// Number of Command Slots
    pub fn hba_number_cmd_slots(&self) -> u8 {
        (1 + ((unsafe { core::ptr::read_volatile(&self.cap as *const u32) } >> 8) & 0b11111)) as u8
    }
    /// Indicates if a port within the controller has an interrupt pending.
    pub fn port_has_interrupt_pending(&self, x: u8) -> bool {
        (unsafe { core::ptr::read_volatile(&self.isr as *const u32) } & (1 << x)) != 0
    }
    /// Reset the interrupt status of every port.
    pub fn reset_pending_interrupts(&mut self) {
        unsafe { core::ptr::write_volatile(&mut self.isr as *mut u32, 0) }
    }
    /// Indicates if a port is exposed by the HBA.
    pub fn is_port_implemented(&self, x: u8) -> bool {
        (unsafe { core::ptr::read_volatile(&self.pi as *const u32) } & (1 << x)) != 0
    }
    /// Lists all ports exposed by the HBA.
    pub fn ports_implemented(&self) -> alloc::vec::Vec<u8> {
        (0..32).filter(|&i| self.is_port_implemented(i)).collect()
    }
    /// AHCI Minor Version
    pub fn ahci_minor_version(&self) -> u8 {
        let minor_version_lb: u8 = (self.vs & 0xff) as u8;
        let minor_version_hb: u8 = ((self.vs & 0xff00) >> 8) as u8;

        minor_version_hb * 10 + minor_version_lb
    }
    /// AHCI Major Version
    pub fn ahci_major_version(&self) -> u8 {
        let major_version_lb: u8 = ((self.vs & 0xff0000) >> 16) as u8;
        let major_version_hb: u8 = ((self.vs & 0xff000000) >> 24) as u8;

        major_version_hb * 10 + major_version_lb
    }
    /// `hCccTimer` is reset to the `timeout_value` on the assertion of each CCC
    pub fn timeout_value(&self) -> u16 {
        ((unsafe { core::ptr::read_volatile(&self.ccc_ctl as *const u32) } & 0xff00) >> 16) as u16
    }
    /// Specifies the number of command completion that are necessary to cause a CCC interrupt.
    pub fn ccc_cmd_compl(&self) -> u8 {
        ((unsafe { core::ptr::read_volatile(&self.ccc_ctl as *const u32) } << 8) & 0xff) as u8
    }
    /// Specifies the interrupt used by the CCC feature.
    pub fn ccc_interrupt(&self) -> u8 {
        ((unsafe { core::ptr::read_volatile(&self.ccc_ctl as *const u32) } << 3) & 0b1111) as u8
    }
    /// Indicates if a port is coalesced as part of the CCC feature.
    pub fn is_port_coalesced(&self, x: u8) -> bool {
        (unsafe { core::ptr::read_volatile(&self.ccc_ports as *const u32) } & (1 << x)) != 0
    }
    /// Specifies the size of the transmit message buffer area in DWORDs.
    pub fn em_buf_size(&self) -> u16 {
        (unsafe { core::ptr::read_volatile(&self.em_loc as *const u32) } & 0xff) as u16
    }
    /// The offset of the message buffer in DWORDs from the beginning of the `ABAR`
    pub fn em_buf_offset(&self) -> u16 {
        ((unsafe { core::ptr::read_volatile(&self.em_loc as *const u32) } & 0xff00) >> 16) as u16
    }
    hba_reg_field!(
        HBA_EM_STSMR,
        0,
        "Enclosure Management: Message Received",
        em_ctl,
        hba_em_mr,
        hba_em_mr_clear
    );
    hba_reg_field!(
        HBA_EM_TM,
        8,
        "Enclosure Management: Transmit Message",
        em_ctl,
        hba_em_tm,
        hba_em_transmit
    );
    hba_reg_field!(
        HBA_EM_RST,
        9,
        "Enclosure Management: Reset",
        em_ctl,
        hba_em_is_rst,
        hba_em_reset
    );
    hba_reg_field!(
        HBA_EM_LED_SUPP,
        16,
        "LED Message Types support",
        em_ctl,
        hba_em_supp_led
    );
    hba_reg_field!(
        HBA_EM_SAFTE_SUPP,
        17,
        "SAF-TE Enclosure Management Messages",
        em_ctl,
        hba_em_supp_safte
    );
    hba_reg_field!(
        HBA_EM_SES2_SUPP,
        18,
        "SES-2 Enclosure Management Messages",
        em_ctl,
        hba_em_supp_ses2
    );
    hba_reg_field!(
        HBA_EM_SGPIO_SUPP,
        19,
        "SGPIO Enclosure Management Messages",
        em_ctl,
        hba_em_supp_sgpio
    );
    hba_reg_field!(HBA_EM_SMB, 24, "Single Message Buffer", em_ctl, hba_em_smb);
    hba_reg_field!(
        HBA_EM_XMT,
        25,
        "Transmit Only",
        em_ctl,
        hba_em_transmit_only
    );
    hba_reg_field!(
        HBA_EM_ALHD,
        26,
        "Activity LED Hardware Driven",
        em_ctl,
        hba_em_aled_hw_driven
    );
    hba_reg_field!(
        HBA_EM_PM,
        27,
        "Port Multiplier Support",
        em_ctl,
        hba_em_pm_supp
    );
    hba_reg_field!(
        HBA_CCC_EN,
        0,
        "Command Completion Coalescing Enable",
        ccc_ctl,
        hba_ccc_enable
    );
    hba_reg_field!(HBA_BOHC_BOS, 0, "BIOS Owned Semaphore", bohc, hba_bohc_bos);
    hba_reg_field!(
        HBA_BOHC_OOS,
        1,
        "OS Owned Semaphore",
        bohc,
        hba_bohc_oos,
        hba_request_ownership
    );
    hba_reg_field!(
        HBA_BOHC_SOOE,
        2,
        "SMI on OS Ownership Change Enable",
        bohc,
        hba_bohc_sooe,
        hba_enable_smi_on_ooc
    );
    hba_reg_field!(
        HBA_BOHC_OOC,
        3,
        "OS Ownership Change",
        bohc,
        hba_os_ownership_change,
        hba_clear_oos_bit
    );
    hba_reg_field!(HBA_BOHC_BB, 4, "BIOS Busy", bohc, hba_bios_busy);
    hba_reg_field!(
        HBA_CAP2_BOH,
        0,
        "BIOS/OS Handoff",
        cap2,
        hba_cap_bios_os_handoff
    );
    hba_reg_field!(
        HBA_CAP2_NVMP,
        1,
        "NVMHCI Present",
        cap2,
        hba_cap_nvmhci_present
    );
    hba_reg_field!(
        HBA_CAP2_APST,
        2,
        "Automatic Partial to Slumber Transitions",
        cap2,
        hba_cap_apst
    );
    hba_reg_field!(
        HBA_CAP2_SDS,
        3,
        "Supports Device Sleep",
        cap2,
        hba_cap_sup_device_slp
    );
    hba_reg_field!(
        HBA_CAP2_SADM,
        4,
        "Supports Aggressive Device Sleep Management",
        cap2,
        hba_cpa_sadm
    );
    hba_reg_field!(
        HBA_CAP2_DESO,
        5,
        "DevSleep Entrance from Slumber Only",
        cap2,
        hba_cap_deso
    );
    hba_reg_field!(
        HBA_GHC_HR,
        0,
        "HBA Reset",
        ghc,
        hba_ghc_rst,
        perform_hba_ghc_rst
    );
    hba_reg_field!(
        HBA_GHC_IE,
        1,
        "Interrupt Enable",
        ghc,
        hba_ghc_interrupt_enable,
        set_hba_ghc_interrupt_enable
    );
    hba_reg_field!(
        HBA_GHC_MRSM,
        2,
        "MSI Revert to Single Message",
        ghc,
        hba_ghc_msi_revert_to_single
    );
    hba_reg_field!(
        HBA_GHC_AE,
        31,
        "AHCI Enable",
        ghc,
        hba_ghc_ahci_enable,
        set_hba_ghc_ahci_enable
    );
    hba_reg_field!(
        HBA_CAP_S64A,
        31,
        "Supports 64-bit Addressing",
        cap,
        hba_cap_64_addr_support
    );
    hba_reg_field!(
        HBA_CAP_SNCQ,
        30,
        "Supports Native Command Queuing",
        cap,
        hba_cap_native_cmdq_support
    );
    hba_reg_field!(
        HBA_CAP_SSNTF,
        29,
        "Supports SNotification Register",
        cap,
        hba_cap_snotif_reg_support
    );
    hba_reg_field!(
        HBA_CAP_SMPS,
        28,
        "Supports Mechanical Presence Switch",
        cap,
        hba_cap_mech_presw_support
    );
    hba_reg_field!(
        HBA_CAP_SSS,
        27,
        "Supports Staggered Spin-up",
        cap,
        hba_cap_ss_support
    );
    hba_reg_field!(
        HBA_CAP_SALP,
        26,
        "Supports Aggressive Link Power Management",
        cap,
        hba_cap_aggr_linkpow_mgmt_support
    );
    hba_reg_field!(
        HBA_CAP_SAL,
        25,
        "Supports Activity LED",
        cap,
        hba_cap_act_led_support
    );
    hba_reg_field!(
        HBA_CAP_SCLO,
        24,
        "Supports Command List Override",
        cap,
        hba_cap_cmd_list_override_support
    );
    hba_reg_field!(
        HBA_CAP_SAM,
        18,
        "Supports AHCI mode only",
        cap,
        hba_cap_ahci_only
    );
    hba_reg_field!(
        HBA_CAP_SPM,
        17,
        "Supports Port Multiplier",
        cap,
        hba_cap_port_mul_support
    );
    hba_reg_field!(
        HBA_CAP_FBSS,
        16,
        "FIS-based Switching Supported",
        cap,
        hba_cap_fis_switching_support
    );
    hba_reg_field!(
        HBA_CAP_PMD,
        15,
        "PIO Multiple DRQ Block",
        cap,
        hba_cap_pio_mul_drq_blk
    );
    hba_reg_field!(
        HBA_CAP_SSC,
        14,
        "Slumber State Capable",
        cap,
        hba_cap_slumber_state
    );
    hba_reg_field!(
        HBA_CAP_PSC,
        13,
        "Partial State Capable",
        cap,
        hba_cap_partial_state
    );
    hba_reg_field!(
        HBA_CAP_CCCS,
        7,
        "Command Completion Coalescing Supported",
        cap,
        hba_cap_cmd_compl_coalescing_support
    );
    hba_reg_field!(
        HBA_CAP_EMS,
        6,
        "Enclosure Management Supported",
        cap,
        hba_cap_enclosure_mgmt_support
    );
    hba_reg_field!(
        HBA_CAP_SXS,
        5,
        "Supports External SATA",
        cap,
        hba_cap_external_sata
    );
}
