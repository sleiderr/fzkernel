pub mod flags;

#[cfg(feature = "alloc")]
pub mod idt;

pub mod int {
    use core::arch::asm;
    pub fn interrupts_disabled() -> bool {
        let flags = super::flags::Flags::read();
        flags.ipts_disabled()
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
}
