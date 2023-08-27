use core::{mem, slice};

use crate::drivers::pci::{pci_read_long, pci_write_long, DeviceClass, PCICommonHeader, PCIHeader};

pub const BAR_32_WIDTH: u32 = 0x00;
pub const BAR_64_WIDTH: u32 = 0x02;

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
/// and therefore all usual methods for slices can also be used with `PCIMappedMemory` : `&PCIMappedMemory`
/// coerces to `&[u8]`.
pub struct PCIMappedMemory<'d> {
    /// Memory segment mapped to the I/O controller memory.
    buffer: &'d mut [u8],

    /// Width of the memory addresses (32-bit or 64-bit).
    width: u8,
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

                bar_size &= !0x1111;
                let seg_size = !bar_size + 1;
                pci_write_long(bus, device, function, entry_offset, bar);

                let seg_base = bar & !0x1111;

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

                bar_size &= !0x1111;
                let seg_size = !bar_size + 1;

                let seg_base = ((bar_2 as u64) << 32) + (bar & !0x1111) as u64;

                let mapped_mem = unsafe {
                    PCIMappedMemory::from_raw(seg_base as *mut u8, seg_size as usize, 64)
                };

                Self::Memory(mapped_mem)
            }
            _ => Self::Unavailable,
        }
    }
}

impl<'d> PCIDevice<'d> {
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
