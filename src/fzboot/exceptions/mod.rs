use exception_vectors::{DOUBLE_FAULT, GENERAL_PROT_FAULT, PAGE_FAULT};
use fzproc_macros::interrupt_handler;
use panic::panic_entry_exception;

use crate::irq::manager::get_interrupt_manager;

pub mod panic;

pub mod exception_vectors {
    use crate::x86::apic::InterruptVector;

    pub const DOUBLE_FAULT: InterruptVector = InterruptVector::new(0x8);
    pub const GENERAL_PROT_FAULT: InterruptVector = InterruptVector::new(0xD);
    pub const PAGE_FAULT: InterruptVector = InterruptVector::new(0xE);
}

pub fn register_exception_handlers() {
    get_interrupt_manager().register_static_handler(DOUBLE_FAULT, double_fault_handler);
    get_interrupt_manager().register_static_handler(GENERAL_PROT_FAULT, unhandled_gpf_handler);
    get_interrupt_manager().register_static_handler(PAGE_FAULT, unhandled_page_fault_handler);
}

#[interrupt_handler(exception = true)]
pub fn double_fault_handler(frame: ExceptionStackFrame) {
    panic_entry_exception("DOUBLE_FAULT", frame)
}

#[interrupt_handler(exception = true)]
pub fn unhandled_page_fault_handler(frame: ExceptionStackFrame) {
    panic_entry_exception("PAGE_FAULT", frame)
}

#[interrupt_handler(exception = true)]
pub fn unhandled_gpf_handler(frame: ExceptionStackFrame) {
    panic_entry_exception("GENERAL_PROTECTION_FAULT", frame)
}
