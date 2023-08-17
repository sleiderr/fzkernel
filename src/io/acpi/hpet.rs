//! HPET controller.
//!
//! Offers an API to use HPET as a main clock source, and to access and configure the various
//! timers offered.
//!
//! The clock is accessible through a shared [`HPETClock`], after its initialization using `hpet_clk_init`.

use core::mem;

use conquer_once::spin::OnceCell;

use crate::{
    info,
    io::acpi::{sdt::ACPISDTHeader, ACPIAddress},
    sdt_getter,
};

/// Shared [`HPETClock`]  to work with the physical HPET.
pub static HPET_CLK: OnceCell<HPETClock> = OnceCell::uninit();

/// Initialize the global HPET clock through ACPI, if available.
pub fn hpet_clk_init() {
    if let Some(hpet_desc) = HPETDescriptionTable::load() {
        let mut clock = HPETClock::from_acpi_table(hpet_desc);
        clock.enable_clk();
        info!("hpet", "initializing HPET clock");
        info!(
            "hpet",
            "hpet   freq = {} MHz   time = {} microsecs   width = {} bits   timer_count = {}",
            clock.clk_frequency(),
            clock.clk_time(),
            clock.clk_width(),
            clock.timer_count(),
        );
        HPET_CLK.init_once(|| clock);
    } else {
        info!("hpet", "no HPET clock available");
    }
}

pub struct HPETClock<'t> {
    description_table: &'static mut HPETDescriptionTable,
    registers: &'t mut HPETMemRegisters,
    clk_freq: f64,
}

impl<'t> HPETClock<'t> {
    /// Creates a `HPETClock` from its ACPI description as a [`HPETDescriptionTable`].
    pub fn from_acpi_table(desc: &'static mut HPETDescriptionTable) -> Self {
        let registers: &mut HPETMemRegisters =
            unsafe { mem::transmute(desc.base_addr.address as u32) };
        let clk_freq = 1_000_000_000_f64 / (registers.__counter_clk_period()) as f64;

        Self {
            description_table: desc,
            registers,
            clk_freq,
        }
    }

    /// Returns the counter width in bits.
    pub fn clk_width(&self) -> u8 {
        match self.registers.__count_size_cap() {
            1 => 64,
            _ => 32,
        }
    }

    /// Enables the `HPETClock` main counter.
    pub fn enable_clk(&mut self) {
        self.registers.__set_enable_cnf(0x1);
    }

    /// Disables the `HPETClock` main counter
    pub fn disable_clk(&mut self) {
        self.registers.__set_enable_cnf(0x0);
    }

    /// Returns the `HPETClock` frequency in MHz.
    pub fn clk_frequency(&self) -> f64 {
        self.clk_freq
    }

    /// Returns the number of available timers.
    pub fn timer_count(&self) -> u8 {
        self.registers.__num_tim_cap() + 1
    }

    /// Returns the current time indicated by the `HPETClock`, in microseconds.
    ///
    /// The clock is monotonic, two consecutive reads of the current time may return the same value
    /// if the access latency to the timer is less than the clock period, but the second value can
    /// never be less that the first one, unless the counter rolled over (for a 32-bits wide clock
    /// counter).
    pub fn clk_time(&self) -> f64 {
        self.registers.main_counter_value as f64 / self.clk_freq
    }
}

#[repr(C, align(64))]
pub struct HPETDescriptionTable {
    pub header: ACPISDTHeader,
    pub event_time_block_id: u32,
    pub base_addr: ACPIAddress,
    pub hpet_number: u8,
    pub min_clock_tick_periodic: u16,
    pub page_prot_oem_attr: u8,
}

#[derive(Debug)]
#[repr(C, packed)]
pub struct HPETMemRegisters {
    data: u64,
    reserved_1: u64,
    general_conf: u64,
    reserved_2: u64,
    general_int_status: u64,
    reserved_3: [u64; 25],
    main_counter_value: u64,
    reserved_4: u64,
    timer0: HPETTimerData,
    reserved_5: u64,
    timer1: HPETTimerData,
    reserved_6: u64,
    timer2: HPETTimerData,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug)]
pub struct HPETTimerData {
    data: u16,
    reserved: u16,

    // Timer n Interrupt Routing Capability.
    //
    // This 32-bit R/O field indicates to which interrupts in the I/O APIC this timer's
    // interrupt can be routed to. Each bit is this field correspond to a particular interrupt.
    //
    // If this timer's interrupt can be mapped to interrupt 16, then bit 16 will be set.
    tn_int_route_cap: u32,

    // Reads to this register return the current value of the comparator.
    //
    // If the timer is configured in non-periodic mode, writes to this register load the value
    // against which the main counter value should be compared. When the main counters equals the
    // last value written to this register, the corresponding interupt can be generated (if enabled).
    //
    // If the timer is configured in periodic mode, when the main counter value equals the value
    // last written to this register, the corresponding interrupt is generated. After the main
    // counter equals the last value in this register, the value is in register is increased by the
    // value last written to the register. It wraps around when reaching the maximum possible
    // value.
    tn_comparator_register: u64,

    // Software should set this 32-bit field to indicate the value that is written during the FSB
    // interrupt message.
    tn_fsb_int_val: u32,

    // Software should set this 32-bit field to indicate what location the FSB interrupt message
    // should be written to.
    tn_fsb_int_addr: u32,
}

impl HPETTimerData {
    /// Timer n Interrupt Type.
    ///
    /// If this bit is set, the timer interrupt is Level Triggered (the interrupt
    /// will be held active until it is cleared by writing to the General Interupt
    /// Status register).
    ///
    /// If this bit is clear, the timer interrupt is Edge Triggered.
    fn __tn_int_type_cnf(&self) -> u8 {
        ((self.data >> 1) & 0x1) as u8
    }

    /// Timer n Interrupt Enable.
    ///
    /// This bit must be set to enable the timer to cause an interrupt when the timer
    /// event fires.
    fn __tn_int_enb_cnf(&self) -> u8 {
        ((self.data >> 2) & 0x1) as u8
    }

    /// Set the nth Timer Interrupt Enable.
    ///
    /// This bit must be set to enable the timer to cause an interrupt when the timer
    /// event fires.
    fn __set_tn_int_enb_cnf(&mut self, val: u8) {
        let new_cnf = (self.data | 0x2) & ((val << 1) as u16);
        self.data = new_cnf;
    }

    /// Timer n Type.
    ///
    /// If `__tn_per_int_cap` is clear, this will always return 0.
    ///
    /// If `__tn_per_int_cap` is set, writing a 1 to this bit enables the timer to generate
    /// a periodic interrupt.
    fn __tn_type_cnf(&self) -> u8 {
        ((self.data >> 3) & 0x1) as u8
    }

    /// Timer n Type.
    ///
    /// If `__tn_per_int_cap` is clear, this will always return 0.
    ///
    /// If `__tn_per_int_cap` is set, writing a 1 to this bit enables the timer to generate
    /// a periodic interrupt.
    fn __set_tn_type_cnf(&mut self, val: u8) {
        let new_cnf = (self.data | 0x8) & ((val << 3) as u16);
        self.data = new_cnf;
    }

    /// Timer n Periodic Interrupt Capable.
    ///
    /// If this bit is set, the hardware supports a periodic mode for this timer's interrupt.
    fn __tn_per_int_cap(&self) -> u8 {
        ((self.data >> 4) & 0x1) as u8
    }

    /// Timer n Size.
    ///
    /// If this bit is set, the timer is 64-bits wide.
    /// If this bit is clear, the timer is 32-bits wide.
    fn __tn_size_cap(&self) -> u8 {
        ((self.data >> 5) & 0x1) as u8
    }

    /// Timer n Value Set.
    ///
    /// Used for timers that have been set to periodic mode.
    /// If set, software is allowed to directly set a periodc timer's accumulator.
    ///
    /// This should stay clear if the timer is set to non-periodic mode.
    fn __tn_val_set_cnf(&self) -> u8 {
        ((self.data >> 6) & 0x1) as u8
    }

    /// Set Timer n Value Set.
    ///
    /// Used for timers that have been set to periodic mode.
    /// If set, software is allowed to directly set a periodc timer's accumulator.
    ///
    /// This should stay clear if the timer is set to non-periodic mode.
    fn __set_tn_val_set_cnf(&mut self, val: u8) {
        let new_cnf = (self.data | 0x40) & ((val << 6) as u16);
        self.data = new_cnf;
    }

    /// Timer n 32-bit Mode.
    ///
    /// Software can set this bit to force a 64-bit wide timer to behave as a 32-bit timer.
    fn __tn_32mode_cnf(&self) -> u8 {
        ((self.data >> 8) & 0x1) as u8
    }

    /// Set Timer n 32-bit Mode.
    ///
    /// Software can set this bit to force a 64-bit wide timer to behave as a 32-bit timer.
    fn __set_tn_32mode_cnf(&mut self, val: u8) {
        let new_cnf = (self.data | 0x100) & ((val as u16) << 8);
        self.data = new_cnf;
    }

    /// Timer n Interrupt Route.
    ///
    /// This 5-bit field indicates the routing for the interrupt to the I/O APIC.
    fn __tn_int_route_cnf(&self) -> u8 {
        ((self.data >> 9) & 0x1f) as u8
    }

    /// Set Timer n Interrupt Route.
    ///
    /// This 5-bit field indicates the routing for the interrupt to the I/O APIC.
    /// A valid 5-bit unsigned integer must be provided.
    fn __set_int_route_cnf(&mut self, val: u8) {
        let new_cnf = (self.data | 0x3e00) & ((val as u16) << 9);
        self.data = new_cnf;
    }

    /// Timer n FSB Interrupt Enable.
    ///
    /// If `__tn_fsb_int_del_cap` is set, software can set this flag to force the interrupts
    /// to be delivered directly as FSB messages, rather than using the I/O APIC.
    ///
    /// If set, `__tn_int_route_cnf` will be ignored.
    fn __tn_fsb_en_cnf(&self) -> u8 {
        ((self.data >> 14) & 0x1) as u8
    }

    /// Set Timer n FSB Interrupt Enable.
    ///
    /// If `__tn_fsb_int_del_cap` is set, software can set this flag to force the interrupts
    /// to be delivered directly as FSB messages, rather than using the I/O APIC.
    ///
    /// If set, `__tn_int_route_cnf` will be ignored.
    fn __set_tn_fsb_en_cnf(&mut self, val: u8) {
        let new_cnf = (self.data | 0x4000) & ((val as u16) << 14);
        self.data = new_cnf;
    }

    /// Timer n FSB Interrupt Delivery.
    ///
    /// If this R/O bit is set, hardware supports direct front-side bus delivery of this timer's
    /// interrupt.
    fn __tn_fsb_int_del_cap(&self) -> u8 {
        ((self.data >> 15) & 0x1) as u8
    }
}

impl HPETMemRegisters {
    /// Main Counter Register.
    ///
    /// 32-bits counter will always return 0 for the upper 32-bits of this register.
    fn __read_main_counter(&self) -> u64 {
        self.main_counter_value
    }

    /// Set the Timer N Interrupt Active bit.
    /// The functionnality of this bit depends on the mode used for this timer.
    ///
    /// In level-triggered mode, this bit defaults to 0, and will be set by hardware
    /// if the corresponding timer interrupt is active. When this bit is set, it can be
    /// cleared by writing a 1 to the same position (a write of 0 has no effect).
    ///
    /// In edge-triggered mode, this bit should be ignored.
    fn __write_tn_int_sts(&mut self, n: u8) {
        let new_cnf = (self.general_int_status) | (1 << n);
        self.general_int_status = new_cnf;
    }

    /// Timer N Interrupt Active.
    ///
    /// The functionnality of this bit depends on the mode used for this timer.
    ///
    /// In level-triggered mode, this bit defaults to 0, and will be set by hardware
    /// if the corresponding timer interrupt is active. When this bit is set, it can be
    /// cleared by writing a 1 to the same position (a write of 0 has no effect).
    ///
    /// In edge-triggered mode, this bit should be ignored.
    fn __tn_int_sts(&self, n: u8) -> u8 {
        ((self.general_int_status >> n) & 0x1) as u8
    }

    /// Set the LegacyReplacement Route bit.
    ///
    /// If this bit is set, LegacyReplacement Route are supported.
    ///
    /// If both this bit and `ENABLE_CNF` are set:
    ///
    /// - Timer 0 will be routed to IRQ0 in Non-APIC or IRQ2 in I/O APIC
    /// - Timer 1 will be routed to IRQ8 in Non-APIC or IRQ8 in I/O APIC
    ///
    /// If this bit is not set, the individual routing bits for each timer will
    /// be used.
    fn __set_leg_rt_cnf(&mut self, val: u8) {
        let new_cnf = (self.general_conf | 0x2) & ((val << 1) as u64);
        self.general_conf = new_cnf;
    }

    /// LegacyReplacement Route.
    ///
    /// If this bit is set, LegacyReplacement Route are supported.
    ///
    /// If both this bit and `ENABLE_CNF` are set:
    ///
    /// - Timer 0 will be routed to IRQ0 in Non-APIC or IRQ2 in I/O APIC
    /// - Timer 1 will be routed to IRQ8 in Non-APIC or IRQ8 in I/O APIC
    ///
    /// If this bit is not set, the individual routing bits for each timer will
    /// be used.
    fn __leg_rt_cnf(&self) -> u8 {
        (self.general_conf & 0x2) as u8
    }

    /// Set the Overall Enable bit.
    ///
    /// This bit must be set for the main counter to increment, and for any of the
    /// timer to generate interrupts.
    /// If set to 0, the main counter will halt.
    fn __set_enable_cnf(&mut self, val: u8) {
        let new_cnf = (self.general_conf | 0x1) & (val as u64);
        self.general_conf = new_cnf;
    }

    /// Overall Enable.
    ///
    /// This bit must be set for the main counter to increment, and for any of the
    /// timer to generate interrupts.
    /// If set to 0, the main counter will halt.
    fn __enable_cnf(&self) -> u8 {
        (self.general_conf & 0x1) as u8
    }

    /// Number of timers.
    ///
    /// This number indicates the last timer (eg: a value of 0x2 means 3 timers).
    fn __num_tim_cap(&self) -> u8 {
        ((self.data >> 8) & 0x1f) as u8
    }

    /// Counter Size.
    ///
    /// If this bit is set (= 1), the main counter is 64 bits wide.
    /// Otherwise it is 32 bits wide and cannot operate in 64 bits mode.
    fn __count_size_cap(&self) -> u8 {
        (self.data >> 13 & 0x1) as u8
    }

    /// LegacyReplacement Route Capable.
    ///
    /// If set, this bit indicates the support of the LegacyReplacement Interrupt
    /// Route option.
    fn __leg_rt_cap(&self) -> u8 {
        ((self.data >> 15) & 0x1) as u8
    }

    fn __counter_clk_period(&self) -> u32 {
        ((self.data >> 32) & 0xffffffff) as u32
    }
}

impl HPETDescriptionTable {
    sdt_getter!("HPET");
}
