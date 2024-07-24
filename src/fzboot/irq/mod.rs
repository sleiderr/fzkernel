use crate::io::outb;
use crate::io::IOPort;
use crate::mem::VirtAddr;
use crate::video::vesa::text_buffer;
use crate::x86::apic::local_apic::local_apic;
use crate::x86::registers::x86_64::GeneralPurposeRegisters;

#[cfg(feature = "alloc")]
pub mod manager;

#[cfg(feature = "alloc")]
pub mod handlers;

/// Content of the _Interrupt Stack Frame_, set up by the CPU when an interrupt is raised.
///
/// Interrupt handlers receive this structure as their first argument.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct InterruptStackFrame {
    /// Saved content of the `RIP` (_instruction pointer_ register) prior to the interrupt.
    pub(crate) rip: VirtAddr,

    /// Saved content of the `CS` (_code segment_ register) prior to the interrupt.
    pub(crate) cs: u64,

    /// Saved content of the `RFLAGS` register prior to the interrupt.
    pub(crate) rflags: u64,

    /// Saved value of the stack pointer (`RSP`) prior to the interrupt.
    pub(crate) stack_ptr: VirtAddr,

    /// Saved value of the stack segment (`SS`) prior to the interrupt.
    pub(crate) stack_segment: u64,

    /// Saved values of all general purpose registers.
    pub(crate) registers: GeneralPurposeRegisters,
}

/// Content of the _Exception Stack Frame_, set up by the CPU when an exception that defines an error code
/// is raised.
///
/// It differs from a usual [`InterruptStackFrame`] with the presence of an error code, pushed when the exception
/// is raised.
///
/// Interrupt handlers with `exception` set as true receive this structure as their first argument.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct ExceptionStackFrame {
    /// Error code associated with the exception.
    pub(crate) error_code: u64,

    /// Saved content of the `RIP` (_instruction pointer_ register) prior to the exception.
    pub(crate) rip: VirtAddr,

    /// Saved content of the `CS` (_code segment_ register) prior to the exception.
    pub(crate) cs: u64,

    /// Saved content of the `RFLAGS` register prior to the exception.
    pub(crate) rflags: u64,

    /// Saved value of the stack pointer (`RSP`) prior to the exception.
    pub(crate) stack_ptr: VirtAddr,

    /// Saved value of the stack segment (`SS`) prior to the exception.
    pub(crate) stack_segment: u64,

    /// Saved values of all general purpose registers.
    pub(crate) registers: GeneralPurposeRegisters,
}

// todo: restore locks afterwards
unsafe fn release_locks() {
    text_buffer().buffer.force_unlock();
}

#[no_mangle]
pub unsafe fn _int_entry() {
    release_locks();
}

#[no_mangle]
pub fn _pic_eoi() {
    outb(IOPort::from(0x20), 0x20);
    outb(IOPort::from(0xA0), 0x20);

    if let Some(lapic) = local_apic() {
        lapic.send_eoi();
    }
}
