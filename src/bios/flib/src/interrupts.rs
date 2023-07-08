use core::arch::asm;

pub fn disable_interrupts() {

    unsafe {
        asm!("cli");
    }

}

pub fn enable_interrupts() {

    unsafe {
        asm!("sti");
    }

}
