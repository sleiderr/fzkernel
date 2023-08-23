mod queue;

use alloc::rc::Rc;
use alloc::collections::VecDeque;
use queue::VirtualQueue;
use crate::network::{NetworkDevice, Packet};

pub struct VirtualIODevice {

    rx_queue : VirtualQueue,
    tx_queue : VirtualQueue,
    control_queue : VirtualQueue,

    // rx_mempool : ?
    // tx_mempool : ?
    // control_mempool : ?

    rx_packets : u64,
    tx_packets : u64,
    rx_bytes : u64,
    tx_bytes : u64,

}

impl NetworkDevice for VirtualIODevice {
    fn rx_batch(&mut self, queue_id: u16, buffer: &mut VecDeque<Packet>, nbr_packet: usize) -> usize {
        todo!()
    }

    fn tx_batch(&mut self, queue_id: u16, buffer: &mut VecDeque<Packet>) -> usize {
        todo!()
    }

    // No speed link because full virtual
    fn get_link_speed(&self) -> u16 {
        1000
    }
}