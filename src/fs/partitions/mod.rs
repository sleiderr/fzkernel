//! Partition formats code.
//!
//! Contains the implementation of the two standards partition scheme, _GPT_ and _MBR_.

use crate::fs::partitions::{
    gpt::{GPTPartitionEntry, GUIDPartitionTable},
    mbr::{MBRPartitionEntry, MBRPartitionTable},
};

pub mod gpt;
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
    pub fn from_metadata(metadata: PartitionMetadata) -> Self {
        Self { metadata }
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
