use core::arch::asm;

use crate::x86::cpuid::cpu_id;

pub fn msr_get(msr: u32) -> Option<u64> {
    let edx: u32;
    let eax: u32;

    if !msr_support() {
        return None;
    }

    unsafe {
        asm!("rdmsr", in("ecx") msr, out("edx") edx, out("eax") eax, options(nostack, nomem));
    }

    Some(((edx as u64) << 32) | eax as u64)
}

pub fn msr_support() -> bool {
    if let Some(id) = cpu_id(0x1) {
        if id[3] & 0x20 != 0 {
            return true;
        }
    }

    false
}
