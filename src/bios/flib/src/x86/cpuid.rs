use core::arch::asm;

pub fn cpu_id(eax: u32) -> Option<[u32; 4]> {
    let mut result = [1u32; 4];

    unsafe {
        asm!("cpuid", inout("eax") eax => result[0], out("ebx") result[1], out("ecx") result[2], out("edx") result[3]);
    }

    Some(result)
}
