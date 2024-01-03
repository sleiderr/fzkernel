//! `MP Configuration Table` data structures implementation.
//!
//! These tables let an operating system access information about the multiprocessor configuration of the computer.
//! The BIOS takes care of setting them up after boot.
//!
//! This contains MP-tables parsing methods, and associated data structures.
//!
//! Follows the _Intel 1.4 MultiProcessor Specification_

use crate::mem::PhyAddr32;
use crate::x86::apic::local_apic::{DeliveryMode, PinPolarity, ProcLocalApicID, TriggerMode};
use alloc::string::String;
use alloc::vec::Vec;
use bytemuck::{bytes_of, from_bytes, try_from_bytes, Pod, Zeroable};
use core::fmt::{Debug, Formatter};
use core::mem::size_of;
use core::slice;
use modular_bitfield::prelude::{B22, B24, B4, B6, B7};
use modular_bitfield::{bitfield, BitfieldSpecifier};
use pod_enum::pod_enum;

/// Header of the `MP Configuration Table` ([`MPTable`]).
///
/// Contains basic information, such as the number of entries in the table, its length, version numbers and
/// OEM-related information.
///
/// Contains the checksum of the entire [`MPTable`]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
#[repr(C)]
pub(super) struct MPConfigurationTableHeader {
    signature: [u8; 4],
    base_table_length: u16,
    spec_rev: u8,
    chksum: u8,
    oem_id: [u8; 8],
    product_id: [u8; 12],
    pub(super) oem_table_ptr: PhyAddr32,
    oem_table_size: u16,
    entry_count: u16,
    pub(crate) local_apic_addr: PhyAddr32,
    ext_table_len: u16,
    ext_table_chksum: u8,
    reserved: u8,
}

/// Different types of entries that may appear in the `MP Configuration Table` ([`MPTable`]).
#[derive(Debug)]
pub(crate) enum MPConfigurationEntry {
    Processor(MPProcessorEntry),
    Bus(MPBusEntry),
    IOApic(MPIOApicEntry),
    IOInterrupt(MPIOInterruptEntry),
    LocalInterrupt(MPLocalInterruptEntry),
}

/// ID associated with every type of entry that may appear in the _MP Configuration Table_ ([`MPTable`]).
#[pod_enum]
#[repr(u8)]
pub(crate) enum MPConfigurationEntryType {
    Processor = 0,
    Bus = 1,
    IOApic = 2,
    IOInterrupt = 3,
    LocalInterrupt = 4,
}

/// Processor Entry in the _MP Configuration Table_ ([`MPTable`]).
///
/// One entry per processor. These entries are filled by the _BIOS_ (which issues `CPUID` instructions to every
/// processor available on the system).
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
#[repr(C)]
pub(crate) struct MPProcessorEntry {
    entry_type: MPConfigurationEntryType,

    /// Local APIC ID for this specific processor. Unique for a given system (each CPU is assigned a different ID).
    pub(crate) lapic_id: ProcLocalApicID,
    lapic_version: u8,

    /// CPU Flags:
    ///
    /// - ENABLE: clear if the OS cannot use this processor
    /// - BP: set if this processor is the bootstrap processor
    pub(crate) flags: MPProcessorEntryFlags,
    signature: u8,
    family: u8,
    reserved1: u16,

    /// Feature flags for this processor, as return by the `CPUID` instruction.
    pub(crate) feature_flags: MPProcessorEntryFeatureFlags,
    reserved2: u64,
}

/// CPU Flags, used in the [`MPProcessorEntry`] structure.
#[bitfield]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(C)]
pub(crate) struct MPProcessorEntryFlags {
    /// _ENABLE_ flag: clear if the OS cannot use this processor.
    pub(crate) usable: bool,

    /// _BSP_ flag: set if this processor is the bootstrap processor.
    pub(crate) is_bsp: bool,
    #[skip]
    __: B6,
}

/// Feature flags for this CPU, used in the [`MPProcessorEntry`] structure.
#[bitfield]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(C)]
pub(crate) struct MPProcessorEntryFeatureFlags {
    /// Floating Point Unit.
    ///
    /// If set, the processor contains an FPU that supports the _Intel387_ processor floating point
    /// instruction set.
    pub(crate) has_fpu: bool,
    #[skip]
    __: B6,
    /// Machine Check Exception.
    ///
    /// If set, Exception 18 is defined for machine checks, including _CRC4.MCE_ to control the feature.
    pub(crate) has_mce: bool,

    /// CMPXCHG8B instruction support.
    ///
    /// If set, the 8 bytes compare-and-exchange instruction is supported by this processor.
    pub(crate) cmpxchg64_support: bool,

    /// On-chip APIC.
    ///
    /// If set, the processor comes with an integrated _APIC_, which both present and hardware enabled.
    pub(crate) has_integrated_apic: bool,
    #[skip]
    __: B22,
}

/// Bus entry in the _MP Configuration Table_ ([`MPTable`]).
///
/// One entry per bus. Theses entries identify the different kinds of buses available on the system.
/// Each bus is assigned a unique id, which is used to associate interrupt lines to specific buses.
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C, packed)]
pub(crate) struct MPBusEntry {
    entry_type: MPConfigurationEntryType,

    /// Bus unique identifier. Assigned sequentially by the _BIOS_, starting at zero.
    pub(crate) bus_id: MPBusId,

    /// Bus type identifier. 6-char _ASCII_ string (whitespace filled).
    pub(crate) bus_type: MPBusType,
}

/// System bus unique identifier. Assigned sequentially by the _BIOS_ at boot time, starting at zero.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct MPBusId(u8);

/// Bus type identifier. 6-char _ASCII_ string (whitespace filled).
#[derive(Clone, Copy, Default, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct MPBusType([u8; 6]);

impl MPBusType {
    /// Returns an [`Iterator`] over the characters of the bus type.
    pub(crate) fn chars(self) -> impl Iterator<Item = char> {
        self.0
            .into_iter()
            .filter(|&b| b != 0 && b != b' ')
            .map(char::from)
    }
}

impl From<MPBusType> for String {
    fn from(value: MPBusType) -> Self {
        value.chars().collect::<String>()
    }
}

impl Debug for MPBusType {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_str(&String::from(*self))
    }
}

/// I/O APIC entry in the _MP Configuration Table_ ([`MPTable`]).
///
/// One entry per I/O APIC. These entries give basic information about every I/O APIC available on the system (unique
/// identifier and base address for memory-mapped registers).
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C, packed)]
pub(crate) struct MPIOApicEntry {
    entry_type: MPConfigurationEntryType,

    /// I/O APIC unique identifier.
    pub(crate) ioapic_id: MPIOApicId,
    ioapic_version: u8,
    ioapic_flags: u8,

    /// Base physical address for the memory-mapped registers of this I/O APIC.
    pub(crate) addr: PhyAddr32,
}

/// I/O APIC unique identifier.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct MPIOApicId(pub(super) u8);

impl MPIOApicId {
    /// Indicates that a signal is connected to every I/O APIC available on the system.
    pub(crate) const ALL_IO_APIC: Self = Self(0xFF);
}

impl From<MPIOApicId> for ProcLocalApicID {
    fn from(value: MPIOApicId) -> Self {
        Self::from(value.0)
    }
}

/// Identifier for a _INTIN_ pin on the I/O APIC.
///
/// Used to link IRQs to physical pin on the chip.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct IOApicIntPin(pub(super) u8);

impl From<IOApicIntPin> for u8 {
    fn from(value: IOApicIntPin) -> Self {
        value.0
    }
}

impl From<u8> for IOApicIntPin {
    fn from(value: u8) -> Self {
        Self(value)
    }
}

/// Identifier for a _INTIN_ pin on the Local APIC.
///
/// Useful to know to which pin a bus signal is connected to.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct MPLocalApicIntPin(pub(super) u8);

impl MPLocalApicIntPin {
    /// _LINTIN0_ pin.
    pub(crate) const LINTIN_0: Self = Self(0);

    /// _LINTIN1_ pin.
    pub(crate) const LINTIN_1: Self = Self(1);
}

impl MPIOApicEntry {
    /// If clear, the I/O APIC is not usable, and the operating system should not attempt to access it.
    pub(crate) fn usable(self) -> bool {
        self.ioapic_flags & 0b1 == 1
    }
}

/// I/O Interrupt Assignment entry in the _MP Configuration Table_ ([`MPTable`]).
///
/// These entries indicate which interrupt source is connected to each I/O APIC interrupt input.
/// There is one entry for each I/O APIC interrupt that is connected.
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C, packed)]
pub(crate) struct MPIOInterruptEntry {
    entry_type: MPConfigurationEntryType,

    /// Interrupt type (_INT_, _NMI_, _SMI_, external).
    pub(crate) int_type: MPInterruptType,

    pub(crate) int_mode: MPInterruptFlags,
    reserved: u8,

    /// Bus ID ([`MPBusId`]) from which the interrupt signal comes from.
    pub(crate) source_bus_id: MPBusId,

    /// Identifier the interrupt signal from the source bus.
    pub(crate) source_bus_irq: MPBusIrq,

    /// I/O APIC ID ([`MPIOApicId`]) to which the signal is connected.
    ///
    /// The signal may be connected to every I/O APIC ([`MPIOApicId::ALL_IO_APIC`]).
    pub(crate) dest_ioapic_id: MPIOApicId,

    /// Identifies the _INTINn_ pin to which the signal is connected.
    pub(crate) dest_ioapic_intin: IOApicIntPin,
}

/// Used to identify the interrupt signal from the source bus.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct MPBusIrq(u8);

/// Every available interrupt type.
#[pod_enum]
#[repr(u8)]
pub(crate) enum MPInterruptType {
    /// Signal is a vectored interrupt, the vector is supplied by the APIC redirection table.
    Vectored = 0,

    /// Signal is a non-maskable interrupt.
    NonMaskable = 1,

    /// Signal is a system management interrupt.
    SystemManagement = 2,

    /// Signal is a vectored interrupt, the vector is supplied by the external PIC.
    External = 3,
}

impl From<MPInterruptType> for DeliveryMode {
    fn from(value: MPInterruptType) -> Self {
        match value {
            MPInterruptType::NonMaskable => DeliveryMode::NonMaskableInterrupt,
            MPInterruptType::SystemManagement => DeliveryMode::SystemManagementInterrupt,
            MPInterruptType::External => DeliveryMode::ExternalInterrupt,
            _ => DeliveryMode::Fixed,
        }
    }
}

/// Used to specify the polarity of the corresponding interrupt pin.
#[derive(BitfieldSpecifier, Clone, Copy, Debug)]
#[bits = 2]
#[repr(C)]
pub(crate) enum MPPinPolarity {
    BusSpec = 0,
    ActiveHigh = 1,
    ActiveLow = 3,
}

impl From<MPPinPolarity> for PinPolarity {
    fn from(value: MPPinPolarity) -> Self {
        match value {
            MPPinPolarity::ActiveLow => Self::ActiveLow,
            // should depend on the bus
            _ => Self::ActiveHigh,
        }
    }
}

/// Used to select the trigger mode for the corresponding interrupt (edge sensitive or level sensitive).
#[derive(BitfieldSpecifier)]
#[bits = 2]
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub(crate) enum MPTriggerMode {
    BusSpec = 0,
    Edge = 1,
    Level = 3,
}

impl From<MPTriggerMode> for TriggerMode {
    fn from(value: MPTriggerMode) -> Self {
        match value {
            MPTriggerMode::Level => Self::Level,
            _ => Self::Edge,
        }
    }
}

/// Interrupt mode for an int entry in the `MPTable`.
#[bitfield]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub(crate) struct MPInterruptFlags {
    pub(crate) polarity: MPPinPolarity,
    pub(crate) trigger_mode: MPTriggerMode,
    #[skip]
    __: B4,
}

/// Local Interrupt Assignment entry in the _MP Configuration Table_ ([`MPTable`]).
///
/// These entries indicate which interrupt source is connected to each local APIC interrupt input.
/// There is one entry for each local APIC interrupt that is connected.
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C, packed)]
pub(crate) struct MPLocalInterruptEntry {
    entry_type: MPConfigurationEntryType,

    /// Interrupt type (_INT_, _NMI_, _SMI_, external).
    pub(crate) int_type: MPInterruptType,
    pub(crate) int_mode: MPInterruptFlags,
    reserved: u8,

    /// Bus ID ([`MPBusId`]) from which the interrupt signal comes from.
    pub(crate) source_bus_id: MPBusId,

    /// Identifies the interrupt signal from the source bus.
    pub(crate) source_bus_irq: MPBusIrq,

    /// Local APIC ID ([`ProcLocalApicId`]) to which the signal is connected.
    ///
    /// The signal may be connected to every I/O APIC ([`ProcLocalApicID::ALL_LAPIC`]).
    pub(crate) dest_lapic_id: ProcLocalApicID,

    /// Identifies the _LINTINn_ pin to which the signal is connected.
    pub(crate) dest_lapic_lintin: MPLocalApicIntPin,
}

/// _MP Configuration Table_ main data structure.
///
/// It contains information about various devices or chips:
///
/// - _APICs_ ([`MPIOApicEntry`], [`MPLocalInterruptEntry`])
/// - Processors ([`MPProcessorEntry`])
/// - Buses ([`MPBusEntry`])
/// - Interrupts ([`MPIOInterruptEntry`], [`MPLocalInterruptEntry`])
///
/// These data structures are set up by the _BIOS_ at boot time and kept in memory
/// (_EBDA_, _BIOS_ read-only memory, ...)
#[derive(Debug)]
pub(crate) struct MPTable {
    floating_ptr: MPFloatingPointer,
    header: MPConfigurationTableHeader,
    pub entries: Vec<MPConfigurationEntry>,
}

impl MPTable {
    /// Loads the _MP Configuration Table_ from memory, using the information contained in the associated
    /// [`MPFloatingPointer`] structure.
    ///
    /// Initializes all entries and verifies the validity of the table.
    pub(crate) fn load() -> Option<Self> {
        let mp_float_ptr = MPFloatingPointer::load()?;
        let mp_table_ptr = mp_float_ptr.mp_table_ptr;

        if mp_table_ptr == PhyAddr32::from(0) {
            return None;
        }

        let mp_table_header: MPConfigurationTableHeader = *from_bytes(unsafe {
            slice::from_raw_parts(
                mp_table_ptr.as_ptr(),
                size_of::<MPConfigurationTableHeader>(),
            )
        });

        let mut curr_addr = mp_table_ptr
            + u32::try_from(size_of::<MPConfigurationTableHeader>()).expect("invalid entry size");

        let mp_entries: Vec<MPConfigurationEntry> = (0..mp_table_header.entry_count)
            .filter_map(|_| load_mp_conf_entry(&mut curr_addr))
            .collect();

        let mp_table = Self {
            floating_ptr: mp_float_ptr,
            header: mp_table_header,
            entries: mp_entries,
        };

        if !mp_table.verify_chksum() {
            return None;
        }

        Some(mp_table)
    }

    /// Returns all `I/O APIC` entries in the `MPTable`.
    pub(crate) fn get_io_apic(&self) -> Vec<MPIOApicEntry> {
        self.entries
            .iter()
            .filter(|entry| {
                if let MPConfigurationEntry::IOApic(io_apic) = entry {
                    return true;
                }

                false
            })
            .map(|entry| {
                if let MPConfigurationEntry::IOApic(io_apic) = entry {
                    return Some(*io_apic);
                }

                None
            })
            .map(Option::unwrap)
            .collect()
    }

    /// Returns all processors entries in the `MPTable`.
    pub(crate) fn get_processors(&self) -> Vec<MPProcessorEntry> {
        self.entries
            .iter()
            .filter(|entry| {
                if let MPConfigurationEntry::Processor(_) = entry {
                    return true;
                }

                false
            })
            .map(|entry| {
                if let MPConfigurationEntry::Processor(proc) = entry {
                    return Some(*proc);
                }

                None
            })
            .map(Option::unwrap)
            .collect()
    }

    /// Returns all Local interrupt that are connected to a given Local APIC.
    ///
    /// The Local APIC is identified by its unique identifier ([`ProcLocalApicID`]).
    pub(crate) fn get_int_to_local_apic(&self, id: ProcLocalApicID) -> Vec<MPLocalInterruptEntry> {
        self.entries
            .iter()
            .filter(|entry| {
                if let MPConfigurationEntry::LocalInterrupt(int) = entry {
                    if int.dest_lapic_id == id {
                        return true;
                    }
                }

                false
            })
            .map(|entry| {
                if let MPConfigurationEntry::LocalInterrupt(int) = entry {
                    return Some(*int);
                }
                None
            })
            .map(Option::unwrap)
            .collect()
    }

    /// Returns all I/O interrupt that are connected to a given I/O APIC.
    ///
    /// The I/O APIC is identified by its unique identifier ([`MPIOApicId`]).
    pub(crate) fn get_int_to_io_apic(&self, id: MPIOApicId) -> Vec<MPIOInterruptEntry> {
        self.entries
            .iter()
            .filter(|entry| {
                if let MPConfigurationEntry::IOInterrupt(int) = entry {
                    if int.dest_ioapic_id == id {
                        return true;
                    }
                }

                false
            })
            .map(|entry| {
                if let MPConfigurationEntry::IOInterrupt(int) = entry {
                    return Some(*int);
                }
                None
            })
            .map(Option::unwrap)
            .collect()
    }

    /// Returns the I/O Interrupt entry connected to a pin of a given I/O APIC.
    ///
    /// The pin is identified by its number (as a [`IOApicIntPin`]), and the I/O APIC by its unique
    /// identifier ([`MPIOApicId`]).
    ///
    /// Returns [`None`] if there is no interrupt connected to that pin.
    pub(crate) fn get_io_int_connected_to_pin(
        &self,
        io_apic_id: MPIOApicId,
        pin: IOApicIntPin,
    ) -> Option<MPIOInterruptEntry> {
        let int = self.entries.iter().find(|entry| {
            if let MPConfigurationEntry::IOInterrupt(int) = entry {
                if int.dest_ioapic_id == io_apic_id && int.dest_ioapic_intin == pin {
                    return true;
                }
            }

            false
        });

        if let Some(MPConfigurationEntry::IOInterrupt(int)) = int {
            Some(*int)
        } else {
            None
        }
    }

    /// Returns the Local Interrupt entry connected to a pin of a given Local APIC.
    ///
    /// The pin is identified by its number (0 or 1, as a [`MPLocalApicIntPin`]), and the local APIC by its unique
    /// identifier ([`ProcLocalApicId`]).
    ///
    /// Returns [`None`] if there is no interrupt connected to that pin.
    pub(crate) fn get_local_int_connected_to_pin(
        &self,
        lapic_id: ProcLocalApicID,
        pin: MPLocalApicIntPin,
    ) -> Option<MPLocalInterruptEntry> {
        let int = self.entries.iter().find(|entry| {
            if let MPConfigurationEntry::LocalInterrupt(int) = entry {
                if (int.dest_lapic_id == lapic_id
                    || int.dest_lapic_id == ProcLocalApicID::ALL_LAPIC)
                    && int.dest_lapic_lintin == pin
                {
                    return true;
                }
            }

            false
        });

        if let Some(MPConfigurationEntry::LocalInterrupt(int)) = int {
            Some(*int)
        } else {
            None
        }
    }

    /// Returns the Processor entry in the _MP Configuration Table_, given its Local APIC ID ([`ProcLocalApicID`]),
    /// or [`None`] if there is no processor with such Local APIC ID.
    pub(crate) fn get_proc_entry_by_id(&self, id: ProcLocalApicID) -> Option<MPProcessorEntry> {
        let entry = self.entries.iter().find(|elem| {
            if let MPConfigurationEntry::Processor(proc) = elem {
                if proc.lapic_id == id {
                    return true;
                }
                return false;
            }

            false
        });

        if let Some(MPConfigurationEntry::Processor(proc)) = entry {
            Some(*proc)
        } else {
            None
        }
    }

    /// Returns the Bus entry in the _MP Configuration Table_, given its ID ([`MPBusId`]), or [`None`] if
    /// there is no Bus with such ID.
    pub(crate) fn get_bus_entry_by_id(&self, id: MPBusId) -> Option<MPBusEntry> {
        let entry = self.entries.iter().find(|elem| {
            if let MPConfigurationEntry::Bus(bus) = elem {
                if bus.bus_id == id {
                    return true;
                }
                return false;
            }

            false
        });

        if let MPConfigurationEntry::Bus(bus) = entry.unwrap() {
            Some(*bus)
        } else {
            None
        }
    }

    /// Returns the I/O APIC entry in the _MP Configuration Table_, given its ID ([`MPIOApicId`]), or [`None`] if
    /// there is no I/O APIC with such ID.
    pub(crate) fn get_io_apic_entry_by_id(&self, id: MPIOApicId) -> Option<MPIOApicEntry> {
        let entry = self.entries.iter().find(|elem| {
            if let MPConfigurationEntry::IOApic(io_apic) = elem {
                if io_apic.ioapic_id == id {
                    return true;
                }
                return false;
            }

            false
        });

        if let MPConfigurationEntry::IOApic(io_apic) = entry.unwrap() {
            Some(*io_apic)
        } else {
            None
        }
    }

    pub(super) fn imcr_present(&self) -> bool {
        self.floating_ptr.feature_information.imcr_presence()
    }

    /// Verifies that the structure's checksum (sum of all bits being null) is valid.
    fn verify_chksum(&self) -> bool {
        let table_length = self.header.base_table_length;
        let table_addr = self.floating_ptr.mp_table_ptr;
        let table_bytes: &[u8] =
            unsafe { slice::from_raw_parts(table_addr.as_ptr(), usize::from(table_length)) };

        let mut chksum = 0;

        for &b in table_bytes {
            chksum += b;
        }

        chksum == 0
    }
}

/// The _MP Floating Pointer Structure_ contains basic information about the _MP Configuration Table_ ([`MPTable`]).
///
/// Contains a pointer to the configuration table, as well as MP feature information (_IMCR_ presence, ...).
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
#[repr(C, packed)]
struct MPFloatingPointer {
    signature: [u8; 4],
    mp_table_ptr: PhyAddr32,
    length: u8,
    spec_rev: u8,
    chksum: u8,
    feature_information: MPFeatureInformation,
}

impl MPFloatingPointer {
    /// Locates the `MP Floating Pointer Structure` in memory, by locating its signature _"\_MP\_"_
    fn load() -> Option<Self> {
        let mp_float_ptr: Self = locate_struct_in_mem(
            "_MP_".as_bytes(),
            &[
                (PhyAddr32::from(0xF0000), PhyAddr32::from(0x100_000)),
                (PhyAddr32::from(0x80000), PhyAddr32::from(0x9FFFF)),
            ],
            16,
        )?;

        if mp_float_ptr.verify_chksum() {
            Some(mp_float_ptr)
        } else {
            None
        }
    }

    /// Verifies that the structure's checksum (sum of all bits being null) is valid.
    fn verify_chksum(&self) -> bool {
        let struct_bytes = bytes_of(self);
        let mut bytes_sum = 0;

        for &b in struct_bytes {
            bytes_sum += b;
        }

        bytes_sum == 0
    }
}

/// _MP Floating Pointer Structure_ feature information field.
///
/// Specifies various information about the [`MPTable`]: default configuration type (if applicable), _IMCR_ presence
#[bitfield]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(C)]
pub(super) struct MPFeatureInformation {
    /// _MP_ System Configuration Type.
    ///
    /// When this byte is null, the _MP Configuration Table_ is present.
    /// Otherwise, the value contained in that field indicates which default configuration the system implements.
    config_type: u8,
    #[skip]
    __: B7,

    /// _IMCR_ presence bit.
    ///
    /// When this bit is set, the `IMCR` (_Interrupt Mode Control Register_) is present and PIC mode is implemented.
    /// Otherwise, _Virtual Wire Mode_ is implemented.
    imcr_presence: bool,
    #[skip]
    __: B24,
}

/// Loads a `MP Configuration Table` ([`MPTable`]) from a memory address.
fn load_mp_conf_entry(addr: &mut PhyAddr32) -> Option<MPConfigurationEntry> {
    let entry_type: MPConfigurationEntryType = unsafe { *addr.as_ptr() };

    return match entry_type {
        MPConfigurationEntryType::Processor => {
            let entry_bytes: &[u8] =
                unsafe { slice::from_raw_parts(addr.as_ptr(), size_of::<MPProcessorEntry>()) };
            *addr += u32::try_from(entry_bytes.len()).expect("invalid entry size");

            Some(MPConfigurationEntry::Processor(*from_bytes(entry_bytes)))
        }

        MPConfigurationEntryType::Bus => {
            let entry_bytes: &[u8] =
                unsafe { slice::from_raw_parts(addr.as_ptr(), size_of::<MPBusEntry>()) };
            *addr += u32::try_from(entry_bytes.len()).expect("invalid entry size");

            Some(MPConfigurationEntry::Bus(*from_bytes(entry_bytes)))
        }

        MPConfigurationEntryType::IOApic => {
            let entry_bytes: &[u8] =
                unsafe { slice::from_raw_parts(addr.as_ptr(), size_of::<MPIOApicEntry>()) };
            *addr += u32::try_from(entry_bytes.len()).expect("invalid entry size");

            Some(MPConfigurationEntry::IOApic(*from_bytes(entry_bytes)))
        }

        MPConfigurationEntryType::IOInterrupt => {
            let entry_bytes: &[u8] =
                unsafe { slice::from_raw_parts(addr.as_ptr(), size_of::<MPIOInterruptEntry>()) };
            *addr += u32::try_from(entry_bytes.len()).expect("invalid entry size");

            Some(MPConfigurationEntry::IOInterrupt(*from_bytes(entry_bytes)))
        }

        MPConfigurationEntryType::LocalInterrupt => {
            let entry_bytes: &[u8] =
                unsafe { slice::from_raw_parts(addr.as_ptr(), size_of::<MPLocalInterruptEntry>()) };
            *addr += u32::try_from(entry_bytes.len()).expect("invalid entry size");

            Some(MPConfigurationEntry::LocalInterrupt(*from_bytes(
                entry_bytes,
            )))
        }

        _ => None,
    };
}

/// Tries to locate a data structure in memory, identified by its signature.
///
/// Only searches in the `ranges` area of memory.
/// The data structure is assumed to begin with the signature (which can be of any size).
///
/// The structure alignment has to be given, and ranges have to start with an address that is aligned
/// with the structure alignment.
pub(crate) fn locate_struct_in_mem<T: Pod + Zeroable>(
    sig: &[u8],
    ranges: &[(PhyAddr32, PhyAddr32)],
    align: u32,
) -> Option<T> {
    for range in ranges {
        let mut curr_addr = range.0;

        while curr_addr < range.1 {
            let entry: &[u8] = unsafe { slice::from_raw_parts(curr_addr.as_ptr(), sig.len()) };
            if entry == sig {
                let entry_bytes: &[u8] =
                    unsafe { slice::from_raw_parts(curr_addr.as_ptr(), size_of::<T>()) };

                return try_from_bytes(entry_bytes).ok().copied();
            }

            curr_addr += align;
        }
    }

    None
}
