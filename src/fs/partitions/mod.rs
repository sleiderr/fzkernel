//! Partition formats code.
//!
//! Contains the implementation of the two standards partition scheme, _GPT_ and _MBR_.

use crate::fs::partitions::mbr::{MBRPartitionEntry, MBRPartitionTable};

pub mod mbr;

/// A partition structure, that does not depend on the partition format (_GPT_ or _MBR_).
///
/// Offers several method needed when dealing with partitions.
#[derive(Debug, Clone, Copy)]
pub struct Partition {
    metadata: PartitionMetadata,
}

impl Partition {
    /// Loads a `Partition` from a _MBR_ partition table entry.
    pub fn from_mbr_metadata(metadata: MBRPartitionEntry) -> Self {
        Self {
            metadata: PartitionMetadata::MBR(metadata),
        }
    }

    /// Returns this partition's starting LBA.
    pub fn start_lba(&self) -> u64 {
        match self.metadata {
            PartitionMetadata::MBR(meta) => meta.start_lba() as u64,
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
}

#[derive(Debug)]
pub enum PartitionTable {
    MBR(MBRPartitionTable),
    GPT,
    Unknown,
}
