use crate::{
    error,
};

// Initialize the network card at the pci address
pub fn init_card(
    pci_device: u16,
    pci_bus: u8,
    nbr_tx_queues: u8,
    nbr_rx_queues: u8
) -> () { // Box<dyn NetworkDevice>, ()> {

    let mut vendor = 0u16;
    let mut class = 0u8;

    if class != 2 {
        error!("PCI", format!("Device {} is not a network card", pci_device));
    }

    if vendor == 0x1af4 && pci_device == 0x1000 { // QEMU PCI ID (or used for virtio more generally)
        if nbr_rx_queues > 1 || nbr_tx_queues > 1 {
            error!("PCI", format!("Device {} does not support more than one queue", pci_device));
        }
        // return a virtual device
    } else if vendor == 0x8086 && // Intel vendor ID
        (pci_device == 0x10ed || pci_device == 0x1515 || pci_device == 0x1520) // Ethernet controller virtual
    {
        if nbr_rx_queues > 1 || nbr_tx_queues > 1 {
            error!("PCI", format!("Device {} does not support more than one queue", pci_device));
        }
        // return a real device with virtual functions
    } else {
        // return a real device
    }

}