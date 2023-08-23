mod virt;

use alloc::collections::VecDeque; // Or we could implement our own double-ended queue with a growable ring buffer
use alloc::vec::Vec;

use crate::{
    error,
};

pub struct Packet {}

pub trait NetworkDevice {
    // Pushes up to nbr_packet packets to buffer (depends on number of packets available on card).
    // Returns the number of received packets.
    fn rx_batch(&mut self, queue_id: u16, buffer: &mut VecDeque<Packet>, nbr_packet: usize) -> usize;

    // Takes from buffer until buffer empty or queue full.
    // Returns the number of sent packets.
    fn tx_batch(&mut self, queue_id: u16, buffer: &mut VecDeque<Packet>) -> usize;

    // Return the network card link speed in Mbit/s
    fn get_link_speed(&self) -> u16;

    // Wait for all packets in buffer to be queued
    fn tx_batch_wait(&mut self, queue_id: u16, buffer: &mut VecDeque<Packet>) -> () {
        while !buffer.is_empty() {
            self.tx_batch(queue_id, buffer);
        }
    }
}

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