use exception_vectors::DOUBLE_FAULT;
use fzproc_macros::interrupt_handler;
use panic::panic_entry_exception;

use crate::irq::manager::get_interrupt_manager;

pub mod panic;

pub mod exception_vectors {
    use crate::x86::apic::InterruptVector;

    pub const DOUBLE_FAULT: InterruptVector = InterruptVector::new(0x8);
}

pub fn register_exception_handlers() {
    get_interrupt_manager().register_static_handler(DOUBLE_FAULT, double_fault_handler);
}

#[interrupt_handler(exception = true)]
pub fn double_fault_handler(frame: ExceptionStackFrame) {
    panic_entry_exception("DOUBLE_FAULT", frame)
}
