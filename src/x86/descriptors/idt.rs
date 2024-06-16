//! x86 _Interrupt Descriptor Table_ (`IDT`) related structures and methods.
//!
//! This table associated every exception or interrupt vector with a gate descriptor for the procedure or
//! task used to service the associated exception or interrupt.

use core::arch::asm;

use modular_bitfield::{
    bitfield,
    specifiers::{B16, B2, B3, B32, B5},
    BitfieldSpecifier,
};

use crate::{
    error,
    errors::{BaseError, CanFail},
    mem::{MemoryAddress, PhyAddr},
    println,
    x86::{
        descriptors::idt,
        int::{disable_interrupts, enable_interrupts, interrupts_disabled},
        privilege::PrivilegeLevel,
    },
};

use super::gdt::{SegmentSelector, SegmentSelectorInner};

/// Defines the type of gate an _Interrupt Descriptor_ represents.
#[derive(BitfieldSpecifier)]
#[bits = 4]
pub enum GateType {
    /// Task Gate
    ///
    /// A Task Gate is specifically used for hardware task switching.
    /// It contains the segment selector for a _TSS_ for an exception and/or interrupt handler task.
    TaskGate = 0b0101,

    /// 16-bit Interrupt Gate
    ///
    /// Contains a far pointer (segment selector and offset) used by the processor to transfer program
    /// execution to a handler procedure in an exception- or interrupt-handler code segment.
    ///
    /// Interrupt Gates differs from Trap Gates in the way the _IF_ flag is handled. For an Interrupt Gate,
    /// the processor clears the IF flag to prevent other interrupts to interfer with the current interrupt handler.
    InterruptGate16 = 0b0110,

    /// 32/64-bit Interrupt Gate
    ///
    /// Contains a far pointer (segment selector and offset) used by the processor to transfer program
    /// execution to a handler procedure in an exception- or interrupt-handler code segment.
    ///
    /// Interrupt Gates differ from Trap Gates in the way the _IF_ flag is handled. For an Interrupt Gate,
    /// the processor clears the IF flag to prevent other interrupts to interfer with the current interrupt handler.
    InterruptGate = 0b1110,

    /// 16-bit Trap Gate
    ///
    /// Contains a far pointer (segment selector and offset) used by the processor to transfer program
    /// execution to a handler procedure in an exception- or interrupt-handler code segment.
    ///
    /// Trap Gates differ from Interrupt Gates in the way the _IF_ flag is handled. For a Trap Gate,
    /// the processor does not change the value of the _IF_ flag.
    TrapGate16 = 0b0111,

    /// 32/64-bit Trap Gate
    ///
    /// Contains a far pointer (segment selector and offset) used by the processor to transfer program
    /// execution to a handler procedure in an exception- or interrupt-handler code segment.
    ///
    /// Trap Gates differ from Interrupt Gates in the way the _IF_ flag is handled. For a Trap Gate,
    /// the processor does not change the value of the _IF_ flag.
    TrapGate = 0b1111,
}

/// _Interrupt Descriptor Table_ (`IDT`) structure.
///
/// This table associated every exception or interrupt vector with a gate descriptor for the procedure or task used to
/// service the associated exception or interrupt.
///
/// The `IDT` is an array of a 8-byte (in protected mode) or 16-byte (in long mode) descriptor.
pub struct InterruptDescriptorTable<A: MemoryAddress> {
    base_addr: A,
    length: u16,
    entries: [GateDescriptor; 256],
}

#[derive(Clone, Copy)]
pub(crate) struct GateDescriptor {
    inner: GateDescriptorInner,
}

#[bitfield]
#[repr(u128)]
#[derive(Clone, Copy)]
struct GateDescriptorInner {
    offset: u16,
    segment_selector: SegmentSelectorInner,
    ist: B3,
    #[skip]
    __: B5,
    gate_type: GateType,
    #[skip]
    __: bool,
    dpl: B2,
    present: bool,
    offset_mid: B16,
    offset_hi: u32,
    #[skip]
    __: B32,
}

impl Default for GateDescriptor {
    fn default() -> Self {
        Self {
            inner: GateDescriptorInner::default(),
        }
    }
}

impl Default for GateDescriptorInner {
    fn default() -> Self {
        Self::new().with_present(true)
    }
}

impl<A: MemoryAddress> InterruptDescriptorTable<A> {
    pub fn new(base_addr: A) -> Self {
        Self {
            base_addr,
            length: u16::try_from(256 * core::mem::size_of::<A::AsPrimitive>())
                .expect("invalid primitive size for the IDT"),
            entries: [GateDescriptor::default(); 256],
        }
    }

    pub fn set_entry(&mut self, ivt: usize, descriptor: GateDescriptor) -> CanFail<IDTError> {
        if !descriptor.present() {
            return Err(IDTError::InvalidEntry);
        }

        *self
            .entries
            .get_mut(ivt)
            .ok_or(IDTError::OutOfBoundsVector)? = descriptor;

        Ok(())
    }

    pub unsafe fn enable(&self) {
        let irq_disabled = interrupts_disabled();
        disable_interrupts();

        let idt_ptr = self.base_addr.as_mut_ptr::<u8>();

        asm!("lidt [{}]", in(reg) idt_ptr, options(nostack, readonly, preserves_flags));

        if !irq_disabled {
            enable_interrupts();
        }
    }

    pub unsafe fn write_table(&self) -> CanFail<IDTError> {
        self.write_header()?;

        if core::mem::size_of::<A::AsPrimitive>() == 0x8 {
            let mut idt_entries_ptr = (self.base_addr + 0x10)
                .as_nonnull_ptr::<u128>()
                .map_err(|_| IDTError::InvalidBaseAddress)?;

            for i in 0..256 {
                idt_entries_ptr.write(u128::from_le_bytes(self.entries[i].inner.bytes));

                idt_entries_ptr = idt_entries_ptr.add(0x1);
            }
        }

        if core::mem::size_of::<A::AsPrimitive>() == 0x4 {
            let mut idt_entries_ptr = (self.base_addr + 0x10)
                .as_nonnull_ptr::<u64>()
                .map_err(|_| IDTError::InvalidBaseAddress)?;

            for i in 0..256 {
                idt_entries_ptr.write(u64::from_le_bytes(
                    self.entries[i].inner.bytes[..8]
                        .try_into()
                        .expect("infaillible conversion"),
                ));

                idt_entries_ptr = idt_entries_ptr.add(0x1);
            }
        }

        Ok(())
    }

    unsafe fn write_header(&self) -> CanFail<IDTError> {
        let base_idt_ptr = self.base_addr.as_mut_ptr::<u8>();

        base_idt_ptr.cast::<u16>().write(self.length);

        let mut offset_idt_ptr = base_idt_ptr.add(0x2).cast::<A::AsPrimitive>();
        let base_idt_entries = self.base_addr + 0x10;

        offset_idt_ptr.write(Into::<A::AsPrimitive>::into(base_idt_entries));

        Ok(())
    }
}

impl GateDescriptor {
    pub(crate) fn new(gate_type: GateType) -> Self {
        Self {
            inner: GateDescriptorInner::new().with_gate_type(gate_type),
        }
    }

    pub(crate) fn present(&self) -> bool {
        self.inner.present()
    }

    pub(crate) fn set_present(&mut self, present: bool) {
        self.inner = self.inner.with_present(present);
    }

    pub(crate) fn with_present(self, present: bool) -> Self {
        Self {
            inner: self.inner.with_present(present),
        }
    }

    pub(crate) fn with_offset<M: MemoryAddress>(self, offset: M) -> Self {
        if matches!(self.inner.gate_type(), GateType::TaskGate) {
            error!("idt", "attempted to set the offset field of a task gate");
            return self;
        }

        let offset_num: u64 = offset.into();

        let offset_lo = offset_num & 0xFFFF;
        let offset_mid = (offset_num >> 16) & 0xFFFF;
        let offset_hi = (offset_num >> 32) & 0xFFFF_FFFF;

        let new_inner = self
            .inner
            .with_offset(u16::try_from(offset_lo).expect("infaillible conversion"))
            .with_offset_mid(u16::try_from(offset_mid).expect("infallible conversion"))
            .with_offset_hi(u32::try_from(offset_hi).expect("infaillible conversion"));

        Self { inner: new_inner }
    }

    pub(crate) fn with_dpl(self, dpl: PrivilegeLevel) -> Self {
        let dpl_bits = match dpl {
            PrivilegeLevel::Ring0 => 0b00,
            PrivilegeLevel::Ring1 => 0b01,
            PrivilegeLevel::Ring2 => 0b10,
            PrivilegeLevel::Ring3 => 0b11,
        };

        let new_inner = self.inner.with_dpl(dpl_bits);

        Self { inner: new_inner }
    }

    pub(crate) fn with_segment_selector(self, selector: SegmentSelector) -> Self {
        Self {
            inner: self.inner.with_segment_selector(selector.inner),
        }
    }
}

#[derive(Debug)]
pub enum IDTError {
    InvalidBaseAddress,

    InvalidEntry,

    OutOfBoundsVector,

    Unknown,
}

impl BaseError for IDTError {}
