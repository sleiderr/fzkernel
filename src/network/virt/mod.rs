mod queue;

use alloc::rc::Rc;
use alloc::collections::VecDeque;
use queue::VirtualQueue;
use crate::network::{NetworkDevice};
use crate::network::mem::{MemoryPool, Packet};

#[repr(C)]
pub struct virtio_net_hdr {
    pub flags: u8,
    pub gso_type: u8,
    pub hdr_len: u16,     // Ethernet + IP + tcp/udp hdrs
    pub gso_size: u16,    // Bytes to append to hdr_len per frame
    pub csum_start: u16,  // Position to start checksumming from
    pub csum_offset: u16, // Offset after that to place checksum
}

static NET_HEADER: virtio_net_hdr = virtio_net_hdr {
    flags: 0,
    gso_type: 0,
    hdr_len: 14 + 20 + 8,
    // ignored fields
    csum_offset: 0,
    csum_start: 0,
    gso_size: 0,
};

const HEADER_SIZE: usize = core::mem::size_of::<virtio_net_hdr>();
const VIRTQ_DESC_F_WRITE: u16 = 2;

pub struct VirtualIODevice {

    rx_queue : VirtualQueue,
    tx_queue : VirtualQueue,
    control_queue : VirtualQueue,

    rx_mempool : Rc<MemoryPool>,
    // tx_mempool : ?
    control_mempool : Rc<MemoryPool>,

    rx_packets_inflight : VecDeque<Packet>,
    tx_packets_inflight : VecDeque<Packet>,

    rx_packets : u64,
    tx_packets : u64,
    rx_bytes : u64,
    tx_bytes : u64,

}

impl NetworkDevice for VirtualIODevice {
    fn rx_batch(&mut self, queue_id: u16, buffer: &mut VecDeque<Packet>, nbr_packet: usize)  {
        for _ in 0..nbr_packet {
            if self.rx_queue.last_used_idx == self.rx_queue.used.idx {
                break;
            }
            let used = self.rx_queue.used[self.rx_queue.last_used_idx % self.rx_queue.size].clone();
            self.rx_queue.last_used_idx += 1;

            // Descriptor
            let descriptor = &mut self.rx_queue.mut_descriptors()[used.id as usize];
            assert_eq!(descriptor.flags, VIRTQ_DESC_F_WRITE, "unsupported flags {:x}", descriptor.flags);
            descriptor.addr = 0;

            let mut buf = self.rx_packets_inflight.pop_front().unwrap();
            buf.len = used.len as usize - HEADER_SIZE;

            self.rx_bytes += buf.len as u64;
            self.rx_packets += 1;
            buffer.push_back(buf);
        }
        let mut queued = 0;
        for i in 0..self.rx_queue.size {

        }
    }

    fn tx_batch(&mut self, queue_id: u16, buffer: &mut VecDeque<Packet>) -> usize {
        todo!()
    }

    // No speed link because full virtual
    fn get_link_speed(&self) -> u16 {
        1000
    }
}