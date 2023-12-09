//! `GUID Partition Table` handling.
//!
//! Standard layout for storing partitions tables. Part of the UEFI standard.

use core::slice;

use alloc::{boxed::Box, string::String, vec::Vec};

use crate::{
    drivers::ahci::device::SATADrive,
    error,
    fs::partitions::{mbr::load_drive_mbr, Partition},
    info,
};

/// Loads a `GUID Partition Table` from a [`SATADrive`].
pub fn load_drive_gpt(drive: &mut SATADrive) -> Option<GUIDPartitionTable> {
    let pmbr = load_drive_mbr(drive, 0);

    if !pmbr.is_pmbr() {
        return None;
    }

    let mut gpt_header_bytes = alloc::vec![0u8; drive.logical_sector_size() as usize];
    drive.read(1, 1, &mut gpt_header_bytes).ok()?;
    let mut gpt_header = unsafe { core::ptr::read(gpt_header_bytes.as_ptr() as *mut GPTHeader) };

    // fallback to backup header
    if !gpt_header.is_valid() {
        error!("gpt", "invalid primary gpt header");
        drive
            .read(
                drive.maximum_addressable_lba() as u64 - 1,
                1,
                &mut gpt_header_bytes,
            )
            .ok()?;
        gpt_header = unsafe { core::ptr::read(gpt_header_bytes.as_ptr() as *mut GPTHeader) };

        if !gpt_header.is_valid() {
            error!("gpt", "primary and backup gpt headers corrupted, aborting");
            return None;
        }
    }

    let gpt_entries_buffer_size_in_sectors =
        ((gpt_header.partitions_count + gpt_header.part_entry_size) / 0x200) + 1;
    let mut gpt_entries_buffer: Vec<u8> =
        alloc::vec![0; (gpt_header.part_entry_size * gpt_header.partitions_count) as usize];

    let mut partitions: Vec<GPTPartitionEntry> = alloc::vec![];
    drive
        .read(
            gpt_header.part_entry_lba,
            gpt_entries_buffer_size_in_sectors as u16,
            &mut gpt_entries_buffer,
        )
        .ok()?;

    for i in 0..gpt_header.partitions_count {
        let part_buffer = &gpt_entries_buffer[((i * gpt_header.part_entry_size) as usize)
            ..(((i + 1) * gpt_header.part_entry_size) as usize)];

        let partition =
            unsafe { core::ptr::read(part_buffer.as_ptr() as *const GPTPartitionEntry) };

        if partition.is_used() {
            partitions.push(partition);
        }
    }

    let table_crc32 = crc32_calc(&gpt_entries_buffer);
    if gpt_header.part_entry_array_crc32 != table_crc32 {
        error!(
            "gpt",
            "table checksum mismatch (expected {:#x} got {:#x})",
            unsafe {
                core::ptr::read_unaligned(core::ptr::addr_of!(gpt_header.part_entry_array_crc32))
            },
            table_crc32
        );
    }

    info!(
        "gpt",
        "loading disk guid partition table ({} partitions found)",
        partitions.len()
    );
    let gpt = Box::new(GPT::new(gpt_header, partitions));
    Some(gpt)
}

pub type GUIDPartitionTable = Box<GPT>;

/// A `GUID Partition Table` internal representation.
///
/// Contains a GPT Header, as well as the list of all used partitions described in the table.
#[derive(Debug)]
pub struct GPT {
    header: GPTHeader,
    partitions: Vec<GPTPartitionEntry>,
}

impl GPT {
    pub fn new(header: GPTHeader, partitions: Vec<GPTPartitionEntry>) -> Self {
        Self { header, partitions }
    }

    /// Returns a [`Partition`] structure for each valid partition entry in this `GPT`.
    pub fn get_partitions(&self) -> Vec<Partition> {
        let mut partitions = alloc::vec![];

        for partition in &self.partitions {
            partitions.push(Partition::from_metadata(super::PartitionMetadata::GPT(
                *partition,
            )));
        }

        partitions
    }
}

/// `GUID Partition Table Header`
#[repr(packed)]
#[derive(Debug)]
pub struct GPTHeader {
    /// Identifies EFI-compatible partition table header.
    /// Should contain the string "EFI PART".
    sig: u64,

    /// Revision number for this header.
    revision: u32,

    /// Size of the header in bytes.
    size: u32,

    /// CRC32 checksum for the header.
    checksum: u32,
    reserved: u32,

    /// The LBA that contains this structure.
    my_lba: u64,

    /// The LBA of the alternate `GPT` header.
    alternate_lba: u64,

    /// First logical block that may be used by a partition.
    first_usable_lba: u64,

    /// Last logical block that may be used by a partition.
    last_usable_lba: u64,

    /// GUID used to identify the disk.
    disk_guid: u128,

    /// Starting LBA of the GUID Partition Entry array.
    part_entry_lba: u64,

    /// Number of partitions entries in the GUID Partition Entry array.
    partitions_count: u32,

    /// Size in bytes of each entry in the GUID Partition Entry array.
    part_entry_size: u32,

    /// CRC32 of the GUID Partition Entry array.
    part_entry_array_crc32: u32,
}

impl GPTHeader {
    fn new_empty() -> Self {
        Self {
            sig: 0,
            revision: 0,
            size: 0,
            checksum: 0,
            reserved: 0,
            my_lba: 0,
            alternate_lba: 0,
            first_usable_lba: 0,
            last_usable_lba: 0,
            disk_guid: 0,
            part_entry_lba: 0,
            partitions_count: 0,
            part_entry_size: 0,
            part_entry_array_crc32: 0,
        }
    }

    /// Checks if this `GPTHeader` is valid (valid checksum and valid signature)
    pub fn is_valid(&mut self) -> bool {
        if self.sig != 0x5452415020494645 {
            error!("gpt", "gpt header invalid signature");
            return false;
        }

        let crc_32 = self.checksum;
        self.checksum = 0;

        let new_crc32 = unsafe {
            crc32_calc(slice::from_raw_parts(
                self as *const _ as *const u8,
                self.size as usize,
            ))
        };

        if crc_32 != new_crc32 {
            error!(
                "gpt",
                "gpt header checksum mismatch (expected {:#x} got {:#x})", crc_32, new_crc32
            );

            return false;
        }
        self.checksum = crc_32;

        true
    }
}

#[repr(packed)]
#[derive(Debug, Clone, Copy)]
pub struct GPTPartitionEntry {
    /// Defines the purpose and type of this partition.
    type_guid: u128,

    /// GUID unique for every partition entry.
    partition_guid: u128,

    /// Starting LBA of this partition.
    starting_lba: u64,

    /// Last LBA of this partition.
    last_lba: u64,

    /// Partition's attributes bits.
    attributes: u64,

    /// Null-terminated string containing a human-readable name of this partition.
    partition_name: [u16; 36],
}

impl GPTPartitionEntry {
    pub fn new_empty() -> Self {
        Self {
            type_guid: 0,
            partition_guid: 0,
            starting_lba: 0,
            last_lba: 0,
            attributes: 0,
            partition_name: [0u16; 36],
        }
    }
    /// Returns this partition's starting LBA.
    ///
    /// # Examples
    ///
    /// Check the starting LBA of the first partition in the table (may _panic_, as the first
    /// partition on the table is not necessarily located in the first sectors of the disk).
    ///
    /// ```
    /// let part = load_drive_gpt(drive);
    /// assert_eq!(part.get_partition_metadata()[0].start_lba(), 1);
    /// ```
    pub fn start_lba(&self) -> u64 {
        self.starting_lba
    }

    /// Returns this partition's unique GUID.
    ///
    /// # Examples
    ///
    /// Check if the GUID of the first partition in the table is not null.
    ///
    /// ```
    /// let part = load_drive_gpt(drive);
    /// assert_neq!(part.get_partition_metadata()[0].guid(), 0);
    /// ```
    pub fn guid(&self) -> u128 {
        self.partition_guid
    }

    /// Returns this partition's sectors count.
    ///
    /// The _sector count_ is encoded using 32 bits, which limits the maximum partition size to 2TB.
    ///
    /// # Examples
    ///
    /// Check the length of the first partition in the table
    ///
    /// ```
    /// let part = load_drive_gpt(drive);
    /// println!("{}", part.get_partition_metadata()[0].size_in_sectors());
    /// ```
    pub fn size_in_sectors(&self) -> u64 {
        self.last_lba - self.starting_lba
    }

    /// Checks if this partition is used (valid).
    ///
    /// # Examples
    ///
    /// Check the first partition in a table is valid.
    ///
    /// ```
    /// let part = load_drive_gpt(drive);
    /// assert!(part.get_partition_metadata()[0].is_used());
    /// ```
    pub fn is_used(&self) -> bool {
        self.type_guid != 0
    }

    /// Returns this partition's name.
    ///
    /// Null-terminated string containing a human-readable name of the partition.
    ///
    /// # Examples
    ///
    /// Display the name of the first partition on the disk.
    ///
    /// ```
    /// let part = load_drive_gpt(drive);
    /// println!("{}", part.get_partition_metadata()[0].name());
    /// ```
    pub fn name(&self) -> String {
        let name_bytes =
            unsafe { core::ptr::read_unaligned(core::ptr::addr_of!(self.partition_name)) };
        let valid_bytes: Vec<u16> = name_bytes.into_iter().filter(|&c| c != 0).collect();

        String::from_utf16(&valid_bytes).unwrap()
    }

    /// Checks if the partition is required for the platform to function.
    ///
    /// # Examples
    ///
    /// Make sure that a partition is not required (before deletion for instance).
    ///
    /// ```
    /// let part = load_drive_gpt(drive);
    /// assert!(part.get_partition_metadata()[0].is_required());
    /// ```
    pub fn is_required(&self) -> bool {
        self.attributes & 0x1 != 0
    }

    /// Checks if firmware should not produce a `EFI_BLOCK_IO_PROTOCOL` device for this partition.
    ///
    /// If such a device is not produced, file system mappings will not be created for this
    /// partition in `UEFI`.
    pub fn no_blockio_prot(&self) -> bool {
        self.attributes & 0x2 != 0
    }

    pub fn legacy_bootable(&self) -> bool {
        self.attributes & 0x8 != 0
    }
}

/// CCITT32 ANSI CRC lookup table
const CRC_32_ANSI_TAB: [u32; 256] = [
    /* CRC polynomial 0xedb88320 */
    0x00000000, 0x77073096, 0xee0e612c, 0x990951ba, 0x076dc419, 0x706af48f, 0xe963a535, 0x9e6495a3,
    0x0edb8832, 0x79dcb8a4, 0xe0d5e91e, 0x97d2d988, 0x09b64c2b, 0x7eb17cbd, 0xe7b82d07, 0x90bf1d91,
    0x1db71064, 0x6ab020f2, 0xf3b97148, 0x84be41de, 0x1adad47d, 0x6ddde4eb, 0xf4d4b551, 0x83d385c7,
    0x136c9856, 0x646ba8c0, 0xfd62f97a, 0x8a65c9ec, 0x14015c4f, 0x63066cd9, 0xfa0f3d63, 0x8d080df5,
    0x3b6e20c8, 0x4c69105e, 0xd56041e4, 0xa2677172, 0x3c03e4d1, 0x4b04d447, 0xd20d85fd, 0xa50ab56b,
    0x35b5a8fa, 0x42b2986c, 0xdbbbc9d6, 0xacbcf940, 0x32d86ce3, 0x45df5c75, 0xdcd60dcf, 0xabd13d59,
    0x26d930ac, 0x51de003a, 0xc8d75180, 0xbfd06116, 0x21b4f4b5, 0x56b3c423, 0xcfba9599, 0xb8bda50f,
    0x2802b89e, 0x5f058808, 0xc60cd9b2, 0xb10be924, 0x2f6f7c87, 0x58684c11, 0xc1611dab, 0xb6662d3d,
    0x76dc4190, 0x01db7106, 0x98d220bc, 0xefd5102a, 0x71b18589, 0x06b6b51f, 0x9fbfe4a5, 0xe8b8d433,
    0x7807c9a2, 0x0f00f934, 0x9609a88e, 0xe10e9818, 0x7f6a0dbb, 0x086d3d2d, 0x91646c97, 0xe6635c01,
    0x6b6b51f4, 0x1c6c6162, 0x856530d8, 0xf262004e, 0x6c0695ed, 0x1b01a57b, 0x8208f4c1, 0xf50fc457,
    0x65b0d9c6, 0x12b7e950, 0x8bbeb8ea, 0xfcb9887c, 0x62dd1ddf, 0x15da2d49, 0x8cd37cf3, 0xfbd44c65,
    0x4db26158, 0x3ab551ce, 0xa3bc0074, 0xd4bb30e2, 0x4adfa541, 0x3dd895d7, 0xa4d1c46d, 0xd3d6f4fb,
    0x4369e96a, 0x346ed9fc, 0xad678846, 0xda60b8d0, 0x44042d73, 0x33031de5, 0xaa0a4c5f, 0xdd0d7cc9,
    0x5005713c, 0x270241aa, 0xbe0b1010, 0xc90c2086, 0x5768b525, 0x206f85b3, 0xb966d409, 0xce61e49f,
    0x5edef90e, 0x29d9c998, 0xb0d09822, 0xc7d7a8b4, 0x59b33d17, 0x2eb40d81, 0xb7bd5c3b, 0xc0ba6cad,
    0xedb88320, 0x9abfb3b6, 0x03b6e20c, 0x74b1d29a, 0xead54739, 0x9dd277af, 0x04db2615, 0x73dc1683,
    0xe3630b12, 0x94643b84, 0x0d6d6a3e, 0x7a6a5aa8, 0xe40ecf0b, 0x9309ff9d, 0x0a00ae27, 0x7d079eb1,
    0xf00f9344, 0x8708a3d2, 0x1e01f268, 0x6906c2fe, 0xf762575d, 0x806567cb, 0x196c3671, 0x6e6b06e7,
    0xfed41b76, 0x89d32be0, 0x10da7a5a, 0x67dd4acc, 0xf9b9df6f, 0x8ebeeff9, 0x17b7be43, 0x60b08ed5,
    0xd6d6a3e8, 0xa1d1937e, 0x38d8c2c4, 0x4fdff252, 0xd1bb67f1, 0xa6bc5767, 0x3fb506dd, 0x48b2364b,
    0xd80d2bda, 0xaf0a1b4c, 0x36034af6, 0x41047a60, 0xdf60efc3, 0xa867df55, 0x316e8eef, 0x4669be79,
    0xcb61b38c, 0xbc66831a, 0x256fd2a0, 0x5268e236, 0xcc0c7795, 0xbb0b4703, 0x220216b9, 0x5505262f,
    0xc5ba3bbe, 0xb2bd0b28, 0x2bb45a92, 0x5cb36a04, 0xc2d7ffa7, 0xb5d0cf31, 0x2cd99e8b, 0x5bdeae1d,
    0x9b64c2b0, 0xec63f226, 0x756aa39c, 0x026d930a, 0x9c0906a9, 0xeb0e363f, 0x72076785, 0x05005713,
    0x95bf4a82, 0xe2b87a14, 0x7bb12bae, 0x0cb61b38, 0x92d28e9b, 0xe5d5be0d, 0x7cdcefb7, 0x0bdbdf21,
    0x86d3d2d4, 0xf1d4e242, 0x68ddb3f8, 0x1fda836e, 0x81be16cd, 0xf6b9265b, 0x6fb077e1, 0x18b74777,
    0x88085ae6, 0xff0f6a70, 0x66063bca, 0x11010b5c, 0x8f659eff, 0xf862ae69, 0x616bffd3, 0x166ccf45,
    0xa00ae278, 0xd70dd2ee, 0x4e048354, 0x3903b3c2, 0xa7672661, 0xd06016f7, 0x4969474d, 0x3e6e77db,
    0xaed16a4a, 0xd9d65adc, 0x40df0b66, 0x37d83bf0, 0xa9bcae53, 0xdebb9ec5, 0x47b2cf7f, 0x30b5ffe9,
    0xbdbdf21c, 0xcabac28a, 0x53b39330, 0x24b4a3a6, 0xbad03605, 0xcdd70693, 0x54de5729, 0x23d967bf,
    0xb3667a2e, 0xc4614ab8, 0x5d681b02, 0x2a6f2b94, 0xb40bbe37, 0xc30c8ea1, 0x5a05df1b, 0x2d02ef8d,
];

pub fn crc32_calc(buf: &[u8]) -> u32 {
    let mut crc_32: u32 = 0xFFFFFFFF;

    for &b in buf {
        crc_32 = CRC_32_ANSI_TAB[((crc_32 ^ b as u32) & 0xff) as usize] ^ (crc_32 >> 8);
    }

    !crc_32
}
