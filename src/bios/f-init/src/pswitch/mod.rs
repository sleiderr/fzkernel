use core::{arch::asm, hint::unreachable_unchecked};

pub mod a20;
pub mod gdt;

pub fn protected_jump() -> ! {
    unsafe {
        asm!(
            "mov ax, 0x08",
            "mov ds, ax",
            "mov es, ax",
            "mov fs, ax",
            "mov gs, ax",
            "mov ss, ax",
            "mov eax, cr0",
            "or eax, 1",
            "mov cr0, eax",
            /*".byte 0x66",
            ".byte 0xea",
            ".int 0x01000000",
            ".word 0x10"*/
        );
        unreachable_unchecked();
    }
}
