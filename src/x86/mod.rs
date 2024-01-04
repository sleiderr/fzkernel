#![allow(unreachable_pub)]
#![allow(trivial_casts)]
#![allow(clippy::no_effect_underscore_binding)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::identity_op)]
#![allow(clippy::semicolon_if_nothing_returned)]

pub mod cpuid;
pub mod flags;
pub mod msr;
pub mod tsc;

#[cfg(feature = "alloc")]
pub mod idt;

#[cfg(feature = "alloc")]
pub mod apic;

pub mod registers;

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
