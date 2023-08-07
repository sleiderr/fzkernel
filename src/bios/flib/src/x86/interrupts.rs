use core::arch::asm;

#[inline(always)]
pub fn io_delay() {
    unsafe {
        asm!("xor al, al", "out 0x80, al");
    }
}

pub fn interrupts_disabled() -> bool {
    let flags = crate::x86::flags::Flags::read();

    return flags.ipts_disabled();
}

#[inline]
pub fn disable_interrupts() {
    unsafe {
        asm!("cli");
    }
}

#[inline]
pub fn enable_interrupts() {
    unsafe {
        asm!("sti");
    }
}
