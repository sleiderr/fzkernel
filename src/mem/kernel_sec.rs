//! `kernelsec` module
//!
//! Contains code supposed to ensure the kernel security and help mitigate classic attacks.

use core::{
    arch::asm,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::{
    info,
    x86::{
        cpuid::{cpu_id, cpu_id_subleaf},
        msr::{Ia32ExtendedFeature, ModelSpecificRegister},
        registers::control::{ControlRegister, Cr4},
    },
};

static NX_PROT_AVAILABLE: AtomicBool = AtomicBool::new(false);

/// Checks whether the `NX` bit is available on this system or not.
///
/// The `NX` bit is used to mark some virtual memory pages as non executable, to help mitigate some attacks.
pub fn nx_prot_enabled() -> bool {
    NX_PROT_AVAILABLE.load(Ordering::Relaxed)
}

pub(crate) fn nx_bit_available() -> bool {
    if let Some(cpuid_ext_info) = cpu_id(0x80000001) {
        cpuid_ext_info[3] & (1 << 20) != 0
    } else {
        false
    }
}

pub(crate) fn smap_support() -> bool {
    if let Some(cpuid_ext_info) = cpu_id_subleaf(0x7, 0) {
        cpuid_ext_info[1] & (1 << 20) != 0
    } else {
        false
    }
}

pub(crate) fn smep_support() -> bool {
    if let Some(cpuid_ext_info) = cpu_id_subleaf(0x7, 0) {
        cpuid_ext_info[1] & (1 << 7) != 0
    } else {
        false
    }
}

/// Sets the `AC` bit in the flags register (_Access Control_ / _Alignment Check_ bit).
///
/// This can be used to temporarily disable the `SMAP` protection, which can the be enabled back using the
/// [`clac`] instruction.
#[inline(always)]
pub fn stac() {
    unsafe {
        asm!("stac", options(nomem, nostack));
    }
}

/// Clears the `AC` bit in the flags register (_Access Control_ / _Alignment Check_ bit).
///
/// This enables back the `SMAP` protection if it was temporarily disabled using the [`stac`] instruction.
#[inline(always)]
pub fn clac() {
    unsafe {
        asm!("clac", options(nomem, nostack));
    }
}

/// Checks if some memory-related security features are available on the system, and enables them if applicable.
///
/// Tries to enable `SMAP` (_Supervisor Mode Access Prevention_) and `SMEP` (_Supervisor Mode Execution Prevention_), to prevent
/// the kernel from accessing or fetching instructions from pages that are accessible in user mode.
///
/// Also enables the `NX` bit for memory pages if available, allowing some parts of the virtual memory to be marked as non executable, such as
/// the kernel stack, to help mitigate some attacks.
pub fn enable_kernel_mem_sec() {
    if nx_bit_available() {
        if let Some(msr) = Ia32ExtendedFeature::read() {
            Ia32ExtendedFeature::write(msr.with_nxe(true));
        }

        NX_PROT_AVAILABLE.store(true, Ordering::Release);

        info!("vmsec", "enabling NX bit for kernel memory pages");
    }

    if smap_support() {
        let mut cr4_reg = Cr4::read();
        Cr4::write(cr4_reg.with_smap(true));

        clac();

        info!(
            "vmsec",
            "SMAP available on this system, enabling ... (vmsec_user_krw_prot)"
        );
    }

    if smep_support() {
        Cr4::write(Cr4::read().with_smep(true));

        info!(
            "vmsec",
            "SMEP available on this system, enabling ... (vmsec_user_kexec_prot)"
        );
    }
}
