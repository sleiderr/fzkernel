use core::mem;

use crate::io::{inl, outl};

/// Configuration Space Header for a PCI device.
///
/// The first 16 bytes of the header are common to every layout, and the last 48 bytes can vary
/// depending on the type of the entry.
#[repr(C)]
pub struct PCIHeader {
    common: PCICommonHeader,
    var: PCIHeaderVar,
}

/// Various layout for the second part of the PCI Header
pub enum PCIHeaderVar {
    /// Oh header type
    Type0(PCIHeaderType0),

    /// 01h header type (PCI-PCI bridge)
    Type1(PCIHeaderType1),

    /// 02h header type (CardBus bridge)
    Type2(PCIHeaderType2),
}

/// Basic PCI specific header layout (00h)
#[repr(C)]
pub struct PCIHeaderType0 {
    /// Base address #0
    ///
    /// `Bit 0`: Specifies whether the registers maps into memory or I/O space (if set).
    ///
    /// BAR registers that maps into I/O space are always 32-bits wide, with bit 1 reserved.
    ///
    /// BAR registers that maps into Memory space can be either 32-bits wide or 64-bits wide. If
    /// bits `[1::2]` are 00h, the register is 32-bits wide, if they are 10h, the register is
    /// 64-bits wide.
    ///
    /// `Bit 3`: Specifies whether the data is prefetchable or not. It can be set if there are no
    /// side effects on reads.
    ///
    /// Make sure to mask the last bits when determining the base address .
    ///
    /// # Size calculation
    ///
    /// Address range's size calculation can be done using the following procedure:
    ///
    /// - Save the original value of the BAR
    /// - Write `0xFFFFFFFF` to it
    /// - Read the value back
    /// - Clear encoding information bits
    /// - Invert all 32 bits, and increment by 1
    /// - The resultant value is the memory range's size.
    ///
    /// The upper 16 bits are ignored if the BAR is for I/O.
    ///
    /// 64-bits BAR can be sized using the same procedure, the second register is considered an
    /// extension of the first. `0xFFFFFFFF` should be written to both registers, and the the value
    /// of both registers must be read and combined into one 64-bit value. Size calculation can
    /// then be performed on this final value.
    bar_0: u32,

    /// Base address #1
    ///
    /// `Bit 0`: Specifies whether the registers maps into memory or I/O space (if set).
    ///
    /// BAR registers that maps into I/O space are always 32-bits wide, with bit 1 reserved.
    ///
    /// BAR registers that maps into Memory space can be either 32-bits wide or 64-bits wide. If
    /// bits `[1::2]` are 00h, the register is 32-bits wide, if they are 10h, the register is
    /// 64-bits wide.
    ///
    /// `Bit 3`: Specifies whether the data is prefetchable or not. It can be set if there are no
    /// side effects on reads.
    ///
    /// Make sure to mask the last bits when determining the base address .
    ///
    /// # Size calculation
    ///
    /// Address range's size calculation can be done using the following procedure:
    ///
    /// - Save the original value of the BAR
    /// - Write `0xFFFFFFFF` to it
    /// - Read the value back
    /// - Clear encoding information bits
    /// - Invert all 32 bits, and increment by 1
    /// - The resultant value is the memory range's size.
    ///
    /// The upper 16 bits are ignored if the BAR is for I/O.
    ///
    /// 64-bits BAR can be sized using the same procedure, the second register is considered an
    /// extension of the first. `0xFFFFFFFF` should be written to both registers, and the the value
    /// of both registers must be read and combined into one 64-bit value. Size calculation can
    /// then be performed on this final value.
    bar_1: u32,

    /// Base address #2
    ///
    /// `Bit 0`: Specifies whether the registers maps into memory or I/O space (if set).
    ///
    /// BAR registers that maps into I/O space are always 32-bits wide, with bit 1 reserved.
    ///
    /// BAR registers that maps into Memory space can be either 32-bits wide or 64-bits wide. If
    /// bits `[1::2]` are 00h, the register is 32-bits wide, if they are 10h, the register is
    /// 64-bits wide.
    ///
    /// `Bit 3`: Specifies whether the data is prefetchable or not. It can be set if there are no
    /// side effects on reads.
    ///
    /// Make sure to mask the last bits when determining the base address .
    ///
    /// # Size calculation
    ///
    /// Address range's size calculation can be done using the following procedure:
    ///
    /// - Save the original value of the BAR
    /// - Write `0xFFFFFFFF` to it
    /// - Read the value back
    /// - Clear encoding information bits
    /// - Invert all 32 bits, and increment by 1
    /// - The resultant value is the memory range's size.
    ///
    /// The upper 16 bits are ignored if the BAR is for I/O.
    ///
    /// 64-bits BAR can be sized using the same procedure, the second register is considered an
    /// extension of the first. `0xFFFFFFFF` should be written to both registers, and the the value
    /// of both registers must be read and combined into one 64-bit value. Size calculation can
    /// then be performed on this final value.
    bar_2: u32,

    /// Base address #3
    ///
    /// `Bit 0`: Specifies whether the registers maps into memory or I/O space (if set).
    ///
    /// BAR registers that maps into I/O space are always 32-bits wide, with bit 1 reserved.
    ///
    /// BAR registers that maps into Memory space can be either 32-bits wide or 64-bits wide. If
    /// bits `[1::2]` are 00h, the register is 32-bits wide, if they are 10h, the register is
    /// 64-bits wide.
    ///
    /// `Bit 3`: Specifies whether the data is prefetchable or not. It can be set if there are no
    /// side effects on reads.
    ///
    /// Make sure to mask the last bits when determining the base address .
    ///
    /// # Size calculation
    ///
    /// Address range's size calculation can be done using the following procedure:
    ///
    /// - Save the original value of the BAR
    /// - Write `0xFFFFFFFF` to it
    /// - Read the value back
    /// - Clear encoding information bits
    /// - Invert all 32 bits, and increment by 1
    /// - The resultant value is the memory range's size.
    ///
    /// The upper 16 bits are ignored if the BAR is for I/O.
    ///
    /// 64-bits BAR can be sized using the same procedure, the second register is considered an
    /// extension of the first. `0xFFFFFFFF` should be written to both registers, and the the value
    /// of both registers must be read and combined into one 64-bit value. Size calculation can
    /// then be performed on this final value.
    bar_3: u32,

    /// Base address #4
    ///
    /// `Bit 0`: Specifies whether the registers maps into memory or I/O space (if set).
    ///
    /// BAR registers that maps into I/O space are always 32-bits wide, with bit 1 reserved.
    ///
    /// BAR registers that maps into Memory space can be either 32-bits wide or 64-bits wide. If
    /// bits `[1::2]` are 00h, the register is 32-bits wide, if they are 10h, the register is
    /// 64-bits wide.
    ///
    /// `Bit 3`: Specifies whether the data is prefetchable or not. It can be set if there are no
    /// side effects on reads.
    ///
    /// Make sure to mask the last bits when determining the base address .
    ///
    /// # Size calculation
    ///
    /// Address range's size calculation can be done using the following procedure:
    ///
    /// - Save the original value of the BAR
    /// - Write `0xFFFFFFFF` to it
    /// - Read the value back
    /// - Clear encoding information bits
    /// - Invert all 32 bits, and increment by 1
    /// - The resultant value is the memory range's size.
    ///
    /// The upper 16 bits are ignored if the BAR is for I/O.
    ///
    /// 64-bits BAR can be sized using the same procedure, the second register is considered an
    /// extension of the first. `0xFFFFFFFF` should be written to both registers, and the the value
    /// of both registers must be read and combined into one 64-bit value. Size calculation can
    /// then be performed on this final value.
    bar_4: u32,

    /// Base address #5
    ///
    /// `Bit 0`: Specifies whether the registers maps into memory or I/O space (if set).
    ///
    /// BAR registers that maps into I/O space are always 32-bits wide, with bit 1 reserved.
    ///
    /// BAR registers that maps into Memory space can be either 32-bits wide or 64-bits wide. If
    /// bits `[1::2]` are 00h, the register is 32-bits wide, if they are 10h, the register is
    /// 64-bits wide.
    ///
    /// `Bit 3`: Specifies whether the data is prefetchable or not. It can be set if there are no
    /// side effects on reads.
    ///
    /// Make sure to mask the last bits when determining the base address .
    ///
    /// # Size calculation
    ///
    /// Address range's size calculation can be done using the following procedure:
    ///
    /// - Save the original value of the BAR
    /// - Write `0xFFFFFFFF` to it
    /// - Read the value back
    /// - Clear encoding information bits
    /// - Invert all 32 bits, and increment by 1
    /// - The resultant value is the memory range's size.
    ///
    /// The upper 16 bits are ignored if the BAR is for I/O.
    ///
    /// 64-bits BAR can be sized using the same procedure, the second register is considered an
    /// extension of the first. `0xFFFFFFFF` should be written to both registers, and the the value
    /// of both registers must be read and combined into one 64-bit value. Size calculation can
    /// then be performed on this final value.
    bar_5: u32,

    /// CardBus CIS Pointer.
    ///
    /// Pointer to Card Information Structure (used by devices that share silicon between PCI and
    /// CardBus).
    cardbus_cis_ptr: u32,

    /// Subsystem vendor ID
    ///
    /// Used to uniquely identify the expansion board or subsystem where the PCI device resides.
    subsystem_vendor_id: u16,

    /// Subsystem ID
    ///
    /// Used to uniquely identify the expansion board or subsystem where the PCI device resides.
    subsystem_id: u16,

    /// Expansion ROM base address
    ///
    /// Some PCI devices require local EPROMs for expansion ROM. Information about this expansion
    /// ROM are encoded in this register.
    ///
    /// This behaves like a BAR, but the encoding of the bottom bits is different. The upper 21
    /// bits correspond to the upper 21 bits of the expansiion ROM base address.
    ///
    /// The address space required can be obtained by writing 1s to the address portion of the
    /// register and then reading the value back.
    ///
    /// `Bit 0`: Controls whether or not the device accepts accesses to its expansion ROM (if set)
    rom_base_addr: u32,

    /// Capabilities Pointer.
    ///
    /// Points to a linked list of new capabilities implemented by the device.
    /// Used if bit 4 of the `status` register is set. The bottom two bits are reserved (
    /// and should therefore be masked).
    cap_ptr: u8,
    reserved1: u8,
    reserved2: u16,
    reserved3: u32,

    /// Specifies which input of the system interrupt controllers the device's interrupt pin is
    /// connected to.
    ///
    /// For x86, this corresponds to PIC IRQ 0-15 (and not I/O APIC IRQ numbers).
    interrupt_line: u8,

    /// Specifies which interrupt pin the device uses.
    ///
    /// `0x1` corresponds to `INTA#`, `0x2` to `INTB#` and so on.
    /// 0 means the device does not use an interrupt pin.
    interrupt_pin: u8,

    /// Specifies the burst period length, in quarter of microseconds, that the devices need.
    min_grant: u8,

    /// Specifies how often the device needs to access the PCI bus, in quarter of microseconds.
    max_latency: u8,
}

/// PCI-PCI bridge header layout (type 01h)
#[repr(C)]
pub struct PCIHeaderType1 {
    /// Base address #0
    ///
    /// `Bit 0`: Specifies whether the registers maps into memory or I/O space (if set).
    ///
    /// BAR registers that maps into I/O space are always 32-bits wide, with bit 1 reserved.
    ///
    /// BAR registers that maps into Memory space can be either 32-bits wide or 64-bits wide. If
    /// bits `[1::2]` are 00h, the register is 32-bits wide, if they are 10h, the register is
    /// 64-bits wide.
    ///
    /// `Bit 3`: Specifies whether the data is prefetchable or not. It can be set if there are no
    /// side effects on reads.
    ///
    /// Make sure to mask the last bits when determining the base address .
    ///
    /// # Size calculation
    ///
    /// Address range's size calculation can be done using the following procedure:
    ///
    /// - Save the original value of the BAR
    /// - Write `0xFFFFFFFF` to it
    /// - Read the value back
    /// - Clear encoding information bits
    /// - Invert all 32 bits, and increment by 1
    /// - The resultant value is the memory range's size.
    ///
    /// The upper 16 bits are ignored if the BAR is for I/O.
    ///
    /// 64-bits BAR can be sized using the same procedure, the second register is considered an
    /// extension of the first. `0xFFFFFFFF` should be written to both registers, and the the value
    /// of both registers must be read and combined into one 64-bit value. Size calculation can
    /// then be performed on this final value.
    bar_0: u32,

    /// Base address #1
    ///
    /// `Bit 0`: Specifies whether the registers maps into memory or I/O space (if set).
    ///
    /// BAR registers that maps into I/O space are always 32-bits wide, with bit 1 reserved.
    ///
    /// BAR registers that maps into Memory space can be either 32-bits wide or 64-bits wide. If
    /// bits `[1::2]` are 00h, the register is 32-bits wide, if they are 10h, the register is
    /// 64-bits wide.
    ///
    /// `Bit 3`: Specifies whether the data is prefetchable or not. It can be set if there are no
    /// side effects on reads.
    ///
    /// Make sure to mask the last bits when determining the base address .
    ///
    /// # Size calculation
    ///
    /// Address range's size calculation can be done using the following procedure:
    ///
    /// - Save the original value of the BAR
    /// - Write `0xFFFFFFFF` to it
    /// - Read the value back
    /// - Clear encoding information bits
    /// - Invert all 32 bits, and increment by 1
    /// - The resultant value is the memory range's size.
    ///
    /// The upper 16 bits are ignored if the BAR is for I/O.
    ///
    /// 64-bits BAR can be sized using the same procedure, the second register is considered an
    /// extension of the first. `0xFFFFFFFF` should be written to both registers, and the the value
    /// of both registers must be read and combined into one 64-bit value. Size calculation can
    /// then be performed on this final value.
    bar_1: u32,

    /// Stores the bus number of the PCI bus segment to which the primary interface of the bridge
    /// is connected. Software programs the value in this register.
    primary_bus: u8,

    /// Stores the bus number of the PCI bus segment to which the secondary interface of the bridge
    /// is connected. Software programs the value in this register.
    secondary_bus: u8,

    /// Stores the bus number of the highest numbered PCI bus segment which is behind (or
    /// subordinate to) the bridge. Software programs the value in this register..=
    subordinate_bus: u8,

    /// Specifies, in units of PCI bus clocks, the value of the Latency Timer for this PCI bus
    /// master. Applies to the secondary interface of a bridge.
    secondary_latency: u8,

    /// (Optional) Specifies the base address of the address range used by the bridge to determine
    /// when to forward I/O transactions from one interface to the other.
    ///
    /// If a bridge does not implement an I/O address range, the field must be implemented as a
    /// read-only register that return zero when read.
    io_base: u8,

    /// (Optional) Specifies the base address of the address range used by the bridge to determine
    /// when to forward I/O transactions from one interface to the other.
    ///
    /// If a bridge does not implement an I/O address range, the field must be implemented as a
    /// read-only register that return zero when read.
    io_limit: u8,

    /// Similar to the `Status` register, but reflects status conditions of the secondary interface
    /// (instead of the primary one).
    secondary_status: u16,

    /// Defines a memory-mapped address range used by the bridge to determine when
    /// to forward memory transactions from one interface to the other.
    ///
    /// Must be initialized by software.
    ///
    /// The upper 12 bits correspond to the upper 12 address bits `[31::20]`. The lower 20 address
    /// bits are assumed to be 0s.
    ///
    /// The bottom 4 bits are read-only and return 0 when read.
    memory_base: u16,

    /// Defines a memory-mapped address range used by the bridge to determine when
    /// to forward memory transactions from one interface to the other.
    ///
    /// Must be initialized by software.
    ///
    /// The upper 12 bits correspond to the upper 12 address bits `[31::20]`. The lower 20 address
    /// bits are assumed to be 1s.
    ///
    /// The bottom 4 bits are read-only and return 0 when read.
    memory_limit: u16,

    /// `(Optional)` Defines a prefetchable memory address range used by the bridge to determine when
    /// to forward memory transactions from one interface to the other.
    ///
    /// If a bridge does not implement a prefetchable memory address range,
    /// `prefetchable_memory_base` must be implemented as a read-only register that returns 0 when
    /// read.
    ///
    /// Must be initialized by software.
    ///
    /// The upper 12 bits correspond to the upper 12 address bits `[31::20]`. The lower 20 address
    /// bits are assumed to be 0s.
    ///
    /// The bottom 4 bits encode whether or not the bridge supports 64-bit addresses.
    prefetchable_memory_base: u16,

    /// `(Optional)` Defines a prefetchable memory address range used by the bridge to determine when
    /// to forward memory transactions from one interface to the other.
    ///
    /// If a bridge does not implement a prefetchable memory address range,
    /// `prefetchable_memory_base` must be implemented as a read-only register that returns 0 when
    /// read.
    ///
    /// Must be initialized by software.
    ///
    /// The upper 12 bits correspond to the upper 12 address bits `[31::20]`. The lower 20 address
    /// bits are assumed to be 1.
    ///
    /// The bottom 4 bits encode whether or not the bridge supports 64-bit addresses.
    prefetchable_memory_limit: u16,

    /// If `prefetchable_memory_base` indicates support for 32-bit addressing, this is implemented
    /// as a read-only register that returns 0 when read.
    ///
    /// If it indicates support for 64-bit addressing, this specifies the upper 32-bits `[63::32]`
    /// of the 64-bit base address that specifies the prefetchable memory address range.
    prefetchable_base_hi: u32,

    /// If `prefetchable_memory_base` indicates support for 32-bit addressing, this is implemented
    /// as a read-only register that returns 0 when read.
    ///
    /// If it indicates support for 64-bit addressing, this specifies the upper 32-bits `[63::32]`.
    /// of the 64-bit limit address that specifies the prefetchable memory address range.
    prefetchable_limit_hi: u32,

    /// If the `io_base` register indicates support for 16-bit addressing, this is implemented as a
    /// read-only register that returns 0 when read.
    ///
    /// If it indicates support for 32-bit addressing, this specifies the upper 16-bits `[31::16]`,
    /// of the 32-bit base address that specifies the I/O address range.
    io_base_hi: u16,

    /// If the `io_base` register indicates support for 16-bit addressing, this is implemented as a
    /// read-only register that returns 0 when read.
    ///
    /// If it indicates support for 32-bit addressing, this specifies the upper 16-bits `[31::16]`,
    /// of the 32-bit limit address that specifies the I/O address range.
    io_limit_hi: u16,

    /// `(Optional)` Points to a linked list of additional capabilities implemented by the device.
    ///
    /// If the `Capabilities List` bit (bit 4) in the status register is zero, the default state of
    /// this register is zero after reset. Otherwise, this register should be implemented as a
    /// read-only register.
    cap: u8,
    reserved1: u8,
    reserved2: u16,

    /// `(Optional)` Expansion rom base address
    expansion_rom_base_addr: u32,

    /// Read/Write register used to communicate interrupt line routing information between
    /// initialization code and the device driver.
    ///
    /// The value written specifies the routing of the device's `INTx#` pin to the system interrupt
    /// controllers.
    interrupt_line: u8,

    /// R/O register, used to indicate which interrupt pin the bridge uses. A value of 1 correspond
    /// to `INTA#` and so on.
    ///
    /// A bridge that does not implement any interrupt pins must return 0.
    interrupt_pin: u8,

    /// Provides extensions to the `command` register specific to a bridge.
    bridge_control: u16,
}

/// CardBus bridge header (type 02h)
#[repr(C)]
pub struct PCIHeaderType2 {
    cardbus_sock_base_addr: u32,
    offset_cap_list: u8,
    reserved1: u8,
    secondary_status: u16,
    pci_bus_number: u8,
    cardbus_bus_number: u8,
    subordinate_bus_number: u8,
    cardbus_latency_timer: u8,
    mem_base_addr_0: u32,
    mem_limit_0: u32,
    mem_base_addr_1: u32,
    mem_limit_1: u32,
    io_base_addr_0: u32,
    io_limit_0: u32,
    io_base_addr_1: u32,
    io_limit_1: u32,
    interrupt_line: u8,
    interrupt_pin: u8,
    bridge_ctrl: u16,
    subsystem_device_id: u16,
    subsystem_vendor_id: u16,
    legacy_mode_base_addr: u32,
}

/// Common part of the Configuration Space Header
#[repr(C)]
pub struct PCICommonHeader {
    /// Identifies the manufacturer of the device.
    vendor_id: u16,

    /// Identifies the particular device, and is assigned by the vendor.
    device_id: u16,

    /// Controls the behaviour on the interface.
    command: u16,

    /// Provides information about the interface.
    status: u16,

    /// Provides a device-specific revision identifier.
    revision_id: u8,

    /// Progamming interface, used to identify the function of the device
    prog_if: u8,

    /// Sub-class code, used to identify the function of the device
    subclass: u8,

    /// Base class code, used to identify the function of the device
    class_code: u8,

    /// Used when terminating a transaction that uses the `Memory Write and Invalidate` command and
    /// when prefetching.
    ///
    /// Only cacheline sizes that are power of two are valid.
    cache_line_size: u8,

    /// Required if a bridge is capable of burst transfer of more than two data phases on its
    /// interface. If implemented, that register is a R/W register,
    latency_timer: u8,

    /// Indicates the layout of the second part of the header.
    header_type: u8,

    /// `(Optional)` Used for control and status reporting of built-in self test capabilities.
    ///
    /// If `BIST` is not implemented, this register is read-only and returns 0 when read.
    bist: u8,
}

impl PCIHeader {
    /// Checks if the corresponding PCI device is present or not.
    pub fn is_present(&self) -> bool {
        self.common.vendor_id != 0xffff
    }

    /// Checks if the corresponding PCI device is multifunction or not.
    pub fn is_multifunction(&self) -> bool {
        self.common.header_type & 0x80 != 0
    }

    /// Loads a PCI device's header from its bus and device number.
    ///
    /// Always returns a header, even if the device is non-present, so a presence check should be
    /// performed after loading the `PCIHeader`.
    pub fn read(bus: u8, device: u8, function: u8) -> Self {
        let mut conf_header = [0u32; 4];
        (0..4).for_each(|i| {
            conf_header[i] = pci_read_long(bus, device, function, i as u8);
        });

        let common = unsafe { mem::transmute::<[u32; 4], PCICommonHeader>(conf_header) };

        let var = match common.header_type & 0x7f {
            2 => {
                let mut var_header = [0u32; 14];
                (0..14).for_each(|i| {
                    var_header[i] = pci_read_long(bus, device, function, i as u8 + 4);
                });
                PCIHeaderVar::Type2(unsafe {
                    mem::transmute::<[u32; 14], PCIHeaderType2>(var_header)
                })
            }
            1 => {
                let mut var_header = [0u32; 12];
                (0..12).for_each(|i| {
                    var_header[i] = pci_read_long(bus, device, function, i as u8 + 4);
                });
                PCIHeaderVar::Type1(unsafe {
                    mem::transmute::<[u32; 12], PCIHeaderType1>(var_header)
                })
            }
            _ => {
                let mut var_header = [0u32; 12];
                (0..12).for_each(|i| {
                    var_header[i] = pci_read_long(bus, device, function, i as u8 + 4);
                });
                PCIHeaderVar::Type0(unsafe {
                    mem::transmute::<[u32; 12], PCIHeaderType0>(var_header)
                })
            }
        };

        Self { common, var }
    }
}

/// Performs a recursive PCI devices discovery.
///
/// Assumes that PCI bridges between buses were properly set up beforehand.
pub fn pci_enumerate_traversal() {
    let pci_host_0 = PCIHeader::read(0, 0, 0);

    if !pci_host_0.is_multifunction() {
        // Only one PCI host controller
        pci_bus_scan(0);
    } else {
        // Multiple PCI host controller
        for func in 1..8 {
            let pci_aux_host = PCIHeader::read(0, 0, func);
            if !pci_aux_host.is_present() {
                break;
            }
            pci_bus_scan(func);
        }
    }
}

/// Checks if the function is a PCI to PCI bridge, and checks the secondary bus of the bridge.
pub(super) fn pci_function_secbus_check(bus: u8, device: u8, function: u8) {
    let header = PCIHeader::read(bus, device, function);

    if !header.is_present() {
        return;
    }

    if (header.common.class_code == 0x6) && (header.common.subclass == 0x4) {
        if let PCIHeaderVar::Type1(bridge) = header.var {
            let secondary_bus = bridge.secondary_bus;
            pci_bus_scan(secondary_bus);
        }
    }
}

/// Scans every slot of one `bus` for connected devices.
pub(super) fn pci_bus_scan(bus: u8) {
    for device in 0..32 {
        let header = PCIHeader::read(bus, device, 0);
        if !header.is_present() {
            continue;
        }
        pci_function_secbus_check(bus, device, 0);

        if header.is_multifunction() {
            for func in 1..8 {
                pci_function_secbus_check(bus, device, func);
            }
        }
    }
}

/// Performs a full PCI enumeration, by checking if every possible slot contains a device or not.
///
/// The `pci_enumerate_traversal` will be quicker if available, as it avoids checking for devices
/// that we know cannot be there.
pub fn pci_enumerate_all() {
    for bus in 0..=255 {
        for device in 0..32 {
            pci_device_check(bus, device);
        }
    }
}

/// Checks if a device is present, and enumerates its functions.
pub(super) fn pci_device_check(bus: u8, device: u8) {
    let header = PCIHeader::read(bus, device, 0);
    if !header.is_present() {
        return;
    }

    if header.is_multifunction() {
        for func in 1..8 {
            let header = PCIHeader::read(bus, device, func);
            if !header.is_present() {
                continue;
            }
        }
    }
}

/// Reads a `long` ([`u32`]) from the PCI Configuration Space.
pub fn pci_read_long(bus: u8, device: u8, func: u8, offset: u8) -> u32 {
    let mut config_address: u32 = 0;

    config_address |= 0x80000000;
    config_address |= (bus as u32) << 16;
    config_address |= (device as u32) << 11;
    config_address |= (func as u32) << 8;

    // Reads must be 32-bits aligned, so we set the lowest 2 bits to zero, so that we access a
    // multiple of 4 bytes (a `long`, or `dword`).
    config_address |= ((offset as u32) << 2) & 0xfc;

    // `0xcf8` is the `CONFIG_ADDRESS` I/O port, used to specify the configuration address required
    // to be accessed.
    outl(0xcf8, config_address);

    // `0xcfc` is the `CONFIG_DATA` I/O port, it contains the data to transfert to or from the
    // `CONFIG_DATA` register.
    inl(0xcfc)
}
