use core::{mem, slice};

use alloc::vec::Vec;

use crate::{
    drivers::pci::{pci_read_long, pci_write_long, DeviceClass, PCICommonHeader, PCIHeader},
    errors::{CanFail, IOError},
    println,
};

pub const BAR_32_WIDTH: u32 = 0x00;
pub const BAR_64_WIDTH: u32 = 0x02;

/// `PCIDevices` holds a vector of [`PCIDevice`].
///
/// This is the base component of the PCI device inventory, obtained after the initial enumeration.
/// It offers several methods for easier device lookup (based on class for instance).
#[derive(Debug)]
pub struct PCIDevices {
    devices: Vec<PCIDevice<'static>>,
}

impl core::ops::Deref for PCIDevices {
    type Target = [PCIDevice<'static>];

    fn deref(&self) -> &Self::Target {
        &self.devices
    }
}

impl core::ops::DerefMut for PCIDevices {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.devices
    }
}

impl PCIDevices {
    pub fn from_devices(devices: Vec<PCIDevice<'static>>) -> Self {
        Self { devices }
    }

    /// Retrieve the PCI devices corresponding to a given [`DeviceClass`].
    ///
    /// Returns a new `PCIDevices` containing all devices matching the provided [`DeviceClass`].
    pub fn get_by_class(&self, class: DeviceClass) -> PCIDevices {
        PCIDevices::from_devices(
            self.devices
                .iter()
                .filter_map(|dev| {
                    (u32::from(dev.class) == u32::from(class)).then_some(PCIDevice::load(
                        dev.bus,
                        dev.device,
                        dev.function,
                    ))
                })
                .collect(),
        )
    }
}

/// Internal representation of a PCI device.
///
/// Holds most of the useful information when interacting with a PCI I/O device.
/// Offers low-level control of the corresponding device:
///
/// - Device location
/// - Mapped registers
/// - EPROM (if available)
#[derive(Debug)]
pub struct PCIDevice<'d> {
    pub class: DeviceClass,
    pub registers: [MappedRegister<'d>; 6],
    pub eprom: Option<PCIMappedMemory<'d>>,
    bus: u8,
    device: u8,
    function: u8,
}

impl<'d> core::fmt::Display for PCIDevice<'d> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{} ({:#08x} / {}-{}-{})",
            self.class,
            u32::from(self.class),
            self.bus,
            self.device,
            self.function,
        )
    }
}

/// I/O controller memory-mapped address space.
///
/// Implements [`Deref<Target = [u8]>`](core::ops::Deref) and [`DerefMut`](core::ops::DerefMut),
/// and therefore all usual methods for slices can also be used with `PCIMappedMemory`, as `&PCIMappedMemory`
/// coerces to `&[u8]`.
pub struct PCIMappedMemory<'d> {
    /// Memory segment mapped to the I/O controller memory.
    buffer: &'d mut [u8],

    /// Width of the memory addresses (32-bit or 64-bit).
    width: u8,
}

impl<'d> PCIMappedMemory<'d> {
    pub unsafe fn copy_ref(&self) -> PCIMappedMemory<'d> {
        PCIMappedMemory {
            buffer: slice::from_raw_parts_mut(self.buffer.as_ptr() as *mut u8, self.buffer.len()),
            width: self.width,
        }
    }
}

impl<'d> core::ops::Deref for PCIMappedMemory<'d> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.buffer
    }
}

impl<'d> core::ops::DerefMut for PCIMappedMemory<'d> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.buffer
    }
}

impl<'d> core::fmt::Debug for PCIMappedMemory<'d> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "pci_mapped_mem: base = {:#010x}    len = {:#010x}",
            self.buffer.as_ptr() as usize,
            self.buffer.len()
        )
    }
}

/// Mapping to an I/O controller address space.
///
/// It can either map into memory space (`Memory` variant), or in I/O space (`IO` variant).
///
/// A `MappedRegister` is associated to a Base Address Register (BAR), whose information are
/// contained the Configuration Space header of the PCIDevice.
/// Invalid BAR content yields the `Unavailable` variant of the enum.
#[derive(Debug)]
pub enum MappedRegister<'d> {
    Memory(PCIMappedMemory<'d>),
    IO(u16),
    Unavailable,
}

impl<'d> PCIMappedMemory<'d> {
    /// Turns a pointer, length and width into a `PCIMappedMemory`.
    ///
    /// # Safety
    ///
    /// `base`, `len` must point to a valid memory segment.
    /// `width` can either be 32 or 64.
    pub unsafe fn from_raw(base: *mut u8, len: usize, width: u8) -> Self {
        let buffer = slice::from_raw_parts_mut(base, len);

        Self { buffer, width }
    }
}

impl<'d> Default for MappedRegister<'d> {
    fn default() -> Self {
        Self::Unavailable
    }
}

impl<'d> MappedRegister<'d> {
    /// Converts the content of a Base Address Register (BAR) to a `MappedRegister`.
    ///
    /// Requires the location of the PCI device (`bus`, `device` and `function`), and the number of
    /// the BAR register to load (`bar_offset`).
    pub fn from_bar(bus: u8, device: u8, function: u8, bar_offset: u32) -> Self {
        let entry_offset = (mem::size_of::<PCICommonHeader>() / 4) as u8 + bar_offset as u8;

        let bar = pci_read_long(bus, device, function, entry_offset);

        if bar == 0 || bar == 0xFFFFFFFF {
            return Self::Unavailable;
        }
        let reg_type = bar & 0x1;

        if reg_type == 1 {
            let io = bar & 0xfffe;

            return Self::IO(io as u16);
        }

        let reg_size = (bar >> 1) & 0x3;

        match reg_size {
            BAR_32_WIDTH => {
                pci_write_long(bus, device, function, entry_offset, 0xFFFFFFFF);
                let mut bar_size = pci_read_long(bus, device, function, entry_offset);

                bar_size &= !0b1111;
                let seg_size = !bar_size + 1;
                pci_write_long(bus, device, function, entry_offset, bar);

                let seg_base = bar & !0b1111;

                let mapped_mem = unsafe {
                    PCIMappedMemory::from_raw(seg_base as *mut u8, seg_size as usize, 32)
                };

                Self::Memory(mapped_mem)
            }
            BAR_64_WIDTH => {
                let bar_2 = pci_read_long(bus, device, function, entry_offset + 1);
                pci_write_long(bus, device, function, entry_offset, 0xFFFFFFFF);
                pci_write_long(bus, device, function, entry_offset + 1, 0xFFFFFFFF);

                let bar_size_1 = pci_read_long(bus, device, function, entry_offset);
                let bar_size_2 = pci_read_long(bus, device, function, entry_offset + 1);

                let mut bar_size = ((bar_size_2 as u64) << 32) + bar_size_1 as u64;
                pci_write_long(bus, device, function, entry_offset, bar);
                pci_write_long(bus, device, function, entry_offset + 1, bar_2);

                bar_size &= !0b1111;
                let seg_size = !bar_size + 1;

                let seg_base = ((bar_2 as u64) << 32) + (bar & !0b1111) as u64;

                let mapped_mem = unsafe {
                    PCIMappedMemory::from_raw(seg_base as *mut u8, seg_size as usize, 64)
                };

                Self::Memory(mapped_mem)
            }
            _ => Self::Unavailable,
        }
    }
}

pub(super) const COMMAND_WOFFSET: u8 = 0x1;
pub(super) const STATUS_WOFFSET: u8 = 0x1;
pub(super) const INTERRUPT_WOFFSET: u8 = 0xF;

pub(super) const IO_SPACE_COMMAND_BOFFSET: u8 = 0;
pub(super) const MEM_SPACE_COMMAND_BOFFSET: u8 = 1;
pub(super) const BUS_MSTR_COMMAND_BOFFSET: u8 = 2;
pub(super) const SPEC_CYC_COMMAND_BOFFSET: u8 = 3;
pub(super) const MEM_WR_INVAL_COMMAND_BOFFSET: u8 = 4;
pub(super) const VGA_PAL_SNOOP_COMMAND_BOFFSET: u8 = 5;
pub(super) const PAR_ERR_RESP_COMMAND_BOFFSET: u8 = 6;
pub(super) const STEP_CTRL_COMMAND_BOFFSET: u8 = 7;
pub(super) const SERR_COMMAND_BOFFSET: u8 = 8;
pub(super) const FAST_B2B_TRANS_COMMAND_BOFFSET: u8 = 9;
pub(super) const INTERRUPT_DISABLE: u8 = 10;

pub(super) const CAP_LIST_STATUS_BOFFSET: u8 = 4;
pub(super) const MHZ66_CAP_STATUS_BOFFSET: u8 = 5;
pub(super) const FAST_B2B_CAP_STATUS_BOFFSET: u8 = 7;
pub(super) const MASTER_DATA_PAR_STATUS_BOFFSET: u8 = 8;
pub(super) const DEVSEL_TIM_STATUS_BOFFSET: u8 = 9;
pub(super) const SIG_TARGET_ABORT_STATUS_BOFFSET: u8 = 0xB;
pub(super) const REC_TARGET_ABORT_STATUS_BOFFSET: u8 = 0xC;
pub(super) const REC_MASTER_ABORT_STATUS_BOFFSET: u8 = 0xD;
pub(super) const SIG_SYS_ERROR_STATUS_BOFFSET: u8 = 0xE;
pub(super) const PAR_ERROR_STATUS_BOFFSET: u8 = 0xF;

pub enum DevselTiming {
    Fast,
    Medium,
    Slow,
}

impl<'d> PCIDevice<'d> {
    /// Reads a `long` ([`u32`])  from this device PCI Configuration Space.
    fn read_confl(&self, offset: u8) -> u32 {
        pci_read_long(self.bus, self.device, self.function, offset)
    }

    /// Writes a `long` ([`u32`]) to this device PCI Configuration Space
    unsafe fn write_confl(&mut self, offset: u8, data: u32) {
        pci_write_long(self.bus, self.device, self.function, offset, data);
    }

    /// Reads the content of this device's Status register.`
    fn read_status(&self) -> u16 {
        ((self.read_confl(STATUS_WOFFSET) & 0xffff0000) >> 16) as u16
    }

    /// Clears a flag in this device's Status register.
    fn clear_status_flg(&mut self, offset: u8) {
        let curr_status = self.read_status();
        let new_status = curr_status & !(1 << offset);

        unsafe { self.write_status(new_status) }
    }

    /// Updates the content of this device's Status register.
    unsafe fn write_status(&mut self, data: u16) {
        let curr_statusl = self.read_confl(STATUS_WOFFSET);
        let new_statusl = (curr_statusl & 0xffff) | (data as u32) << 16;

        self.write_confl(STATUS_WOFFSET, new_statusl)
    }

    /// Reads the content of this device's Command register.
    fn read_command(&self) -> u16 {
        (self.read_confl(COMMAND_WOFFSET) & 0xffff) as u16
    }

    /// Updates the status of an entry in this device's Command register.
    fn update_command(&mut self, offset: u8, new_state: bool) -> CanFail<IOError> {
        let curr_command = self.read_command();
        let new_command = if new_state {
            curr_command | (1 << offset)
        } else {
            curr_command & (!(1 << offset))
        };

        unsafe { self.write_command(new_command) };

        ((self.read_command() & (1 << offset) != 0) == new_state)
            .then_some(())
            .ok_or(IOError::Unknown)
    }

    /// Updates the content of thus device's Command register.
    unsafe fn write_command(&mut self, data: u16) {
        let curr_commandl = self.read_confl(COMMAND_WOFFSET);
        let new_commandl = (curr_commandl & 0xffff0000) | data as u32;

        self.write_confl(COMMAND_WOFFSET, new_commandl);
    }

    /// Disable this PCI Device
    ///
    /// # Safety
    ///
    /// This `PCIDevice` must link to a valid and present PCI Device.
    pub unsafe fn disable(&mut self) {
        self.write_command(0);
    }

    pub fn interrupt_line(&self) -> u8 {
        println!("l = {}", self.read_confl(INTERRUPT_WOFFSET));
        (self.read_confl(INTERRUPT_WOFFSET) & 0xff) as u8
    }

    pub fn interrupt_pin(&self) -> u8 {
        ((self.read_confl(INTERRUPT_WOFFSET) >> 8) & 0xff) as u8
    }

    /// Checks if a capability linked list is available.
    pub fn capabilities_list_available(&self) -> bool {
        self.read_status() & (1 << CAP_LIST_STATUS_BOFFSET) != 0
    }

    /// Checks if the device is capable of running at 66MHz.
    pub fn device_66mhz_support(&self) -> bool {
        self.read_status() & (1 << MHZ66_CAP_STATUS_BOFFSET) != 0
    }

    /// Checks if the target is capable of accepting fast back-to-back transactions when the
    /// transactions are not to the same agent.
    pub fn fast_b2b_transactions_support(&self) -> bool {
        self.read_status() & (1 << FAST_B2B_CAP_STATUS_BOFFSET) != 0
    }

    /// Slowest time that a device will assert `DEVSEL#` for any bus command.
    pub fn devsel_timing(&self) -> DevselTiming {
        match (self.read_status()
            & ((1 << DEVSEL_TIM_STATUS_BOFFSET) | (1 << (DEVSEL_TIM_STATUS_BOFFSET + 1))))
            >> DEVSEL_TIM_STATUS_BOFFSET
        {
            0b00 => DevselTiming::Fast,
            0b01 => DevselTiming::Medium,
            _ => DevselTiming::Slow,
        }
    }

    /// Checks if target device terminated a transaction with 'Target-Abort'
    pub fn target_abort_terminated(&self) -> bool {
        self.read_status() & (1 << SIG_TARGET_ABORT_STATUS_BOFFSET) != 0
    }

    /// Clears the target device `Target-Abort` transaction termination bit.
    pub fn clear_target_abort_terminated(&mut self) {
        self.clear_status_flg(SIG_TARGET_ABORT_STATUS_BOFFSET);
    }

    /// Checks if master device's transaction was terminated with `Target-Abort.
    pub fn received_target_abort(&self) -> bool {
        self.read_status() & (1 << REC_TARGET_ABORT_STATUS_BOFFSET) != 0
    }

    /// Clears the master device's `Target-Abort` transaction termination bit.
    pub fn clear_received_target_abort(&mut self) {
        self.clear_status_flg(REC_TARGET_ABORT_STATUS_BOFFSET);
    }

    /// Checks if master device's transaction was terminated with `Target-Abort.
    pub fn received_master_abort(&self) -> bool {
        self.read_status() & (1 << REC_MASTER_ABORT_STATUS_BOFFSET) != 0
    }

    /// Clears the master device's `Target-Abort` transaction termination bit.
    pub fn clear_received_master_abort(&mut self) {
        self.clear_status_flg(REC_MASTER_ABORT_STATUS_BOFFSET);
    }

    /// Checks if `SERR#` was asserted.
    ///
    /// `SERR#` reports address parity errors, data parity errors on special cycle commands, or any
    /// other system errors that may have serious consequences.
    pub fn signaled_system_error(&self) -> bool {
        self.read_status() & (1 << SIG_SYS_ERROR_STATUS_BOFFSET) != 0
    }

    /// Clears the `SERR#` asserted bit.
    pub fn clear_signaled_system_error(&mut self) {
        self.clear_status_flg(SIG_SYS_ERROR_STATUS_BOFFSET);
    }

    /// Checks if the device detected a parity error.
    pub fn parity_error(&self) -> bool {
        self.read_status() & (1 << PAR_ERROR_STATUS_BOFFSET) != 0
    }

    /// Clears the parity error detection bit.
    pub fn clear_parity_error(&mut self) {
        self.clear_status_flg(PAR_ERROR_STATUS_BOFFSET);
    }

    /// Checks if the device responds to I/O space accesses.
    pub fn io_space_access(&self) -> bool {
        self.read_command() & (1 << IO_SPACE_COMMAND_BOFFSET) != 0
    }

    /// Sets if the device should respond to I/O space accesses.
    pub fn set_io_space_access(&mut self, new_state: bool) -> CanFail<IOError> {
        self.update_command(IO_SPACE_COMMAND_BOFFSET, new_state)
    }

    /// Checks if the device responds to Memory Space accesses.
    pub fn memory_space_access(&self) -> bool {
        self.read_command() & (1 << MEM_SPACE_COMMAND_BOFFSET) != 0
    }

    /// Sets if the device should reponse to Memory Space accesses.
    pub fn set_memory_space_access(&mut self, new_state: bool) -> CanFail<IOError> {
        self.update_command(MEM_SPACE_COMMAND_BOFFSET, new_state)
    }

    /// Checks if the device can act as a master on the PCI bus.
    pub fn bus_master(&self) -> bool {
        self.read_command() & (1 << BUS_MSTR_COMMAND_BOFFSET) != 0
    }

    /// Sets if the device can act as a master on the PCI bus.
    pub fn set_bus_master(&mut self, new_state: bool) -> CanFail<IOError> {
        self.update_command(BUS_MSTR_COMMAND_BOFFSET, new_state)
    }

    /// Should the device monitor Special Cycle operations.
    pub fn special_cycle(&self) -> bool {
        self.read_command() & (1 << SPEC_CYC_COMMAND_BOFFSET) != 0
    }

    /// Sets if the device should monitor Special Cycle operations.
    pub fn set_special_cycle(&mut self, new_state: bool) -> CanFail<IOError> {
        self.update_command(SPEC_CYC_COMMAND_BOFFSET, new_state)
    }

    /// Checks if the `Memory Write and Invalidate` command is available.
    ///
    /// This must be implemented for master devices that can generate the `Memory Write and
    /// Invalidate` command.
    pub fn mem_write_invalidate(&self) -> bool {
        self.read_command() & (1 << MEM_WR_INVAL_COMMAND_BOFFSET) != 0
    }

    /// Enables / disables the support of the `Memory Write and Invalidate` command.
    pub fn set_mem_write_invalidate(&mut self, new_state: bool) -> CanFail<IOError> {
        self.update_command(MEM_WR_INVAL_COMMAND_BOFFSET, new_state)
    }

    /// Checks if palette snooping is enabled (implemented for VGA devices)
    pub fn vga_palette_snoop(&self) -> bool {
        self.read_command() & (1 << VGA_PAL_SNOOP_COMMAND_BOFFSET) != 0
    }

    /// Enables / disables VGA palette snooping.
    pub fn set_vga_palette_snoop(&mut self, new_state: bool) -> CanFail<IOError> {
        self.update_command(VGA_PAL_SNOOP_COMMAND_BOFFSET, new_state)
    }

    /// Should the device take its normal action on parity error.
    pub fn parity_error_response(&self) -> bool {
        self.read_command() & (1 << PAR_ERR_RESP_COMMAND_BOFFSET) != 0
    }

    /// Enables / disables normal action on parity error.
    pub fn set_parity_error_response(&mut self, new_state: bool) -> CanFail<IOError> {
        self.update_command(PAR_ERR_RESP_COMMAND_BOFFSET, new_state)
    }

    /// Checks if device does address / data stepping.
    pub fn stepping_control(&self) -> bool {
        self.read_command() & (1 << STEP_CTRL_COMMAND_BOFFSET) != 0
    }

    /// Enables / disables address / data stepping.
    pub fn set_stepping_control(&mut self, new_state: bool) -> CanFail<IOError> {
        self.update_command(STEP_CTRL_COMMAND_BOFFSET, new_state)
    }

    /// Checks if `SERR#` driver is enabled.
    pub fn serr_driver(&self) -> bool {
        self.read_command() & (1 << SERR_COMMAND_BOFFSET) != 0
    }

    /// Enables / disables `SERR#` driver.
    pub fn set_serr_driver(&mut self, new_state: bool) -> CanFail<IOError> {
        self.update_command(SERR_COMMAND_BOFFSET, new_state)
    }

    /// Can master do fast back-to-back transactions to different device=;
    pub fn fast_b2b_transactions(&self) -> bool {
        self.read_command() & (1 << FAST_B2B_TRANS_COMMAND_BOFFSET) != 0
    }

    /// Enables / disables the capability of master to generate fast back-to-back transactions to
    /// different agents.
    pub fn set_fast_b2b_transactions(&mut self, new_state: bool) -> CanFail<IOError> {
        self.update_command(FAST_B2B_TRANS_COMMAND_BOFFSET, new_state)
    }

    pub fn interrupt_disable(&self) -> bool {
        self.read_command() & (1 << INTERRUPT_DISABLE) != 0
    }

    pub fn set_interrupt_disable(&mut self, new_state: bool) -> CanFail<IOError> {
        self.update_command(INTERRUPT_DISABLE, new_state)
    }

    /// Loads a PCI device information into a `PCIDevice` structure.
    pub fn load(bus: u8, device: u8, function: u8) -> Self {
        let header = PCIHeader::read(bus, device, function);

        let mut registers: [MappedRegister; 6] = [
            MappedRegister::default(),
            MappedRegister::default(),
            MappedRegister::default(),
            MappedRegister::default(),
            MappedRegister::default(),
            MappedRegister::default(),
        ];

        let mut i = 0;
        (0..6).for_each(|_| {
            if i > 5 {
                return;
            }
            let mapped_reg = MappedRegister::from_bar(bus, device, function, i as u32);
            registers[i] = mapped_reg;
            if let MappedRegister::Memory(mem) = &registers[i] {
                if mem.width == 64 {
                    i += 1;
                }
            }

            i += 1;
        });

        // Encode class number as 3 2-digits hex number concatenated together.
        //
        // From left to right, class number, then subclass number and finally programming interface
        // number.
        let class_num = ((header.common.class_code as u32) << 16)
            + ((header.common.subclass as u32) << 8)
            + (header.common.prog_if as u32);

        let device_class = DeviceClass::from(class_num);
        let eprom_data = pci_read_long(bus, device, function, 12);

        // First bit indicates if EPROM is available.
        let eprom = match eprom_data & 0x1 {
            1 => {
                let eprom_addr = eprom_data & !0x3ff;

                let size_chk_data = ((!0) << 11) & (eprom_data & 0x3ff);

                pci_write_long(bus, device, function, 12, size_chk_data);

                let size_unparsed = pci_read_long(bus, device, function, 12);
                pci_write_long(bus, device, function, 12, eprom_data);

                let size_bits = !(size_unparsed & !0x3ff);

                Some(unsafe {
                    PCIMappedMemory::from_raw(eprom_addr as *mut u8, size_bits as usize, 32)
                })
            }
            _ => None,
        };

        Self {
            class: device_class,
            registers,
            eprom,
            bus,
            device,
            function,
        }
    }
}
