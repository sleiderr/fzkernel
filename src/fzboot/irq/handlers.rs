//! Definition of Interrupt handlers related structures and types.
//!
//! Two types of interrupt handlers are available:
//!
//! - Static handler: run without overhead, directly called when receiving an interrupt with the associated [`InterruptVector`]
//!
//! - Dynamic handler: run with minor overhead, allows multiple dynamic handlers to be defined for a same [`InterruptVector`].
//!
//! Dynamic handlers are executed in decreasing order of priority, and are called by a standard handler called when any interrupt with
//! dynamic handlers defined is raised.

use crate::mem::LocklessCell;
use crate::x86::apic::InterruptVector;
use alloc::collections::BinaryHeap;
use alloc::vec::Vec;
use fzproc_macros::{generate_runtime_handlers_wrapper, interrupt_handler};

use super::manager::{get_long_interrupt_manager, get_prot_interrupt_manager};

generate_runtime_handlers_wrapper!();

/// Default interrupt handler when initializing the `InterruptManager`, corresponds to a no-op.
#[interrupt_handler]
pub(super) fn _default_int_handler() {
    ()
}

/// Relative priority for dynamic handlers.
pub type InterruptHandlerPriority = u16;

/// Maximum priority allowed for dynamic handlers (used when a static handler is turned into a dynamic one).
pub const MAX_INT_PRIORITY: InterruptHandlerPriority = 0xFFFF;

/// Available types of interrupt handler.
pub enum InterruptHandler {
    /// Static handlers run without overhead, as they are directly called when an interrupt is raised with the proper [`InterruptVector`].
    ///
    /// They must be defined as `interrupt_handler` using the corresponding macro, to make them directly usable as handlers (which required
    /// naked function calls).
    Static(StaticInterruptHandler),

    /// Dynamic handlers run with minor overhead, they allow multiple handlers to be defined for a same [`InterruptVector`]. Each dynamic handler
    /// comes with a priority, and all handlers for a same vector are run in decreasing order of priority.
    Dynamic(RuntimeInterruptHandler),
}

/// Static interrupt handler function pointer.
pub type StaticInterruptHandler = fn();

/// Dynamic interrupt handler function pointer and corresponding priority.
pub type DynamicInterruptHandler = (fn(), InterruptHandlerPriority);

/// `RuntimeInterruptHandler` manages dynamic handlers defined for an [`InterruptVector`].
///
/// At this time, two types of handlers can be used:
///
/// - standard dynamic handlers, that are run each time an interrupt with the proper [`InterruptVector`] is raised.
///
/// - one-shot dynamic handlers, that are run only once.
pub(crate) struct RuntimeInterruptHandler {
    handlers: Vec<DynamicInterruptHandler>,
    once_handlers: LocklessCell<BinaryHeap<DynamicInterruptHandler>>,
}

impl RuntimeInterruptHandler {
    /// Initializes a new `RuntimeInterruptHandler`.
    pub(crate) fn new() -> Self {
        Self {
            handlers: Vec::new(),
            once_handlers: LocklessCell::new(BinaryHeap::new()),
        }
    }

    #[inline]
    fn exec_handlers_in_order(&self) {
        for handler in self.handlers.iter() {
            handler.0()
        }

        while let Some(handler) = self.once_handlers.get().pop() {
            handler.0()
        }
    }

    #[cold]
    pub(super) fn insert_handler(&mut self, handler: fn(), priority: InterruptHandlerPriority) {
        self.handlers.push((handler, priority));
        self.handlers.sort_by(|a, b| b.1.cmp(&a.1))
    }

    pub(super) fn insert_once_handler(
        &mut self,
        handler: fn(),
        priority: InterruptHandlerPriority,
    ) {
        self.once_handlers.get().push((handler, priority));
    }
}

#[no_mangle]
#[link_section = ".int"]
extern "C" fn _runtime_int_entry(vector: InterruptVector) {
    #[cfg(feature = "x86_64")]
    if let InterruptHandler::Dynamic(dyn_handler) = get_long_interrupt_manager()
        .handler_registry
        .read()
        .get(&vector)
        .expect("failed to locate runtime handler for this irq")
    {
        dyn_handler.exec_handlers_in_order();
    }

    #[cfg(not(feature = "x86_64"))]
    if let InterruptHandler::Dynamic(dyn_handler) = get_prot_interrupt_manager()
        .handler_registry
        .read()
        .get(&vector)
        .expect("failed to locate runtime handler for this irq")
    {
        dyn_handler.exec_handlers_in_order();
    }
}
