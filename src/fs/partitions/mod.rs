//! Partition formats code.
//!
//! Contains the implementation of the two standards partition scheme, _GPT_ and _MBR_.

use crate::errors::{CanFail, MountError};
use crate::fs::{
    ext4::Ext4Fs,
    partitions::{
        gpt::{GPTPartitionEntry, GUIDPartitionTable},
        mbr::{MBRPartitionEntry, MBRPartitionTable},
    },
    Fs, PartFS,
};

pub mod gpt;
pub mod mbr;

/// A partition structure, that does not depend on the partition format (_GPT_ or _MBR_).
///
/// Offers several method needed when dealing with partitions.
#[derive(Clone)]
pub struct Partition {
    id: usize,
    drive_id: usize,
    metadata: PartitionMetadata,
    pub fs: PartFS,
}

impl Partition {
    /// Loads a `Partition` from a _MBR_ partition table entry.
    pub fn from_metadata(
        part_id: usize,
        drive_id: usize,
        metadata: PartitionMetadata,
    ) -> Option<Self> {
        Some(Self {
            metadata,
            id: part_id,
            drive_id,
            fs: PartFS::Unknown,
        })
    }

    pub fn load_fs(&mut self) -> CanFail<MountError> {
        self.fs = match self.metadata {
            PartitionMetadata::MBR(meta) => match meta.partition_type() {
                mbr::PartitionType::Empty => PartFS::Unknown,
                mbr::PartitionType::DOSFat12 => todo!(),
                mbr::PartitionType::XenixRoot => todo!(),
                mbr::PartitionType::XenixUsr => todo!(),
                mbr::PartitionType::DOS3Fat16 => todo!(),
                mbr::PartitionType::Extended => todo!(),
                mbr::PartitionType::DOS331Fat16 => todo!(),
                mbr::PartitionType::OS2IFS => todo!(),
                mbr::PartitionType::NTFS => todo!(),
                mbr::PartitionType::Fat32 => todo!(),
                mbr::PartitionType::Fat32LBA => todo!(),
                mbr::PartitionType::EXFAT => todo!(),
                mbr::PartitionType::DOSFat16LBA => todo!(),
                mbr::PartitionType::ExtendedLBA => todo!(),
                mbr::PartitionType::LinuxSwap => todo!(),
                mbr::PartitionType::LinuxNative => {
                    if Ext4Fs::identify(self.drive_id, meta.start_lba() as u64)
                        .map_err(|_| MountError::IOError)?
                    {
                        let fs = Ext4Fs::mount(self.drive_id, self.id, meta.start_lba() as u64)?;
                        PartFS::Ext4(alloc::boxed::Box::new(fs))
                    } else {
                        PartFS::Unknown
                    }
                }
                mbr::PartitionType::LinuxExtended => todo!(),
                mbr::PartitionType::LinuxLVM => todo!(),
                mbr::PartitionType::BSDI => todo!(),
                mbr::PartitionType::OpenBSD => todo!(),
                mbr::PartitionType::MacOSX => todo!(),
                mbr::PartitionType::MacOSXBoot => todo!(),
                mbr::PartitionType::MacOSXHFS => todo!(),
                mbr::PartitionType::LUKS => todo!(),
                mbr::PartitionType::GPT => PartFS::Unknown,
                mbr::PartitionType::Unknown => PartFS::Unknown,
            },
            PartitionMetadata::GPT(meta) => {
                if Ext4Fs::identify(self.drive_id, meta.start_lba())
                    .map_err(|_| MountError::IOError)?
                {
                    let fs = Ext4Fs::mount(self.drive_id, self.id, meta.start_lba())?;
                    PartFS::Ext4(alloc::boxed::Box::new(fs))
                } else {
                    PartFS::Unknown
                }
            }
        };

        Ok(())
    }

    /// Returns this partition's starting LBA.
    pub fn start_lba(&self) -> u64 {
        match self.metadata {
            PartitionMetadata::MBR(meta) => meta.start_lba() as u64,
            PartitionMetadata::GPT(meta) => meta.start_lba(),
        }
    }

    /// Returns the partition format dependent metadatas.
    ///
    /// They contain the original table entry for this partition.
    pub fn metadata(&self) -> &PartitionMetadata {
        &self.metadata
    }

    /// Returns the partition format dependent metadatas.
    ///
    /// They contain the original table entry for this partition.
    pub fn metadata_mut(&mut self) -> &mut PartitionMetadata {
        &mut self.metadata
    }
}

/// `PartitionMetadata` contains the original table entry for a [`Partition`].
#[derive(Debug, Clone, Copy)]
pub enum PartitionMetadata {
    MBR(MBRPartitionEntry),
    GPT(GPTPartitionEntry),
}

#[derive(Debug)]
pub enum PartitionTable {
    MBR(MBRPartitionTable),
    GPT(GUIDPartitionTable),
    Unknown,
}
