use core::arch::asm;

#[inline(always)]
pub fn io_delay() {

    unsafe {
        asm!(
        "xor al, al",
        "out 0x80, al"
        );
    }

}

pub fn interrupts_disabled() -> bool {

    let flags = crate::x86::flags::Flags::read();

    return flags.ipts_disabled();

}

pub fn disable_interrupts() -> Result<(), ()> {

    unsafe {
        asm!("cli");
    }

    if interrupts_disabled() {
        return Ok(());
    }

    Err(())

}

pub fn enable_interrupts() -> Result<(), ()>{

    unsafe {
        asm!("sti");
    }

    if !interrupts_disabled() {
        return Ok(());
    }

    Err(())

}
