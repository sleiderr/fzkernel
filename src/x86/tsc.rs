//! `TSC` Time-Stamp Counter.
//!
//! The Time-Stamp Counter is a 64-bit counter that is set to 0 after a CPU reset. After a reset,
//! the counter is always increasing. The counter can be incremented:
//!
//! - With every internal processor clock cycle
//! - At a constant rate (the TSC frequency)
//!
//! A globally shared clock can be initialized using `TSCClock::init()`, and then accessed through
//! [`TSC_CLK`]. When initializing, the clock's frequency has to be determined, that is the
//! "calibration phase". That calibration can either be made through information provided by the
//! CPU (through `CPUID` and Model-Specific Registers), or using another clock source already
//! initialized (such as the HPET clock).
//!
//! # Examples:
//!
//! ```
//! use flib::x86::tsc;
//!
//! // Initialize the clock.
//! tsc::TSCClock::init();
//!
//! // That show the current time in microseconds of the counter.
//! println!("{}", TSC_CLK.get().unwrap().tsc_time());
//! ```
//!
//! The clock should regularly be recalibrated (as it might depend on the non-constant CPU
//! frequency) through the `tsc_recalibrate` method.

use core::{arch::asm, hint};

use conquer_once::spin::OnceCell;

use crate::{
    errors::{CanFail, ClockError},
    info,
    io::acpi::hpet::HPET_CLK,
    x86::{
        cpuid::{
            cpu_family_id, cpu_feature_support, cpu_id, cpu_model_id, IntelCpuModel, CPU_FEAT_TSC,
        },
        msr::msr_read,
    },
};

/// Shared [`TSCClock`], available everywhere after its initialization.
#[no_mangle]
pub static TSC_CLK: OnceCell<TSCClock> = OnceCell::uninit();

const TSC_EXT_CALIBRATION_DELAY: u32 = 1000;

/// [`TSCClock`] stores the required information to work with a calibrated TSC clock.
pub struct TSCClock {
    tsc_freq: f64,
    invariant: bool,
    hpet_calibration: bool,
}

impl TSCClock {
    /// Initializes the shared TSC clock.
    ///
    /// It should be only called once. It automatically calibrates the clock using `CPUID` or the
    /// shared [`HPET_CLK`] if available.
    ///
    /// Returns an error if there are no available calibration methods, or if `TSC` is not
    /// supported on the device.
    pub fn init() -> CanFail<ClockError> {
        if !cpu_feature_support(CPU_FEAT_TSC).ok_or(ClockError::NotPresent)? {
            info!("tsc", "Time-Stamp Counter is not available on this CPU");
            return Err(ClockError::NotPresent);
        }
        let invariant = __tsc_invariant_support();
        if invariant {
            info!("tsc", "invariant TSC available");
        }

        let mut clk = TSCClock {
            tsc_freq: 0.0,
            invariant,
            hpet_calibration: false,
        };
        info!("tsc", "beginning TSC clock calibration");
        clk.__calibrate_tsc_cpuid()
            .or_else(|_| clk.__calibrate_tsc_with_hpet())
            .map(|x| {
                info!("tsc", "failed CPUID calibration, using HPET clock instead");
                x
            })?;

        info!(
            "tsc",
            "clock frequency = {} hz  time = {} microsecs  invariant = {}",
            clk.tsc_freq,
            clk.tsc_time(),
            clk.invariant
        );

        TSC_CLK.init_once(|| clk);

        Ok(())
    }

    /// Reads the current value of the TSC counter, using a serialized read.
    ///
    /// As it is serialized, it waits untill all previous instructions have been executed before
    /// reading the counter.
    ///
    /// Use `tsc_read` for a standard non-serializing read.
    pub fn tsc_serialized_read(&self) -> u64 {
        let low_msr: u32;
        let high_msr: u32;

        unsafe {
            asm!("lfence", "rdtsc", out("eax") low_msr, out("edx") high_msr, options(nostack, nomem));
        }

        ((high_msr as u64) << 32) + low_msr as u64
    }

    /// Re-calibrates the TSC clock.
    ///
    /// In case of a non-invariant clock, it may be necessary to regularly recalibrate the clock,
    /// as the frequency might change over time.
    pub fn tsc_recalibrate(&mut self) -> Result<f64, ClockError> {
        // No need to recalibrate it if invariant.
        if self.invariant {
            return Ok(self.tsc_freq);
        }
        self.__calibrate_tsc_cpuid()
            .or_else(|_| self.__calibrate_tsc_with_hpet())
    }

    /// Reads the current value of the TSC counter.
    ///
    /// This read is not serializing, and thus does not necessarily wait until all previous instructions
    /// have been executed before reading the counter.
    ///
    /// `tsc_serialized_read` provides a way to make a serialized read of the counter.
    pub fn tsc_read(&self) -> u64 {
        let low_msr: u32;
        let high_msr: u32;

        unsafe {
            asm!("rdtsc", out("eax") low_msr, out("edx") high_msr, options(nostack, nomem));
        }

        ((high_msr as u64) << 32) + low_msr as u64
    }

    /// Returns the current time in microseconds of the TSC counter.
    ///
    /// Uses a non-serialized read, and the TSC frequency that was determined during the clock
    /// initialization.
    pub fn tsc_time(&self) -> f64 {
        (1_000_000_f64 * self.tsc_read() as f64) / self.tsc_freq
    }

    /// Converts raw TSC counter value to microseconds.
    pub fn tsc_ticks_to_micro(&self, ticks: f64) -> f64 {
        (1_000_000_f64 * ticks) / self.tsc_freq
    }

    /// Calibrates the TSC using the [`HPETClock`].
    ///
    /// Returns the frequency of the TSC, in Hz, or fails if there is no available [`HPETClock'].
    fn __calibrate_tsc_with_hpet(&mut self) -> Result<f64, ClockError> {
        let hpet = HPET_CLK.get().ok_or(ClockError::CalibrationError)?;

        let entry_tsc = self.tsc_read();
        let entry_us = hpet.clk_time();

        while entry_us + TSC_EXT_CALIBRATION_DELAY as f64 > hpet.clk_time() {
            // Without this, the compiler seems to be over optimizing, which ends up in an infinite
            // loop. The HPET uses memory-mapped IO, which is "volatile" (as in C).
            // That seems to fix the issue, for now.
            hint::spin_loop();
        }

        let exit_tsc = self.tsc_read();
        let exit_us = hpet.clk_time();

        let freq = ((exit_tsc - entry_tsc) as f64) * 1_000_000_f64 / (exit_us - entry_us);

        self.tsc_freq = freq;
        self.hpet_calibration = true;
        Ok(freq)
    }

    /// Calibrates the TSC using `CPUID` and Model Specific Registers.
    ///
    /// First attempts to retrieve the TSC frequency using CPUID leaf 15H,
    /// then falls back to known values if all leaf 15H information are not available.
    ///
    /// If none of this worked, it uses the information contained in `MSR_PLATFORM_INFO` as a
    /// final attemps at retrieving the TSC frequency.
    fn __calibrate_tsc_cpuid(&mut self) -> Result<f64, ClockError> {
        // TSC and Crystal clock Information Leaf -> CPUID.15H
        // EAX : Denominator of TSC/"core crystal clock" ratio
        // EBX : Numerator of TSC/"core crystal clock" ratio
        // ECX : Nominal frequency of core crystal clock
        let res = cpu_id(0x15).ok_or(ClockError::CalibrationError)?;

        if res[0] != 0 && res[1] != 0 {
            if res[2] != 0 {
                let tsc_freq = (res[1] as f64 / res[0] as f64) * res[2] as f64;
                self.tsc_freq = tsc_freq;
                return Ok(tsc_freq);
            }

            if let Some(cpu_family) = cpu_family_id() {
                // If CPU family is available, so is the model.
                if cpu_family == 0x06 {
                    // For some models, we can use the known crystal clock frequency.
                    match cpu_model_id().unwrap() {
                        IntelCpuModel::SkylakeS
                        | IntelCpuModel::SkylakeY
                        | IntelCpuModel::KabylakeS
                        | IntelCpuModel::KabylakeY => {
                            return Ok((res[1] as f64 / res[0] as f64) * 24_000_000_f64)
                        }
                        IntelCpuModel::SkylakeServer => {
                            return Ok((res[1] as f64 / res[0] as f64) * 25_000_000_f64)
                        }
                        IntelCpuModel::GoldmontA => {
                            return Ok((res[1] as f64 / res[0] as f64) * 19_200_000_f64)
                        }
                        _ => {}
                    }
                }
            }
        }
        // `MSR_PLATFORM_INFO` is MSR 0xCE
        if let Some(model) = cpu_model_id() {
            if let Some(platform_info) = msr_read(0xce) {
                if platform_info != 0 {
                    let scalable_bus_freq = (platform_info >> 8) & 0xff;

                    match model {
                        // 100 MHz bus speed for Nehalem architecture
                        IntelCpuModel::Nehalem => {
                            return Ok(100_000_000_f64 * scalable_bus_freq as f64);
                        }
                        // 133.33 MHz bus speed for Sandy / Ivy Bridge, Haswell and Broadwell
                        IntelCpuModel::SandyBridge
                        | IntelCpuModel::IvyBridge
                        | IntelCpuModel::HaswellU
                        | IntelCpuModel::HaswellS
                        | IntelCpuModel::HaswellG
                        | IntelCpuModel::BroadwellH
                        | IntelCpuModel::BroadwellS => {
                            return Ok(133_330_000_f64 * scalable_bus_freq as f64);
                        }
                        // TODO: add silvermont support
                        _ => {}
                    }
                }
            }
        }

        Err(ClockError::CalibrationError)
    }
}

/// Checks if the TSC is invariant (fixed frequency).
fn __tsc_invariant_support() -> bool {
    if let Some(id) = cpu_id(0x80000008) {
        if id[3] != 0 {
            return true;
        }
    }

    false
}
