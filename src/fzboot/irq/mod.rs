use crate::io::outb;
use crate::io::IOPort;
use crate::mem::VirtAddr;
use crate::video::vesa::text_buffer;
use crate::x86::apic::local_apic::local_apic;
use crate::x86::registers::x86_64::GeneralPurposeRegisters;

pub mod manager;

#[cfg(feature = "alloc")]
pub mod handlers;

/// Content of the _Interrupt Stack Frame_, set up by the CPU when an interrupt is raised.
///
/// Interrupt handlers receive this structure as their first argument.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct InterruptStackFrame {
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

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct ExceptionStackFrame {
    register_dump: GeneralPurposeRegisters,
    error_code: u64,
    rip: VirtAddr,
    cs: u64,
    rflags: u64,
    stack_ptr: VirtAddr,
    stack_segment: u64,
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
