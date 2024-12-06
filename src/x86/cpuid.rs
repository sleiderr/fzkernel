//! `CPUID` related utilities.
//!
//! The `CPUID` instruction, if available, provides valuable information about the processor and
//! its supported features.
//!
//! Several "leaves" are available, either basic or extended, and correspond to different types of
//! information. The maximum value for these leaves can be checked using `cpu_id_max_leaf` and
//! `cpu_id_max_extended_leaf`.
//!
//! Finally, the `CPUID` instruction is serializing, and can therefore be used also to serialize
//! instruction execution.
//!
//! # Safety
//!
//! All `CPUID` related functions can be used safely.
//!
//! # Examples:
//!
//! ```
//! use flib::x86::cpuid;
//!
//! let res: Option<[u8; 4]> = cpuid::cpu_id(0);
//!
//! // `CPUID` instructions are available.
//! assert!(res.is_some());
//!
//! // EAX (first element of the resulting array) contains the maximum value for basic `CPUID`.
//! println!("Maximum basic leaf number: {}", res[0]);
//! ```
//!
//! This can also be used to check if a CPU feature is available:
//!
//! ```
//! use flib::x86::cpuid;
//!
//! // Make sure that the system supports the SSE3 extension.
//! assert!(cpuid::cpu_feature_support(cpuid::CPU_FEAT_SSE3));
//! ```

use core::arch::asm;

#[cfg(feature = "alloc")]
use alloc::{string::String, vec::Vec};

/// Defines a CPU feature retrieved from the 01h leaf in the `ecx` register.
macro_rules! cpu_feature_ecx {
    ($name: tt, $mask: expr, $desc: literal) => {
        #[doc = $desc]
        pub const $name: (u8, u32) = (0, $mask);
    };
}

/// Defines a CPU feature retrieved from the 01h leaf in the `edx` register.
macro_rules! cpu_feature_edx {
    ($name: tt, $mask: expr, $desc: literal) => {
        #[doc = $desc]
        pub const $name: (u8, u32) = (1, $mask);
    };
}

cpu_feature_ecx!(CPU_FEAT_SSE3, 1 << 0, "Streaming SIMD Extensions 3");

cpu_feature_ecx!(
    CPU_FEAT_PCLMULQDQ,
    1 << 1,
    "PCLMULQDQ instruction support.
    \n\nComputes the 128-bit carry-less product of two 64-bits values"
);

cpu_feature_ecx!(CPU_FEAT_DTES64, 1 << 2, "64-bit DS Area support.");

cpu_feature_ecx!(CPU_FEAT_MONITOR, 1 << 3, "MONITOR/MWAIT feature support.");

cpu_feature_ecx!(CPU_FEAT_DSCPL, 1 << 4, "CPL Qualified Debug Store");

cpu_feature_ecx!(CPU_FEAT_VMX, 1 << 5, "Virtual Machine Extensions support.");

cpu_feature_ecx!(CPU_FEAT_SMX, 1 << 6, "Safer Mode Extensions support.");

cpu_feature_ecx!(
    CPU_FEAT_EIST,
    1 << 7,
    "Enhanced Intel SpeedStep technology."
);

cpu_feature_ecx!(CPU_FEAT_TM2, 1 << 8, "Thermal Monitor 2 support.");

cpu_feature_ecx!(
    CPU_FEAT_SSSE3,
    1 << 9,
    "Presence of the Supplemental Streaming SIMD Extensions 3."
);

cpu_feature_ecx!(CPU_FEAT_CNXTID, 1 << 10, "L1 Context ID.
    \n\nIf set, indicates that the L1 data cache mode can be set to either adaptive mode or shared mode.");

cpu_feature_ecx!(
    CPU_FEAT_SDBG,
    1 << 11,
    "Support of `IA32_DEBUG_INTERFACE MSR` for silicon debug"
);

cpu_feature_ecx!(
    CPU_FEAT_FMA,
    1 << 12,
    "FMA Extensions (SSE Extensions) using YMM state"
);

cpu_feature_ecx!(
    CPU_FEAT_CMPXCHG16B,
    1 << 13,
    "CMPXCHG16B (Compare and Exchange Bytes) support"
);

cpu_feature_ecx!(
    CPU_FEAT_XTPR_CTRL,
    1 << 14,
    "CPU supports changing `IA32_MISC_ENABLE[23]`"
);

cpu_feature_ecx!(
    CPU_FEAT_PDCM,
    1 << 15,
    "Perfmon and Debug Capability.
    \n\nCPU supports the performance and debug feature indication MSR `IA32_PERF_CAPABILITIES`"
);

cpu_feature_ecx!(
    CPU_FEAT_PCID,
    1 << 17,
    "Process-context identifiers support.
    \n\nSoftware may set CR4.PCIDE to 1"
);

cpu_feature_ecx!(
    CPU_FEAT_DCA,
    1 << 18,
    "CPU supports ability to prefetch data from a mmapped device"
);

cpu_feature_ecx!(CPU_FEAT_SSE4_1, 1 << 19, "CPU supports SSE4.1");

cpu_feature_ecx!(CPU_FEAT_SSE4_2, 1 << 20, "CPU supports SSE4.2");

cpu_feature_ecx!(CPU_FEAT_X2APIC, 1 << 21, "CPU supports the x2APIC feature.");

cpu_feature_ecx!(
    CPU_FEAT_MOVBE,
    1 << 22,
    "CPU supports MOVBE (move after swapping bytes) instruction"
);

cpu_feature_ecx!(
    CPU_FEAT_POPCNT,
    1 << 23,
    "CPU supports POPCNT (counts of bit set to 1) instruction"
);

cpu_feature_ecx!(
    CPU_FEAT_TSC_DEADLINE,
    1 << 24,
    "CPU's local ACPI timer supports one-shot operation
    using TSC deadline"
);

cpu_feature_ecx!(
    CPU_FEAT_AESNI,
    1 << 25,
    "CPU supports AESNI extensions (AES-related instructions)"
);

cpu_feature_ecx!(
    CPU_FEAT_XSAVE,
    1 << 26,
    "CPU supports XSAVE/XRSTOR processor extended state features,
    the XSETBV/XGETBV instructions and XCR0"
);

cpu_feature_ecx!(
    CPU_FEAT_OSXSAVE,
    1 << 27,
    "OS has set `CR4.OSXSAVE[18]` to enable XSETBV/XGETBV instructions
    to access XCR0 and to support processor extended state management using XSAVE/XRSTOR"
);

cpu_feature_ecx!(
    CPU_FEAT_AVX,
    1 << 28,
    "CPU supports AVX (Advanced Vector Extensions) extensions"
);

cpu_feature_ecx!(
    CPU_FEAT_F16C,
    1 << 29,
    "CPU supports 16-bit floating point conversion instructions"
);

cpu_feature_ecx!(
    CPU_FEAT_RDRAND,
    1 << 30,
    "CPU supports RDRAND (Read Random Number) instruction"
);

cpu_feature_edx!(
    CPU_FEAT_FPU,
    1 << 0,
    "Floating-Point Unit On-Chip.
    \n\nThe processor contains an x87 FPU."
);

cpu_feature_edx!(CPU_FEAT_VME, 1 << 1, "Virtual 8086 Mode Enhancements.");

cpu_feature_edx!(CPU_FEAT_DE, 1 << 2, "Debugging Extensions.
    \n\nSupport for I/O breakpoints, CR4.DE to control the feature and trapping accesses to DR4 and DR5");

cpu_feature_edx!(CPU_FEAT_PSE, 1 << 3, "Page Size Extension.");

cpu_feature_edx!(CPU_FEAT_TSC, 1 << 4, "Time Stamp Counter support.");

cpu_feature_edx!(
    CPU_FEAT_MSR,
    1 << 5,
    "Model Specific Registers RDMSR and WDMSR Instructions"
);

cpu_feature_edx!(
    CPU_FEAT_PAE,
    1 << 6,
    "Physical Address Extension.
    \n\nPhysical addresses greater than 32 bits are supported."
);

cpu_feature_edx!(CPU_FEAT_MCE, 1 << 7, "Machine Check Exception");

cpu_feature_edx!(
    CPU_FEAT_CX8,
    1 << 8,
    "CMPXCHG8B (Compare And Exchange Bytes) Instruction support"
);

cpu_feature_edx!(CPU_FEAT_APIC, 1 << 9, "APIC On-Chip");

cpu_feature_edx!(CPU_FEAT_SEP, 1 << 11, "SYSENTER and SYSEXIT Instructions");

cpu_feature_edx!(CPU_FEAT_MTRR, 1 << 12, "Memory Type Range Registers");

cpu_feature_edx!(CPU_FEAT_PGE, 1 << 13, "Page Global Bit");

cpu_feature_edx!(CPU_FEAT_MCA, 1 << 14, "Machine Check Architecture.");

cpu_feature_edx!(
    CPU_FEAT_CMOV,
    1 << 15,
    "Conditional Move Instructions CMOV support.
    \n\nIf x87 FPU is present, FCOMI et FCMOV instructions are supported"
);

cpu_feature_edx!(CPU_FEAT_PAT, 1 << 16, "Page Attribute Table support.");

cpu_feature_edx!(CPU_FEAT_PSE36, 1 << 17, "36-Bit Page Size Extension");

cpu_feature_edx!(
    CPU_FEAT_PSN,
    1 << 18,
    "Processor Serial Number.
    \n\nThis CPU supports the 96-bit processor identification number"
);

cpu_feature_edx!(
    CPU_FEAT_CLFSH,
    1 << 19,
    "CLFLUSH (Flush Cache Line) Instruction support."
);

cpu_feature_edx!(CPU_FEAT_DS, 1 << 21, "Debug Store.
    \n\nThis processor supports the ability to write debug information into a memory resident buffer.");

cpu_feature_edx!(
    CPU_FEAT_ACPI,
    1 << 22,
    "Thermal Monitor and Software Controlled Clock Facilities."
);

cpu_feature_edx!(CPU_FEAT_MMX, 1 << 23, "Intel MMX Technology support");

cpu_feature_edx!(CPU_FEAT_FXSR, 1 << 24, "FXSAVE and FXRSTOR Instructions.
    \n\nFXSAVE and FXRSTOR instructions are supported for fast save and restore of the floating-point context.");

cpu_feature_edx!(CPU_FEAT_SSE, 1 << 25, "SSE Extensions support.");

cpu_feature_edx!(CPU_FEAT_SSE2, 1 << 26, "SSE2 Extensions support.");

cpu_feature_edx!(CPU_FEAT_SS, 1 << 27, "Self Snoop");

cpu_feature_edx!(
    CPU_FEAT_HTT,
    1 << 28,
    "Max APIC IDs reserved field is Valid."
);

cpu_feature_edx!(CPU_FEAT_TM, 1 << 29, "Thermal Monitor (TCC implemented).");

cpu_feature_edx!(CPU_FEAT_PBE, 1 << 31, "Pending Break Enable.");

/// Intel's CPU models.
pub enum IntelCpuModel {
    RaptorLakeS,
    RaptorLakeP,
    AlderLakeS,
    AlderLakeP,
    RocketLakeS,
    TigerLakeH,
    TigerLakeU,
    IceLake,
    CometLakeS,
    CannonLake,
    KabylakeS,
    KabylakeY,
    SkylakeS,
    SkylakeY,
    BroadwellS,
    BroadwellH,
    HaswellS,
    HaswellU,
    HaswellG,
    IvyBridge,
    SandyBridge,
    Westmere,
    Nehalem,
    SkylakeServer,
    BroadwellServerE,
    BroadwellServerDE,
    HaswellServer,
    IvyBridgeServer,
    SandyBridgeServer,
    WestmereServer,
    NehalemServer,
    GoldmontA,
    Unknown(u8),
}

impl From<u8> for IntelCpuModel {
    fn from(value: u8) -> Self {
        match value {
            0xB7 => Self::RaptorLakeS,
            0xBA => Self::RaptorLakeP,
            0x97 => Self::AlderLakeS,
            0x9A => Self::AlderLakeP,
            0xA7 => Self::RocketLakeS,
            0x8D => Self::TigerLakeH,
            0x8C => Self::TigerLakeU,
            0x7E => Self::IceLake,
            0xA5 => Self::CometLakeS,
            0x66 => Self::CannonLake,
            0x8E => Self::KabylakeY,
            0x9E => Self::KabylakeS,
            0x5E => Self::SkylakeS,
            0x4E => Self::SkylakeY,
            0x3D => Self::BroadwellS,
            0x47 => Self::BroadwellH,
            0x3C => Self::HaswellS,
            0x45 => Self::HaswellU,
            0x46 => Self::HaswellG,
            0x3A => Self::IvyBridge,
            0x2A => Self::SandyBridge,
            0x25 => Self::Westmere,
            0x1E => Self::Nehalem,
            0x55 => Self::SkylakeServer,
            0x4F => Self::BroadwellServerE,
            0x56 => Self::BroadwellServerDE,
            0x3F => Self::HaswellServer,
            0x3E => Self::IvyBridgeServer,
            0x2D => Self::SandyBridgeServer,
            0x2C => Self::WestmereServer,
            0x2F => Self::WestmereServer,
            0x2E => Self::NehalemServer,
            0x1A => Self::NehalemServer,
            0x5C => Self::GoldmontA,
            unknown => Self::Unknown(unknown),
        }
    }
}

/// Check if a CPU feature is supported.
///
/// Uses the `CPUID` instruction, that may not be supported on some systems.
pub fn cpu_feature_support(code: (u8, u32)) -> Option<bool> {
    let features = cpu_id(0x1)?;
    match code.0 {
        0 => Some((features[2] & code.1) != 0),
        1 => Some((features[3] & code.1) != 0),
        _ => None,
    }
}

#[cfg(feature = "alloc")]
/// Returns the CPU Brand String, if available.
pub fn cpu_brand_string() -> Option<String> {
    let mut str_bytes_1: Vec<u8> = cpu_id(0x80000002)?
        .into_iter()
        .flat_map(|b| b.to_ne_bytes())
        .collect();
    let mut str_bytes_2: Vec<u8> = cpu_id(0x80000003)?
        .into_iter()
        .flat_map(|b| b.to_ne_bytes())
        .collect();
    let mut str_bytes_3: Vec<u8> = cpu_id(0x80000004)?
        .into_iter()
        .flat_map(|b| b.to_ne_bytes())
        .collect();

    str_bytes_1.append(&mut str_bytes_2);
    str_bytes_1.append(&mut str_bytes_3);

    str_bytes_1.retain(|b| b.is_ascii());
    str_bytes_1.retain(|b| (*b != 0));

    let str = core::str::from_utf8(str_bytes_1.as_slice()).ok()?;

    Some(String::from(str))
}

#[cfg(feature = "alloc")]
/// Returns the CPU Vendor String, if available.
pub fn cpu_vendor_string() -> Option<String> {
    let res = cpu_id(0)?;

    let str_bytes_1 = res[1].to_ne_bytes();
    let str_bytes_2 = res[3].to_ne_bytes();
    let str_bytes_3 = res[2].to_ne_bytes();
    let mut str_bytes: [u8; 12] = [0; 12];

    str_bytes[..4].copy_from_slice(&str_bytes_1);
    str_bytes[4..8].copy_from_slice(&str_bytes_2);
    str_bytes[8..12].copy_from_slice(&str_bytes_3);

    let str: &str = core::str::from_utf8(&str_bytes).unwrap();

    Some(String::from(str))
}

pub fn cpu_family_id() -> Option<u8> {
    let res = cpu_id(1)?;
    let family_id = ((res[0] >> 8) & 0xf) as u8;

    if family_id == 0x0f {
        let extended_family = ((res[0] >> 20) & 0xff) as u8;

        return Some(family_id + extended_family);
    }

    Some(family_id)
}

pub fn cpu_model_id() -> Option<IntelCpuModel> {
    let res = cpu_id(1)?;
    let model_id = ((res[0]) >> 4 & 0xf) as u8;
    let family_id = ((res[0] >> 8) & 0xf) as u8;

    if family_id == 0x0f || family_id == 0x06 {
        let extended_model_id = ((res[0] >> 16) & 0xf) as u8;
        return Some(((extended_model_id << 4) + model_id).into());
    }

    Some(model_id.into())
}

/// Executes a `CPUID` operation, using the argument as the input value in `eax`.
///
/// It returns processor identification and available features information.
/// Two types of information are available, basic or extended (0x80000000 and above).
///
/// It checks if the requested CPUID leaf is available on this system, and then
/// returns the content of eax, ebx, ecx and edx in order, after the `CPUID` call.
pub fn cpu_id(eax: u32) -> Option<[u32; 4]> {
    let mut result = [0u32; 4];

    // Check if the CPUID instruction is supported, and if the requested leaf is available.
    if !(cpu_id_support() & cpu_id_leaf_support(eax)) {
        return None;
    }

    #[cfg(not(feature = "x86_64"))]
    unsafe {
        asm!("cpuid", inout("eax") eax => result[0], out("ebx") result[1], out("ecx") result[2], out("edx") result[3]);
    }

    #[cfg(feature = "x86_64")]
    unsafe {
        asm!("push rbx", "cpuid", "mov edi, ebx", "pop rbx", inout("eax") eax => result[0], out("edi") result[1], out("ecx") result[2], out("edx") result[3]);
    }

    Some(result)
}

pub fn cpu_id_subleaf(eax: u32, ecx: u32) -> Option<[u32; 4]> {
    let mut result = [0u32; 4];

    // Check if the CPUID instruction is supported, and if the requested leaf is available.
    if !(cpu_id_support() & cpu_id_leaf_support(eax)) {
        return None;
    }

    #[cfg(not(feature = "x86_64"))]
    unsafe {
        asm!("cpuid", inout("eax") eax => result[0], out("ebx") result[1], inout("ecx") ecx => result[2], out("edx") result[3]);
    }

    #[cfg(feature = "x86_64")]
    unsafe {
        asm!("push rbx", "cpuid", "mov edi, ebx", "pop rbx", inout("eax") eax => result[0], out("edi") result[1], inout("ecx") ecx => result[2], out("edx") result[3]);
    }

    Some(result)
}

/// Checks if a CPUID leaf (basic or extended) is available on this system.
pub fn cpu_id_leaf_support(val: u32) -> bool {
    if val & 0x80000000 != 0 {
        return val <= (0x80000000 | cpu_id_max_extended_leaf());
    }

    val <= cpu_id_max_leaf()
}

/// Returns the maximum basic CPUID leaf available on this system.
pub fn cpu_id_max_leaf() -> u32 {
    let result: u32;

    if cfg!(feature = "x86_64") {
        unsafe {
            asm!("push rbx", "xor eax, eax", "cpuid", "pop rbx", out("eax") result);
        }
    } else {
        unsafe {
            asm!("xor eax, eax", "cpuid", out("eax") result);
        }
    }

    result
}

/// Returns the maximum extended CPUID leaf available on this system.
pub fn cpu_id_max_extended_leaf() -> u32 {
    let result_ext: u32;

    if cfg!(feature = "x86_64") {
        unsafe {
            asm!("push rbx", "mov eax, 0x80000000", "cpuid", "pop rbx", out("eax") result_ext);
        }
    } else {
        unsafe {
            asm!("mov eax, 0x80000000", "cpuid", out("eax") result_ext);
        }
    }

    result_ext
}

/// Checks if the system supports the CPUID operation.
///
/// It tries to flip the bit 21 of the EFLAGS register, which shows if CPUID is available or not.
pub fn cpu_id_support() -> bool {
    let eax: u32;

    unsafe {
        #[cfg(not(feature = "x86_64"))]
        asm!(
            // Save the EFLAGS register.
            "pushfd",
            // Push it again, this time to modify it.
            "pushfd",
            // We flip the bit 21 of the EFLAGS register.
            "xor dword ptr [esp], 0x200000",
            // We put it back in the EFLAGS register.
            "popfd",
            // We push it again, to check if the bit flip succeeded.
            "pushfd",
            // We put the initial EFLAGS value into eax.
            "pop eax",
            // And finally we compare it to the value after the bit flip.
            "xor eax, [esp]",
            "popfd",
            // If eax != 0, the bit flip was successful.
            "and eax, 0x200000",
            out("eax") eax
        );

        #[cfg(feature = "x86_64")]
        asm!(
            // Save the EFLAGS register.
            "pushfq",
            // Push it again, this time to modify it.
            "pushfq",
            // We flip the bit 21 of the EFLAGS register.
            "xor dword ptr [rsp], 0x200000",
            // We put it back in the EFLAGS register.
            "popfq",
            // We push it again, to check if the bit flip succeeded.
            "pushfq",
            // We put the initial EFLAGS value into eax.
            "pop rax",
            // And finally we compare it to the value after the bit flip.
            "xor eax, [rsp]",
            "popfq",
            // If eax != 0, the bit flip was successful.
            "and eax, 0x200000",
            out("eax") eax
        );
    }

    eax != 0
}
