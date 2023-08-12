//! MSR 'Model-specific registers' related utilities.
//!
//! The processor may provide various MSRs that are used to control and gather information from the
//! processor and several of its features. The MSRs are 64-bits wide registers.
//!
//! The availability of MSRs vary greatly between the different processor implementation, and
//! should be check before attempting to read an MSR.
//!
//!
//!
//! # Safety
//!
//! All MSR reading-related instructions can be used safely. Writing to a Model-specific register
//! is obviously a highly unsafe operation that supposes that a valid value was provided. Great
//! care should be taken when updating an MSR.
//!
//! # Examples
//!
//! ```
//! use flib::x86::msr;
//!
//! let tsc = msr::msr_read(0x10);
//!
//! // The requested MSR is available.
//! assert!(tsc.is_some());
//!
//! // The TSC "Time-Stamp Counter" is actually implemented as a MSR, that prints the current value
//! // of the TSC (equivalent to the `rdtsc` instruction).
//! println!("{}", tsc.unwrap());
//! ```

use core::arch::asm;

use crate::x86::cpuid::{cpu_feature_support, CPU_FEAT_MSR};

/// Reads the content of an MSR as an `Option<u64>`.
///
/// Returns `None` if the requested MSR is not available on the current system.
///
/// # Examples
///
/// ```
/// use flib::x86::msr;
///
/// let ia32_platform_id = msr::msr_read(0x17);
///
/// // MSRs are supported on this system.
/// assert!(ia32_platform_id.is_some());
///
/// // Prints the binary representation of the `IA32_PLATFORM_ID` MSR.
/// println!("{:#b}", ia32_platform_id.unwrap());
/// ```
pub fn msr_read(msr: u32) -> Option<u64> {
    let edx: u32;
    let eax: u32;

    if !cpu_feature_support(CPU_FEAT_MSR)? {
        return None;
    }

    unsafe {
        asm!("rdmsr", in("ecx") msr, out("edx") edx, out("eax") eax, options(nostack, nomem));
    }

    Some(((edx as u64) << 32) | eax as u64)
}

/// Writes to a Model-specific register.
///
/// Does nothing if MSRs are not supported on the system.
/// The `WRMSR` instructions used to update the register is serializing.
///
/// # Safety
///
/// Writing to a Model-specific register is obviously unsafe. This assumes that a valid value
/// for this MSR was provided, and that the update will not cause Undefined Behavior.
///
/// # Examples
///
/// ```
/// use flib::x86::msr;
///
/// // Setting the first bit of the `IA32_PM_ENABLE` MSR enables Hardware-Controlled
/// // Performance States.
/// msr::msr_write(0x770, 1);
/// ```
pub unsafe fn msr_write(msr: u32, value: u64) {
    let hi_val = ((value >> 32) & 0xffffffff) as u32;
    let lo_val = (value & 0xffffffff) as u32;

    if cpu_feature_support(CPU_FEAT_MSR).is_none() || !cpu_feature_support(CPU_FEAT_MSR).unwrap() {
        return;
    }

    unsafe {
        asm!("wrmsr", in("ecx") msr, in("edx") hi_val, in("eax") lo_val, options(nostack, nomem));
    }
}
