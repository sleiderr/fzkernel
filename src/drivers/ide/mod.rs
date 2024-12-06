pub mod ata_command;
pub(super) mod ata_pio;

use crate::drivers::generics::dev_disk::SataDeviceType;
use crate::drivers::ide::ata_pio::{ata_devices, AtaDevice};
use crate::drivers::pci::{pci_devices, DeviceClass};
use crate::io::IOPort;
use crate::irq::manager::get_interrupt_manager;
use crate::irq::InterruptStackFrame;
use crate::x86::apic::InterruptVector;
use alloc::vec::Vec;
use conquer_once::spin::OnceCell;
use core::cmp::Ordering;
use core::fmt::{Display, Formatter};
use fzproc_macros::interrupt_handler;
use modular_bitfield::bitfield;
use modular_bitfield::prelude::{B2, B4};
use spin::RwLock;

use super::pci::device::MappedRegister;
use super::pci::device::PCIDevice;

#[interrupt_handler]
pub fn ata_irq_entry(frame: InterruptStackFrame) {
    for ata_dev in ata_devices().read().values() {
        if ata_dev.may_expect_irq() {
            ata_dev.handle_irq();
        }
    }
}

pub fn ide_controllers() -> &'static RwLock<Vec<IdeController>> {
    static IDE_CONTROLLERS: OnceCell<RwLock<Vec<IdeController>>> = OnceCell::uninit();

    IDE_CONTROLLERS
        .try_get_or_init(|| RwLock::new(Vec::<IdeController>::new()))
        .unwrap()
}

pub fn ide_init() {
    let ide_controller = pci_devices().get_by_class(DeviceClass::IDEControllerBusMaster);

    for controller in ide_controller.iter() {
        IdeController::init_from_pci(controller);
    }
}

#[derive(Copy, Clone, Debug)]
pub struct AtaDeviceIdentifier {
    pub disk_type: SataDeviceType,
    pub(in crate::drivers) ide_controller: usize,
    pub(in crate::drivers) device_id: usize,
}

impl PartialEq for AtaDeviceIdentifier {
    fn eq(&self, other: &Self) -> bool {
        self.internal_identifier().eq(&other.internal_identifier())
    }
}

impl Eq for AtaDeviceIdentifier {}

impl PartialOrd for AtaDeviceIdentifier {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.internal_identifier()
            .partial_cmp(&other.internal_identifier())
    }
}

impl Ord for AtaDeviceIdentifier {
    fn cmp(&self, other: &Self) -> Ordering {
        self.internal_identifier().cmp(&other.internal_identifier())
    }
}

impl AtaDeviceIdentifier {
    pub fn new(disk_type: SataDeviceType, ide_controller: usize, device_id: usize) -> Self {
        Self {
            disk_type,
            ide_controller,
            device_id,
        }
    }

    fn internal_identifier(self) -> usize {
        self.ide_controller * 4 + self.device_id
    }
}

impl Display for AtaDeviceIdentifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let disk_type_str = match self.disk_type {
            SataDeviceType::IDE => "IDE",
            SataDeviceType::AHCI => "AHCI",
        };
        f.write_fmt(format_args!(
            "ATA device   device_type = {}    controller_id = {}    device_id = {}",
            disk_type_str, self.ide_controller, self.device_id
        ))
    }
}

pub struct IdeController {
    primary_master: Option<AtaDeviceIdentifier>,
    primary_slave: Option<AtaDeviceIdentifier>,
    secondary_master: Option<AtaDeviceIdentifier>,
    secondary_slave: Option<AtaDeviceIdentifier>,
}

impl IdeController {
    pub fn init_from_pci(pci_dev: &PCIDevice) {
        let prim_chan = &pci_dev.registers[0];
        let prim_chan_ctrl = &pci_dev.registers[1];
        let sec_chan = &pci_dev.registers[2];
        let sec_chan_ctrl = &pci_dev.registers[3];

        get_interrupt_manager().register_static_handler(InterruptVector::from(0x76), ata_irq_entry);
        get_interrupt_manager().register_static_handler(InterruptVector::from(0x2E), ata_irq_entry);

        let ports = match (prim_chan, prim_chan_ctrl, sec_chan, sec_chan_ctrl) {
            (
                MappedRegister::IO(prim_portl),
                MappedRegister::IO(prim_ctrl_portl),
                MappedRegister::IO(sec_portl),
                MappedRegister::IO(sec_ctrl_portl),
            ) => {
                let prim_port = if *prim_portl != 0 {
                    IOPort::from(*prim_portl)
                } else {
                    IOPort::PRIM_ATA
                };
                let prim_ctrl_port = if *prim_ctrl_portl != 0 {
                    IOPort::from(*prim_ctrl_portl)
                } else {
                    IOPort::PRIM_ATA_CTRL
                };
                let sec_port = if *sec_portl != 0 {
                    IOPort::from(*sec_portl)
                } else {
                    IOPort::SEC_ATA
                };
                let sec_ctrl_port = if *sec_ctrl_portl != 0 {
                    IOPort::from(*sec_ctrl_portl)
                } else {
                    IOPort::SEC_ATA_CTRL
                };

                (prim_port, prim_ctrl_port, sec_port, sec_ctrl_port)
            }
            _ => (
                IOPort::PRIM_ATA,
                IOPort::PRIM_ATA_CTRL,
                IOPort::SEC_ATA,
                IOPort::SEC_ATA_CTRL,
            ),
        };

        let mut controller_list = ide_controllers().write();
        let controller_id = controller_list.len();
        let primary_master = AtaDevice::init(
            AtaDeviceIdentifier::new(SataDeviceType::IDE, controller_id, 0),
            ports.0,
            ports.1,
            false,
            controller_id,
            true,
        )
        .ok();
        let primary_slave = AtaDevice::init(
            AtaDeviceIdentifier::new(SataDeviceType::IDE, controller_id, 1),
            ports.0,
            ports.1,
            true,
            controller_id,
            true,
        )
        .ok();
        let secondary_master = AtaDevice::init(
            AtaDeviceIdentifier::new(SataDeviceType::IDE, controller_id, 2),
            ports.2,
            ports.3,
            false,
            controller_id,
            true,
        )
        .ok();
        let secondary_slave = AtaDevice::init(
            AtaDeviceIdentifier::new(SataDeviceType::IDE, controller_id, 3),
            ports.2,
            ports.3,
            true,
            controller_id,
            true,
        )
        .ok();

        controller_list.push(Self {
            primary_master: primary_master,
            primary_slave: primary_slave,
            secondary_master: secondary_master,
            secondary_slave: secondary_slave,
        });
    }
}

struct IdeControllerRegister {}

#[bitfield]
struct IdeCommandRegister {
    start_bus_master: bool,
    #[skip]
    __: B2,
    write_control: bool,
    #[skip]
    __: B4,
}

#[bitfield]
struct IdeStatusRegister {
    active: bool,
    error: bool,
    int: bool,
    #[skip]
    __: B2,
    driveO_dma: bool,
    drive1_dma: bool,
    simplex_only: bool,
}

struct IdeDescriptorTablePtrRegister {
    address: u32,
}
