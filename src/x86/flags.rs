use core::arch::asm;

pub struct Flags {
    flags_reg: u16,
}

impl Flags {
    pub fn read() -> Self {
        if cfg!(feature = "x86_64") {
            let register: u32;
            unsafe {
                asm!(
                "pushfq",
                "pop rax",
                out("rax") register
                );
            }
            Self {
                flags_reg: u16::try_from(register & 0xFFFF).expect("infaillible conversion"),
            }
        } else {
            let register: u16;
            unsafe {
                asm!(
                "pushf",
                "pop {0:x}",
                out(reg) register
                );
            }
            Self {
                flags_reg: register,
            }
        }
    }

    pub fn ipts_disabled(&self) -> bool {
        if (self.flags_reg & 0x200) == 0 {
            return true;
        }
        false
    }
}
