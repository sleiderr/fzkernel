//! x86 `PageTable` structures implementation.

#[cfg(feature = "kernel")]
mod frame_alloc;
pub mod mapper;
pub mod translate;

use crate::errors::CanFail;
use crate::mem::{Alignment, MemoryAddress, PhyAddr};
use crate::x86::paging::{Frame, PageMappingError};
use core::ops::BitOr;
use modular_bitfield::bitfield;
use modular_bitfield::prelude::{B3, B51};

/// Stores mapping information between virtual [`Page`] and physical memory [`Frame`].
///
/// The structure of the table is hierarchical, and it contains several layers depending on the current execution
/// context. Each entry may either map a [`Page`] directly to a [`Frame`], or to another table structure one layer
/// deeper.
///
/// The table must be page-aligned.
#[repr(align(4096))]
#[derive(Debug)]
pub struct PageTable {
    entries: [PageTableEntry; 512],
}

impl PageTable {
    /// Returns a mutable reference to an entry in this table.
    pub fn get_mut(&mut self, id: u16) -> &mut PageTableEntry {
        &mut self.entries[id as usize]
    }
}

impl Default for PageTable {
    fn default() -> Self {
        Self {
            entries: [PageTableEntry::default(); 512],
        }
    }
}

/// Represents a paging structure entry.
///
/// Contains the physical address of the physical memory [`Frame`] referenced by this entry, as well as various flags
/// to describe how the page entry must be used ([`PageTableFlags`]).
///
/// An entry may either map a [`Page`], reference other paging structures ([`PageTable`]), or neither if the `Presence`
/// bit is clear.
///
/// It may also include access right modifiers, such as protection keys.
#[derive(Clone, Copy, Debug, Default)]
#[repr(transparent)]
pub struct PageTableEntry {
    entry: u64,
}

impl PageTableEntry {
    /// 64-bit number with all bits corresponding to the address part of the entry set.
    const ADDR_BITS: PhyAddr = PhyAddr::new(0x000f_ffff_ffff_f000);

    /// Returns whether this entry is used.
    #[must_use]
    pub fn used(&self) -> bool {
        self.entry != 0
    }

    /// Returns the physical memory [`Frame`] that this entry maps to.
    #[must_use]
    pub fn frame(&self) -> Frame {
        Frame {
            addr: PhyAddr::new(0),
        }
    }

    /// Maps this entry to the given physical memory [`Frame`], and updates the entry flags.
    pub fn map_to_frame(
        &mut self,
        frame: Frame,
        flags: PageTableFlags,
    ) -> CanFail<PageMappingError> {
        self.map_to_addr(frame.addr, flags)
    }

    /// Maps this entry to the physical memory [`Frame`] starting at the given address.
    ///
    /// # Errors
    ///
    /// May return [`PageMappingError::BadAlignment`] if the given physical address is not properly aligned (must
    /// be aligned on page size).
    pub fn map_to_addr(
        &mut self,
        addr: PhyAddr,
        flags: PageTableFlags,
    ) -> CanFail<PageMappingError> {
        if !addr.is_aligned_with(Alignment::ALIGN_4KB).unwrap() {
            return Err(PageMappingError::BadAlignment);
        }
        self.entry = u64::from(addr) | u64::from(flags);

        Ok(())
    }

    /// Updates this entry's flags.
    pub fn set_flags(&mut self, flags: PageTableFlags) {
        self.entry = (self.entry & u64::from(Self::ADDR_BITS)) | u64::from(flags);
    }

    /// Returns this entry's flags.
    pub fn flags(self) -> PageTableFlags {
        PageTableFlags::from(self.entry & !u64::from(Self::ADDR_BITS))
    }
}

/// Flags associated to a [`PageTableEntry`].
///
/// They describe how the entry should be used, and may also be used for access-rights control.
#[bitfield]
#[derive(Clone, Copy)]
#[repr(u64)]
pub struct PageTableFlags {
    /// Present bit.
    ///
    /// Must be set to map to a [`Frame`].
    pub present: bool,

    /// Read/write bit.
    ///
    /// If clear, writes to the [`Page`] referenced by this entry are not allowed.
    pub write: bool,

    /// User/supervisor bit.
    ///
    /// If clear, user mode access are not allowed to the [`Page`] referenced by this entry.
    pub user_access: bool,

    /// Page-level write-through bit.
    pub write_through: bool,

    /// Page-level cache disable bit.
    pub cache_disable: bool,

    /// Accessed bit.
    ///
    /// Indicates whether software has accessed the [`Page`] referenced by this entry.
    pub accessed: bool,

    /// Dirty bit.
    ///
    /// Indicates whether software has written to the [`Page`] referenced by this entry.
    pub dirty: bool,

    /// Page size bit.
    ///
    /// If set, this entry references a large (huge) page.
    pub huge_page: bool,

    /// Global bit.
    ///
    /// Determines whether the transaction is global.
    pub global: bool,
    #[skip]
    __: B51,

    /// Protection key.
    ///
    /// This may be used to control the page's access rights in some contexts.
    pub pke: B3,

    /// Execute-disable bit.
    ///
    /// If set, instruction fetches from the [`Page`] referenced by this entry may not be allowed.
    pub nxe: bool,
}

impl BitOr for PageTableFlags {
    type Output = PageTableFlags;

    fn bitor(self, rhs: Self) -> Self::Output {
        PageTableFlags::from(u64::from(self) | u64::from(rhs))
    }
}
