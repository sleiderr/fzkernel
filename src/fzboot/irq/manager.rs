//! The `InterruptManager` manages the `IDT` (_Interrupt Descriptor Table_).
//!
//! It must be used to register interrupt handlers, that will override a default handler for every interrupt vector, which is close to a
//! no-op.
//!
//! The different types of handlers available are defined in [`fzboot::irq::handlers`].
//!
//! The current `InterruptManager` for the system should be retrieved using the [`get_interrupt_manager`] function.
//! The actual `InterruptManager` implementation depends on whether the architecture is `32-bit` or `64-bit`.
//!
//! One can choose to use a `32-bit` or `64-bit` `InterruptManager` regardless of the system architecture using [`get_prot_interrupt_manager`] or
//! [`get_long_interrupt_manager`].
//!
//! Please note that `32-bit` and `64-bit` tables cannot co-exist on the system at this time.
//!
//! # Examples
//!
//! Register a static interrupt handler
//!
//! ```
//! let int_mgr = get_interrupt_manager();
//!
//! fn test_handler() {
//!     crate::println!("Int handler !");
//! }
//!
//! int_mgr.register_static_handler(InterruptVector::from(0x80), test_handler);
//! ```

use alloc::collections::{btree_map::Entry, BTreeMap};
use conquer_once::spin::OnceCell;
use spin::{Mutex, RwLock};

use crate::{
    errors::CanFail,
    mem::{MemoryAddress, PhyAddr, PhyAddr32, VirtAddr},
    x86::{
        apic::local_apic::InterruptVector,
        descriptors::{
            gdt::SegmentSelector,
            idt::{GateDescriptor, GateType, InterruptDescriptorTable},
        },
        int::{disable_interrupts, enable_interrupts, interrupts_disabled},
        privilege::PrivilegeLevel,
    },
};

use super::handlers::{
    InterruptHandler, InterruptHandlerPriority, RuntimeInterruptHandler, _default_int_handler,
    MAX_INT_PRIORITY,
};

/// Returns the current `InterruptManager` compatible with the current CPU mode (_protected mode_, _long mode_).
///
/// `32-bit` and `64-bit` compatible `InterruptManager` cannot coexist, thus it is usually wise to use this function to retrieve
/// the `InterruptManager`.
///
/// # Example
///
/// ```
/// let int_mgr = get_interrupt_manager();
///
/// unsafe {
///     int_mgr.load_idt();
/// }
/// ```
#[cfg(feature = "x86_64")]
pub fn get_interrupt_manager() -> &'static InterruptManager<PhyAddr> {
    get_long_interrupt_manager()
}

/// Returns the current `InterruptManager` compatible with the current CPU mode (_protected mode_, _long mode_).
///
/// `32-bit` and `64-bit` compatible `InterruptManager` cannot coexist, thus it is usually wise to use this function to retrieve
/// the `InterruptManager`.
///
/// # Example
///
/// ```
/// let int_mgr = get_interrupt_manager();
///
/// unsafe {
///     int_mgr.load_idt();
/// }
/// ```
#[cfg(not(feature = "x86_64"))]
pub fn get_interrupt_manager() -> &'static InterruptManager<PhyAddr32> {
    get_prot_interrupt_manager()
}

/// Returns a `32-bit` compatible `InterruptManager`, to be used in protected mode.
///
/// If you choose to use this `InterruptManager`, keep in mind that you cannot use a `64-bit` compatible `InterruptManager`
/// at the same time, as both would use the same memory space.
///
/// # Example
///
/// ```
/// let int_mgr = get_prot_interrupt_manager();
///
/// unsafe {
///     int_mgr.load_idt();
/// }
/// ```
pub fn get_prot_interrupt_manager() -> &'static InterruptManager<PhyAddr32> {
    static INT_MANAGER: OnceCell<InterruptManager<PhyAddr32>> = OnceCell::uninit();

    INT_MANAGER.get_or_init(|| InterruptManager::new())
}

/// Returns a `64-bit` compatible `InterruptManager`, to be used in protected mode.
///
/// If you choose to use this `InterruptManager`, keep in mind that you cannot use a `32-bit` compatible `InterruptManager`
/// at the same time, as both would use the same memory space.
///
/// # Example
///
/// ```
/// let int_mgr = get_long_interrupt_manager();
///
/// unsafe {
///     int_mgr.load_idt();
/// }
/// ```
pub fn get_long_interrupt_manager() -> &'static InterruptManager<PhyAddr> {
    static INT_MANAGER: OnceCell<InterruptManager<PhyAddr>> = OnceCell::uninit();

    INT_MANAGER.get_or_init(|| InterruptManager::new())
}

/// The `InterruptManager` provides an interface to interact with the system `IDT` (_Interrupt Descriptor Table_).
///
/// It manages the `IDT` and the various interrupt handlers. Various type of handlers can be used, as described in
/// [`fzboot::irq::handlers`].
///
/// The `InterruptManager` implementation depends on the current CPU mode of the system (_protected mode_, _long mode_),
/// as the `IDT` structure varies.
///
/// This should not be constructed directly, but instead be retrieved using [`get_interrupt_manager`] that returns the
/// correct `InterruptManager` for the current CPU mode.
pub struct InterruptManager<A: MemoryAddress> {
    idt: Mutex<InterruptDescriptorTable<A>>,
    pub(super) handler_registry: RwLock<BTreeMap<InterruptVector, InterruptHandler>>,
}

impl<A: MemoryAddress> InterruptManager<A> {
    fn new() -> Self {
        let imgr = Self {
            idt: Mutex::new(InterruptDescriptorTable::new(A::NULL_PTR)),
            handler_registry: RwLock::new(BTreeMap::new()),
        };

        let default_handler_ptr: fn() = _default_int_handler;
        let descriptor = if A::WIDTH == 8 {
            let handler_ptr = VirtAddr::new(
                u64::try_from(default_handler_ptr as usize).expect("invalid handler pointer"),
            );

            GateDescriptor::new(GateType::InterruptGate)
                .with_dpl(PrivilegeLevel::Ring0)
                .with_offset(handler_ptr)
                .with_present(true)
                .with_segment_selector(
                    SegmentSelector::gdt_selector()
                        .with_index(0x10)
                        .expect("invalid segment selector while registering static handler")
                        .with_rpl(PrivilegeLevel::Ring0),
                )
        } else {
            let handler_ptr = PhyAddr::new(
                u64::try_from(default_handler_ptr as usize).expect("invalid handler pointer"),
            );

            GateDescriptor::new(GateType::InterruptGate)
                .with_dpl(PrivilegeLevel::Ring0)
                .with_offset(handler_ptr)
                .with_present(true)
                .with_segment_selector(
                    SegmentSelector::gdt_selector()
                        .with_index(0x10)
                        .expect("invalid segment selector while registering static handler")
                        .with_rpl(PrivilegeLevel::Ring0),
                )
        };

        // Setup default handlers for every interrupt vector when first loading the table.
        for int_vec in 0..=255 {
            imgr.idt
                .lock()
                .set_entry(usize::from(u8::from(int_vec)), descriptor)
                .unwrap();
        }

        unsafe {
            imgr.idt.lock().write_table().unwrap();
        }

        imgr
    }

    /// Loads the underlying `IDT` (_Interrupt Descriptor Table_) in the `IDTR` register.
    ///
    /// That should be only called once after a proper initialization of the `InterruptManager`.
    ///
    /// # Safety
    ///
    /// Highly unsafe, as it relies on raw memory writes and changes the CPU internal state.
    /// Will cause crashes or undefined behaviour if used with an incorrect `InterruptManager` configuration.
    ///
    /// # Example
    ///
    /// Enables the current configuration of the `InterruptManager`.
    ///
    /// ```
    /// let int_mgr = get_interrupt_manager();
    ///
    /// unsafe {
    ///     int_mgr.load_idt();
    /// }
    /// ```
    pub unsafe fn load_idt(&self) {
        self.idt.lock().enable();
    }

    /// Registers a dynamic interrupt handler for the given [`InterruptVector`].
    ///
    /// Dynamic interrupt handlers allow multiple handlers to be defined at runtime for a same `InterruptVector`.
    /// This however induces a slight runtime overhead compared to static handlers.
    ///
    /// The handler function does not need to be defined as an `interrupt_handler`, since it will not be used as a handler
    /// directly, but rather will be called by a standard handler that will call every dynamic handlers registered.
    ///
    /// Dynamic handlers will be executed in decreasing order of priority.
    ///
    /// Interrupts will be disabled when inserting the new handler, and enabled again after proper initialization
    /// if they were initially enabled.
    ///
    /// # Example
    ///
    /// Register a new dynamic handler for the `0x20` interrupt vector.
    ///
    /// ```
    /// let int_mgr = get_interrupt_manager();
    /// let vector = InterruptVector::from(0x20);
    ///
    /// fn test_handler() {
    ///     println("received irq !");
    /// }
    ///
    /// int_mgr.register_dynamic_handler(vector, test_handler, 10);
    /// ```
    pub fn register_dynamic_handler(
        &self,
        int_vector: InterruptVector,
        handler: fn(),
        priority: InterruptHandlerPriority,
    ) -> CanFail<HandlerRegistrationError> {
        let irq_disabled = interrupts_disabled();
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
                    let mut opt_previous_handler: Option<fn()> = None;
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

        let runtime_entry_ptr: fn() = super::handlers::__RUNTIME_HANDLER_WRAPPER_MAPPINGS
            .get(&u8::from(int_vector))
            .ok_or(HandlerRegistrationError::NoRuntimeHandlerMapping)?
            .clone();

        let descriptor = if A::WIDTH == 8 {
            let handler_ptr = VirtAddr::new(
                u64::try_from(runtime_entry_ptr as usize).expect("invalid handler pointer"),
            );

            GateDescriptor::new(GateType::InterruptGate)
                .with_dpl(PrivilegeLevel::Ring0)
                .with_offset(handler_ptr)
                .with_present(true)
                .with_segment_selector(
                    SegmentSelector::gdt_selector()
                        .with_index(0x10)
                        .expect("invalid segment selector while registering static handler")
                        .with_rpl(PrivilegeLevel::Ring0),
                )
        } else {
            let handler_ptr = PhyAddr::new(
                u64::try_from(runtime_entry_ptr as usize).expect("invalid handler pointer"),
            );

            GateDescriptor::new(GateType::InterruptGate)
                .with_dpl(PrivilegeLevel::Ring0)
                .with_offset(handler_ptr)
                .with_present(true)
                .with_segment_selector(
                    SegmentSelector::gdt_selector()
                        .with_index(0x10)
                        .expect("invalid segment selector while registering static handler")
                        .with_rpl(PrivilegeLevel::Ring0),
                )
        };

        self.idt
            .lock()
            .set_entry(usize::from(u8::from(int_vector)), descriptor)
            .map_err(|_| HandlerRegistrationError::IDTWriteError)?;

        unsafe {
            self.idt
                .lock()
                .write_table()
                .map_err(|_| HandlerRegistrationError::IDTWriteError)?;
        }

        if !irq_disabled {
            enable_interrupts();
        }

        Ok(())
    }

    /// Registers a static interrupt handler for the given [`InterruptVector`].
    ///
    /// Static interrupts handler are called without any overhead unlike dynamic interrupt handlers.
    ///
    /// The handler function should be defined as an `interrupt_handler` using the corresponding macro, in order to
    /// be properly wrapped to be directly used as a handler (that requires naked calls).
    ///
    /// Interrupts will be disabled when inserting the new handler, and enabled again after proper initialization
    /// if they were initially enabled.
    ///
    /// # Example
    ///
    /// Register a new handler for the `0x20` interrupt vector.
    ///
    /// ```
    /// let int_mgr = get_interrupt_manager();
    /// let vector = InterruptVector::from(0x20);
    ///
    /// #[interrupt_handler]
    /// fn test_handler() {
    ///     println("received irq !");
    /// }
    ///
    /// int_mgr.register_static_handler(vector, test_handler);
    /// ```
    pub fn register_static_handler(
        &self,
        int_vector: InterruptVector,
        handler: fn(),
    ) -> CanFail<HandlerRegistrationError> {
        let irq_disabled = interrupts_disabled();
        disable_interrupts();

        if self.handler_registry.read().get(&int_vector).is_some() {
            todo!()
        }

        self.handler_registry
            .write()
            .insert(int_vector, InterruptHandler::Static(handler));

        let descriptor = if A::WIDTH == 8 {
            let handler_ptr =
                VirtAddr::new(u64::try_from(handler as usize).expect("invalid handler pointer"));

            GateDescriptor::new(GateType::InterruptGate)
                .with_dpl(PrivilegeLevel::Ring0)
                .with_offset(handler_ptr)
                .with_present(true)
                .with_segment_selector(
                    SegmentSelector::gdt_selector()
                        .with_index(0x10)
                        .expect("invalid segment selector while registering static handler")
                        .with_rpl(PrivilegeLevel::Ring0),
                )
        } else {
            let handler_ptr =
                PhyAddr::new(u64::try_from(handler as usize).expect("invalid handler pointer"));

            GateDescriptor::new(GateType::InterruptGate)
                .with_dpl(PrivilegeLevel::Ring0)
                .with_offset(handler_ptr)
                .with_present(true)
                .with_segment_selector(
                    SegmentSelector::gdt_selector()
                        .with_index(0x10)
                        .expect("invalid segment selector while registering static handler")
                        .with_rpl(PrivilegeLevel::Ring0),
                )
        };

        self.idt
            .lock()
            .set_entry(usize::from(u8::from(int_vector)), descriptor)
            .map_err(|_| HandlerRegistrationError::IDTWriteError)?;

        unsafe {
            self.idt
                .lock()
                .write_table()
                .map_err(|_| HandlerRegistrationError::IDTWriteError)?;
        }

        if !irq_disabled {
            enable_interrupts();
        }

        Ok(())
    }
}

/// Errors that may happen while registering a new handler to the `InterruptManager`.
#[derive(Clone, Copy, Debug)]
pub enum HandlerRegistrationError {
    IDTWriteError,

    NoRuntimeHandlerMapping,
}
