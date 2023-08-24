use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::slice;

const HEADROOM_SIZE: usize = 32;

pub struct Packet {
    pub(crate) virtual_address: *mut u8,
    pub(crate) physical_address: usize,
    pub(crate) length: usize,
    pub(crate) pool_entry: usize,
    pub(crate) pool: Rc<MemoryPool>,
}

pub struct MemoryPool {
    base_address: *mut u8,
    entry_size: usize,
    entry_count: usize,
    physical_addresses: Vec<usize>,
    pub(crate) free_entries_stack: RefCell<Vec<usize>>,
}

impl Packet {

    pub(crate) unsafe fn new(
        virtual_address: *mut u8,
        physical_address: usize,
        length: usize,
        pool: Rc<MemoryPool>,
        pool_entry: usize,
    ) -> Packet {
        Packet {
            virtual_address,
            physical_address,
            length,
            pool_entry,
            pool,
        }
    }

    pub fn get_virtual_address(&self) -> *mut u8 {
        self.virtual_address
    }

    pub fn get_physical_address(&self) -> usize {
        self.physical_address
    }

    pub fn get_pool(&self) -> &Rc<MemoryPool> {
        &self.pool
    }

    pub fn shrink_packets(&mut self, trunc: usize) -> () {
        self.length = self.length.min(trunc);
    }

    pub fn mut_headroom(&mut self, len: usize) -> &mut [u8] {
        assert!(len <= HEADROOM_SIZE);
        unsafe {
            slice::from_raw_parts_mut(self.virtual_address.sub(len), len)
        }
    }

}