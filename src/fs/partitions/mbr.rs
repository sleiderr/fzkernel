//! MBR (_Master Boot Record_) partion table handling
//!
//! Legacy structure used to store partition information on hard drives, stored on the first
//! logical block of the drive.
//!
//! It limits the number of partition to 4 (without using _EBR_), and the partition sizes to 2 Terabytes at most.

use alloc::vec::Vec;

use crate::{drivers::ahci::device::SATADrive, fs::partitions::Partition};

/// Offset of the `Parition table` in the `Master Boot Record`.
const MBR_PART_OFFSET: isize = 0x1BE;

/// Load the `Master Boot Record` partition table from a [`SATADrive`].
pub fn load_drive_mbr(drive: &mut SATADrive, sectors_offset: u64) -> MBRPartitionTable {
    let mut first_sector = [0u8; 512];
    drive.read(sectors_offset, 1, &mut first_sector).unwrap();

    unsafe {
        core::ptr::read(first_sector.as_ptr().offset(MBR_PART_OFFSET) as *const MBRPartitionTable)
    }
}

/// A `Master Boot Record` partition table.
///
/// Contains at most 4 partitions, it is the legacy way of storing partition information on the
/// disk.
#[repr(packed)]
#[derive(Debug)]
pub struct MBRPartitionTable {
    partitions: [MBRPartitionEntry; 4],
}

impl MBRPartitionTable {
    /// Returns the [`MBRPartitionEntry`] corresponding to the 4 partition entry in this `MBR`.
    pub fn get_partition_metadata(&self) -> [MBRPartitionEntry; 4] {
        self.partitions
    }

    /// Returns a [`Partition`] structure for each valid partition entry in this `MBR`.
    pub fn get_partitions(&self) -> Vec<Partition> {
        let mut partitions = alloc::vec![];

        for partition_metadata in self.partitions {
            if partition_metadata.is_used() {
                let partition = Partition::from_mbr_metadata(partition_metadata);
                partitions.push(partition);
            }
        }

        partitions
    }
}

/// A `Master Boot Record` partition entry.
///
/// Stored on the device's first logical block, it is the legacy way of storing partition
/// information on the disk.
///
/// All related methods should use _LBA_ instead of the legacy _CHS_ addressing.
#[repr(packed)]
#[derive(Debug, Clone, Copy)]
pub struct MBRPartitionEntry {
    attributes: u8,
    chs_start: [u8; 3],
    part_type: u8,
    chs_last: [u8; 3],
    lba_start: u32,
    sectors_count: u32,
}

impl MBRPartitionEntry {
    /// Checks if this partition is _active_ (or bootable).
    ///
    /// Only one partition should be active for a given [`MBRPartitionTable`]
    ///
    /// # Examples
    ///
    /// Check if the first partition in a table is bootable.
    ///
    /// ```
    /// let part = load_drive_mbr(drive, 0);
    /// assert!(part.get_partition_metadata()[0].is_active())
    /// ```
    pub fn is_active(&self) -> bool {
        if (0x80..0x8F).contains(&self.attributes) {
            return true;
        }

        false
    }

    /// Sets this partition as _active_ (or bootable) or not.
    ///
    /// Only one partition should be active for a given [`MBRPartitionTable`]
    ///
    /// # Examples
    ///
    /// Set the first partition in a table as bootable.
    ///
    /// ```
    /// let part = load_drive_mbr(drive, 0);
    /// part.get_partition_metadata()[0].set_active(true);
    /// ```
    pub fn set_active(&mut self, active: bool) {
        self.attributes = if active { 0x80 } else { 0 };
    }

    /// Checks if this partition is used (valid).
    ///
    /// # Examples
    ///
    /// Check the first partition in a table is valid.
    ///
    /// ```
    /// let part = load_drive_mbr(drive, 0);
    /// assert!(part.get_partition_metadata()[0].is_used());
    /// ```
    pub fn is_used(&self) -> bool {
        self.part_type != 0
    }

    fn starting_head(&self) -> u8 {
        self.chs_start[0]
    }

    fn set_starting_head(&mut self, head: u8) {
        self.chs_start[0] = head;
    }

    fn starting_sector(&self) -> u8 {
        self.chs_start[1] & 0b111111
    }

    fn set_starting_sector(&mut self, sector: u8) {
        self.chs_start[1] = (self.chs_start[1] & !0b111111) | sector;
    }

    fn starting_cylinder(&self) -> u16 {
        (((self.chs_start[1] as u16) & !0b111111) << 2) | self.chs_start[2] as u16
    }

    fn set_starting_cylinder(&mut self, cylinder: u16) {
        self.chs_start[1] = (self.chs_start[1] & 0b111111) | ((cylinder >> 2) & !0xff) as u8;
        self.chs_start[2] = (cylinder & 0xff) as u8;
    }

    /// Returns this partition's starting LBA.
    ///
    /// The _LBA_ is encoded using 32 bits, which limits the maximum partition size to 2TB.
    ///
    /// # Examples
    ///
    /// Check the starting LBA of the first partition in the table (may _panic_, as the first
    /// partition on the table is not necessarily located in the first sectors of the disk).
    ///
    /// ```
    /// let part = load_drive_mbr(drive, 0);
    /// assert_eq!(part.get_partition_metadata()[0].start_lba(), 1);
    /// ```
    pub fn start_lba(&self) -> u32 {
        self.lba_start
    }

    /// Sets this partition's starting LBA.
    ///
    /// The _LBA_ is encoded using 32 bits, which limits the maximum partition size to 2TB.
    ///
    /// # Examples
    ///
    /// ```
    /// let part = load_drive_mbr(drive, 0);
    /// part.get_partition_metadata()[0].set_start_lba(1);
    /// ```
    pub fn set_start_lba(&mut self, lba: u32) {
        /*
        let start_chs = lba_to_chs(lba, sectors_per_track, heads_count);

        if start_chs[0] > 1024 {
            self.set_starting_cylinder(1023);
            self.set_starting_head(255);
            self.set_starting_sector(63);
        }

        self.set_starting_cylinder(start_chs[0] as u16);
        self.set_starting_head(start_chs[1] as u8);
        self.set_starting_sector(start_chs[2] as u8);
        */

        self.lba_start = lba;
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
    /// let part = load_drive_mbr(drive, 0);
    /// println!("{}", part.get_partition_metadata()[0].sectors_count());
    /// ```
    pub fn sectors_count(&self) -> u32 {
        self.sectors_count
    }

    /// Sets this partition's sectors count.
    ///
    /// The _sector count_ is encoded using 32 bits, which limits the maximum partition size to 2TB.
    ///
    /// # Examples
    ///
    /// Sets the length of the first partition in the table
    ///
    /// ```
    /// let part = load_drive_mbr(drive, 0);
    /// part.get_partition_metadata()[0].set_sectors_count(512);
    /// ```
    pub fn set_sectors_count(&mut self, count: u32) {
        self.sectors_count = count;
    }

    /// Returns the [`PartitionType`] defined for this partition.
    ///
    /// Should indicate the filesystem contained in this partition.
    ///
    /// # Examples
    ///
    /// Check if the first partition in the table uses _FAT32_
    ///
    /// ```
    /// let part = load_drive_mbr(drive, 0);
    /// assert!(matches!(part.get_partition_metadata()[0].partition_type(), PartitionType::Fat32LBA));
    /// ```
    pub fn partition_type(&self) -> PartitionType {
        Into::<PartitionType>::into(self.part_type)
    }

    /// Sets the [`PartitionType`] defined for this partition.
    ///
    /// Should indicate the filesystem contained in this partition.
    ///
    /// # Examples
    ///
    /// Set the first partition's filesystem indicator to _FAT32_
    ///
    /// ```
    /// let part = load_drive_mbr(drive, 0);
    /// part.get_partition_metadata()[0].set_partition_type(PartitionType::Fat32LBA);
    /// ```
    pub fn set_partition_type(&mut self, part_type: PartitionType) {
        self.part_type = part_type.into();
    }
}

impl From<PartitionType> for u8 {
    fn from(value: PartitionType) -> Self {
        match value {
            PartitionType::Empty => 0,
            PartitionType::DOSFat12 => 1,
            PartitionType::XenixRoot => 2,
            PartitionType::XenixUsr => 3,
            PartitionType::DOS3Fat16 => 4,
            PartitionType::Extended => 5,
            PartitionType::DOS331Fat16 => 6,
            PartitionType::OS2IFS => 7,
            PartitionType::NTFS => 7,
            PartitionType::EXFAT => 7,
            PartitionType::Fat32 => 0xB,
            PartitionType::Fat32LBA => 0xC,
            PartitionType::DOSFat16LBA => 0xE,
            PartitionType::ExtendedLBA => 0xF,
            PartitionType::LinuxSwap => 0x82,
            PartitionType::LinuxNative => 0x83,
            PartitionType::LinuxExtended => 0x85,
            PartitionType::LinuxLVM => 0x8E,
            PartitionType::BSDI => 0x9F,
            PartitionType::OpenBSD => 0xA6,
            PartitionType::MacOSX => 0xA8,
            PartitionType::MacOSXBoot => 0xAB,
            PartitionType::MacOSXHFS => 0xAF,
            PartitionType::LUKS => 0xE8,
            PartitionType::GPT => 0xEE,
            PartitionType::Unknown => 0xEA,
        }
    }
}

impl From<u8> for PartitionType {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Empty,
            1 => Self::DOSFat12,
            2 => Self::XenixRoot,
            3 => Self::XenixUsr,
            4 => Self::DOS3Fat16,
            5 => Self::Extended,
            6 => Self::DOS331Fat16,
            7 => Self::NTFS,
            0xB => Self::Fat32,
            0xC => Self::Fat32LBA,
            0xE => Self::DOSFat16LBA,
            0xF => Self::ExtendedLBA,
            0x82 => Self::LinuxSwap,
            0x83 => Self::LinuxNative,
            0x85 => Self::LinuxExtended,
            0x8E => Self::LinuxLVM,
            0x9F => Self::BSDI,
            0xA6 => Self::OpenBSD,
            0xA8 => Self::MacOSX,
            0xAB => Self::MacOSXBoot,
            0xAF => Self::MacOSXHFS,
            0xE8 => Self::LUKS,
            0xEE => Self::GPT,
            _ => Self::Unknown,
        }
    }
}

/// Known partition IDs for various filesystems, used in MBR partition entries.
pub enum PartitionType {
    Empty,
    DOSFat12,
    XenixRoot,
    XenixUsr,
    DOS3Fat16,
    Extended,
    DOS331Fat16,
    OS2IFS,
    NTFS,
    Fat32,
    Fat32LBA,
    EXFAT,
    DOSFat16LBA,
    ExtendedLBA,
    LinuxSwap,
    LinuxNative,
    LinuxExtended,
    LinuxLVM,
    BSDI,
    OpenBSD,
    MacOSX,
    MacOSXBoot,
    MacOSXHFS,
    LUKS,
    GPT,
    Unknown,
}
