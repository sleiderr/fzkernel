//! `Local APIC` (_Local Advanced Programmable Interrupt Controller_) implementation.
//!
//! The _Local APIC_ manages external interrupts for a specific CPU, and are able to accept and generate
//! _IPIs_ (Interprocessor interrupts).
//!
//! There is one _Local APIC_ per CPU, each is assigned a ID unique for a given system.

#![allow(clippy::as_conversions)]

use crate::io::{outb, IOPort};
use crate::mem::{LocklessCell, PhyAddr32};
use crate::x86::apic::io_apic::IOApic;
use crate::x86::apic::mp_table::{MPInterruptType, MPLocalApicIntPin, MPTable};
use crate::x86::cpuid::cpu_id;
use crate::x86::int::{disable_interrupts, enable_interrupts};
use crate::x86::msr::Ia32ApicBase;
use bytemuck::{Contiguous, Pod, Zeroable};
use conquer_once::spin::OnceCell;
use core::ops::Add;
use core::ptr::{read_volatile, write_volatile};
use hashbrown::HashMap;
use modular_bitfield::error::{InvalidBitPattern, OutOfBounds};
use modular_bitfield::prelude::{B1, B13, B15, B19, B2, B24, B3, B36, B4, B7};
use modular_bitfield::{bitfield, BitfieldSpecifier, Specifier};

/// Contains all `LocalAPIC` already initialized.
static LOCAL_APICS: OnceCell<LocklessCell<HashMap<ProcLocalApicID, LocklessCell<LocalAPIC>>>> =
    OnceCell::uninit();

/// Returns the [`LocalAPIC`] associated with the current processor, if available.
///
/// The underlying structure is lock-free, as it can only be accessed by one processor at a time, as this can
/// only return the local apic of the current CPU.
/// Initializes the [`LocalAPIC`] if that was not done already.
#[allow(clippy::missing_panics_doc)]
pub fn local_apic() -> Option<&'static mut LocalAPIC> {
    let apics = LOCAL_APICS
        .try_get_or_init(|| LocklessCell::new(HashMap::new()))
        .ok()?;

    if let Some(lapic) = apics.get().get(&ProcLocalApicID::get()) {
        Some(lapic.get())
    } else {
        apics.get().insert(
            ProcLocalApicID::get(),
            LocklessCell::new(LocalAPIC::init().ok()?),
        );
        Some(apics.get().get(&ProcLocalApicID::get()).unwrap().get())
    }
}

/// Local APIC unique identifier.
///
/// At power up, every `LocalAPIC` on the system is assigned a unique identifier, based on the system topology.
/// This unique identifier is the _local APIC ID_, and is also used as a processor identifier for multi-processor
/// systems.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct ProcLocalApicID(u8);

impl ProcLocalApicID {
    /// This identifier is reserved, and used to indicate every `LocalAPIC` on the system.
    pub(crate) const ALL_LAPIC: Self = Self(0xFF);

    /// Returns the `LocalAPIC` identifier fo the current processor, using the _CPUID_ instruction.
    pub(crate) fn get() -> Self {
        Self(cpu_id(0x1).unwrap()[1].to_le_bytes()[3])
    }
}

impl From<ProcLocalApicID> for u8 {
    fn from(value: ProcLocalApicID) -> Self {
        value.0
    }
}

impl From<u8> for ProcLocalApicID {
    fn from(value: u8) -> Self {
        Self(value)
    }
}

impl core::ops::AddAssign<u8> for ProcLocalApicID {
    fn add_assign(&mut self, rhs: u8) {
        self.0 += rhs;
    }
}

/// Offset of specific registers in the `LocalAPIC` address space.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct LocalAPICRegisterOffset(u32);

impl LocalAPICRegisterOffset {
    const VERSION_REGISTER: Self = Self(0x30);

    const EOI_REGISTER: Self = Self(0xB0);

    const ERROR_REGISTER: Self = Self(0x280);

    const CMCI_REGISTER: Self = Self(0x2F0);

    const ICR_LOW: Self = Self(0x300);

    const ICR_HIGH: Self = Self(0x310);

    const TIMER_REGISTER: Self = Self(0x320);

    const THERMAL_MON_REGISTER: Self = Self(0x330);

    const PERF_COUNT_REGISTER: Self = Self(0x340);

    const LINT0_REGISTER: Self = Self(0x350);

    const LINT1_REGISTER: Self = Self(0x360);

    const LVT_ERR_REGISTER: Self = Self(0x370);

    const SVR: Self = Self(0xF0);
}

impl Add<LocalAPICRegisterOffset> for PhyAddr32 {
    type Output = PhyAddr32;

    fn add(self, rhs: LocalAPICRegisterOffset) -> Self::Output {
        Self::from(u32::from(self).saturating_add(rhs.0))
    }
}

/// `LocalAPIC` version register structure.
///
/// Contains information about the `APIC` version, and the maximum _LVT_ entry available.
#[bitfield]
#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub(crate) struct LocalAPICVersionRegister {
    version: u8,
    #[skip]
    __: u8,
    max_lvt_entry: u8,
    eoi_broadcast_suppr: bool,
    #[skip]
    __: B7,
}

/// `SVR` (_Spurious Vector Register_) structure.
///
/// Indicates the vector number to be delivered to the processor when the `LocalAPIC` generates a spurious vector.
/// Also contains a bit to software enable/disable the `LocalAPIC`.
#[bitfield]
#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub(crate) struct LocalAPICSpuriousVectorRegister {
    vector: InterruptVector,
    soft_enable: bool,
    focus_proc_checking: bool,
    #[skip]
    __: B2,
    eoi_broadcast_suppr: bool,
    #[skip]
    __: B19,
}

#[repr(u8)]
#[derive(BitfieldSpecifier, Debug)]
#[bits = 2]
pub(crate) enum LVTTimerMode {
    OneShot = 0,
    Periodic = 1,
    TSCDeadline = 2,
}

/// Used to specify the type of interrupt to be sent to the processor for an [`ApicLVT`] entry.
#[repr(u8)]
#[derive(BitfieldSpecifier, Copy, Clone, Debug)]
#[bits = 3]
pub(crate) enum DeliveryMode {
    /// Delivers the interrupt specified in the `vector` field.
    Fixed = 0,

    /// Delivers an `SMI` (_System Management Interrupt_) to the processor.
    ///
    /// The `vector` field should be clear for that delivery mode.
    SystemManagementInterrupt = 2,

    /// Delivers an `NMI` (_Non-Maskable Interrupt_) to the processor.
    NonMaskableInterrupt = 4,

    /// Delivers an `INIT` request to the processor.
    ///
    /// The `vector` field should be clear for that delivery mode. Incompatible with certain types of LVT entries.
    Init = 5,

    /// The processor will respond to the interrupt as if it came from an externally connected controller.
    ///
    /// Only one processor on the system should have a LVT entry configured as _ExtINT_.
    /// Incompatible with certain types of LVT entries.
    ExternalInterrupt = 7,
}

/// Indicates the interrupt source delivery status.
#[repr(u8)]
#[derive(BitfieldSpecifier, Copy, Clone, Debug)]
pub(super) enum DeliveryStatus {
    /// No current activity for this interrupt source.
    Idle = 0,

    /// An interrupt from this source has been delivered to the processor core, but not yet accepted.
    SendPending = 1,
}

/// Used to select the trigger mode for the _LINT0_ and _LINT1_ pins (edge sensitive or level sensitive).
///
/// _LINT1_ does not support level sensitive interrupts.
#[repr(u8)]
#[derive(BitfieldSpecifier, Copy, Clone, Debug)]
pub(crate) enum TriggerMode {
    /// Edge-triggered interrupt
    Edge = 0,

    /// Level-triggered interrupt
    Level = 1,
}

/// Used to specify the polarity of the corresponding interrupt pin.
#[repr(u8)]
#[derive(BitfieldSpecifier, Copy, Clone, Debug)]
pub(crate) enum PinPolarity {
    /// Active-high interrupt.
    ActiveHigh = 0,

    /// Active-low interrupt.
    ActiveLow = 1,
}

/// Interrupt vector priority class.
///
/// The interrupt-priority class is contained in the high 4-bits of an interrupt vector, and goes from 1 to 15 (priority
/// class 0 is reserved). Software should not use priority class 1 as well, as interrupt 16-31 are usually reserved.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct VectorPriorityClass(u8);

/// Interrupt vector relative priority.
///
/// Each interrupt-priority class regroups 16 vectors, and the relative priority of each vector in a given class
/// depends on the low 4-bits of the interrupt vector, the higher they are the higher the priority.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct VectorRelativePriority(u8);

/// Interrupt vector.
///
/// Used to identify each interrupt. It is also used as a way to determine priority between different interrupts,
/// through the [`VectorPriorityClass`] and [`VectorRelativePriority`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, PartialOrd, Ord, Hash, Pod, Zeroable)]
#[repr(transparent)]
pub(crate) struct InterruptVector(u8);

impl InterruptVector {
    /// Spurious vector interrupt vector.
    pub(super) const SPURIOUS_VECTOR: Self = Self(0xFF);
}

impl From<u8> for InterruptVector {
    fn from(value: u8) -> Self {
        Self(value)
    }
}

impl From<InterruptVector> for u8 {
    fn from(value: InterruptVector) -> Self {
        value.0
    }
}

impl InterruptVector {
    /// Returns the relative priority of this interrupt vector.
    pub(crate) fn relative_priority(self) -> VectorRelativePriority {
        VectorRelativePriority(self.0 & 0xf)
    }

    /// Sets the relative priority of this interrupt vector.
    pub(crate) fn set_relative_priority(&mut self, priority: VectorRelativePriority) {
        assert!(priority.0 < 16);

        self.0 = (self.priority_class().0 << 4) | priority.0;
    }

    /// Returns the priority class of the interrupt vector.
    pub(crate) fn priority_class(self) -> VectorPriorityClass {
        VectorPriorityClass((self.0 >> 4) & 0xf)
    }

    /// Sets the priority class of the interrupt vector.
    pub(crate) fn set_priority_class(&mut self, priority: VectorPriorityClass) {
        assert_ne!(priority.0, 0);
        assert!(priority.0 < 16);

        self.0 = (priority.0 << 4) | (self.0 & 0xf);
    }
}

impl Specifier for InterruptVector {
    const BITS: usize = 8;
    type Bytes = u8;
    type InOut = Self;

    fn into_bytes(input: Self::InOut) -> Result<Self::Bytes, OutOfBounds> {
        Ok(input.0)
    }

    fn from_bytes(bytes: Self::Bytes) -> Result<Self::InOut, InvalidBitPattern<Self::Bytes>> {
        Ok(Self(bytes))
    }
}

/// `LVTCMCIEntry` structure represents a _CMCI_ entry in the _Local Vector Table_.
///
/// Provides method to load and fill the _LVT CMCI Register_.
///
/// Such entry has the following general structure :
///
/// ```plaintext
/// 31                           17 16 15 14 13 12 11 10      8 7                0
///  - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
/// |xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx|  | x | x | x |  | x |         |      Vector      |
///  - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
///                                 |               |         |
///                                 |               |          - - - - - - - - - Delivery Mode
///                                 |                - - - - - - - - - - - - - - Delivery Status
///                                  - - - - - - - - - - - - - - - - - - - - - Mask
///                                   
///```
///
/// To learn more about how to configure each kind of entry, refer to Intel documentation.
#[bitfield]
#[derive(Clone, Copy, Default, Debug)]
#[repr(u32)]
pub(crate) struct LVTCMCIEntry {
    vector: InterruptVector,
    delivery_mode: DeliveryMode,
    #[skip]
    __: B1,
    delivery_status: DeliveryStatus,
    #[skip]
    __: B3,
    masked: bool,
    #[skip]
    __: B15,
}

#[bitfield]
#[derive(Clone, Copy, Default, Debug)]
#[repr(u32)]
pub(crate) struct LVTTimerEntry {
    vector: InterruptVector,
    delivery_mode: DeliveryMode,
    #[skip]
    __: B1,
    delivery_status: DeliveryStatus,
    #[skip]
    __: B3,
    masked: bool,
    timer_mode: LVTTimerMode,
    #[skip]
    __: B13,
}

/// `LVTThermalMonitorEntry` structure represents a _Thermal Monitor_ entry in the _Local Vector Table_.
///
/// Provides method to load and fill the _LVT Thermal Monitor Register_.
///
/// Such entry has the following general structure :
///
/// ```plaintext
/// 31                           17 16 15 14 13 12 11 10      8 7                0
///  - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
/// |xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx|  | x | x | x |  | x |         |      Vector      |
///  - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
///                                 |               |         |
///                                 |               |          - - - - - - - - - Delivery Mode
///                                 |                - - - - - - - - - - - - - - Delivery Status
///                                  - - - - - - - - - - - - - - - - - - - - - Mask
///                                   
///```
///
/// To learn more about how to configure each kind of entry, refer to Intel documentation.
#[bitfield]
#[derive(Clone, Copy, Debug, Default)]
#[repr(u32)]
pub(crate) struct LVTThermalMonitorEntry {
    vector: InterruptVector,
    delivery_mode: DeliveryMode,
    #[skip]
    __: B1,
    delivery_status: DeliveryStatus,
    #[skip]
    __: B3,
    masked: bool,
    #[skip]
    __: B15,
}

/// `LVTPerformanceCounterEntry` structure represents a _Performance Counter_ entry in the _Local Vector Table_.
///
/// Provides method to load and fill the _LVT Performance Counter Register_.
/// It is used to specify interrupt delivery when a performance counter generates an interrupt on overflow, or when
/// _Intel PT_ signals a _ToPA PMI_.
/// This entry might not be implemented.
///
/// Such entry has the following general structure :
///
/// ```plaintext
/// 31                           17 16 15 14 13 12 11 10      8 7                0
///  - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
/// |xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx|  | x | x | x |  | x |         |      Vector      |
///  - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
///                                 |               |         |
///                                 |               |          - - - - - - - - - Delivery Mode
///                                 |                - - - - - - - - - - - - - - Delivery Status
///                                  - - - - - - - - - - - - - - - - - - - - - Mask
///                                   
///```
///
/// To learn more about how to configure each kind of entry, refer to Intel documentation.
#[bitfield]
#[derive(Clone, Copy, Debug, Default)]
#[repr(u32)]
pub(crate) struct LVTPerformanceCounterEntry {
    vector: InterruptVector,
    delivery_mode: DeliveryMode,
    #[skip]
    __: B1,
    delivery_status: DeliveryStatus,
    #[skip]
    __: B3,
    masked: bool,
    #[skip]
    __: B15,
}

/// `LVTLINTEntry` structure represents an _LINTn_ entry in the _Local Vector Table_.
///
/// Provides method to load and fill the _LVT LINTn Register_.
/// It is used to specify interrupt delivery when an interrupt is signaled at the _LINTN_ pin.
///
/// Such entry has the following general structure :
///
/// ```plaintext
/// 31                           17 16 15 14 13 12 11 10      8 7                0
///  - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
/// |xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx|  |  |  |  |  |xx|         |      Vector      |
///  - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
///                                  |  |  |  |  |         |
///                                  |  |  |  |  |          - - - - - - - - - - Delivery Mode
///                                  |  |  |  |   - - - - - - - - - - - - - - - Delivery Status
///                                  |  |  |    - - - - - - - - - - - - - - - - Polarity
///                                  |  |   - - - - - - - - - - - - - - - - - - Remote IRR
///                                  |    - - - - - - - - - - - - - - - - - - - Trigger Mode
///                                   - - - - - - - - - - - - - - - - - - - - - Mask
///```
#[bitfield]
#[derive(Clone, Copy, Debug, Default)]
#[repr(u32)]
pub(crate) struct LVTLINTEntry {
    vector: InterruptVector,
    delivery_mode: DeliveryMode,
    #[skip]
    __: B1,
    delivery_status: DeliveryStatus,
    pin_polarity: PinPolarity,
    remote_irr: bool,
    trigger_mode: TriggerMode,
    masked: bool,
    #[skip]
    __: B15,
}

/// `LVTErrorEntry` structure represents an _Error_ entry in the _Local Vector Table_.
///
/// Provides method to load and fill the _LVT Error Register_.
/// It is used to specify interrupt delivery when the _APIC_ detects an internal error.
///
/// Such entry has the following general structure :
///
/// ```plaintext
/// 31                           17 16 15 14 13 12 11 10      8 7                0
///  - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
/// |xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx|  | x | x | x |  | x |         |      Vector      |
///  - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
///                                 |               |         |
///                                 |               |          - - - - - - - - - Delivery Mode
///                                 |                - - - - - - - - - - - - - - Delivery Status
///                                  - - - - - - - - - - - - - - - - - - - - - Mask
///```
///
/// To learn more about how to configure each kind of entry, refer to Intel documentation.
#[bitfield]
#[derive(Clone, Copy, Default, Debug)]
#[repr(u32)]
pub(crate) struct LVTErrorEntry {
    vector: InterruptVector,
    #[skip]
    __: B4,
    delivery_status: DeliveryStatus,
    #[skip]
    __: B3,
    masked: bool,
    #[skip]
    __: B15,
}

/// `APIC LVT` (_Local Vector Table_) implementation.
///
/// It contains entries that describe how the `LocalAPIC` should handle local interrupts, and how they should be
/// delivered to the processor core.
///
/// It contains up to 7 different registers, for various types of interrupt source, but all may not be available on
/// every platform.
#[derive(Default, Debug)]
pub(crate) struct ApicLVT {
    cmci: LVTCMCIEntry,
    timer: LVTTimerEntry,
    thermal_mon: LVTThermalMonitorEntry,
    perf_count: LVTPerformanceCounterEntry,
    lint0: LVTLINTEntry,
    lint1: LVTLINTEntry,
    error: LVTErrorEntry,
}

/// `LocalAPIC` error register.
///
/// It indicates any error detected during interrupt handling. Must be written to to update its content, before
/// attempting to read its value.
#[bitfield]
#[repr(u32)]
pub(crate) struct LocalAPICErrorRegister {
    /// Checksum error for a message sent on the _APIC_ bus.
    pub(super) send_chksum_error: bool,

    /// Checksum error for a message received on the _APIC_ bus.
    pub(super) receive_chksum_error: bool,

    /// No `APIC` accepted the message on the _APIC_ bus.
    pub(super) send_accept_error: bool,

    /// The message received was not accept by any `APIC` on the _APIC_ bus, including itself.
    pub(super) receive_accept_error: bool,

    /// The `APIC` detected an attempt to send an _IPI_ with the lowest-priority delivery mode, that is not
    /// supported.
    pub(super) redirectable_ipi: bool,

    /// The `APIC` detected an illegal vector in the message that it is sending (after an _ICR_ write).
    pub(super) send_illegal_vector: bool,

    /// The `APIC` detected an illegal vector in an interrupt it received, or locally generated.
    pub(super) received_illegal_vector: bool,

    /// Software attempted to access a register reserved in the `APIC` address space.
    pub(super) illegal_register_address: bool,
    #[skip]
    __: B24,
}

/// Describes the various operating mode for the [`LocalApic`].
///
/// The system may run in different interrupt mode, each one using the [`LocalAPIC`] differently.
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug)]
pub(crate) enum APICOperatingMode {
    /// Bypasses all _APIC_ components.
    PIC,

    /// Uses the _APIC_ as a wire, and operates the same as in `PIC` mode.
    VirtualWire,

    /// Uses the _APIC_, enables the system to run with multiple processors.
    SymmetricIO,
}

/// `Local APIC` (_Local Advanced Programmable Interrupt Controller_) implementation.
///                                                                                                     
/// Contains various register used as interface with the physical _Local APIC_. Can be bypassed with the `PIC`
/// ([`APICOperatingMode::PIC`]) operating mode, or transparent with the `VirtualWire`
/// ([`APICOperatingMode::VirtualWire`]) one.
///
/// The _Local APIC_ manages external interrupts for a specific CPU, and are able to accept and generate
/// _IPIs_ (Interprocessor interrupts).
///                                                                                                     
/// There is one _Local APIC_ per CPU, each is assigned a ID unique for a given system.
#[derive(Debug)]
pub struct LocalAPIC {
    apic_id: ProcLocalApicID,
    msr_register: Ia32ApicBase,
    version_register: LocalAPICVersionRegister,
    lvt: ApicLVT,
    svr: LocalAPICSpuriousVectorRegister,
    mp_table: MPTable,
    operating_mode: APICOperatingMode,
    interrupt_cmd: LocalAPICInterruptCmdRegister,
}

impl LocalAPIC {
    pub fn init() -> Result<Self, ()> {
        disable_interrupts();
        let mp_table = MPTable::load().ok_or(())?;

        let operating_mode = if mp_table.imcr_present() {
            APICOperatingMode::PIC
        } else {
            APICOperatingMode::VirtualWire
        };

        let mut local_apic = Self {
            apic_id: ProcLocalApicID::get(),
            msr_register: Ia32ApicBase::read().ok_or(())?,
            version_register: LocalAPICVersionRegister::from(0),
            lvt: ApicLVT::default(),
            svr: LocalAPICSpuriousVectorRegister::default(),
            mp_table,
            operating_mode,
            interrupt_cmd: LocalAPICInterruptCmdRegister::from(0),
        };

        local_apic.switch_from_pic_mode();
        local_apic.load_version_register();
        local_apic.load_lvt();
        local_apic.set_spurious_vector();

        // setup I/O APIC if this processor is the BSP
        if local_apic.msr_register.is_bsp() {
            for io_apic in local_apic.mp_table.get_io_apic() {
                IOApic::init(io_apic, &local_apic.mp_table);
            }
        }

        enable_interrupts();

        Ok(local_apic)
    }

    /// Software disable the _Local APIC_.
    ///
    /// Software can temporarily enable or disable the _Local APIC_. Contrary to hard disabling it, the `APIC` can
    /// always be enabled back after soft disabling it.
    pub(crate) fn soft_disable_apic(&mut self) {
        self.svr = self.svr.with_soft_enable(false);
        self.write_reg(LocalAPICRegisterOffset::SVR, self.svr.into());
    }

    /// Software enable the _Local APIC_.
    ///
    /// Software can temporarily enable or disable the _Local APIC_. Contrary to hard disabling it, the `APIC` can
    /// always be enabled back after soft disabling it.
    pub(crate) fn soft_enable_apic(&mut self) {
        self.svr = self.svr.with_soft_enable(true);
        self.write_reg(LocalAPICRegisterOffset::SVR, self.svr.into());
    }

    /// Hard disable the _Local APIC_.
    ///
    /// Clears the `APIC` global enable flag in the [`Ia32ApicBase`] MSR.
    /// On some systems, the `APIC` cannot be enabled after hard disabling it once.
    pub(crate) fn disable_apic(&mut self) {
        self.msr_register.global_disable();
    }

    /// Hard enable the _Local APIC_.
    ///
    /// Sets the `APIC` global enable flag in the [`Ia32ApicBase`] MSR.
    /// On some systems, the `APIC` cannot be enabled after hard disabling it once.
    pub(crate) fn enable_apic(&mut self) {
        self.msr_register.global_disable();
    }

    /// Updates the `EOI` (_End of Interrupt_) register upon interrupt completion.
    pub(crate) fn send_eoi(&self) {
        self.write_reg(LocalAPICRegisterOffset::EOI_REGISTER, 0);
    }

    /// Reads the [`LocalAPICErrorRegister`] from the corresponding _APIC_ register.
    ///
    /// It indicates any error detected during interrupt handling. Must be written to to update its content, before
    /// attempting to read its value.
    pub(super) fn read_error_register(&self) -> LocalAPICErrorRegister {
        self.write_reg(LocalAPICRegisterOffset::ERROR_REGISTER, 0);
        self.read_reg(LocalAPICRegisterOffset::ERROR_REGISTER)
            .into()
    }

    /// Sets up the _Spurious-interrupt Vector_, used to initialize the `LocalAPIC`.
    ///
    /// Sets the spurious interrupt vector number, and soft enables the APIC.
    fn set_spurious_vector(&mut self) {
        self.svr = LocalAPICSpuriousVectorRegister::new()
            .with_soft_enable(true)
            .with_vector(InterruptVector::SPURIOUS_VECTOR);

        self.write_reg(LocalAPICRegisterOffset::SVR, self.svr.into());
    }

    /// Disconnects the `LocalApic`, switching back to either `PIC` or `Virtual Wire` mode, depending on what is
    /// available on the system.
    pub(crate) fn disconnect_apic(&mut self) {
        if self.mp_table.imcr_present() {
            self.switch_to_pic_mode();
            self.operating_mode = APICOperatingMode::PIC;
            return;
        }

        self.switch_to_virtual_wire_mode();
        self.operating_mode = APICOperatingMode::VirtualWire;
    }

    /// Issues an [`IPI`] (_Interprocessor Interrupt_) from this `Local APIC`.
    ///
    /// Writes the requested `IPI` to the `ICR` (_Interrupt Command Register_), using two 32-bits writes.
    pub(crate) fn dispatch_ipi(&self, ipi: IPI) {
        let icr = LocalAPICInterruptCmdRegister::new()
            .with_vector(ipi.vector)
            .with_delivery_mode(ipi.delivery_mode)
            .with_destination_mode(ipi.destination_mode)
            .with_level(ipi.level)
            .with_trigger_mode(ipi.trigger_mode)
            .with_destination_shorthand(ipi.destination_shorthand)
            .with_destination(ipi.destination);

        self.write_reg(
            LocalAPICRegisterOffset::ICR_HIGH,
            u32::try_from((u64::from(icr) >> 32) & u64::from(u32::MAX_VALUE))
                .expect("invalid conversion"),
        );

        self.write_reg(
            LocalAPICRegisterOffset::ICR_LOW,
            u32::try_from(u64::from(icr) & u64::from(u32::MAX_VALUE)).expect("invalid conversion"),
        );
    }

    /// Switchs the `LocalAPIC` back to _Virtual Wire Mode_ ([`APICOperatingMode::VirtualWire`]).
    ///
    /// The `LocalAPIC` of the BSP becomes a simple wire, that delivers interrupt from the `PIC` via its local
    /// interrupt pin _LINTIN0_, programmed as `ExtINT`. The `I/O APIC` is not used in that mode.
    fn switch_to_virtual_wire_mode(&mut self) {
        if self.msr_register.is_bsp() {
            self.lvt.lint0 = LVTLINTEntry::new()
                .with_delivery_mode(DeliveryMode::ExternalInterrupt)
                .with_pin_polarity(PinPolarity::ActiveHigh)
                .with_trigger_mode(TriggerMode::Edge)
                .with_masked(false)
                .with_vector(InterruptVector(0));
            self.write_lvt();
            self.operating_mode = APICOperatingMode::VirtualWire;
        }
    }

    /// Switches the `LocalAPIC` back to _PIC Mode_ ([`APICOperatingMode::PIC`]).
    ///
    /// In `PIC Mode`, the `APIC` components are bypassed, and the interrupt signals that reach the BSP come from
    /// the master `PIC` instead of the `LocalAPIC`.
    fn switch_to_pic_mode(&mut self) {
        if self.mp_table.imcr_present() {
            outb(IOPort::IMCR_ADDR, 0x70);
            outb(IOPort::IMCR_DATA, 0x0);
        }
    }

    /// Switches the `LocalAPIC` from _PIC Mode_ ([`APICOperatingMode::PIC`]) to _Symmetric I/O_
    /// ([`APICOperatingMode::SymmetricIO`]).
    ///
    /// In `PIC Mode`, the `APIC` components are bypassed, and the interrupt signals that reach the BSP come from
    /// the master `PIC` instead of the `LocalAPIC`.
    fn switch_from_pic_mode(&mut self) {
        if self.mp_table.imcr_present() {
            outb(IOPort::IMCR_ADDR, 0x70);
            outb(IOPort::IMCR_DATA, 0x1);
        }
    }

    /// Loads the _Local Vector Table_ into the corresponding registers of the `LocalAPIC`.
    ///
    /// Sets up default entries, based on data provided from _MP Tables_ or the ACPI _MADT_.
    fn load_lvt(&mut self) {
        let cmci = LVTCMCIEntry::new()
            .with_delivery_mode(DeliveryMode::Fixed)
            .with_masked(true)
            .with_vector(InterruptVector(0xF8));

        let mut lint0 = LVTLINTEntry::new()
            .with_delivery_mode(DeliveryMode::ExternalInterrupt)
            .with_pin_polarity(PinPolarity::ActiveHigh)
            .with_trigger_mode(TriggerMode::Edge)
            .with_masked(true)
            .with_vector(InterruptVector(0xFC));

        let mut lint1 = LVTLINTEntry::new()
            .with_delivery_mode(DeliveryMode::NonMaskableInterrupt)
            .with_pin_polarity(PinPolarity::ActiveHigh)
            .with_trigger_mode(TriggerMode::Edge)
            .with_masked(true)
            .with_vector(InterruptVector(0xFD));

        // weird stuff happening here
        let lapic_lintin1_entry = self
            .mp_table
            .get_local_int_connected_to_pin(self.apic_id, MPLocalApicIntPin::LINTIN_1);
        let lapic_lintin0_entry = self
            .mp_table
            .get_local_int_connected_to_pin(self.apic_id, MPLocalApicIntPin::LINTIN_0);

        if let (Some(lintin0_int), Some(lintin1_int)) = (lapic_lintin0_entry, lapic_lintin1_entry) {
            match (lintin0_int.int_type, lintin1_int.int_type) {
                (MPInterruptType::External, MPInterruptType::NonMaskable) => {
                    lint0.set_delivery_mode(DeliveryMode::ExternalInterrupt);
                    lint1.set_delivery_mode(DeliveryMode::NonMaskableInterrupt);
                }
                (MPInterruptType::Vectored, MPInterruptType::NonMaskable) => {
                    lint1.set_delivery_mode(DeliveryMode::NonMaskableInterrupt);
                }
                (MPInterruptType::NonMaskable, MPInterruptType::External) => {
                    lint0.set_delivery_mode(DeliveryMode::NonMaskableInterrupt);
                    lint1.set_delivery_mode(DeliveryMode::ExternalInterrupt);
                }
                _ => {}
            }
        }

        let error = LVTErrorEntry::new()
            .with_masked(true)
            .with_vector(InterruptVector(0xFE));

        let perf_count = LVTPerformanceCounterEntry::new()
            .with_delivery_mode(DeliveryMode::Fixed)
            .with_masked(true)
            .with_vector(InterruptVector(0xFB));

        let thermal_mon = LVTThermalMonitorEntry::new()
            .with_delivery_mode(DeliveryMode::Fixed)
            .with_masked(true)
            .with_vector(InterruptVector(0xFA));

        let timer = LVTTimerEntry::new()
            .with_delivery_mode(DeliveryMode::Fixed)
            .with_timer_mode(LVTTimerMode::Periodic)
            .with_masked(true)
            .with_vector(InterruptVector(0xF9));

        self.lvt = ApicLVT {
            cmci,
            timer,
            thermal_mon,
            perf_count,
            lint0,
            lint1,
            error,
        };

        self.write_lvt();
    }

    /// Updates the [`ApicLVT`] structure contained in this `LocalAPIC`, with the values contained in the corresponding
    /// registers in memory.
    fn update_lvt(&mut self) {
        self.lvt.cmci = self.read_reg(LocalAPICRegisterOffset::CMCI_REGISTER).into();
        self.lvt.timer = self
            .read_reg(LocalAPICRegisterOffset::TIMER_REGISTER)
            .into();
        self.lvt.thermal_mon = self
            .read_reg(LocalAPICRegisterOffset::THERMAL_MON_REGISTER)
            .into();
        self.lvt.perf_count = self
            .read_reg(LocalAPICRegisterOffset::PERF_COUNT_REGISTER)
            .into();
        self.lvt.lint0 = self
            .read_reg(LocalAPICRegisterOffset::LINT0_REGISTER)
            .into();
        self.lvt.lint1 = self
            .read_reg(LocalAPICRegisterOffset::LINT1_REGISTER)
            .into();
        self.lvt.error = self
            .read_reg(LocalAPICRegisterOffset::LVT_ERR_REGISTER)
            .into();
    }

    /// Updates this `LocalAPIC` _LVT_ registers, with the values contained in the [`ApicLVT`] structure.
    fn write_lvt(&self) {
        self.write_reg(LocalAPICRegisterOffset::CMCI_REGISTER, self.lvt.cmci.into());
        self.write_reg(
            LocalAPICRegisterOffset::TIMER_REGISTER,
            self.lvt.timer.into(),
        );
        self.write_reg(
            LocalAPICRegisterOffset::THERMAL_MON_REGISTER,
            self.lvt.thermal_mon.into(),
        );
        self.write_reg(
            LocalAPICRegisterOffset::PERF_COUNT_REGISTER,
            self.lvt.perf_count.into(),
        );
        self.write_reg(
            LocalAPICRegisterOffset::LINT0_REGISTER,
            self.lvt.lint0.into(),
        );
        self.write_reg(
            LocalAPICRegisterOffset::LINT1_REGISTER,
            self.lvt.lint1.into(),
        );
        self.write_reg(
            LocalAPICRegisterOffset::LVT_ERR_REGISTER,
            self.lvt.error.into(),
        );
    }

    /// Loads the `LocalAPIC` version register, as a [`LocalAPICVersionRegister`].
    ///
    /// Contains information about the `APIC` version, and the maximum _LVT_ entry available.
    fn load_version_register(&mut self) {
        self.version_register = LocalAPICVersionRegister::from(
            self.read_reg(LocalAPICRegisterOffset::VERSION_REGISTER),
        );
    }

    /// Reads APIC register at given offset. This could be used to
    /// read registers that don't have any abstraction implemented in
    /// this module.
    fn read_reg(&self, register: LocalAPICRegisterOffset) -> u32 {
        unsafe { read_volatile((self.msr_register.apic_register_base() + register).as_ptr()) }
    }

    /// Writes the given value in the APIC register at given offset.
    fn write_reg(&self, offset: LocalAPICRegisterOffset, value: u32) {
        self.read_reg(offset);
        unsafe {
            write_volatile(
                (self.msr_register.apic_register_base() + offset).as_mut_ptr(),
                value,
            );
        }
        self.read_reg(offset);
    }
}

// verify the uniqueness of I/O APIC ids, the assignement must begin from the lowest possible number after assigning to
// the local apcics

/// The APIC `ICR` (_Interrupt Command Register_) enables the sending of `IPIs` (_Interprocessor Interrupts).
///
/// It is a 64-bit register, that allows software to specify _IPIs_ to other processors on the system.
/// Writing to the low doubleword of the `ICR` causes the _IPI_ to be sent.
#[bitfield]
#[repr(u64)]
#[derive(Clone, Copy, Debug)]
struct LocalAPICInterruptCmdRegister {
    vector: InterruptVector,
    delivery_mode: IPIDeliveryMode,
    destination_mode: DestinationMode,
    delivery_status: DeliveryStatus,
    #[skip]
    __: bool,
    level: IPILevel,
    trigger_mode: TriggerMode,
    #[skip]
    __: B2,
    destination_shorthand: IPIDestinationShorthand,
    #[skip]
    __: B36,
    destination: u8,
}

/// Used to specify the type of [`IPI`] to be sent (message type).
#[repr(u8)]
#[derive(BitfieldSpecifier, Debug)]
#[bits = 3]
pub(crate) enum IPIDeliveryMode {
    /// Delivers the interrupt specified in the _vector_ field to the target processor(s).
    Fixed = 0,

    /// Same as `Fixed`, but the interrupt is delivered to the processor executing at the lowest priority amongst the
    /// set of processors specified in the destination field.
    ///
    /// This is model specific, and should be avoided.
    LowestPriority = 1,

    /// Delivers an `SMI` (_System Management Interrupt_) to the target processor(s).
    SystemManagement = 2,

    /// Delivers a `NMI` (_Non Maskable Interrupt_) to the target processor(s).
    NonMaskable = 4,

    /// Delivers an `INIT` request to the target processor(s).
    Init = 5,

    /// Delivers a `SIPI` (_Start-up IPI_) to the target processor(s).
    ///
    /// The `vector` field must point to a start-up routine. It is up to software to make sure the `SIPI` was
    /// successfully delivered, or to issue it again otherwise.
    StartUp = 6,
}

/// Used to select between _INIT level de-assert_ or standard _INIT_ when creating such type of `IPI` message
#[repr(u8)]
#[derive(BitfieldSpecifier, Debug)]
pub(crate) enum IPILevel {
    /// `Level` field must be set to _De-assert_ to perform a _INIT level de-assert_.
    DeAssert = 0,

    /// For most type of deliveries, the corresponding field must be set to `Assert`
    Assert = 1,
}

/// Used to select either _Physical_ or _Logical_ addressing mode.
#[repr(u8)]
#[derive(BitfieldSpecifier, Copy, Clone, Debug)]
pub(crate) enum DestinationMode {
    /// Physical destination mode.
    Physical = 0,

    /// Logical destination mode.
    Logical = 1,
}

/// Used to send the `IPI` to special destination, overrides the `destination` field is set.
/// Necessary when issuing self-interrupts or to broadcast interrupts.
#[repr(u8)]
#[derive(BitfieldSpecifier, Debug)]
pub(crate) enum IPIDestinationShorthand {
    /// Indicates that the destination is contained in the `destination` field.
    NoShorthand = 0,

    /// Issues a self interrupt.
    ///
    /// The issuing _Local APIC_ is the one and only to receive the interrupt.
    SelfInt = 1,

    /// Broadcasts an interrupt, the [`IPI`] is sent to all processors in the system, _including_ the issuer.
    All = 2,

    /// Broadcasts an interrupt, the [`IPI`] is sent to all processors in the system, _excluding_ the issuer.
    AllButSelf = 3,
}

/// Main structure used to issue an `IPI` (_Interprocessor Interrupt_) from software.
///
/// The `IPI` is then issued by the _Local APIC_, by writing to the `ICR` (_Interrupt Command Register_).
/// Interrupts can be:
///
/// - Sent to another processor
/// - If not services, forwarded to another processor for servicing.
/// - Delivered to itself (self-interrupt)
///
/// There are a few special _IPIs_, such as _INIT_ or _SIPI_ (start-up `IPI`) messages, used to start other processors.
#[allow(clippy::upper_case_acronyms)]
pub(crate) struct IPI {
    /// Vector number of the interrupt being sent.
    pub(crate) vector: InterruptVector,

    /// Type of `IPI` to be sent.
    pub(crate) delivery_mode: IPIDeliveryMode,

    /// Used to select either _Physical_ or _Logical_ addressing mode.
    pub(crate) destination_mode: DestinationMode,

    /// Used to select between _INIT level de-assert_ or standard _INIT_, in all other cases that should be set
    /// to [`IPILevel::Assert`].
    pub(crate) level: IPILevel,

    /// Selects the trigger mode when using the _INIT level de-assert_ `IPI` (either edge or level).
    pub(crate) trigger_mode: TriggerMode,

    /// Used to send the `IPI` to special destination, overrides the `destination` field is set.
    /// Necessary when issuing self-interrupts or to broadcast interrupts.
    pub(crate) destination_shorthand: IPIDestinationShorthand,

    /// Specifies the target processor(s). Only used when the `destination_shorthand` field is clear.
    pub(crate) destination: u8,
}

impl IPI {
    /// Generates a basic `IPI` message.
    ///
    /// Delivers the interrupt specified in the `vector` field to the destination processor(s) specified in
    /// the `destination_shorthand` and `destination` fields.
    pub(crate) fn std_int(
        vector: InterruptVector,
        destination_shorthand: IPIDestinationShorthand,
        destination: u8,
    ) -> Self {
        Self {
            vector,
            delivery_mode: IPIDeliveryMode::Fixed,
            destination_mode: DestinationMode::Physical,
            level: IPILevel::Assert,
            trigger_mode: TriggerMode::Edge,
            destination_shorthand,
            destination,
        }
    }

    /// Generates a basic broadcast `IPI` message.
    ///
    /// Delivers the interrupt specified in the `vector` field to the every processor.
    pub(crate) fn broadcast_std_int(vector: InterruptVector) -> Self {
        Self {
            vector,
            delivery_mode: IPIDeliveryMode::Fixed,
            destination_mode: DestinationMode::Physical,
            level: IPILevel::Assert,
            trigger_mode: TriggerMode::Edge,
            destination_shorthand: IPIDestinationShorthand::All,
            destination: 0,
        }
    }

    /// Generates a basic broadcast `IPI` message.
    ///
    /// Delivers the interrupt specified in the `vector` field to every processor, except the issuer.
    pub(crate) fn broadcast_others_std_int(vector: InterruptVector) -> Self {
        Self {
            vector,
            delivery_mode: IPIDeliveryMode::Fixed,
            destination_mode: DestinationMode::Physical,
            level: IPILevel::Assert,
            trigger_mode: TriggerMode::Edge,
            destination_shorthand: IPIDestinationShorthand::AllButSelf,
            destination: 0,
        }
    }

    /// Generates a _NMI_ (Non-Maskable Interrupt) `IPI` message.
    ///
    /// Delivers a `NMI` to the destination processor, specified in the `destination` field.
    pub(crate) fn nmi(destination: ProcLocalApicID) -> Self {
        Self {
            vector: InterruptVector(0),
            delivery_mode: IPIDeliveryMode::NonMaskable,
            destination_mode: DestinationMode::Physical,
            level: IPILevel::Assert,
            trigger_mode: TriggerMode::Edge,
            destination_shorthand: IPIDestinationShorthand::NoShorthand,
            destination: u8::from(destination),
        }
    }

    /// Generates a _NMI_ (Non-Maskable Interrupt) broadcast `IPI` message.
    ///
    /// Delivers a `NMI` to every processor.
    pub(crate) fn broadcast_nmi() -> Self {
        Self {
            vector: InterruptVector(0),
            delivery_mode: IPIDeliveryMode::Fixed,
            destination_mode: DestinationMode::Physical,
            level: IPILevel::Assert,
            trigger_mode: TriggerMode::Edge,
            destination_shorthand: IPIDestinationShorthand::All,
            destination: 0,
        }
    }

    /// Generates an _INIT_ request `IPI` message.
    ///
    /// Delivers an `INIT` request to the destination processor, specified in the `destination field`.
    pub(crate) fn init_proc(destination: ProcLocalApicID) -> Self {
        Self {
            vector: InterruptVector(0),
            delivery_mode: IPIDeliveryMode::Init,
            destination_mode: DestinationMode::Physical,
            level: IPILevel::Assert,
            trigger_mode: TriggerMode::Edge,
            destination_shorthand: IPIDestinationShorthand::NoShorthand,
            destination: u8::from(destination),
        }
    }

    /// Generates an _INIT_ request broadcast `IPI` message.
    ///
    /// Delivers an `INIT` request to every processor, except the issuer.
    pub(crate) fn init_others() -> Self {
        Self {
            vector: InterruptVector(0),
            delivery_mode: IPIDeliveryMode::Init,
            destination_mode: DestinationMode::Physical,
            level: IPILevel::Assert,
            trigger_mode: TriggerMode::Edge,
            destination_shorthand: IPIDestinationShorthand::AllButSelf,
            destination: 0,
        }
    }
}
