pub mod ata_command;
pub(crate) mod ata_pio;

use crate::drivers::ide::ata_pio::{ata_devices, AtaDevice};
use crate::drivers::pci::{pci_devices, DeviceClass};
use crate::io::IOPort;
use alloc::vec::Vec;
use conquer_once::spin::OnceCell;
use modular_bitfield::bitfield;
use modular_bitfield::prelude::{B2, B4};
use spin::RwLock;

use super::pci::device::MappedRegister;
use super::pci::device::PCIDevice;

pub fn ata_irq_entry() {
    for ata_dev in ata_devices().read().iter() {
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

pub struct AtaDeviceIdentifier {
    ide_controller: usize,
    device_id: usize,
}

impl AtaDeviceIdentifier {
    pub fn new(ide_controller: usize, device_id: usize) -> Self {
        Self {
            ide_controller,
            device_id,
        }
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

        let primary_master = AtaDevice::init(ports.0, ports.1, false).ok();
        let primary_slave = AtaDevice::init(ports.0, ports.1, true).ok();
        let secondary_master = AtaDevice::init(ports.2, ports.3, false).ok();
        let secondary_slave = AtaDevice::init(ports.2, ports.3, true).ok();

        let mut controller_list = ide_controllers().write();
        let controller_id = controller_list.len();
        controller_list.push(Self {
            primary_master: primary_master.map(|dev| AtaDeviceIdentifier::new(controller_id, dev)),
            primary_slave: primary_slave.map(|dev| AtaDeviceIdentifier::new(controller_id, dev)),
            secondary_master: secondary_master
                .map(|dev| AtaDeviceIdentifier::new(controller_id, dev)),
            secondary_slave: secondary_slave
                .map(|dev| AtaDeviceIdentifier::new(controller_id, dev)),
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
