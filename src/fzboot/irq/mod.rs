use core::arch::asm;

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
#[derive(Clone, Copy, Debug, Default)]
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

impl InterruptStackFrame {
    /// Performs an `iret`.
    ///
    /// Restores the previous execution context using the value defined in the structure.
    ///
    /// Used to return to original execution context after an interrupt was processed.
    /// Can also be used to change the current privilege level (`CPL`) of the CPU.
    pub unsafe fn iret(self) {
        let frame_ptr = core::ptr::addr_of!(self);
        #[cfg(target_arch = "x86_64")]
        unsafe {
            asm!(
             "mov rsp, r8",
             "mov rax, [rsp + 0x28]
                mov rbx, [rsp + 0x30]
                mov rcx, [rsp + 0x38]
                mov rdx, [rsp + 0x40]
                mov rsi, [rsp + 0x48]
                mov rdi, [rsp + 0x50]
                mov rbp, [rsp + 0x58]
                mov r8, [rsp + 0x60]
                mov r9, [rsp + 0x68]
                mov r10, [rsp + 0x70]
                mov r11, [rsp + 0x78]
                mov r12, [rsp + 0x80]
                mov r13, [rsp + 0x88]
                mov r14, [rsp + 0x90]
                mov r15, [rsp + 0x98]",
             "iretq",
             in("r8") frame_ptr
            );
        }
    }

    /// Performs an `iret`, preserving the current value of `RFLAGS` (and of the stack pointer, if `stack_ptr_override` is set to false).
    ///
    /// Restores the previous execution context using the value defined in the structure.
    ///
    /// Used to return to original execution context after an interrupt was processed.
    /// Can also be used to change the current privilege level (`CPL`) of the CPU.
    pub unsafe fn iret_preserve_flags(&self, stack_ptr_override: bool) -> ! {
        // emulates a fake InterruptStackFrame with data and code segment selector set to `CPL` = 3
        #[cfg(target_arch = "x86_64")]
        unsafe {
            if !stack_ptr_override {
                asm!("push {0:r}",
                    "push rsp",
                    "pushfq",
                    "push {1:r}",
                    "push {2:r}",
                    "iretq",
                    in(reg) self.stack_segment,
                    in(reg) self.cs,
                    in(reg) u64::from(self.rip),
                    options(noreturn)
                )
            } else {
                asm!("push {0:r}",
                    "push {1:r}",
                    "pushfq",
                    "push {2:r}",
                    "push {3:r}",
                    "iretq",
                    in(reg) self.stack_segment,
                    in(reg) u64::from(self.stack_ptr),
                    in(reg) self.cs,
                    in(reg) u64::from(self.rip),
                    options(noreturn))
            }
        }

        unreachable!()
    }
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
