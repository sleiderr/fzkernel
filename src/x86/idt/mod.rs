use alloc::vec::Vec;
use core::arch::asm;
use core::ptr::write_volatile;

use crate::x86::int::{disable_interrupts, enable_interrupts};
use modular_bitfield;
use modular_bitfield::bitfield;
use modular_bitfield::prelude::{B1, B11, B13, B2, B4, B8};
use modular_bitfield::BitfieldSpecifier;

use crate::mem::gdt::SegmentDescriptor;

pub enum GateType {
    TaskGate = 0b0101,
    InterruptGate16b = 0b0110,
    InterruptGate32b = 0b1110,
    TrapGate16b = 0b0111,
    /// Most of the time you will chose a [GateType::TrapGate32b]
    TrapGate32b = 0b1111,
}

/// [Table] contains entries that describes interrupts
/// It has the following structure :
///  ----------------------
/// |   Address   |  Entry  |
/// | --------------------- |
/// |  Offset + 0 | entry 0 |
/// |  Offset + 8 | entry 1 |
///  -----------------------
pub struct Table {
    entries: Vec<GateDescriptor>,
}

impl Table {
    /// Writes the `Table` to a given offset. You have to ensure that there is enough
    /// free space to write the table (8 bytes per entry)
    pub fn write(&self, offset: u32) {
        let mut cursor = offset;
        for &gate in &self.entries {
            let ptr = cursor as *mut GateDescriptor;
            unsafe { write_volatile(ptr, gate) }
            cursor += 8;
        }
    }

    /// Creates an empty [Table]
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Returns a mutable reference to a [GateDescriptor] wrapped in an [Option].
    /// Index starts at 0.
    pub fn get_entry_mut(&mut self, index: usize) -> Option<&mut GateDescriptor> {
        self.entries.get_mut(index)
    }

    /// Returns an immutable reference to a [GateDescriptor] wrapped in an [Option].
    /// Index starts at 0.
    pub fn get_entry(&self, index: usize) -> Option<&GateDescriptor> {
        self.entries.get(index)
    }

    /// Populates the `Table` with default [`GateDescriptor`] to reach 256 entries (required)
    pub fn populate_default(&mut self) {
        let mut i = self.len();
        while i < 256 {
            self.add_gate(&GateDescriptor::new());
            self.get_entry_mut(i).unwrap().set_valid();
            i = self.len()
        }
    }

    /// Populates the `Table` with given [`GateDescriptor`] to reach 256 entries (required)
    pub fn populate(&mut self, default: GateDescriptor) {
        let mut i = self.len();
        while i < 256 {
            self.add_gate(&default);
            i = self.len()
        }
    }
    /// Returns number of entries in this IDT. Actual memory can
    /// be computed by multiplying it by 8 (8 bytes per entry)
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Adds a [GateDescriptor] to the [Table]
    pub fn add_gate(&mut self, gate: &GateDescriptor) {
        self.entries.push(gate.clone());
    }
}

#[repr(C, packed)]
#[bitfield]
#[derive(BitfieldSpecifier)]
/// The `SegmentSelector` describes the segment where the Routine is stored.
/// It has the following structure :
///  _____________________________________________________
/// | 15                                   3 |  2 |  1  0 |
///  -----------------------------------------------------
/// |             index << 3                 | TI |  RPL  |
///  -----------------------------------------------------
pub struct SegmentSelector {
    rpl: B2,
    ti: B1,
    index: B13,
}

impl SegmentSelector {
    /// Returns the `SegmentSelector` configured to use loaded GDT.
    pub fn with_gdt(mut self) -> Self {
        self.set_ti(0);
        self
    }

    /// Returns the `SegmentSelector` configured with the given privileges.
    pub fn with_privilege(mut self, ring: u8) -> Self {
        self.set_rpl(ring);
        self
    }

    /// Returns the [`SegmentSelector`] configured with the given index in the GDT.
    /// _Note that the offset should be given in bytes, and is always > 8_
    pub fn with_segment_index(mut self, index: u16) -> Self {
        self.set_index(index >> 3);
        self
    }
}

#[repr(C, packed)]
#[bitfield]
#[derive(Clone, Copy)]
pub struct GateDescriptor {
    /// Low offset of the ISR
    pub low_offset: u16,
    selector: SegmentSelector,
    reserved: B8,
    gate_type: B4,
    _space: B1,
    dpl: B2,
    p: B1,
    /// High offset of the ISR
    high_offset: u16,
}

impl GateDescriptor {
    /// Set ISR's offset
    pub fn set_offset(&mut self, offset: u32) {
        let bytes = offset.to_le_bytes();
        self.set_low_offset(
            (*bytes.get(0).unwrap() as u16) + ((*bytes.get(1).unwrap() as u16) << 8),
        );
        self.set_high_offset(
            (*bytes.get(2).unwrap() as u16) + ((*bytes.get(3).unwrap() as u16) << 8),
        );
    }

    /// Set gate type
    pub fn set_type(&mut self, gt: GateType) {
        self.set_gate_type(gt as u8);
    }

    /// Set p bit to 1. One must always call `set_valid()` on a [`GateDescriptor`]
    pub fn set_valid(&mut self) {
        self.set_p(1)
    }

    /// Set [`SegmentSelector`]
    pub fn set_segment_selector(&mut self, s: SegmentSelector) {
        self.set_selector(s)
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
/// The IDT Register contains info needed to load the IDT
/// Its structure is the following :
///  _____________________________________________________
/// | 48                                 16 | 15        0 |
///  -----------------------------------------------------
/// |             Offset - 1                |     Size    |
///  -----------------------------------------------------
pub struct IDTDescriptor {
    size: u16,
    offset: u32,
}

impl IDTDescriptor {
    /// Creates a new [`IDTDescriptor`] and set its size to 256 * 8 - 1
    pub fn new() -> Self {
        Self {
            size: 256 * 8 - 1,
            offset: 0x00,
        }
    }

    /// Set offset of the [`IDTDescriptor`]
    pub fn set_offset(&mut self, offset: u32) {
        self.offset = offset
    }

    /// Stores the [`IDTDescriptor`] to a given location in memory
    pub fn store(&self, offset: usize) {
        let ptr = offset as *mut IDTDescriptor;
        unsafe { write_volatile(ptr, self.clone()) }
    }
}

/// Loads [`IDTDescriptor`] in the CPU IDTR using `lidt` instruction
pub fn load_idt(ptr: usize) {
    disable_interrupts();
    unsafe {
        asm!(
        "lidt [{0}]",
        in(reg) ptr
        )
    }
    enable_interrupts();
}
