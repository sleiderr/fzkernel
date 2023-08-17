//! APIC is a more recent interrupt controller for intel processors.

use crate::io::acpi::madt::MADT;
use crate::io::io_delay;
use crate::{info, println};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::arch::asm;
use core::ptr::{read_volatile, write_volatile};
use fzproc_macros::{field_getter, field_setter};

/// `APIC` provides low-level utilities for local APIC management
/// as well as useful abstractions.
///
/// # Examples
/// A safe way to get a working instance of `APIC` is through calling
/// the init method.
/// ```
/// use flib::io::apic::APIC;
///
/// let apic = APIC::init().expect("Uable to init APIC");
/// ```
pub struct APIC {
    pub register_base: usize,
    dest_format: DestinationFormat,
}

impl Default for APIC {
    /// Creates a default `APIC` instance by using 0xfee00000 as base address (default address)
    fn default() -> Self {
        Self {
            register_base: 0xfee00000,
            dest_format: DestinationFormat::FLAT,
        }
    }
}

impl APIC {
    /// Creates a new instance of `APIC` given the base address for memory-mapped
    /// registers.
    ///
    pub fn new(base: usize) -> Self {
        Self {
            register_base: base,
            dest_format: DestinationFormat::FLAT,
        }
    }

    /// Initiates the `LocalApic`.
    /// This method will :
    /// - retrieve the base address of the memory-mapped APIC registers
    /// - create a `LocalApic` instance pointing to this base address
    /// - retrieve a few basic infos (physical ID, max entry, version)
    /// - enable the local APIC if it has not been enabled so far
    /// - return the instance if every previous steps were successful
    ///
    /// # Errors
    /// Returns [`Err`] if the init failed. This would typically happen
    /// if the local APIC had already been hardly disabled once before.
    ///
    /// # Examples
    /// ```
    /// use flib::io::apic::APIC;
    /// let lapic = APIC::init().expect("Failed to init local APIC");
    /// ```
    ///
    pub fn init() -> Result<Self, String> {
        info!("apic", "Initalizing APIC...");
        let base = Self::apic_base_field();
        info!("apic", "Local APIC is memory mapped at 0x{:x}", base);
        let local_apic = Self::new(base as usize);
        info!("apic", "Retrieving Local APIC infos...");
        let id = local_apic.physical_id();
        info!("apic", "Local APIC has physical id : {} ", id);
        let max_entries = local_apic.max_entry() + 1;
        info!("apic", "Local APIC LVT has size : {} ", max_entries);
        let version = local_apic.version();
        info!("apic", "Local APIC has version : {} ", version);
        local_apic.set_spurious_vector(0xff);
        local_apic.hard_enable();
        local_apic.enable();
        if !local_apic.enabled() {
            info!("apic", "Local APIC disabled, enabling it");
            local_apic.enable();
            if !local_apic.enabled() {
                return Err("Could not enable Local APIC".to_string());
            }
        }
        info!("apic", "Local APIC enabled");
        Ok(local_apic)
    }

    /// Reads APIC register at given offset. This could be used to
    /// read registers that don't have any abstraction implemented in
    /// this module.
    pub fn read_reg(&self, offset: usize) -> u32 {
        unsafe { read_volatile((self.register_base + offset) as *const u32) }
    }

    /// Writes the given value in the APIC register at given offset.
    pub fn write_reg(&self, offset: usize, value: u32) {
        self.read_reg(offset);
        unsafe { write_volatile((self.register_base + offset) as *mut u32, value) }
        self.read_reg(offset);
    }

    /// Max entry is the size of the LVT minus 1.
    #[field_getter(0x30, 16, 23)]
    pub fn max_entry(&self) -> u32 {
        0
    }

    #[field_getter(0x30, 0, 7)]
    pub fn version(&self) -> u32 {
        0
    }

    /// Logical ID is used to send IPI to specific groups of CPU
    /// It's not necessary the same as the physical ID.
    #[field_getter(0xD0, 24, 31)]
    pub fn logical_id(&self) -> u32 {
        0
    }

    /// Set the logical ID
    #[field_setter(0xD0, 24, 31)]
    pub fn set_logical_id(&self, value: u8) {}

    /// Physical ID of the local APIC is commonly used as a processor ID
    /// This ID is hardware-defined at power up.
    /// According to Intel, one should avoid changing this ID.
    #[field_getter(0x20, 24, 27)]
    pub fn physical_id(&self) -> u32 {
        0
    }

    #[field_setter(0x20, 24, 31)]
    pub fn set_physical_id(&self, value: u8) {}

    /// Send an EOI to local APIC. This method has to be called to allow
    /// further interrupts.
    pub fn eoi(&self) {
        self.write_reg(0xB0, 1);
    }

    /// Returns the APIC MSR.
    /// It has the following structure :
    /// ```plaintext
    /// 63                           36 35                   12 11  10  9  8          0
    ///  - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
    /// | //////////////////////////// |    APIC BASE >> 12    |   | /// |   | ////// |
    ///  - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
    ///                                                          |         |
    ///                                                          |          - - - - BSP flag
    ///                                                           - - - - - - - - - Enable/Disable flag
    ///```
    ///
    pub fn load_apic_msr() -> u64 {
        let low: u32;
        let high: u32;
        unsafe {
            asm!(
            "rdmsr",
            in("ecx") 0x1B,
            out("eax") low,
            out("edx") high
            )
        }
        ((high as u64) << 32) + (low as u64)
    }

    pub fn set_apic_msr(msr: u64) {
        let low = msr as u32;
        let high = (msr >> 32) as u32;
        unsafe {
            asm!(
            "wrmsr",
            in("ecx") 0x1B,
            in("eax") low,
            in("edx") high
            )
        }
    }

    /// Returns the apic base address from the MSR
    pub fn apic_base_field() -> u32 {
        let msr = Self::load_apic_msr();
        println!("msr : {:#064b}", msr);
        let low = msr as u32;
        let high = (msr >> 32) as u32;
        let base_low = get_value_inside(low, 12, 31);
        let base_high = get_value_inside(high, 0, 3);
        ((base_high << 19) | base_low) << 12
    }

    /// Relocates APIC registers by overwriting MSR
    pub fn relocate_apic_reg(address: u32) {
        let msr = Self::load_apic_msr();
        let mut low = msr as u32;
        low = set_value_inside(low, 12, 35, address);
        let high = msr >> 32 as u32;
        let msr = low as u64 + ((high) << 32);
        Self::set_apic_msr(msr);
    }

    /// Tells if local APIC is enabled.
    pub fn enabled(&self) -> bool {
        self.soft_enabled()
    }

    /// Tells if local APIC is soft-enabled.
    pub fn soft_enabled(&self) -> bool {
        let svr = self.read_reg(0xF0);
        let enabled_flag = get_value_inside(svr, 8, 8);
        enabled_flag == 1
    }

    /// Tells if local APIC is hard-enabled.
    pub fn hard_enabled(&self) -> bool {
        let msr = Self::load_apic_msr();
        get_value_inside(msr as u32, 11, 11) == 1
    }

    /// Hardly enables local APIC by setting bit 11 os APIC MSR to 1
    /// If the local APIC has been hardly disabled once, only a reset of the
    /// processor can re-enable it.
    pub fn hard_enable(&self) {
        let msr = Self::load_apic_msr();
        let low = msr as u32;
        let high = (msr >> 32) as u32;
        let low = set_value_inside(low, 11, 11, 1);
        let msr = ((high as u64) << 32) + (low as u64);
        Self::set_apic_msr(msr);
    }

    /// Hardly disables local APIC by setting bit 11 os APIC MSR to 0
    /// If the local APIC has been hardly disabled once, only a reset of the
    /// processor can re-enable it.
    /// When hardly disabled, the processor acts like if he had no APIC chip.
    ///
    /// # Examples
    /// ```
    /// use flib::io::apic::APIC;
    /// let local_apic = APIC::new(0xfee00000);
    /// // Hard disable local APIC
    /// local_apic.hard_disable();
    /// // Try to re enable it
    /// local_apic.hard_enable();
    /// // This would panic :
    /// assert!(local_apic.hard_enabled())
    /// ```
    ///
    pub fn hard_disable(&self) {
        let msr = Self::load_apic_msr();
        let mut low = msr as u32;
        let high = (msr >> 32) as u32;
        low = set_value_inside(low, 11, 11, 0);
        let msr = ((high as u64) << 32) + (low as u64);
        Self::set_apic_msr(msr);
    }

    /// Enables the local APIC. This method is hardly recommended for
    /// temporary enabling/disabling the local chip as it uses [`Self::soft_enable`].
    ///
    pub fn enable(&self) {
        self.soft_enable()
    }

    /// Enables the local APIC. This method is hardly recommended for
    /// temporary enabling/disabling the local chip as it uses [`Self::soft_disable`].
    ///
    /// # Examples
    /// ```
    /// use flib::io::apic::APIC;
    ///
    /// let local_apic = APIC::init().unwrap();
    ///
    /// local_apic.enable();
    /// assert!(local_apic.soft_enabled());
    ///
    /// local_apic.disable();
    /// assert!(!local_apic.soft_enabled());
    /// ```
    ///
    pub fn disable(&self) {
        self.soft_disable()
    }

    /// Sets the Spurious Vector.
    /// _Note_ : See Intel Documentation for model specificity.
    ///
    #[field_setter(0xF0, 0, 7)]
    pub fn set_spurious_vector(&self, value: u8) {}

    /// Returns the Spurious Vector
    #[field_getter(0xF0, 0, 7)]
    pub fn get_spurious_vector(&self) -> u32 {
        0
    }

    /// Sends an [`IPI`] by writing it to the ICR register.
    ///
    pub fn send(&self, ipi: IPI) {
        let (low, high) = ipi.craft();
        self.write_reg(0x310, high);
        self.write_reg(0x300, low);
    }

    /// The Delivery Status tells if the local APIC has completed sending
    /// last IPI.
    ///
    #[field_getter(0x300, 12, 12)]
    pub fn delivery_status(&self) -> u32 {
        0
    }

    pub fn is_delivered(&self) -> bool {
        self.delivery_status() == 0
    }

    /// This method could be used to awake other processors in a Multi-Processing
    /// context using INIT - SIPI - SIPI protocol.
    /// The wake value is used to tell other processor where to jump in memory after
    /// awaking.
    /// If the awake value is 0xVV, then processors will jump to 0x00VV0000.
    /// That means that processors can only jump to 4k align space.
    ///
    pub fn awake_proc(&self, wake_value: u8) {
        info!("lapic", "Broadcasting INIT");
        let mut ipi = IPI::default();
        ipi.set_shorthand(Shorthand::EXCL);
        ipi.set_delivery_mode(DeliveryMode::INIT);
        ipi.set_trigger_mode(TriggerMode::EDGE);
        ipi.set_vector(0x0);
        self.send(ipi);

        for _i in 0..50 {
            io_delay();
        }

        info!("lapic", "Broadcasting SIPI");
        let mut ipi = IPI::default();
        ipi.set_shorthand(Shorthand::EXCL);
        ipi.set_delivery_mode(DeliveryMode::SIPI);
        ipi.set_trigger_mode(TriggerMode::EDGE);
        ipi.set_vector(wake_value);
        self.send(ipi);
    }

    /// Returns the value of the given LVT entry.
    ///
    pub fn lvt_entry(&self, entry: LVTEntryType) -> u32 {
        self.read_reg(entry as usize)
    }

    /// Replace the LVT entry with the provided one.
    ///
    /// # Examples
    /// A use-case could be to disable the legacy 8259 PIC Microcontroller, usually
    /// connected to LINT0, in order to use IO/APIC and APICs.
    ///
    /// ```
    /// use flib::io::apic::{APIC, LVTEntry, LVTEntryType};
    /// use flib::io::pic::PIC;
    ///
    /// let pic = PIC::default();
    ///
    /// // Remap IRQs
    /// pic.remap(0x20, 0x28);
    ///
    /// // Mask all IRQs
    /// pic.mask_master(0xff);
    /// pic.mask_slave(0xff);
    ///
    /// // Initiates local APIC
    /// let apic = APIC::init().unwrap();
    ///
    /// let mut lint0 = LVTEntry::new(LVTEntryType::LINT0);
    /// // Mask every possible IRQs
    /// lint0.mask();
    ///
    /// // Set lint0 as LINT0 entry
    /// apic.set_lvt_entry(lint0);
    ///
    /// assert!(NO_LEGACY_IRQs!())
    ///
    /// ```
    pub fn set_lvt_entry(&self, entry: LVTEntry) {
        let offset = entry.entry_type as usize;
        let old = self.read_reg(offset);
        self.write_reg(offset, entry.write_reg(old));
    }

    /// Set timer divide.
    ///
    pub fn set_timer_divide(&self, divide_by: TimerDivide) {
        let divide_by = divide_by as u8;
        let bit3 = divide_by >> 2;
        let bit1 = divide_by & 0b011;
        let mut timer = self.read_reg(0x3e0);
        timer = set_value_inside(timer, 0, 1, bit1 as u32);
        timer = set_value_inside(timer, 3, 3, bit3 as u32);
        self.write_reg(0x3e0, timer);
    }

    /// This function provides an easy way to schedule simple and imprecise periodic
    /// interrupts by providing a vector, an interval (which will be the initial count,
    /// and a divide). To see more about precise scheduling, consider using [`crate::time`].
    ///
    pub fn set_periodic_interrupt(&self, vector: u8, interval: u32, divide: TimerDivide) {
        let mut timer = LVTEntry::new(LVTEntryType::TIMER);
        timer.set_timer_mode(TimerMode::PERIODIC).unwrap();
        timer.set_vector(vector);
        self.set_lvt_entry(timer);
        self.set_timer_divide(divide);
        self.set_initial_count(interval);
    }

    /// Returns current value of the timer.
    ///
    #[field_getter(0x390, 0, 31)]
    pub fn get_current_count(&self) -> u32 {
        0
    }

    /// Returns current value of the timer's initial count.
    ///
    #[field_getter(0x380, 0, 31)]
    pub fn get_initial_count(&self) -> u32 {
        0
    }

    /// Manually set initial count.
    #[field_setter(0x380, 0, 31)]
    pub fn set_initial_count(&self, value: u32) {}

    /// Joins the given [`ProcGroup`].
    /// Note that one `LocalApic` can join multiples [`ProcGroup`].
    ///
    /// # Errors
    /// To prevent undefined behaviour, this function may result as an [`Err`]
    /// if the `LocalApic`is not currently running in [`DestinationFormat::FLAT`] mode.
    ///
    pub fn join(&self, group: ProcGroup) -> Result<(), String> {
        if !matches!(self.dest_format, DestinationFormat::FLAT) {
            return Err(
                "Local APIC is not running in flat mode. Consider calling run_in_flat()."
                    .to_string(),
            );
        }
        let mut logical_id = self.logical_id();
        logical_id |= group as u32;
        self.set_logical_id(logical_id as u8);
        Ok(())
    }

    /// Leaves the given [`ProcGroup`]
    ///
    /// # Errors
    /// To prevent undefined behaviour, this function may result as an [`Err`]
    /// if the `LocalApic`is not currently running in [`DestinationFormat::FLAT`] mode.
    ///
    pub fn leave(&self, group: ProcGroup) -> Result<(), String> {
        if !matches!(self.dest_format, DestinationFormat::FLAT) {
            return Err(
                "Local APIC is not running in flat mode. Consider calling run_in_flat()."
                    .to_string(),
            );
        }
        let mut logical_id = self.logical_id();
        logical_id &= !(group as u32);
        self.set_logical_id(logical_id as u8);
        Ok(())
    }

    /// Joins the given cluster.
    /// # Errors
    /// To prevent undefined behaviour, this function may result as an [`Err`]
    /// if the `LocalApic`is not currently running in [`DestinationFormat::CLUSTER`] mode.
    ///
    pub fn join_cluster(&self, cluster: u8) -> Result<(), String> {
        if !matches!(self.dest_format, DestinationFormat::CLUSTER) {
            return Err(
                "Local APIC is not running in cluster mode. Consider calling run_in_cluster()."
                    .to_string(),
            );
        };
        if cluster >= 16 {
            return Err("There is at most 15 clusters".to_string());
        };
        let mut ldr = self.read_reg(0xD0);
        ldr = set_value_inside(ldr, 28, 31, cluster as u32);
        self.write_reg(0xD0, ldr);
        Ok(())
    }

    /// Take the given id in the current cluster.
    /// The id has to be given as power of 2. (beginning at 2^0 = 1)
    ///
    /// # Errors
    /// Cluster can have at most 4 members. That's why an error could be raised
    /// if the given id doesn't fit this rule.
    ///
    pub fn take_id_in_cluster(&self, id: u8) -> Result<(), String> {
        if !matches!(self.dest_format, DestinationFormat::CLUSTER) {
            return Err(
                "Local APIC is not running in cluster mode. Consider calling run_in_cluster()."
                    .to_string(),
            );
        };
        if id > 2u8.pow(3) {
            return Err("There is at most 4 APICs per cluster".to_string());
        };
        let mut ldr = self.read_reg(0xD0);
        ldr = set_value_inside(ldr, 24, 27, id as u32);
        self.write_reg(0xD0, ldr);
        Ok(())
    }

    /// Tries to switch to [`DestinationFormat::CLUSTER`] destination format.
    /// This function __must__ be called before running any operation
    /// such as [`Self::join_cluster`]
    /// # Errors
    /// There is no reason for this function to fail but we need to ensure
    /// that the switch has been effective to prevent undefined behaviour.
    /// That's why an [`Err`] could be returned.
    ///
    pub fn run_in_cluster(&mut self) -> Result<(), String> {
        self.set_destination_format(DestinationFormat::CLUSTER);
        self.set_logical_id(0);
        if (self.get_destination_format() == (DestinationFormat::CLUSTER as u32))
            && (self.logical_id() == 0)
        {
            self.dest_format = DestinationFormat::CLUSTER;
            Ok(())
        } else {
            Err("Enable to switch to cluster mode".to_string())
        }
    }

    /// Tries to switch to [`DestinationFormat::FLAT`] destination format.
    /// This function __must__ be called before running any operation
    /// such as [`Self::join`] or [`Self::leave`]
    /// # Errors
    /// There is no reason for this function to fail but we need to ensure
    /// that the switch has been effective to prevent undefined behaviour.
    /// That's why an [`Err`] could be returned.
    ///
    pub fn run_in_flat(&mut self) -> Result<(), String> {
        self.set_destination_format(DestinationFormat::FLAT);
        self.set_logical_id(0);
        if (self.get_destination_format() == (DestinationFormat::FLAT as u32))
            && (self.logical_id() == 0)
        {
            self.dest_format = DestinationFormat::FLAT;
            Ok(())
        } else {
            Err("Enable to switch to flat mode".to_string())
        }
    }

    /// Manually sets the destination format.
    /// One should consider using [`Self::run_in_flat`] or [`Self::run_in_cluster`] instead
    /// to prevent undefined behaviour.
    ///
    #[field_setter(0xE0, 28, 31)]
    pub fn set_destination_format(&self, value: DestinationFormat) {}

    #[field_getter(0xE0, 28, 31)]
    pub fn get_destination_format(&self) -> u32 {
        0
    }

    /// Softly enables local APIC by setting bit 8 of the SVR to 1
    pub fn soft_enable(&self) {
        let mut svr = self.read_reg(0xF0);
        svr = set_value_inside(svr, 8, 8, 1);
        self.write_reg(0xF0, svr);
    }

    /// Softly disables local APIC by setting bit 8 of the SVR to 0
    pub fn soft_disable(&self) {
        let mut svr = self.read_reg(0xF0);
        svr = set_value_inside(svr, 8, 8, 0);
        self.write_reg(0xF0, svr);
    }
}

/// `LVTEntry` provides method to easily set up an entry for the APIC Local Vector Table.
/// Once created and fully setup, [`APIC`] provides a easy way to load it in its LVT through the
/// [`APIC::set_lvt_entry`] method.
///
/// An entry has the following general structure :
/// ```plaintext
/// 31                           17 16 15 14 13 12 11 10      8 7                0
///  - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
/// |xxxxxxxxxxxxxxxxxxxxxxxxxxxxxx|  |  |  |  |  |xx|         |      Vector      |
///  - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -
///                                  |  |  |  |  |         |
///                                  |  |  |  |  |          - - - - - - - - - - Delivery Mode
///                                  |  |  |  |   - - - - - - - - - - - - - - - Delivery Status
///                                  |  |  |    - - - - - - - - - - - - - - - - Polarity
///                                  |  |   - - - - - - - - - - - - - - - - - - Remote IRR
///                                  |    - - - - - - - - - - - - - - - - - - - Trigger Mode
///                                   - - - - - - - - - - - - - - - - - - - - - Mask
///```
///
/// Note that Timer entry differs from this structure. Moreover, some fields are not present
/// for every entry type, meaning that setting some field is sometime senseless, depending on
/// the entry type you are crafting.
/// Not only is it senseless but it is also undefined behaviour. That's why some method
/// may raise an error to prevent such misconfiguration.
/// At the end of the day, no matter how the entry was configured, the final step achieved
/// by calling [`APIC::set_lvt_entry`] will prevent from overwriting reserved field by matching
/// the [`LVTEntryType`] field.
/// To learn more about how to configure each kind of entry, refer to Intel documentation.
/// # Examples
/// ```
/// use flib::io::apic::{APIC, LVTEntry, LVTEntryType, TimerMode};
///
/// // Create an entry
/// let mut timer = LVTEntry::new(LVTEntryType::TIMER);
///
/// // Setup the entry
/// timer.set_timer_mode(TimerMode::ONESHOT).expect("This won't panic as we are building a Timer entry");
///
/// // Load the entry
/// let apic = APIC::default();
/// apic.set_lvt_entry(timer); // Done
///
/// ```
#[derive(Debug)]
pub struct LVTEntry {
    entry_type: LVTEntryType,
    vector: u8,
    delivery_mode: DeliveryMode,
    delivery_status: DeliveryStatus,
    polarity: Polarity,
    remirr: u8,
    trigger_mode: TriggerMode,
    mask: u8,
    timer_mode: TimerMode,
}

impl LVTEntry {
    /// Given an [`LVTEntryType`], returns a default instance of an `LVTEntry`.
    pub fn new(entry_type: LVTEntryType) -> Self {
        Self {
            entry_type,
            vector: 0,
            delivery_mode: DeliveryMode::FIXED,
            delivery_status: DeliveryStatus::IDLE,
            polarity: Polarity::HIGH,
            remirr: 0,
            trigger_mode: TriggerMode::LEVEL,
            mask: 0,
            timer_mode: TimerMode::ONESHOT,
        }
    }

    /// Sets the vector field for any entry type.
    pub fn set_vector(&mut self, vector: u8) {
        self.vector = vector
    }

    /// Sets the [`DeliveryMode`] for this entry.
    ///
    /// # Errors
    /// Returns an [`Err`] if the entry has type [`LVTEntryType::TIMER`].
    ///
    pub fn set_delivery_mode(&mut self, mode: DeliveryMode) -> Result<(), ()> {
        if !matches!(self.entry_type, LVTEntryType::TIMER) {
            self.delivery_mode = mode;
            return Ok(());
        };
        Err(())
    }

    /// Sets the [`Polarity`] for this entry.
    ///
    /// # Errors
    /// Returns an [`Err`] if the entry is not one of the following type :
    /// - [`LVTEntryType::LINT0`].
    /// - [`LVTEntryType::LINT1`].
    ///
    pub fn set_polarity(&mut self, polarity: Polarity) -> Result<(), ()> {
        if matches!(self.entry_type, LVTEntryType::LINT0)
            || matches!(self.entry_type, LVTEntryType::LINT1)
        {
            self.polarity = polarity;
            return Ok(());
        };
        Err(())
    }

    /// Sets the [`TriggerMode`] for this entry.
    ///
    /// # Errors
    /// Returns an [`Err`] if the entry is not one of the following type :
    /// - [`LVTEntryType::LINT0`].
    /// - [`LVTEntryType::LINT1`].
    ///
    pub fn set_trigger_mode(&mut self, mode: TriggerMode) -> Result<(), ()> {
        if matches!(self.entry_type, LVTEntryType::LINT0)
            || matches!(self.entry_type, LVTEntryType::LINT1)
        {
            self.trigger_mode = mode;
            return Ok(());
        };
        Err(())
    }

    /// Masks the entry.
    pub fn mask(&mut self) {
        self.mask = 1;
    }

    /// Unmasks the entry
    pub fn unmask(&mut self) {
        self.mask = 0
    }

    /// Sets the [`TimerMode`] for this entry.
    ///
    /// # Errors
    /// Returns an [`Err`] if the entry is not of type [`LVTEntryType::TIMER`]
    ///
    pub fn set_timer_mode(&mut self, mode: TimerMode) -> Result<(), ()> {
        if matches!(self.entry_type, LVTEntryType::TIMER) {
            self.timer_mode = mode;
            return Ok(());
        };
        Err(())
    }

    /// Given a register (typically provided by an [`APIC`] instance), overwrite
    /// allowed fields. It will leave reserved fields intact, by matching the entry type.
    /// Usually, you won't have to use this method to write the entry in a register.
    /// You would rather use the [`APIC::set_lvt_entry`] method, that will do it safely.
    pub fn write_reg(&self, reg: u32) -> u32 {
        let mut new = reg;
        new = set_value_inside(new, 0, 7, self.vector as u32);

        if !(matches!(self.entry_type, LVTEntryType::TIMER)
            || matches!(self.entry_type, LVTEntryType::ERROR))
        {
            new = set_value_inside(new, 8, 10, self.delivery_mode as u32);
        }

        if (matches!(self.entry_type, LVTEntryType::LINT0))
            || (matches!(self.entry_type, LVTEntryType::LINT1))
        {
            new = set_value_inside(new, 13, 13, self.polarity as u32);
            new = set_value_inside(new, 14, 14, self.remirr as u32);
            new = set_value_inside(new, 15, 15, self.trigger_mode as u32);
        }

        new = set_value_inside(new, 16, 16, self.mask as u32);

        if matches!(self.entry_type, LVTEntryType::TIMER) {
            new = set_value_inside(new, 17, 18, self.timer_mode as u32);
        }

        new
    }
}

/// `IoApic` is an abstraction for I/O APIC control. I/O APICs allow a better
/// configuration of external IRQs
pub struct IoAPIC {
    address: usize,
    id: u8,
    version: u8,
    arbitration_id: u8,
    max_red_entry: u8,
    entries_count: u8,
}

impl IoAPIC {
    /// Tries to retrieve IO APIC locations by parsing [`MADT`].
    /// This will return a [`Vec`] of initialized instance, eventually empty.
    /// In case where more than one IO APIC chips are present, this function will init
    /// the instances with a unique id, beginning at 0.
    pub fn init() -> Vec<IoAPIC> {
        let madt_table = MADT::load().unwrap();
        // Should not be hardcoded
        let entries = madt_table.parse_entries(0x7fe1a65);
        let mut ioapics = Vec::new();
        let mut i = 0;
        for ioapic in entries.t1 {
            let a = u32::from_le_bytes(ioapic.io_apic_address);
            let mut io = IoAPIC::new(a as usize);
            io.setup(i);
            ioapics.push(io);
            i += 1;
        }
        ioapics
    }

    /// Returns a `IoAPIC` instance given a memory address
    pub fn new(address: usize) -> Self {
        Self {
            address,
            id: 0,
            version: 0,
            arbitration_id: 0,
            max_red_entry: 0,
            entries_count: 0,
        }
    }

    /// This method will perform several actions :
    /// - set an ID
    /// - compute version
    /// - compute max entry
    /// - compute arbitration ID
    pub fn setup(&mut self, arbitration_id: u8) {
        // Compute APIC ID
        self.write_register(0x0, (arbitration_id as u32) << 24);
        let ioid = self.read_register(0x0);
        let id = ((ioid << 4) >> (24 + 4)) as u8;
        self.id = id;
        info!("i/o apic", "Initializing I/O APIC {}", id);

        // Compute APIC Version
        let ioversion = self.read_register(0x1);
        let version = ioversion as u8;
        self.version = version;
        info!("i/o apic", "I/O APIC has version {}", version);

        // Compute max entry
        let max_entry = ((ioversion << 8) >> (16 + 8)) as u8;
        self.max_red_entry = max_entry;
        info!("i/o apic", "I/O APIC can handle {} IRQs", max_entry + 1);

        // Compute APIC Arbitration ID
        self.write_register(0x2, (arbitration_id as u32) << 24);
        let ioarb = self.read_register(0x2);
        let id = ((ioarb << 4) >> (24 + 4)) as u8;
        info!("i/o apic", "I/O APIC Arbitration ID is now set to : {}", id);
    }

    /// Reads the register at given offset
    pub fn read_register(&self, offset: u8) -> u32 {
        let iowin = self.address + 0x10;
        let ioselect = self.address;
        let reg =
            unsafe { read_volatile(ioselect as *const u32) } & ((2u32.pow(32) - 1) & (0u8 as u32));

        let offset = reg | (offset as u32);
        unsafe { write_volatile(ioselect as *mut u32, offset) };
        unsafe { read_volatile(iowin as *const u32) }
    }

    /// Writes the given register to the given value
    pub fn write_register(&self, offset: u8, value: u32) {
        let iowin = self.address + 0x10;
        let ioselect = self.address;
        let reg =
            unsafe { read_volatile(ioselect as *const u32) } & ((2u32.pow(32) - 1) & (0u8 as u32));
        let offset = reg | (offset as u32);
        unsafe { write_volatile(ioselect as *mut u32, offset) };
        unsafe { write_volatile(iowin as *mut u32, value) };
    }

    /// Tries to add a redirect in the table.
    /// This method has no real meaning as you would prefer binding your redirect to specific
    /// IRQs ans thus chose a fixed entry instead of just appending it to the end, which would
    /// redirect a random IRQ.
    ///
    /// # Errors
    /// This could raise an [`Err`] if the table is full. In that case, you
    /// would have to use [`Self::overwrite_entry`] instead.
    pub fn add_redirect(&mut self, redirect: RedReg) -> Result<(), String> {
        if self.entries_count == self.max_red_entry + 1 {
            Err("Redirect Table is full. Consider using overwrite method instead".to_string())
        } else {
            let offset = self.entries_count;
            self.overwrite_entry(offset, redirect);
            Ok(())
        }
    }

    /// Returns the redirect at given offset.
    pub fn get_redirect(&self, offset: u8) -> u64 {
        let low = self.read_register(0x10 + 2 * offset);
        let high = self.read_register(0x10 + 2 * offset + 1);
        low as u64 + ((high as u64) << 32)
    }

    /// Overwrites the redirection entry in the REGREDTBL with the given [`RedReg`].
    /// Offset 0 is the first entry.
    /// This method is the most useful to redirect specific IRQ.
    ///
    /// # Examples
    /// A common example is to overwrite second entry (ie offset 1) in order to handle
    /// keystrokes.
    /// _Note : you must first ensure you masked [`crate::io::pic::PIC`] entry before._
    ///
    /// ```
    /// use flib::io::apic::{APIC, DestinationMode, IoAPIC, RedReg, TriggerMode};
    ///
    /// // Get local apic physical ID
    /// let apic = APIC::default();
    /// let physical_id = apic.physical_id();
    ///
    /// // Create a new redirect
    /// let mut keyboard_redirect = RedReg::new();
    /// keyboard_redirect.set_trigger_mode(TriggerMode::EDGE);
    ///
    /// // Redirect physically to local APIC
    /// keyboard_redirect.redirect_to_apic(physical_id as u8);
    /// keyboard_redirect.set_destination_mode(DestinationMode::PHYSICAL);
    ///
    /// // Redirects IRQ to interrupt vector 0x28
    /// keyboard_redirect.set_int_vec(0x28);
    ///
    /// let ioapic = IoAPIC::new(MY_ADDRESS);
    /// ioapic.overwrite_entry(1,keyboard_redirect);
    /// ```
    pub fn overwrite_entry(&self, offset: u8, entry: RedReg) {
        let offset = 0x10 + 2 * offset;
        let low = self.read_register(offset);
        let high = self.read_register(offset + 1);
        let (new_low, new_high) = entry.to_register(low, high);
        self.write_register(offset, new_low);
        self.write_register(offset + 1, new_high);
    }
}
#[derive(Debug)]
/// `RedReg` is an entry of an IOREDTBL.
/// It describes how an IO interrupt is redirected by the IO APIC.
pub struct RedReg {
    intvec: u8,
    delmod: DeliveryMode,
    destmod: DestinationMode,
    delivs: DeliveryStatus,
    intpol: Polarity,
    remirr: u8,
    triggermod: TriggerMode,
    masked: u8,
    dest: u8,
}

impl RedReg {
    pub fn new() -> Self {
        Self {
            intvec: 0,
            delmod: DeliveryMode::FIXED,
            destmod: DestinationMode::PHYSICAL,
            delivs: DeliveryStatus::IDLE,
            intpol: Polarity::HIGH,
            remirr: 0,
            triggermod: TriggerMode::LEVEL,
            masked: 0,
            dest: 0,
        }
    }

    /// Sets Interrupt Vector
    pub fn set_int_vec(&mut self, int: u8) {
        self.intvec = int
    }

    /// Sets the [`DeliveryMode`]
    pub fn set_delivery_mode(&mut self, mode: DeliveryMode) {
        self.delmod = mode
    }

    /// Sets the [`DestinationMode`]
    pub fn set_destination_mode(&mut self, mode: DestinationMode) {
        self.destmod = mode
    }

    /// Sets the [`Polarity`]
    pub fn set_polarity(&mut self, pol: Polarity) {
        self.intpol = pol
    }

    /// Sets the [`TriggerMode`]
    pub fn set_trigger_mode(&mut self, mode: TriggerMode) {
        self.triggermod = mode
    }

    /// Masks this redirect
    pub fn mask(&mut self) {
        self.masked = 1
    }

    /// Unmasks this redirect
    pub fn unmask(&mut self) {
        self.masked = 0
    }

    /// Sets the destination
    pub fn set_dest(&mut self, dest: u8) {
        self.dest = dest
    }

    /// Redirects to the given APIC id.
    pub fn redirect_to_apic(&mut self, dest: u8) {
        self.set_dest(dest)
    }

    /// Given low and high register, overwrite values to return the new low and high ones.
    pub fn to_register(&self, low: u32, high: u32) -> (u32, u32) {
        let mut new_low = set_value_inside(low, 0, 7, self.intvec as u32);
        new_low = set_value_inside(new_low, 8, 10, self.delmod as u32);
        new_low = set_value_inside(new_low, 11, 11, self.destmod as u32);
        new_low = set_value_inside(new_low, 13, 13, self.intpol as u32);
        new_low = set_value_inside(new_low, 15, 15, self.triggermod as u32);
        new_low = set_value_inside(new_low, 16, 16, self.masked as u32);

        let new_high = set_value_inside(high, 24, 31, self.dest as u32);
        (new_low, new_high)
    }
}

impl Default for RedReg {
    fn default() -> Self {
        Self::new()
    }
}

/// `IPI` is an abstraction for Inter-Processor Interrupts
pub struct IPI {
    dest: u8,
    shorthand: Shorthand,
    trigger_mode: TriggerMode,
    level: u8,
    delivery_status: DeliveryStatus,
    dest_mode: DestinationMode,
    delivery_mode: DeliveryMode,
    vector: u8,
}

impl Default for IPI {
    /// Creates default `IPI` instance.
    fn default() -> Self {
        Self {
            dest: 0,
            shorthand: Shorthand::NO,
            trigger_mode: TriggerMode::EDGE,
            level: 0,
            delivery_status: DeliveryStatus::IDLE,
            dest_mode: DestinationMode::PHYSICAL,
            delivery_mode: DeliveryMode::FIXED,
            vector: 0,
        }
    }
}

impl IPI {
    /// Sets destination.
    pub fn set_destination(&mut self, destination: u8) {
        self.dest = destination
    }

    /// Sets the [`Shorthand`] (default to [`Shorthand::NO`]).
    pub fn set_shorthand(&mut self, shorthand: Shorthand) {
        self.shorthand = shorthand
    }

    /// Sets the [`TriggerMode`].
    pub fn set_trigger_mode(&mut self, mode: TriggerMode) {
        self.trigger_mode = mode
    }

    /// Sets the level. Differs from [`TriggerMode`].
    pub fn set_level(&mut self, level: u8) {
        self.level = level
    }

    /// Sets the [`DestinationMode`].
    pub fn set_dest_mode(&mut self, mode: DestinationMode) {
        self.dest_mode = mode
    }

    /// Sets the [`DeliveryMode`].
    pub fn set_delivery_mode(&mut self, mode: DeliveryMode) {
        self.delivery_mode = mode
    }

    /// Sets the destination vector.
    /// Must be set to 0 if delivery mode matches [`DeliveryMode::INIT`].
    pub fn set_vector(&mut self, vector: u8) {
        self.vector = vector
    }

    /// Crafts the ICR with several fields.
    pub fn craft(&self) -> (u32, u32) {
        let mut low = 0u32;
        low = set_value_inside(low, 0, 7, self.vector as u32);
        low = set_value_inside(low, 8, 10, self.delivery_mode as u32);
        low = set_value_inside(low, 11, 11, self.dest_mode as u32);
        low = set_value_inside(low, 14, 14, self.level as u32);
        low = set_value_inside(low, 15, 15, self.trigger_mode as u32);
        low = set_value_inside(low, 18, 20, self.shorthand as u32);

        let mut high = 0u32;
        high = set_value_inside(high, 24, 31, self.dest as u32);

        (low, high)
    }

    /// Set up the IPI to be sent to the given cluster and the given id.
    ///
    /// # Examples
    /// As an example, this is how you would send an IPI to self using [`DestinationFormat::CLUSTER`]
    ///
    /// ```
    /// use flib::io::apic::{APIC, IPI};
    /// let mut apic = APIC::default();
    ///
    /// // Join cluster 3
    /// apic.run_in_cluster().expect("Can't join cluster");
    /// apic.join_cluster(3).unwrap();
    ///
    /// // Take id 2
    /// apic.take_id_in_cluster(2).unwrap();
    ///
    /// // Craft IPI
    /// let mut self_test = IPI::default();
    /// self_test.to_cluster_and_id(3, 2);
    /// self_test.set_vector(0x3);
    ///
    /// // Send IPI
    /// apic.send(self_test); // Int 0x3 triggered !
    /// ```
    ///
    pub fn to_cluster_and_id(&mut self, cluster: u8, id: u8) {
        let mut mda = 0;
        mda |= cluster << 4;
        mda |= id & 0b1111;
        self.set_dest_mode(DestinationMode::LOGICAL);
        self.set_destination(mda);
    }
}

/// An abstraction for processors group running in [`DestinationFormat::FLAT`] mode.
#[derive(Copy, Clone, Debug)]
pub enum ProcGroup {
    /// 1
    MANICOTTI = 1,
    /// 2
    FETTUCCINE = 2,
    /// 4
    MOSTACCIOLI = 4,
    /// 8
    PIPETTE = 8,
    /// 16
    RIGATI = 16,
    /// 32
    ROTINI = 32,
    /// 64
    GEMELLI = 64,
    /// 128
    CELENNTANI = 128,
}

/// Abstraction for divide value encoding.
#[derive(Clone, Copy, Debug)]
pub enum TimerDivide {
    D1 = 0b111,
    D2 = 0b000,
    D4 = 0b001,
    D8 = 0b010,
    D16 = 0b011,
    D32 = 0b100,
    D64 = 0b101,
    D128 = 0b110,
}

/// Describes an [`IPI`]'s shorthand
#[derive(Copy, Clone, Debug)]
pub enum Shorthand {
    /// No shorthand
    NO = 0b00,
    /// Self
    SELF = 0b01,
    /// All including self
    INCL = 0b10,
    /// All excluding self
    EXCL = 0b11,
}

/// Describes the type of an LVT entry.
#[derive(Copy, Clone, Debug)]
pub enum LVTEntryType {
    /// Correct Machine Check Interrupt
    CMCI = 0x2F0,
    /// Timer entry
    TIMER = 0x320,
    /// Thermal monitor interrupt
    THSENSOR = 0x330,
    /// Performance Monitor Counter
    PMC = 0x340,
    /// LINT0 pin
    LINT0 = 0x350,
    /// LINT1 pin
    LINT1 = 0x360,
    /// Internal error interrupts
    ERROR = 0x370,
}

/// Describes a timer mode
#[derive(Copy, Clone, Debug)]
pub enum TimerMode {
    /// One shot.
    ONESHOT = 0b00,
    /// Periodic.
    PERIODIC = 0b01,
    /// Uses IA32_TSC_DEADLINE MSR
    TSCDEADLINE = 0b11,
}

/// Describes a destination mode.
#[derive(Copy, Clone, Debug)]
pub enum DestinationMode {
    /// Use physical ID.
    PHYSICAL = 0,
    /// Use logical destination.
    LOGICAL = 1,
}

/// Describes a destination mode.
#[derive(Copy, Clone, Debug)]
pub enum DestinationFormat {
    /// Flat mode (belongs to one or more [`ProcGroup`]).
    /// To insert an [`APIC`] instance in a group, consider using
    /// [`APIC::join`] method.
    FLAT = 0b1111,
    /// Cluster mode.
    /// To join a cluster and take an id in it, consider using
    /// [`APIC::join_cluster`] and [`APIC::take_id_in_cluster`] methods.
    CLUSTER = 0b0000,
}

/// Describes the last interrupt delivery status
#[derive(Copy, Clone, Debug)]
pub enum DeliveryStatus {
    /// Nothing.
    IDLE = 0,
    /// Last irq is pending.
    PENDING = 1,
}

/// Describes the polarity
#[derive(Copy, Clone, Debug)]
pub enum Polarity {
    HIGH = 0,
    LOW = 1,
}

/// Describes the delivery mode
#[derive(Copy, Clone, Debug)]
pub enum DeliveryMode {
    /// Fixed interrut value
    FIXED = 0,
    /// Init IPI (used in the INIT - SIPI - SIPI protocol)
    /// See [`APIC::awake_proc`].
    INIT = 0b101,
    /// SIPI IPI (used in the INIT - SIPI - SIPI protocol)
    /// See [`APIC::awake_proc`].
    SIPI = 0b110,
    /// Use it to consider this input as an external interrupt controller
    ///
    /// # Examples
    /// This mode is usually used to let the legacy 8259 PIC use the LINT0 pin.
    /// ```
    /// use flib::io::apic::{APIC, DeliveryMode, LVTEntry, LVTEntryType};
    /// let apic = APIC::default();
    ///
    /// let mut lint0 = LVTEntry::new(LVTEntryType::LINT0);
    /// // Set EXTINT mode
    /// lint0.set_delivery_mode(DeliveryMode::EXTINT).unwrap();
    ///
    /// // Load entry
    /// apic.set_lvt_entry(lint0);
    /// ```
    ///
    EXTINT = 0b111,
}

#[derive(Copy, Clone, Debug)]
pub enum TriggerMode {
    LEVEL = 1,
    EDGE = 0,
}

/// Utils to set a value between two bits in a u32 value.
fn set_value_inside(source: u32, from_byte: u32, to_byte: u32, value: u32) -> u32 {
    let lshift = 32 - (to_byte - from_byte + 1);
    let rshift = 31 - to_byte;
    let mask = !(((2u32.pow(32) - 1) << lshift) >> rshift);
    let init = source & mask;
    (value << from_byte) | init
}

/// Utils to get a value between two bits in a u32 value.
fn get_value_inside(source: u32, from_byte: u32, to_byte: u32) -> u32 {
    let lshift = 31 - to_byte;
    let rshift = (31 - to_byte) + from_byte;
    (source << lshift) >> rshift
}
