pub mod ata_command;
pub(crate) mod ata_pio;

use crate::drivers::ide::ata_pio::{ata_devices, AtaDevice};
use crate::drivers::pci::{pci_devices, DeviceClass};
use crate::io::IOPort;
use crate::println;
use modular_bitfield::bitfield;
use modular_bitfield::prelude::{B2, B4};

pub fn ata_irq_entry() {
    for ata_dev in ata_devices().read().iter() {
        if ata_dev.may_expect_irq() {
            ata_dev.handle_irq();
        }
    }
}

pub fn ide_init() {
    let ide_controller = pci_devices().get_by_class(DeviceClass::IDEControllerBusMaster);

    AtaDevice::init(IOPort::from(0x1F0), IOPort::from(0x3F6), false);
}

pub struct IdeController {}

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
