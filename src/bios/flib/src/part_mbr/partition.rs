use core::ptr::{read_volatile};
use core::array::IntoIter;
use core::iter::IntoIterator;
use core::option::Option;

pub struct PartitionTable {
    offset : *const [u8; 16],
    partitions : [Partition; 4],
    loaded : bool
}

#[derive(Clone, Copy)]
pub struct Partition {
    bootable : bool,
    starting_head : u8,
    starting_sector : u8,
    starting_cylinder : u16,
    system_id : u8,
    ending_head : u8,
    ending_sector : u8,
    ending_cylinder : u16,
    lba_offset : u32,
    sector_number : u32
}

impl Partition {
    // Loads a partition given an offset
    fn from(offset : *const [u8; 16]) -> Self{
        let bytes;
        unsafe { bytes = read_volatile(offset)}
        let bootable = bytes[0] == 1;
        let starting_head = bytes[1];
        let starting_sector = bytes[2] & 0b00111111;
        let starting_cylinder = ((bytes[3] as u16) << 2) | ((bytes[2] as u16 )>> 6);
        let system_id = bytes[4];
        let ending_head = bytes[5];
        let ending_sector = bytes[6] & 0b00111111;
        let ending_cylinder = ((bytes[7] as u16) << 2) | ((bytes[6] as u16) >> 6);
        let mut _lba = [0u8; 4];
        _lba.copy_from_slice(&bytes[8..12]);
        let lba_offset = u32::from_le_bytes(_lba);
        _lba.copy_from_slice(&bytes[12..16]);
        let sector_number = u32::from_le_bytes(_lba);
        Self {
            bootable,
            starting_head,
            starting_sector,
            starting_cylinder,
            system_id,
            ending_head,
            ending_sector,
            ending_cylinder,
            lba_offset,
            sector_number
        }
    }

    pub fn empty() -> Self {
        return Self {
            bootable : false,
            starting_head: 0,
            starting_sector: 0,
            starting_cylinder: 0,
            system_id: 0,
            ending_head: 0,
            ending_sector: 0,
            ending_cylinder: 0,
            lba_offset: 0,
            sector_number: 0,
        }
    }
}

impl PartitionTable {

    pub fn new(offset : *const [u8; 16]) -> Self{
        return Self {
            offset,
            partitions: [Partition::empty(),Partition::empty(),Partition::empty(),Partition::empty()],
            loaded: false,
        }
    }

    // Load a specific table. Returns true if successful.
    pub fn load_partition(&mut self, offset : usize) -> bool{
        return if offset > 4 {
            false
        } else {
            let part = Partition::from((self.offset as u32 + (offset as u32 * 16u32) as u32 ) as *const [u8; 16]);
            self.partitions[offset] = part;
            true
        }
    }

    // Load full partition table. If an error occurs, returns the offset of the sector that failed (load appends in growing order)
    // If the loading is successful, returns 4
    pub fn load(&mut self) -> i8{
        let mut successful = true;
        for i in 0..4 {
            let success = self.load_partition(i);
            successful = success & successful;
            if !success{
                return i as i8
            }
        }
        return 4;
    }

    pub fn get(&self, offset : usize) -> Partition {
        return self.partitions[offset];
    }

    // Returns the first bootable partition, assuming there is at most one.
    pub fn get_bootable(&self) ->Option<Partition> {
        self.partitions.into_iter().find(|x| x.bootable)
    }
}