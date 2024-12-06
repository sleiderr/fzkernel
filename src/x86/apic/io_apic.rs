//! `I/O APIC` (_I/O Advanced Programmable Interrupt Controller_) implementation.
//!
//! With the [`LocalAPIC`], they are an evolution of the old `PIC` chip. It manages the interrupt issued by I/O devices.
//! It also provides multiprocessor interrupt management through 24 programmable interrupts (_ISA_, _PCI_, ...)

use crate::mem::{LocklessCell, MemoryAddress, PhyAddr32};
use crate::x86::apic::local_apic::{
    DeliveryMode, DeliveryStatus, DestinationMode, InterruptVector, PinPolarity, ProcLocalApicID,
    TriggerMode,
};
use crate::x86::apic::mp_table::{IOApicIntPin, MPIOApicEntry, MPIOInterruptEntry, MPTable};
use alloc::vec::Vec;
use conquer_once::spin::OnceCell;
use hashbrown::HashMap;
use modular_bitfield::bitfield;
use modular_bitfield::prelude::{B24, B39, B4};
use spin::Mutex;

/// Contains all `IOApic` already initialized, identified by its `APIC ID`.
static IO_APIC: OnceCell<LocklessCell<HashMap<ProcLocalApicID, Mutex<IOApic>>>> =
    OnceCell::uninit();

/// Returns an initialized [`IOApic`] given its `APIC` identifier (as a [`ProcLocalApicId`]).
///
/// The returned structure is locked, as it is not `CPU` dependent (contrary to the `LocalAPIC`).
/// It does not initialize the [`IOApic`] if it was not already done.
pub fn get_io_apic(id: ProcLocalApicID) -> Option<&'static Mutex<IOApic>> {
    let io_apics = IO_APIC
        .try_get_or_init(|| LocklessCell::new(HashMap::new()))
        .ok()?
        .get();

    io_apics.get(&id)
}

pub fn get_all_io_apics() -> Option<&'static HashMap<ProcLocalApicID, Mutex<IOApic>>> {
    Some(
        IO_APIC
            .try_get_or_init(|| LocklessCell::new(HashMap::new()))
            .ok()?
            .get(),
    )
}

/// Registers an [`IOApic`] after its initialization.
fn register_io_apic(apic: IOApic) {
    let io_apics = IO_APIC
        .get_or_init(|| LocklessCell::new(HashMap::new()))
        .get();

    io_apics.insert(apic.id, Mutex::new(apic));
}

/// Contains the 4-bit `APIC ID`.
///
/// Each `APIC` devices on the `APIC` bus should have a unique identifier. This register must be configured with the
/// proper ID before using the `I/O APIC`.
#[bitfield]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, PartialOrd, Ord, Hash)]
#[repr(u32)]
struct IOApicId {
    #[skip]
    __: B24,
    io_apic_id: B4,
    #[skip]
    __: B4,
}

impl From<IOApicId> for ProcLocalApicID {
    fn from(value: IOApicId) -> Self {
        Self::from(value.io_apic_id())
    }
}

impl IOApicRegister for IOApicId {
    fn offset() -> u8 {
        IOAPICID_OFFSET
    }
}

/// Used to identify the `APIC` hardware version.
///
/// Also indicates the maximum number of entries in the I/O redirection table.
#[bitfield]
#[repr(u32)]
struct IOApicVersion {
    io_apic_version: u8,
    #[skip]
    __: u8,
    maximum_redirection_entry: u8,
    #[skip]
    __: u8,
}

impl IOApicRegister for IOApicVersion {
    fn offset() -> u8 {
        IOAPICVER_OFFSET
    }
}

/// Contains the bus arbitration priority for the `I/O APIC`.
///
/// A one wire arbitration is used to win `APIC` bus ownership. Each turn, the winner of the arbitration become
/// the lowest priority agent, and takes ID 0. Every other agent increments its ID by 1, expect for the one that
/// had ID 15, which takes the ID of the winner and increments it by one.
#[bitfield]
struct IOApicArbitration {
    #[skip]
    __: B24,
    io_apic_arb_id: B4,
    #[skip]
    __: B4,
}

impl IOApicRegister for IOApicArbitration {
    fn offset() -> u8 {
        IOAPICARB_OFFSET
    }
}

/// Redirection table entry for the `I/O APIC`.
///
/// One entry per interrupt signal. Software determines the vector (and thus the priority) for every interrupt input
/// signal, as well as other properties.
/// The redirection table is used to translate the interrupt input signal into an inter-`APIC` message.
#[bitfield]
#[derive(Copy, Clone, Debug)]
#[repr(u64)]
struct RedTblEntry {
    vector: InterruptVector,
    delivery_mode: DeliveryMode,
    destination_mode: DestinationMode,
    delivery_status: DeliveryStatus,
    pin_polarity: PinPolarity,
    remote_irr: bool,
    trigger_mode: TriggerMode,
    masked: bool,
    #[skip]
    __: B39,
    destination: u8,
}

/// Low 32-bits of a redirect table entry register.
struct RedTblRegisterLo(u8);

/// High 32-bits of a redirect table entry register.
struct RedTblRegisterHi(u8);

/// Redirection table entry register for the `I/O APIC`.
///
/// 64-bits wide register which contains a [`RedTblEntry`]. Must be written using two 32-bits writes.
#[derive(Debug)]
struct RedTblRegister {
    id: u8,
    entry: RedTblEntry,
}

impl RedTblRegister {
    /// Turns this 64-bit entry register into two 32-bits part, as a tuple `(reg_id, data)`.
    fn as_writable(&self) -> ((RedTblRegisterLo, u32), (RedTblRegisterHi, u32)) {
        (
            (
                RedTblRegisterLo(self.id),
                u32::try_from(u64::from(self.entry) & 0xffff_ffff).expect("infaillible conversion"),
            ),
            (
                RedTblRegisterHi(self.id),
                u32::try_from((u64::from(self.entry) >> 32) & 0xffff_ffff)
                    .expect("infaillible conversion"),
            ),
        )
    }
}

/// `I/O APIC` (_I/O Advanced Programmable Interrupt Controller_) implementation main structure.
///
/// It manages the interrupt issued by I/O devices.
/// It also provides multiprocessor interrupt management through 24 programmable interrupts (_ISA_, _PCI_, ...)
pub(crate) struct IOApic {
    id: ProcLocalApicID,
    base_addr: PhyAddr32,
    ioregsel: MMIOApicRegister,
    iowin: MMIOApicRegister,
    mp_int_entries: Vec<MPIOInterruptEntry>,
    i8259_pin: Option<IOApicIntPin>,
}

impl IOApic {
    pub(crate) fn init(entry: MPIOApicEntry, table: &MPTable) {
        let mut io_apic = Self {
            id: entry.ioapic_id.into(),
            base_addr: entry.addr,
            ioregsel: MMIOApicRegister(entry.addr),
            iowin: MMIOApicRegister(entry.addr + 0x10_usize),
            mp_int_entries: table.get_int_to_io_apic(entry.ioapic_id),
            i8259_pin: None,
        };

        io_apic.i8259_pin = io_apic.find_i8259_pin();
        io_apic.check_ioapic_id(table);
        io_apic.initialize_redtbl();
        io_apic.unmask_all();

        register_io_apic(io_apic);
    }

    /// Reads the content of an `IOApic` register.
    ///
    /// Reads / writes using the memory mapped `IOApic` registers (`IOREGSEL` and `IOWIN`).
    #[must_use]
    fn read_register<R: IOApicRegister + From<u32>>(&self) -> R {
        self.ioregsel.write(u32::from(R::offset()));
        self.iowin.read().into()
    }

    /// Writes the content of an `IOApic` register.
    ///
    /// Reads / writes using the memory mapped `IOApic` registers (`IOREGSEL` and `IOWIN`).
    fn write_register<R: IOApicRegister + Into<u32>>(&self, reg: R) {
        self.ioregsel.write(u32::from(R::offset()));
        self.iowin.write(reg.into());
    }

    /// Clears the redirection configuration for a pin of the `IOApic`.
    ///
    /// Resets the pin entry in the `Redirection Table` to default, and masks irq issued to that pin.
    /// _SMI_ pins cannot be cleared.
    pub(crate) fn clear_pin(&self, pin: IOApicIntPin) {
        let mut entry = self.read_redirection_entry(pin);

        if matches!(
            entry.entry.delivery_mode(),
            DeliveryMode::SystemManagementInterrupt
        ) {
            return;
        }

        if !entry.entry.masked() {
            entry.entry.set_masked(true);
            self.write_redirection_entry(&entry);
        }

        self.write_redirection_entry(&RedTblRegister {
            id: u8::from(pin),
            entry: RedTblEntry::new().with_masked(true),
        })
    }

    /// Masks all pins on the `I/O APIC`.
    ///
    /// Interrupts signaled on a masked interrupt pin are ignored (not delivered, or held pending). Level sensitive
    /// interrupts on a masked pin have no side-effects.
    pub(crate) fn mask_all(&self) {
        for entry in 0..self
            .read_register::<IOApicVersion>()
            .maximum_redirection_entry()
        {
            self.mask_pin_irq(IOApicIntPin::from(entry));
        }
    }

    /// Unmasks all pins on the `I/O APIC`.
    ///
    /// Interrupts signaled on a masked interrupt pin are ignored (not delivered, or held pending). Level sensitive
    /// interrupts on a masked pin have no side-effects.
    pub(crate) fn unmask_all(&self) {
        for entry in 0..self
            .read_register::<IOApicVersion>()
            .maximum_redirection_entry()
        {
            self.unmask_pin_irq(IOApicIntPin::from(entry));
        }
    }

    /// Masks a system interrupt by masking the underlying physical pin on the `I/O APIC`.
    ///
    /// Finds the pin corresponding to the given [`InterruptVector`], and masks it.
    pub(crate) fn mask_irq(&self, irq: InterruptVector) {
        if let Some(pin) = self.get_pin_from_irq(irq) {
            self.mask_pin_irq(pin);
        }
    }

    /// Unmasks a system interrupt by unmasking the underlying physical pin on the `I/O APIC`.
    ///
    /// Finds the pin corresponding to the given [`InterruptVector`], and unmasks it.
    pub(crate) fn unmask_irq(&self, irq: InterruptVector) {
        if let Some(pin) = self.get_pin_from_irq(irq) {
            self.unmask_pin_irq(pin);
        }
    }

    /// Masks a pin on the `I/O APIC`.
    ///
    /// Interrupts signaled on a masked interrupt pin are ignored (not delivered, or held pending). Level sensitive
    /// interrupts on a masked pin have no side-effects.
    pub(crate) fn mask_pin_irq(&self, pin: IOApicIntPin) {
        let mut entry = self.read_redirection_entry(pin);
        entry.entry.set_masked(true);
        self.write_redirection_entry(&entry);
    }

    /// Unmasks a pin on the `I/O APIC`.
    ///
    /// Interrupts signaled on a masked interrupt pin are ignored (not delivered, or held pending). Level sensitive
    /// interrupts on a masked pin have no side-effects.
    pub(crate) fn unmask_pin_irq(&self, pin: IOApicIntPin) {
        let mut entry = self.read_redirection_entry(pin);
        entry.entry.set_masked(false);
        self.write_redirection_entry(&entry);
    }

    /// Redirects an input pin on the `I/O APIC` to a given `IRQ`.
    ///
    /// When an interrupt is issued on the input pin, it will dispatch a interrupt to the `Local APIC` of the _BSP_,
    /// which the vector specified in the redirection entry.
    ///
    /// Interrupts type ([`PinPolarity`], [`TriggerMode`], ...) match the one contained in the _MP Table_ if
    /// available, or fallbacks to a default entry (Edge triggered, active high).
    pub(crate) fn map_pin_to_irq(&self, pin: IOApicIntPin, vector: InterruptVector) {
        let int_entry = self
            .mp_int_entries
            .iter()
            .find(|entry| entry.dest_ioapic_intin == pin);

        if let Some(int_entry) = int_entry {
            self.write_redirection_entry(&RedTblRegister {
                id: u8::from(pin),
                entry: RedTblEntry::new()
                    .with_vector(vector)
                    .with_delivery_mode(DeliveryMode::from(int_entry.int_type))
                    .with_pin_polarity(PinPolarity::from(int_entry.int_mode.polarity()))
                    .with_trigger_mode(TriggerMode::from(int_entry.int_mode.trigger_mode()))
                    .with_destination_mode(DestinationMode::Physical),
            })
        } else {
            self.write_redirection_entry(&RedTblRegister {
                id: u8::from(pin),
                entry: RedTblEntry::new()
                    .with_vector(vector)
                    .with_destination_mode(DestinationMode::Physical)
                    .with_trigger_mode(TriggerMode::Edge)
                    .with_pin_polarity(PinPolarity::ActiveHigh)
                    .with_delivery_mode(DeliveryMode::Fixed),
            })
        }
    }

    /// Returns the pin of the `I/O APIC` redirected to a given `IRQ`, if it exists.
    pub(crate) fn get_pin_from_irq(&self, irq: InterruptVector) -> Option<IOApicIntPin> {
        for pin in 0..self
            .read_register::<IOApicVersion>()
            .maximum_redirection_entry()
        {
            let entry = self.read_redirection_entry(IOApicIntPin(pin));

            if entry.entry.destination() == u8::from(irq) {
                return Some(IOApicIntPin(pin));
            }
        }

        None
    }

    /// Restores the `Virtual Wire Mode` configuration for the `I/O APIC`.
    ///
    /// The `I/O APIC` is now used as a simple wire, transmitting every interrupt raised by the _Intel 8259_ (`PIC`)
    /// chip to the `Local APIC` of the current processor (should be the BSP).
    /// If no `PIC` chip is connected to this `I/O APIC`, it does nothing. Otherwise, it configures the pin to which
    /// it is connected to deliver external interrupts.
    pub(crate) fn restore_native_virtual_wire(&self) {
        if let Some(pin) = &self.i8259_pin {
            self.write_redirection_entry(&RedTblRegister {
                id: u8::from(*pin),
                entry: RedTblEntry::new()
                    .with_delivery_mode(DeliveryMode::ExternalInterrupt)
                    .with_destination_mode(DestinationMode::Physical)
                    .with_destination(u8::from(ProcLocalApicID::get())),
            })
        }
    }

    /// Attempts to find the pin to which the _Intel 8259_ (`PIC`) chip is connected.
    ///
    /// Useful to restore virtual mode later on.
    fn find_i8259_pin(&self) -> Option<IOApicIntPin> {
        for pin in 0..self
            .read_register::<IOApicVersion>()
            .maximum_redirection_entry()
        {
            let entry = self.read_redirection_entry(IOApicIntPin(pin));

            if !entry.entry.masked()
                && matches!(entry.entry.delivery_mode(), DeliveryMode::ExternalInterrupt)
            {
                return Some(IOApicIntPin(pin));
            }
        }

        None
    }

    /// Initializes the redirection table of this `I/O APIC`.
    ///
    /// Maps all pins to system IRQs, using the pin number + 32 (as the first 32 IRQs are reserved on _Intel_
    /// platforms).
    /// The default `PIT` overrides is implemented, and pin 2 is redirected to IRQ 32, instead of 34.
    fn initialize_redtbl(&self) {
        for entry in 1..=self
            .read_register::<IOApicVersion>()
            .maximum_redirection_entry()
        {
            if entry == 2 {
                self.map_pin_to_irq(IOApicIntPin::from(entry), InterruptVector::from(0x20));
                continue;
            }

            self.map_pin_to_irq(
                IOApicIntPin::from(entry),
                InterruptVector::from(entry + 0x20),
            );
        }
    }

    /// Checks if the `APIC ID` of this `I/O APIC` is valid, and updates it if necessary.
    ///
    /// Each `APIC`-related device communicating on the `APIC Bus` must have a unique identifier ([`ProcLocalApicID`]).
    /// `I/O APIC` id must be determined after having assigned an ID to each `Local APIC` on the system.
    fn check_ioapic_id(&mut self, table: &MPTable) {
        let mut valid_id = true;
        for proc in table.get_processors() {
            if proc.lapic_id == self.id {
                self.id += 1;
                valid_id = false;
                break;
            }
        }

        if !valid_id {
            self.check_ioapic_id(table);
        }

        self.write_register(IOApicId::new().with_io_apic_id(u8::from(self.id)));
    }

    fn write_redirection_entry(&self, entry: &RedTblRegister) {
        self.ioregsel
            .write(u32::from(IOREDTBL_BASE_OFFSET + 2 * entry.id + 1));
        self.iowin.write(
            u32::try_from((u64::from(entry.entry) >> 32) & 0xffff_ffff)
                .expect("infaillible conversion"),
        );

        self.ioregsel
            .write(u32::from(IOREDTBL_BASE_OFFSET + 2 * entry.id));
        self.iowin.write(
            u32::try_from(u64::from(entry.entry) & 0xffff_ffff).expect("infaillible conversion"),
        );
    }

    fn read_redirection_entry(&self, pin: IOApicIntPin) -> RedTblRegister {
        self.ioregsel
            .write(u32::from(IOREDTBL_BASE_OFFSET + 2 * u8::from(pin)));
        let entry_lo = self.iowin.read();

        self.ioregsel
            .write(u32::from(IOREDTBL_BASE_OFFSET + 2 * u8::from(pin) + 1));
        let entry_hi = self.iowin.read();

        let entry = u64::from(entry_lo) | (u64::from(entry_hi) << 32);

        RedTblRegister {
            id: u8::from(pin),
            entry: RedTblEntry::from(entry),
        }
    }
}

/// Memory mapped registers to access `I/O APIC` registers.
#[derive(Copy, Clone)]
struct MMIOApicRegister(PhyAddr32);

impl MMIOApicRegister {
    /// Reads the content of the register, using a 32-bit standard read.
    fn read(&self) -> u32 {
        unsafe { core::ptr::read_volatile(self.0.as_ptr()) }
    }

    /// Writes to the register, using a 32-bit standard write.
    ///
    /// Performs two dummy reads to avoid weird bugs on some platforms.
    fn write(self, data: u32) {
        self.read();
        unsafe {
            core::ptr::write_volatile(self.0.as_ptr::<u32>() as *mut u32, data);
        }
        self.read();
    }
}

/// Memory indexed registers for the `I/O APIC`.
///
/// To access theses registers, you have to select the right one using the `IOREGSEL` register.
/// Read/write can then be performed through the `IOWIN` register.
trait IOApicRegister {
    fn offset() -> u8;
}

/// Index of the `IOAPICID` register.
const IOAPICID_OFFSET: u8 = 0;

/// Index of the `IOAPICVER` register.
const IOAPICVER_OFFSET: u8 = 1;

/// Index of the `IOAPICARB` register.
const IOAPICARB_OFFSET: u8 = 2;

/// Index of the `IOREDTBL_BASE` register.
const IOREDTBL_BASE_OFFSET: u8 = 0x10;
