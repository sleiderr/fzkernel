//! _Control Registers_ implementation (read / write).
//!
//! They are used to control the operating mode of the processor and the characteristics of the currently executing
//! task. These registers are 32-bit wide in 32-bit modes (and compatibility mode), but they are extended to 64-bit
//! for 64-bit mode.

#![allow(clippy::must_use_candidate)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::missing_errors_doc)]

use crate::errors::InvalidAddress;
use crate::mem::{Alignment, MemoryAddress, PhyAddr, PhyAddr32};
use core::arch::asm;
use modular_bitfield::bitfield;
use modular_bitfield::prelude::{B10, B20, B3, B32, B39, B52, B7};

#[cfg(feature = "x86_64")]
#[bitfield]
#[derive(Clone, Copy, Debug)]
#[repr(u64)]
pub struct Cr0 {
    /// Enables protected mode when set.
    protection_enable: bool,

    /// Controls the interaction of the _WAIT_ (or _FWAIT_) instruction with the _TS_ flag.
    monitor_coprocessor: bool,

    /// Indicates that the processor does not have an external _x87 FPU_.
    emulation: bool,

    /// Processor sets this flag on every task switch, and allows the saving of the _x87_ _FPU_/_MMX_/_SSE_ ... context
    /// on a task switch to be delayed until an instruction is actually executed by the new task.
    task_switched: bool,

    extension_type: bool,

    /// Enables the native mechanism for reporting _x87 FPU_ errors.
    numeric_error: bool,
    #[skip]
    __: B10,

    /// Inhibits supervisor-level procedures from writing into read-only pages.
    write_protect: bool,

    #[skip]
    __: bool,

    /// Enables automatic alignment checking when set.
    alignment_mask: bool,

    #[skip]
    __: B10,

    not_write_through: bool,
    cache_disable: bool,

    /// Enables paging.
    paging: bool,

    #[skip]
    __: B32,
}

/// _Control Register 0_ structure.
///
/// Contains system control flags that control operating mode and states of the processor.
#[cfg(not(feature = "x86_64"))]
#[bitfield]
#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub struct Cr0 {
    /// Enables protected mode when set.
    pub protection_enable: bool,

    /// Controls the interaction of the _WAIT_ (or _FWAIT_) instruction with the _TS_ flag.
    pub monitor_coprocessor: bool,

    /// Indicates that the processor does not have an external _x87 FPU_.
    pub emulation: bool,

    /// Processor sets this flag on every task switch, and allows the saving of the _x87_ _FPU_/_MMX_/_SSE_ ... context
    /// on a task switch to be delayed until an instruction is actually executed by the new task.
    pub task_switched: bool,

    pub extension_type: bool,

    /// Enables the native mechanism for reporting _x87 FPU_ errors.
    pub numeric_error: bool,
    #[skip]
    __: B10,

    /// Inhibits supervisor-level procedures from writing into read-only pages.
    pub write_protect: bool,

    #[skip]
    __: bool,

    /// Enables automatic alignment checking when set.
    pub alignment_mask: bool,

    #[skip]
    __: B10,

    pub not_write_through: bool,

    pub cache_disable: bool,

    /// Enables paging.
    pub paging: bool,
}

impl ControlRegister for Cr0 {
    fn read() -> Self {
        #[cfg(not(target_arch = "x86_64"))]
        let mut cr_bits: u32;
        #[cfg(target_arch = "x86_64")]
        let mut cr_bits: u64;
        unsafe {
            asm!(
            "mov {}, cr0",
            out(reg) cr_bits,
            options(nomem, nostack)
            )
        }

        Self::from(cr_bits)
    }

    fn write(self) {
        #[cfg(not(target_arch = "x86_64"))]
        let cr_bits = u32::from(self);
        #[cfg(target_arch = "x86_64")]
        let cr_bits = u64::from(self);
        unsafe {
            asm!(
            "mov cr0, {}",
            in(reg) cr_bits,
            options(nomem, nostack)
            )
        }
    }
}

#[cfg(feature = "x86_64")]
#[bitfield]
pub struct Cr3 {
    #[skip]
    __: B3,

    /// Page-level Cache Disable.
    ///
    /// Controls the memory type used to access the first paging structure of the current paging-structure
    /// hierarchy.
    cache: bool,

    /// Page-level Write-Through.
    ///
    /// Controls the memory type used to access the first paging structure of the current paging-structure
    /// hierarchy.
    write_through: bool,
    #[skip]
    __: B7,
    pdt_addr: B52,
}

/// _Control Register 3_ structure.
///
/// Contains the physical address of the base paging-structure hierarchy, and two additional flags.
#[cfg(not(feature = "x86_64"))]
#[bitfield]
#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub struct Cr3 {
    #[skip]
    __: B3,

    /// Page-level Cache Disable.
    ///
    /// Controls the memory type used to access the first paging structure of the current paging-structure
    /// hierarchy.
    pub cache: bool,

    /// Page-level Write-Through.
    ///
    /// Controls the memory type used to access the first paging structure of the current paging-structure
    /// hierarchy.
    pub write_through: bool,
    #[skip]
    __: B7,
    pdt_addr: B20,
}

impl Cr3 {
    /// Returns the physical address of the base paging-structure hierarchy.
    #[cfg(not(feature = "x86_64"))]
    pub fn page_table_addr(&self) -> PhyAddr32 {
        PhyAddr32::from(self.pdt_addr() << 12)
    }

    /// Sets the physical address of the base paging-structure hierarchy.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidAddress::InvalidAlignment`] if the given address is not page-aligned (4KB aligned).
    #[cfg(not(feature = "x86_64"))]
    pub fn set_page_table_addr(self, addr: PhyAddr32) -> Result<Self, InvalidAddress> {
        if !addr.is_aligned_with(Alignment::ALIGN_4KB).unwrap() {
            return Err(InvalidAddress::InvalidAlignment);
        }
        Ok(self.with_pdt_addr(u32::from(addr) >> 12))
    }

    /// Returns the physical address of the base paging-structure hierarchy.
    #[cfg(feature = "x86_64")]
    pub fn page_table_addr(&self) -> PhyAddr {
        PhyAddr::from(self.pdt_addr() << 12)
    }

    /// Sets the physical address of the base paging-structure hierarchy.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidAddress::InvalidAlignment`] if the given address is not page-aligned (4KB aligned).
    #[cfg(feature = "x86_64")]
    pub fn set_page_table_addr(self, addr: PhyAddr) -> Result<Self, InvalidAddress> {
        if !addr.is_aligned_with(Alignment::ALIGN_4KB) {
            return Err(InvalidAddress::InvalidAlignment);
        }
        Ok(self.with_pdt_addr(u64::from(addr) >> 12))
    }
}

#[cfg(not(feature = "x86_64"))]
impl ControlRegister for Cr3 {
    fn read() -> Self {
        #[cfg(not(target_arch = "x86_64"))]
        let mut cr_bits: u32;
        #[cfg(target_arch = "x86_64")]
        let mut cr_bits: u64;
        unsafe {
            asm!(
                "mov {}, cr3",
                out(reg) cr_bits,
                options(nomem, nostack)
            )
        }

        Self::from(cr_bits)
    }

    fn write(self) {
        #[cfg(not(target_arch = "x86_64"))]
        let cr_bits = u32::from(self);
        #[cfg(target_arch = "x86_64")]
        let cr_bits = u64::from(self);
        unsafe {
            asm!(
                "mov cr3, {}",
                in(reg) cr_bits,
                options(nomem, nostack)
            )
        }
    }
}

/// _Control Register 4_ structure.
///
/// Contains a group of flags that enable several architectural extensions, and indicate operating system support
/// for specific processor capabilities.
#[bitfield]
#[derive(Clone, Copy, Debug)]
#[cfg(not(feature = "x86_64"))]
#[repr(u32)]
pub struct Cr4 {
    /// Virtual-8086 Mode Extensions.
    ///
    /// Enables interrupt- and exception-handling extensions in _vm8086_ when set.
    pub vm8086_ext: bool,

    /// Protected-Mode Virtual Interrupts.
    ///
    /// Enables hardware support for a _virtual interrupt flag_ when set.
    pub pm_vif: bool,

    /// Time Stamp Disable.
    ///
    /// Restricts _RDTSC_ instruction to privilege 0 when set.
    pub tsd: bool,

    /// Debugging extensions.
    pub debug_ext: bool,

    /// Page Size extensions.
    ///
    /// Enables 4MB pages for 32-bit paging when set.
    pub paging_ext: bool,

    /// Physical Address Extension.
    ///
    /// When set, enables paging to produce physical address with more than 32bits.
    pub phys_addr_ext: bool,

    /// Machine-Check Enable.
    ///
    /// Enables the machine-check exception when set.
    pub machine_check: bool,

    /// Page Global Enable.
    ///
    /// Enables the global page feature when set. It allows frequently used pages to be marked as global to all users,
    /// and therefore they are not flushed from the _TLB_ on a task switch.
    pub page_global: bool,

    /// Performance-Monitoring Counter Enable.
    pub perf_mon_count: bool,

    /// OS Support for _FXSAVE_ and _FXRSTOR_ instructions.
    pub osfxsr: bool,

    /// OS Support for _Unmasked SIMD Floating-Point Exceptions_.
    pub osxmmexcpt: bool,

    /// User-Mode Instruction Prevention.
    ///
    /// When set, _SGDT_, _SIDT_, _SLDT_, _STR_ and _SMSW_ instructions cannot be executed in user-mode (_CPL_ > 0).
    pub umip: bool,

    /// 57-bit linear addresses.
    ///
    /// When set in _IA-32e mode_, processor uses 5-level paging. Cannot be modified in _IA-32e mode_.
    pub la57: bool,

    /// VMX-Enable.
    pub vmx: bool,

    /// SMX-Enable.
    pub smx: bool,

    /// Enables the _RDFSBASE_, _RDGSBASE_, _WRFSBASE_ and _WRGSBASE_ instructions.
    pub fsgsbase: bool,

    /// PCID-Enable.
    ///
    /// Enables process-context identifier when set.
    pub pcid: bool,

    /// XSave and Processor Extended States-Enable.
    pub osxsave: bool,

    /// Key-Locker-Enable.
    pub kl: bool,

    /// SMEP-Enable.
    ///
    /// Enables supervisor-mode execution prevention when set.
    pub smep: bool,

    /// SMAP-Enable.
    ///
    /// Enables supervisor-mode access prevention when set.
    pub smap: bool,

    /// Enables protection key for user-mode pages.
    pub pke: bool,

    /// Control-flow Enforcement Technology.
    pub cet: bool,

    /// Enables protection key for supervisor-mode pages.
    pub pks: bool,

    /// User Interrupts Enable.
    ///
    /// Enables user interrupts when set, including user-interrupt delivery, notification identification and
    /// instructions.
    pub uintr: bool,

    #[skip]
    __: B7,
}

impl ControlRegister for Cr4 {
    fn read() -> Self {
        #[cfg(not(target_arch = "x86_64"))]
        let mut cr_bits: u32;
        #[cfg(target_arch = "x86_64")]
        let mut cr_bits: u64;
        unsafe {
            asm!(
            "mov {}, cr4",
            out(reg) cr_bits,
            options(nomem, nostack)
            )
        }

        Self::from(cr_bits)
    }

    fn write(self) {
        #[cfg(not(target_arch = "x86_64"))]
        let cr_bits = u32::from(self);
        #[cfg(target_arch = "x86_64")]
        let cr_bits = u64::from(self);
        unsafe {
            asm!(
            "mov cr4, {}",
            in(reg) cr_bits,
            options(nomem, nostack)
            )
        }
    }
}

pub trait ControlRegister {
    /// Reads the current content of the _Control Register_.
    ///
    /// # Examples
    ///
    /// ```
    /// let cr0 = Cr0::read();
    /// assert!(cr0.protection_enable());
    /// ```
    fn read() -> Self;

    /// Updates the current content of the _Control Register_.
    ///
    /// # Examples
    ///
    /// ```
    /// Cr0::write(Cr0::read().set_protection_enable(true));
    /// assert!(cr0.protection_enable());
    /// ```
    fn write(self);
}
