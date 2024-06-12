use crate::mem::LocklessCell;
use crate::x86::apic::local_apic::InterruptVector;
use alloc::collections::BinaryHeap;
use alloc::vec::Vec;

pub fn _runtime_int_entry(vector: InterruptVector) -> ! {
    unreachable!();
}

pub type InterruptHandlerPriority = u16;

pub const MAX_INT_PRIORITY: InterruptHandlerPriority = 0xFFFF;

pub enum InterruptHandler {
    Static(StaticInterruptHandler),
    Dynamic(RuntimeInterruptHandler),
}

pub type StaticInterruptHandler = fn(InterruptVector) -> !;
pub type DynamicInterruptHandler = (fn(InterruptVector) -> !, InterruptHandlerPriority);

pub struct RuntimeInterruptHandler {
    handlers: Vec<DynamicInterruptHandler>,
    once_handlers: LocklessCell<BinaryHeap<DynamicInterruptHandler>>,
}

impl RuntimeInterruptHandler {
    pub(crate) fn new() -> Self {
        Self {
            handlers: Vec::new(),
            once_handlers: LocklessCell::new(BinaryHeap::new()),
        }
    }

    #[inline]
    pub(crate) fn exec_handlers_in_order(&self, vector: InterruptVector) {
        for handler in self.handlers.iter() {
            handler.0(vector)
        }

        while let Some(handler) = self.once_handlers.get().pop() {
            handler.0(vector)
        }
    }

    #[cold]
    pub(crate) fn insert_handler(
        &mut self,
        handler: fn(InterruptVector) -> !,
        priority: InterruptHandlerPriority,
    ) {
        self.handlers.push((handler, priority));
        self.handlers.sort_by(|a, b| a.1.cmp(&b.1))
    }

    pub(crate) fn insert_once_handler(
        &mut self,
        handler: fn(InterruptVector) -> !,
        priority: InterruptHandlerPriority,
    ) {
        self.once_handlers.get().push((handler, priority));
    }
}
