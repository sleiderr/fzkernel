use core::{
    fmt::Write,
    hint,
    sync::atomic::{AtomicBool, Ordering},
};

use alloc::format;
use fzproc_macros::interrupt_handler;

use crate::{
    irq::manager::get_interrupt_manager,
    mem::{MemoryAddress, PhyAddr},
    video::vesa::{framebuffer::RgbaColor, text_buffer},
    x86::{
        apic::InterruptVector,
        descriptors::idt::{GateDescriptor, GateType, InterruptDescriptorTable},
        int::enable_interrupts,
    },
};

static KEY_PRESSED: AtomicBool = AtomicBool::new(false);

/// Entry point when the kernel explicity panics (usually through the [`core::panic`] macro).
///
/// Only displays the message given at the panic call site, contrary to exceptions handlers that display more
/// information about the current state of the system.
pub fn panic_entry_no_exception(error_msg: &str) -> ! {
    unsafe {
        text_buffer().buffer.force_unlock();
    }
    let mut text_buffer: spin::MutexGuard<crate::video::vesa::framebuffer::TextFrameBuffer<'_>> =
        text_buffer().buffer.lock();

    text_buffer.set_background(Some(RgbaColor(255, 50, 50, 0)));
    text_buffer.clear();

    text_buffer.write_str_bitmap_centered("KERNEL PANIC", true);

    text_buffer.write_str("\n");

    text_buffer.write_str_bitmap(
        "The system encountered a fatal exception and cannot continue properly. \n\n",
    );

    let register_dump = format!("EXPLICIT_PANIC: {}\n", error_msg);
    text_buffer.write_str_bitmap(&register_dump);

    text_buffer.write_str("\n\n\n");
    text_buffer.write_str_bitmap_centered("Press any key to reboot", false);

    #[interrupt_handler]
    fn kb_handler(frame: InterruptStackFrame) {
        KEY_PRESSED.store(true, Ordering::Release);
    }

    get_interrupt_manager().register_static_handler(InterruptVector::from(0x21), kb_handler);
    enable_interrupts();

    unsafe {
        get_interrupt_manager().load_idt();
    }

    while !KEY_PRESSED.load(Ordering::Relaxed) {
        hint::spin_loop();
    }

    unsafe {
        let mut dummy_idt =
            InterruptDescriptorTable::<PhyAddr>::new(PhyAddr::NULL_PTR + 0x1000_usize);
        dummy_idt.set_entry(0x20, GateDescriptor::new(GateType::InterruptGate));

        dummy_idt.write_table();
        dummy_idt.enable();
    }

    loop {}
}
