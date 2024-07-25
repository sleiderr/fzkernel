use crate::{irq::InterruptStackFrame, mem::VirtAddr};

use super::{
    descriptors::gdt::{USERMODE_CODE_SELECTOR, USERMODE_DATA_SELECTOR},
    int::disable_interrupts,
};

/// Switches to usermode using a fake `iret`, and continues execution with the function defined at `func_ptr`.
pub fn usermode_exec(func_ptr: fn()) {
    disable_interrupts();
    let usermode_data_selector: u64 = u64::from(USERMODE_DATA_SELECTOR.bytes());

    let usermode_code_selector = u64::from(USERMODE_CODE_SELECTOR.bytes());

    let mut ist = InterruptStackFrame::default();

    ist.rip = VirtAddr::from(func_ptr as u64);
    ist.cs = usermode_code_selector;
    ist.stack_segment = usermode_data_selector;

    unsafe { ist.iret_preserve_flags(false) }
}
