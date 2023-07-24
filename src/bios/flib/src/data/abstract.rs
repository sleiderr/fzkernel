use core::mem::transmute;
use crate::disk_io::disk::AddressPacket;

pub struct DataSegment<T, S: DataSource> {
    physical_origin: u32,
    abstract_origin: u32,
    virtual_origin: u32,
    length: u32,
    cursor: *const T,
    source: S,
}

impl<T, S: DataSource> DataSegment<T, S> {
    pub fn read_abstract(&mut self, address: u32) -> Option<&T> {
        if !self.move_cursor(address) {
            return None;
        }
        let read_addr =
            (self.cursor as u32 - self.virtual_origin + self.physical_origin) as *const T;
        let mut data_ref: &T = unsafe { transmute(read_addr) };
        Some(data_ref)
    }

    pub fn move_cursor(&mut self, abstract_address: u32) -> bool {
        let block_number = (abstract_address - self.abstract_origin) / self.length;
        if !((abstract_address > self.virtual_origin)
            & (abstract_address < self.virtual_origin + self.length))
        {
            let offset = (abstract_address - self.abstract_origin) % self.length;
            match self
                .source
                .load(block_number, self.physical_origin, self.length)
            {
                Ok(_) => {
                    self.virtual_origin = self.abstract_origin + block_number * self.length;
                    self.cursor = (self.virtual_origin + offset) as *const T;
                    true
                }
                Err(_) => false,
            }
        } else {
            self.cursor = abstract_address as *const T;
            match self
                .source
                .load(block_number, self.physical_origin, self.length)
            {
                Ok(_) => true,
                Err(_) => false,
            }
        }
    }

    pub fn default(source: S) -> Self {
        Self {
            physical_origin: 0,
            abstract_origin: 0,
            virtual_origin: 0,
            length: 0,
            cursor: 0 as *const T,
            source,
        }
    }

    pub fn set_abstract_origin(&mut self, abstract_origin: u32) {
        self.abstract_origin = abstract_origin;
    }

    pub fn set_length(&mut self, length: u32) {
        self.length = length;
    }

    pub fn set_physical_origin(&mut self, physical_origin: u32) {
        self.physical_origin = physical_origin;
    }
}

pub trait DataSource {
    fn load(&self, n: u32, physical_address: u32, length: u32) -> Result<(), ()>;
}

pub struct Disk {
    origin: u32,
}

impl Disk {
    pub fn new(origin: u32) -> Self {
        Self { origin }
    }
}

impl DataSource for Disk {
    fn load(&self, n: u32, physical_address: u32, length: u32) -> Result<(), ()> {
        let address = AddressPacket::new(
            (length / 512) as u16,
            physical_address,
            ((n * length + self.origin) / 512) as u64,
        );
        address.disk_read(0x80)
    }
}
