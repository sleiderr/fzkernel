use core::{arch::asm, hint};

use conquer_once::spin::OnceCell;

use crate::{info, io::acpi::hpet::HPET_CLK, println, x86::cpuid::cpu_id};

pub static TSC_CLK: OnceCell<TSCClock> = OnceCell::uninit();

const TSC_EXT_CALIBRATION_DELAY: u32 = 1000;

pub struct TSCClock {
    tsc_freq: f64,
}

impl TSCClock {
    pub fn init() -> bool {
        if !__rdtsc_support() {
            info!("tsc", "Time-Stamp Counter is not available on this CPU");
            return false;
        }

        if __tsc_invariant_support() {
            info!("tsc", "invariant TSC available");
        }

        let mut clk = TSCClock { tsc_freq: 0.0 };
        clk.__calibrate_tsc_with_hpet();

        info!(
            "tsc",
            "clock frequency = {} hz  time = {} microsecs",
            clk.tsc_freq,
            clk.tsc_time()
        );

        TSC_CLK.init_once(|| clk);

        true
    }

    pub fn tsc_serialized_read() -> u64 {
        let low_msr: u32;
        let high_msr: u32;

        unsafe {
            asm!("lfence", "rdtsc", "lfence", out("eax") low_msr, out("edx") high_msr, options(nostack, nomem));
        }

        ((high_msr as u64) << 32) + low_msr as u64
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

    pub fn tsc_time(&self) -> f64 {
        (1_000_000_f64 * self.tsc_read() as f64) / self.tsc_freq
    }

    /// Calibrates the TSC using the [`HPETClock`].
    ///
    /// Returns the frequency of the TSC, in Hz, or fails if there is no available [`HPETClock'].
    fn __calibrate_tsc_with_hpet(&mut self) -> Result<f64, ()> {
        let hpet = HPET_CLK.get().ok_or(())?;

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
        Ok(freq)
    }
}

fn __rdtsc_support() -> bool {
    if let Some(id) = cpu_id(0x1) {
        if id[3] & 0x10 != 0 {
            return true;
        }
    }
    false
}

fn __tsc_invariant_support() -> bool {
    if let Some(id) = cpu_id(0x80000008) {
        if id[3] != 0 {
            return true;
        }
    }

    false
}
