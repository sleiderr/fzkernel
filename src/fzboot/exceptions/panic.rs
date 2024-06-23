use core::{
    arch::asm,
    fmt::Write,
    hint,
    sync::atomic::{AtomicBool, Ordering},
};

use alloc::format;
use fzproc_macros::interrupt_handler;

use crate::{
    irq::{manager::get_interrupt_manager, ExceptionStackFrame},
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
    write_panic_header();

    let mut text_buffer: spin::MutexGuard<crate::video::vesa::framebuffer::TextFrameBuffer<'_>> =
        text_buffer().buffer.lock();

    let register_dump = format!("EXPLICIT_PANIC: {}\n", error_msg);
    text_buffer.write_str_bitmap(&register_dump);

    let base_ptr: usize;

    unsafe {
        asm!("mov {}, rbp", out(reg) base_ptr);
    }

    print_stack_trace(base_ptr as *const usize);

    drop(text_buffer);
    any_key_or_reboot()
}

pub fn panic_entry_exception(error_msg: &str, frame: ExceptionStackFrame) -> ! {
    unsafe {
        text_buffer().buffer.force_unlock();
    }

    write_panic_header();

    let mut text_buffer: spin::MutexGuard<crate::video::vesa::framebuffer::TextFrameBuffer<'_>> =
        text_buffer().buffer.lock();

    text_buffer.write_str_bitmap(&format!(
        "EXCEPTION_{} (#{:x}) STOP at {} \n",
        error_msg, frame.error_code, frame.rip
    ));

    text_buffer.write_str("\n\n\n");

    text_buffer.write_str_bitmap(&format!(
        "RSP: {:#018x}        RBP: {:#018x}        RFLAGS: {:#018x}
RAX: {:#018x}        RBX: {:#018x}        RCX: {:#018x}
RDX: {:#018x}        RSI: {:#018x}        RDI: {:#018x}
R08: {:#018x}        R09: {:#018x}        R10: {:#018x}
R11: {:#018x}        R12: {:#018x}        R13: {:#018x}
R14: {:#018x}        R15: {:#018x}        RIP: {:#018x}\n",
        u64::from(frame.stack_ptr),
        frame.registers.rbp,
        frame.rflags,
        frame.registers.rax,
        frame.registers.rbx,
        frame.registers.rcx,
        frame.registers.rdx,
        frame.registers.rsi,
        frame.registers.rdi,
        frame.registers.r8,
        frame.registers.r9,
        frame.registers.r10,
        frame.registers.r11,
        frame.registers.r12,
        frame.registers.r13,
        frame.registers.r14,
        frame.registers.r15,
        u64::from(frame.rip)
    ));

    print_stack_trace(frame.registers.rbp as *const usize);

    drop(text_buffer);
    any_key_or_reboot()
}

fn print_stack_trace(mut frame_base_ptr: *const usize) {
    unsafe {
        text_buffer().buffer.force_unlock();
    }
    let mut text_buffer: spin::MutexGuard<crate::video::vesa::framebuffer::TextFrameBuffer<'_>> =
        text_buffer().buffer.lock();

    text_buffer.write_str_bitmap("\n\nStack trace: \n");

    let mut stack_frame_pos = 0;
    while !frame_base_ptr.is_null() {
        if stack_frame_pos > 6 {
            break;
        }
        let return_addr = unsafe { *(frame_base_ptr.offset(1)) };

        if return_addr != 0 {
            text_buffer
                .write_str_bitmap(&format!("[{}] {:#018x?} \n", stack_frame_pos, return_addr));
        }
        frame_base_ptr = unsafe { *(frame_base_ptr) as *const usize };
        stack_frame_pos += 1;
    }
}

fn write_panic_header() {
    let mut text_buffer: spin::MutexGuard<crate::video::vesa::framebuffer::TextFrameBuffer<'_>> =
        text_buffer().buffer.lock();

    text_buffer.set_background(Some(RgbaColor(255, 50, 50, 0)));
    text_buffer.clear();

    text_buffer.write_str_bitmap_centered("KERNEL PANIC", true);

    text_buffer.write_str("\n");

    text_buffer.write_str_bitmap(
        "The system encountered a fatal exception and cannot continue properly. \n\n",
    );
}

fn any_key_or_reboot() -> ! {
    let mut text_buffer: spin::MutexGuard<crate::video::vesa::framebuffer::TextFrameBuffer<'_>> =
        text_buffer().buffer.lock();

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
