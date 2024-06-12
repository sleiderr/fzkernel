use alloc::collections::{btree_map::Entry, BTreeMap};
use conquer_once::spin::OnceCell;
use spin::{Mutex, RwLock};

use crate::{
    mem::{MemoryAddress, PhyAddr, PhyAddr32},
    println,
    x86::{
        apic::local_apic::InterruptVector,
        descriptors::{
            gdt::SegmentSelector,
            idt::{GateDescriptor, GateType, InterruptDescriptorTable},
        },
        int::{disable_interrupts, enable_interrupts},
        privilege::PrivilegeLevel,
    },
};

use super::handlers::{
    InterruptHandler, InterruptHandlerPriority, RuntimeInterruptHandler, _runtime_int_entry,
    MAX_INT_PRIORITY,
};

pub fn get_prot_interrupt_manager() -> &'static InterruptManager<PhyAddr32> {
    static INT_MANAGER: OnceCell<InterruptManager<PhyAddr32>> = OnceCell::uninit();

    INT_MANAGER.get_or_init(|| InterruptManager::new())
}

pub fn get_interrupt_manager() -> &'static InterruptManager<PhyAddr> {
    static INT_MANAGER: OnceCell<InterruptManager<PhyAddr>> = OnceCell::uninit();

    INT_MANAGER.get_or_init(|| InterruptManager::new())
}

pub struct InterruptManager<A: MemoryAddress> {
    idt: Mutex<InterruptDescriptorTable<A>>,
    handler_registry: RwLock<BTreeMap<InterruptVector, InterruptHandler>>,
}

impl<A: MemoryAddress> InterruptManager<A> {
    fn new() -> Self {
        Self {
            idt: Mutex::new(InterruptDescriptorTable::new(A::NULL_PTR)),
            handler_registry: RwLock::new(BTreeMap::new()),
        }
    }

    pub unsafe fn load_idt(&self) {
        self.idt.lock().enable()
    }

    pub fn register_dynamic_handler(
        &self,
        int_vector: InterruptVector,
        handler: fn(InterruptVector) -> !,
        priority: InterruptHandlerPriority,
    ) {
        disable_interrupts();

        match self.handler_registry.write().entry(int_vector) {
            Entry::Vacant(e) => {
                if let InterruptHandler::Dynamic(dynamic) =
                    e.insert(InterruptHandler::Dynamic(RuntimeInterruptHandler::new()))
                {
                    dynamic.insert_handler(handler, priority);
                };
            }
            Entry::Occupied(mut e) => {
                if matches!(e.get_mut(), InterruptHandler::Static(_)) {
                    let mut opt_previous_handler: Option<fn(InterruptVector) -> !> = None;
                    if let InterruptHandler::Static(previous_handler) = e.get_mut() {
                        opt_previous_handler = Some(previous_handler.clone());
                    }
                    *e.get_mut() = InterruptHandler::Dynamic(RuntimeInterruptHandler::new());

                    if let (Some(prev_handler), InterruptHandler::Dynamic(dyn_handler)) =
                        (opt_previous_handler, e.get_mut())
                    {
                        dyn_handler.insert_handler(prev_handler, MAX_INT_PRIORITY);
                    }
                }

                let handler_entry = e.get_mut();

                if let InterruptHandler::Dynamic(dynamic) = handler_entry {
                    dynamic.insert_handler(handler, priority);
                }
            }
        };

        let runtime_entry_ptr: fn(InterruptVector) -> ! = _runtime_int_entry;
        let handler_ptr = PhyAddr::new(
            u64::try_from(runtime_entry_ptr as usize).expect("invalid handler pointer"),
        );

        self.idt.lock().set_entry(
            usize::from(u8::from(int_vector)),
            GateDescriptor::new(GateType::InterruptGate)
                .with_dpl(PrivilegeLevel::Ring0)
                .with_offset(handler_ptr)
                .with_segment_selector(
                    SegmentSelector::gdt_selector()
                        .with_index(0b1000)
                        .expect("invalid segment selector while registering static handler")
                        .with_rpl(PrivilegeLevel::Ring0),
                ),
        );

        unsafe {
            self.idt.lock().write_table();
        }

        enable_interrupts();
    }

    pub fn register_static_handler(
        &self,
        int_vector: InterruptVector,
        handler: fn(InterruptVector) -> !,
    ) {
        disable_interrupts();

        if self.handler_registry.read().get(&int_vector).is_some() {
            // switch to dynamic interrupt handling
        }

        self.handler_registry
            .write()
            .insert(int_vector, InterruptHandler::Static(handler));

        let handler_ptr =
            PhyAddr::new(u64::try_from(handler as usize).expect("invalid handler pointer"));

        self.idt
            .lock()
            .set_entry(
                usize::from(u8::from(int_vector)),
                GateDescriptor::new(GateType::InterruptGate)
                    .with_dpl(PrivilegeLevel::Ring0)
                    .with_offset(handler_ptr)
                    .with_present(true)
                    .with_segment_selector(
                        SegmentSelector::gdt_selector()
                            .with_index(0b1000)
                            .expect("invalid segment selector while registering static handler")
                            .with_rpl(PrivilegeLevel::Ring0),
                    ),
            )
            .unwrap();

        unsafe {
            self.idt.lock().write_table().unwrap();
        }

        //enable_interrupts();
    }
}
