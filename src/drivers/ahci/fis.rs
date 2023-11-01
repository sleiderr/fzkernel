use core::{mem, ops, slice};

/// `Register Host to Device FIS`
///
/// Used to transfer the content of a `Shadow Register Block` from the host to the device.
/// This is the mechanism used to issue ATA commands to the device.
pub struct RegisterHostDeviceFIS {
    dword1: u32,
    dword2: u32,
    dword3: u32,
    dword4: u32,
    dword5: u32,
}

impl RegisterHostDeviceFIS {
    /// Returns a new `RegisterHostDeviceFIS`, with only the `FIS Type` defined.
    pub fn new_empty() -> Self {
        let dword1 = Into::<u8>::into(FISType::RegisterHostToDevice) as u32;

        Self {
            dword1,
            dword2: 0,
            dword3: 0,
            dword4: 0,
            dword5: 0,
        }
    }

    /// Contains parameter values specified on a per command basis.
    pub fn auxiliary(&self) -> u32 {
        self.dword5
    }

    /// Sets parameter values specified on a per command basis.
    pub fn set_auxiliary(&mut self, aux: u32) {
        self.dword5 = aux;
    }

    /// Contains a value set by the host to inform device of a time limit. If a command does not
    /// define the use of this field, it is reserved.
    pub fn isochronous_command_compl(&self) -> u8 {
        ((self.dword4 >> 16) & 0xff) as u8
    }

    /// Sets a vlue to inform device of a time limit. If a command does not
    /// define the use of this field, it is reserved.
    pub fn set_isochronous_command_compl(&mut self, value: u8) {
        self.dword4 = (self.dword4 & !(0xff0000)) | ((value as u32) << 16);
    }

    /// Contains the contents of the `Device` register of the `Shadow Register Block`.
    pub fn device(&self) -> u8 {
        ((self.dword2 >> 24) & 0xff) as u8
    }

    /// Sets the contents of the `Device` register.
    pub fn set_device(&mut self, device: u8) {
        self.dword2 = (self.dword2 & !(0xff000000)) | ((device as u32) << 24);
    }

    /// Contains the contents of the `Device Control` register of the `Shadow Register Block`.
    pub fn control(&self) -> u8 {
        ((self.dword4 >> 24) & 0xff) as u8
    }

    /// Sets the contents of the `Device Control` register.
    pub fn set_control(&mut self, control: u8) {
        self.dword4 = (self.dword4 & !(0xff000000)) | ((control as u32) << 24);
    }

    /// Contains the contents of the `Command` register of the `Shadow Register Block`.
    pub fn command(&self) -> u8 {
        ((self.dword1 >> 16) & 0xff) as u8
    }

    /// Sets the content of the `Command` register.
    pub fn set_command(&mut self, cmd: u8) {
        self.dword1 = (self.dword1 & !(0x00ff0000)) | ((cmd as u32) << 16);
    }

    /// Contains the contents of the `Features` register of the `Shadow Register Block`.
    pub fn features(&self) -> u16 {
        let features_1 = (self.dword1 >> 24) & 0xff;
        let features_2 = (self.dword3 >> 24) & 0xff;

        (features_2 << 8 | features_1) as u16
    }

    /// Sets the contents of the `Features` register.
    pub fn set_features(&mut self, features: u16) {
        let features_1 = features & 0xff;
        let features_2 = (features >> 8) & 0xff;

        self.dword1 = (self.dword1 & !0xff000000) | ((features_1 as u32) << 24);
        self.dword3 = (self.dword3 & !0xff000000) | ((features_2 as u32) << 24);
    }

    /// Contains the contents of the `LBA` register of the `Shadow Register Block`.
    pub fn lba(&self) -> u64 {
        let lba_1 = (self.dword2 & 0xff) as u64;
        let lba_2 = ((self.dword2 >> 8) & 0xff) as u64;
        let lba_3 = ((self.dword2 >> 16) & 0xff) as u64;
        let lba_4 = (self.dword3 & 0xff) as u64;
        let lba_5 = ((self.dword3 >> 8) & 0xff) as u64;
        let lba_6 = ((self.dword3 >> 16) & 0xff) as u64;

        lba_1 | lba_2 << 8 | lba_3 << 16 | lba_4 << 24 | lba_5 << 32 | lba_6 << 40
    }

    /// Sets the contents of the `LBA` register.
    pub fn set_lba(&mut self, lba: u64) {
        let lba_1 = (lba & 0xff) as u32;
        let lba_2 = ((lba >> 8) & 0xff) as u32;
        let lba_3 = ((lba >> 16) & 0xff) as u32;
        let lba_4 = ((lba >> 24) & 0xff) as u32;
        let lba_5 = ((lba >> 32) & 0xff) as u32;
        let lba_6 = ((lba >> 40) & 0xff) as u32;

        self.dword2 = (self.dword2 & !0xffffff) | lba_1 | (lba_2 << 8) | (lba_3 << 16);
        self.dword3 = (self.dword3 & !0xffffff) | lba_4 | (lba_5 << 8) | (lba_6 << 16);
    }

    /// Contains the contents of the `Sectors Count` register of the `Shadow Register Block`.
    pub fn count(&self) -> u16 {
        (self.dword4 & 0xffff) as u16
    }

    /// Sets the contents of the `Sectors Count` register.
    pub fn set_count(&mut self, count: u16) {
        self.dword4 = (self.dword4 & !0xffff) | (count as u32);
    }

    /// Returns the device port address that the `FIS` should be delivered to or is received from, if
    /// an endpoint is attached via a `Port Multiplier`.
    ///
    /// This field is set by the host for Host to Device transmission and is set by the Port
    /// Multiplier for Device to Host transmission.
    pub fn pm_port(&self) -> u8 {
        ((self.dword1 >> 8) & 0xf) as u8
    }

    /// Sets the device port address that the `FIS` should be delivered to or is received from, if
    /// an endpoint is attached via a `Port Multiplier`.
    ///
    /// This field is set by the host for Host to Device transmission and is set by the Port
    /// Multiplier for Device to Host transmission.
    pub fn set_pm_port(&mut self, port: u8) {
        self.dword1 = (self.dword1 & !0xf00) | ((port as u32) << 8);
    }

    /// Returns the value of the `Command Update` bit.
    ///
    /// If set, it indicates that the register transfer is due to an update of the `Command`
    /// register. If clear, the register transfer is due to an update of the `Device Control`
    /// register.
    pub fn command_update_bit(&self) -> bool {
        (self.dword1 & (1 << 15)) != 0
    }

    /// Sets the value of the `Command Update` bit.
    ///
    /// If set, it indicates that the register transfer is due to an update of the `Command`
    /// register. If clear, the register transfer is due to an update of the `Device Control`
    /// register.
    pub fn set_command_update_bit(&mut self, state: bool) {
        self.dword1 = if state {
            self.dword1 & !(1 << 15) | (1 << 15)
        } else {
            self.dword1 & !(1 << 15)
        };
    }
}

impl ops::Deref for RegisterHostDeviceFIS {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { slice::from_raw_parts(self as *const _ as *const u8, mem::size_of::<Self>()) }
    }
}

/// `Register Device to Host FIS`
///
/// Used by the device to update the contents of the host adapter's `Shadow Register Block`.
/// This is the mechanism that devices indicate command completion status or otherwise change the
/// contents of the host adapter's `Shadow Register Block`.
#[derive(Debug)]
pub struct RegisterDeviceHostFIS {
    dword1: u32,
    dword2: u32,
    dword3: u32,
    dword4: u32,
    dword5: u32,
}

impl RegisterDeviceHostFIS {
    /// Returns a new `RegisterDeviceHostFIS`, with only the `FIS Type` defined.
    pub fn new_empty() -> Self {
        let dword1 = Into::<u8>::into(FISType::RegisterDeviceToHost) as u32;

        Self {
            dword1,
            dword2: 0,
            dword3: 0,
            dword4: 0,
            dword5: 0,
        }
    }

    /// Returns the device port address that the `FIS` should be delivered to or is received from, if
    /// an endpoint is attached via a `Port Multiplier`.
    ///
    /// This field is set by the host for Host to Device transmission and is set by the Port
    /// Multiplier for Device to Host transmission.
    pub fn pm_port(&self) -> u8 {
        ((self.dword1 >> 8) & 0xf) as u8
    }

    /// Sets the device port address that the `FIS` should be delivered to or is received from, if
    /// an endpoint is attached via a `Port Multiplier`.
    ///
    /// This field is set by the host for Host to Device transmission and is set by the Port
    /// Multiplier for Device to Host transmission.
    pub fn set_pm_port(&mut self, port: u8) {
        self.dword1 = (self.dword1 & !0xf00) | ((port as u32) << 8);
    }

    /// Returns the value of the `Interrupt` bit.
    ///
    /// If reflects the interrupt bit line of the device.
    pub fn interrupt_bit(&self) -> bool {
        (self.dword1 & (1 << 14)) != 0
    }

    /// Sets the value of the `Interrupt` bit.
    ///
    /// If reflects the interrupt bit line of the device.
    pub fn set_interrupt_bit(&mut self, state: bool) {
        self.dword1 = if state {
            self.dword1 & !(1 << 14) | (1 << 14)
        } else {
            self.dword1 & !(1 << 14)
        };
    }

    /// Contains the new value of the `Status` register of the `Shadow Register Block`.
    pub fn status(&self) -> u8 {
        ((self.dword1 >> 16) & 0xff) as u8
    }

    /// Sets the new value of the `Status` register.
    pub fn set_status(&mut self, cmd: u8) {
        self.dword1 = (self.dword1 & !(0x00ff0000)) | ((cmd as u32) << 16);
    }

    /// Contains the new value of the `Error` register of the `Shadow Register Block`.
    pub fn error(&self) -> u8 {
        ((self.dword1 >> 24) & 0xff) as u8
    }

    /// Sets the new value of the `Error` register.
    pub fn set_error(&mut self, error: u8) {
        self.dword1 = (self.dword1 & !(0xff000000)) | ((error as u32) << 24);
    }

    /// Contain the new value of the `Sectors Count` register of the `Shadow Register Block`.
    pub fn count(&self) -> u16 {
        (self.dword4 & 0xffff) as u16
    }

    /// Sets the new value of the `Sectors Count` register.
    pub fn set_count(&mut self, count: u16) {
        self.dword4 = (self.dword4 & !0xffff) | (count as u32);
    }

    /// Contains the new value of the `Device` register of the `Shadow Register Block`.
    pub fn device(&self) -> u8 {
        ((self.dword2 >> 24) & 0xff) as u8
    }

    /// Sets the new value of the `Device` register.
    pub fn set_device(&mut self, device: u8) {
        self.dword2 = (self.dword2 & !(0xff000000)) | ((device as u32) << 24);
    }

    /// Contains the contents of the `LBA` register of the `Shadow Register Block`.
    pub fn lba(&self) -> u64 {
        let lba_1 = (self.dword2 & 0xff) as u64;
        let lba_2 = ((self.dword2 >> 8) & 0xff) as u64;
        let lba_3 = ((self.dword2 >> 16) & 0xff) as u64;
        let lba_4 = (self.dword3 & 0xff) as u64;
        let lba_5 = ((self.dword3 >> 8) & 0xff) as u64;
        let lba_6 = ((self.dword3 >> 16) & 0xff) as u64;

        lba_1 | lba_2 << 8 | lba_3 << 16 | lba_4 << 24 | lba_5 << 32 | lba_6 << 40
    }

    /// Sets the contents of the `LBA` register.
    pub fn set_lba(&mut self, lba: u64) {
        let lba_1 = (lba & 0xff) as u32;
        let lba_2 = ((lba >> 8) & 0xff) as u32;
        let lba_3 = ((lba >> 16) & 0xff) as u32;
        let lba_4 = ((lba >> 24) & 0xff) as u32;
        let lba_5 = ((lba >> 32) & 0xff) as u32;
        let lba_6 = ((lba >> 40) & 0xff) as u32;

        self.dword2 = (self.dword2 & !0xffffff) | lba_1 | (lba_2 << 8) | (lba_3 << 16);
        self.dword3 = (self.dword3 & !0xffffff) | lba_4 | (lba_5 << 8) | (lba_6 << 16);
    }
}

/// `Set Device Bits FIS`
///
/// Used by the device to load `Shadow Register Block` bits that the device has exclusive write
/// access. These bits are the 8 bits of the `Error` register and 6 of the 8 bits of the `Status`
/// register. This FIS does not alter the `BSY` bit or the `DRQ` bit of the `Status` register.
pub struct SetDeviceBitsFIS {
    dword1: u32,
    dword2: u32,
}

impl SetDeviceBitsFIS {
    /// Returns a new `SetDeviceBitsFIS`, with only the `FIS Type` defined.
    pub fn new_empty() -> Self {
        let dword1 = Into::<u8>::into(FISType::SetDeviceBitsFIS) as u32;

        Self { dword1, dword2: 0 }
    }

    pub fn status(&self) -> u8 {
        let sta_lo = ((self.dword1 >> 16) & 0b111) as u8;
        let sta_hi = ((self.dword1 >> 20) & 0b111) as u8;

        (sta_hi << 4) | sta_lo
    }

    pub fn set_status(&mut self, status: u8) {
        let sta_lo = (status & 0b111) as u32;
        let sta_hi = ((status >> 4) & 0b111) as u32;

        self.dword1 = (self.dword1 & !(0b111 << 16)) | (sta_lo << 16);
        self.dword1 = (self.dword1 & !(0b111 << 20)) | (sta_hi << 20);
    }

    /// Returns the device port address that the `FIS` should be delivered to or is received from, if
    /// an endpoint is attached via a `Port Multiplier`.
    ///
    /// This field is set by the host for Host to Device transmission and is set by the Port
    /// Multiplier for Device to Host transmission.
    pub fn pm_port(&self) -> u8 {
        ((self.dword1 >> 8) & 0xf) as u8
    }

    /// Sets the device port address that the `FIS` should be delivered to or is received from, if
    /// an endpoint is attached via a `Port Multiplier`.
    ///
    /// This field is set by the host for Host to Device transmission and is set by the Port
    /// Multiplier for Device to Host transmission.
    pub fn set_pm_port(&mut self, port: u8) {
        self.dword1 = (self.dword1 & !0xf00) | ((port as u32) << 8);
    }

    /// Contains the new value of the `Error` register of the `Shadow Register Block`.
    pub fn error(&self) -> u8 {
        ((self.dword1 >> 24) & 0xff) as u8
    }

    /// Sets the new value of the `Error` register.
    pub fn set_error(&mut self, error: u8) {
        self.dword1 = (self.dword1 & !(0xff000000)) | ((error as u32) << 24);
    }

    /// Returns the value of the `Interrupt` bit.
    ///
    /// If set, it indicates the host adapter to enter an interrupt pending state.
    pub fn interrupt(&self) -> bool {
        (self.dword1 & (1 << 14)) != 0
    }

    /// Sets the value of the `Interrupt` bit.
    ///
    /// If set, it indicates the host adapter to enter an interrupt pending state.
    pub fn set_interrupt_bit(&mut self, state: bool) {
        self.dword1 = if state {
            self.dword1 & !(1 << 14) | (1 << 14)
        } else {
            self.dword1 & !(1 << 14)
        };
    }
}

/// `DMA Activate FIS`
///
/// Used by the device to signal the host to proceed with a DMA data transfer of data from the host
/// to the device.
pub struct DMAActivateFIS {
    dword1: u32,
}

impl DMAActivateFIS {
    /// Returns a new `DMAActivateFIS`, with only the `FIS Type` defined.
    pub fn new_empty() -> Self {
        let dword1 = Into::<u8>::into(FISType::DMAActivateFIS) as u32;

        Self { dword1 }
    }

    /// Returns the device port address that the `FIS` should be delivered to or is received from, if
    /// an endpoint is attached via a `Port Multiplier`.
    ///
    /// This field is set by the host for Host to Device transmission and is set by the Port
    /// Multiplier for Device to Host transmission.
    pub fn pm_port(&self) -> u8 {
        ((self.dword1 >> 8) & 0xf) as u8
    }

    /// Sets the device port address that the `FIS` should be delivered to or is received from, if
    /// an endpoint is attached via a `Port Multiplier`.
    ///
    /// This field is set by the host for Host to Device transmission and is set by the Port
    /// Multiplier for Device to Host transmission.
    pub fn set_pm_port(&mut self, port: u8) {
        self.dword1 = (self.dword1 & !0xf00) | ((port as u32) << 8);
    }
}

/// `DMA Setup (bidirectional) FIS`
///
/// It is the mechanism that DMA access to host memory is initiated. It is used to request the host
/// or device to program its DMA controller before transferring data. It allows the actual host
/// memory regions to be abstracted by having memory regions referenced via a base memory
/// descriptor representing a memory region that the host has granted the device access to.
pub struct DMASetupFIS {
    dword1: u32,
    dword2: u32,
    dword3: u32,
    dword4: u32,
    dword5: u32,
    dword6: u32,
    dword7: u32,
}

impl DMASetupFIS {
    /// Returns a new `DMASetupFIS`, with only the `FIS Type` defined.
    pub fn new_empty() -> Self {
        let dword1 = Into::<u8>::into(FISType::DMASetupFIS) as u32;

        Self {
            dword1,
            dword2: 0,
            dword3: 0,
            dword4: 0,
            dword5: 0,
            dword6: 0,
            dword7: 0,
        }
    }

    /// Returns the value of the `Direction` bit.
    ///
    /// If set, indicates that the subsequent data transferred after this FIS is from transmitter
    /// to receiver.
    /// If clear, the transfer is from receiver to transmitter.
    pub fn direction(&self) -> bool {
        (self.dword1 & (1 << 13)) != 0
    }

    /// Sets the value of the `Direction` bit.
    ///
    /// If set, indicates that the subsequent data transferred after this FIS is from transmitter
    /// to receiver.
    /// If clear, the transfer is from receiver to transmitter.
    pub fn set_direction_bit(&mut self, state: bool) {
        self.dword1 = if state {
            self.dword1 & !(1 << 13) | (1 << 13)
        } else {
            self.dword1 & !(1 << 13)
        };
    }

    /// Returns the value of the `Auto-Activate` bit.
    ///
    /// If set, in response to a `DMASetupFIS` with data transfer direction Host to Device, causes
    /// the host to initiate transfer of the first [`DataFIS`] to the device after the DMA context
    /// for the transfer has been established.
    ///
    /// If clear, a [`DMAActivateFIS`] is required to trigger the transmission of the first [`DataFIS`] from the host.
    pub fn auto_activate(&self) -> bool {
        (self.dword1 & (1 << 15)) != 0
    }

    /// Set the value of the `Auto-Activate` bit.
    ///
    /// If set, in response to a `DMASetupFIS` with data transfer direction Host to Device, causes
    /// the host to initiate transfer of the first [`DataFIS`] to the device after the DMA context
    /// for the transfer has been established.
    ///
    /// If clear, a [`DMAActivateFIS`] is required to trigger the transmission of the first [`DataFIS`] from the host.
    pub fn set_auto_activate_bit(&mut self, state: bool) {
        self.dword1 = if state {
            self.dword1 & !(1 << 15) | (1 << 15)
        } else {
            self.dword1 & !(1 << 15)
        };
    }

    /// Returns the value of the `Interrupt` bit.
    ///
    /// If set, indicates that an interrupt must be generated if the DMA transfer count is
    /// exhausted.
    pub fn interrupt(&self) -> bool {
        (self.dword1 & (1 << 14)) != 0
    }

    /// Sets the value of the `Interrupt` bit.
    ///
    /// If set, indicates that an interrupt must be generated if the DMA transfer count is
    /// exhausted.
    pub fn set_interrupt_bit(&mut self, state: bool) {
        self.dword1 = if state {
            self.dword1 & !(1 << 14) | (1 << 14)
        } else {
            self.dword1 & !(1 << 14)
        };
    }

    /// Returns the device port address that the `FIS` should be delivered to or is received from, if
    /// an endpoint is attached via a `Port Multiplier`.
    ///
    /// This field is set by the host for Host to Device transmission and is set by the Port
    /// Multiplier for Device to Host transmission.
    pub fn pm_port(&self) -> u8 {
        ((self.dword1 >> 8) & 0xf) as u8
    }

    /// Sets the device port address that the `FIS` should be delivered to or is received from, if
    /// an endpoint is attached via a `Port Multiplier`.
    ///
    /// This field is set by the host for Host to Device transmission and is set by the Port
    /// Multiplier for Device to Host transmission.
    pub fn set_pm_port(&mut self, port: u8) {
        self.dword1 = (self.dword1 & !0xf00) | ((port as u32) << 8);
    }

    /// Returns the number of bytes to be read or written.
    ///
    /// Must be an even value.
    pub fn dma_transfer_count(&self) -> u32 {
        self.dword6
    }

    /// Sets the number of bytes to be read or written.
    ///
    /// Must be an even value.
    pub fn set_dma_transfer_count(&mut self, transfer_count: u32) {
        self.dword6 = transfer_count;
    }

    /// Returns the byte offset into the buffer.
    pub fn dma_buffer_offset(&self) -> u32 {
        self.dword5
    }

    /// Sets the byte offset into the buffer.
    pub fn set_dma_buffer_offset(&mut self, offset: u32) {
        assert_eq!(offset & 0b11, 0);
        self.dword5 = offset;
    }

    /// Returns a value used to identify a DMA buffer region in host memory. The contents are host
    /// dependent, it is supplied by the host to the device and echoed back by the device to the
    /// host. Implementation can pass a physical address, or more complex structures for other
    /// implementations.
    pub fn dma_buffer_identifier(&self) -> u64 {
        ((self.dword2 as u64) << 32) | (self.dword1 as u64)
    }

    /// Sets a value used to identify a DMA buffer region in host memory. The contents are host
    /// dependent, it is supplied by the host to the device and echoed back by the device to the
    /// host. Implementation can pass a physical address, or more complex structures for other
    /// implementations.
    pub fn set_dma_buffer_identifier(&mut self, buffer_id: u64) {
        let buffer_id_lo = (buffer_id & 0xffffffff) as u32;
        let buffer_id_hi = ((buffer_id >> 32) & 0xffffffff) as u32;

        self.dword1 = buffer_id_lo;
        self.dword2 = buffer_id_hi;
    }
}

pub struct BISTActivateFIS {
    dword1: u32,
    dword2: u32,
    dword3: u32,
}

impl BISTActivateFIS {
    pub fn new_empty() -> Self {
        let dword1 = Into::<u8>::into(FISType::BISTActivateFIS) as u32;

        Self {
            dword1,
            dword2: 0,
            dword3: 0,
        }
    }

    /// Returns the device port address that the `FIS` should be delivered to or is received from, if
    /// an endpoint is attached via a `Port Multiplier`.
    ///
    /// This field is set by the host for Host to Device transmission and is set by the Port
    /// Multiplier for Device to Host transmission.
    pub fn pm_port(&self) -> u8 {
        ((self.dword1 >> 8) & 0xf) as u8
    }

    /// Sets the device port address that the `FIS` should be delivered to or is received from, if
    /// an endpoint is attached via a `Port Multiplier`.
    ///
    /// This field is set by the host for Host to Device transmission and is set by the Port
    /// Multiplier for Device to Host transmission.
    pub fn set_pm_port(&mut self, port: u8) {
        self.dword1 = (self.dword1 & !0xf00) | ((port as u32) << 8);
    }

    pub fn data1(&self) -> u32 {
        self.dword1
    }

    pub fn set_data1(&mut self, data: u32) {
        self.dword1 = data;
    }

    pub fn data2(&self) -> u32 {
        self.dword2
    }

    pub fn set_data2(&mut self, data: u32) {
        self.dword2 = data;
    }

    pub fn vendor_specific_test(&self) -> bool {
        (self.dword1 & (1 << 16)) != 0
    }

    pub fn set_vendor_specific_test(&mut self, state: bool) {
        self.dword1 = if state {
            self.dword1 & !(1 << 16) | (1 << 16)
        } else {
            self.dword1 & !(1 << 16)
        };
    }

    pub fn primitive(&self) -> bool {
        (self.dword1 & (1 << 18)) != 0
    }

    pub fn set_primitive_bit(&mut self, state: bool) {
        self.dword1 = if state {
            self.dword1 & !(1 << 18) | (1 << 18)
        } else {
            self.dword1 & !(1 << 18)
        };
    }

    pub fn far_end_analog(&self) -> bool {
        (self.dword1 & (1 << 19)) != 0
    }

    pub fn set_far_end_analog_bit(&mut self, state: bool) {
        self.dword1 = if state {
            self.dword1 & !(1 << 19) | (1 << 19)
        } else {
            self.dword1 & !(1 << 19)
        };
    }

    pub fn far_end_retimed_loopback(&self) -> bool {
        (self.dword1 & (1 << 20)) != 0
    }

    pub fn set_far_end_retimed_loopback_bit(&mut self, state: bool) {
        self.dword1 = if state {
            self.dword1 & !(1 << 20) | (1 << 20)
        } else {
            self.dword1 & !(1 << 20)
        };
    }

    pub fn bypass_scrambling(&self) -> bool {
        (self.dword1 & (1 << 21)) != 0
    }

    pub fn set_bypass_scrambling_bit(&mut self, state: bool) {
        self.dword1 = if state {
            self.dword1 & !(1 << 21) | (1 << 21)
        } else {
            self.dword1 & !(1 << 21)
        };
    }

    pub fn align_bypass(&self) -> bool {
        (self.dword1 & (1 << 22)) != 0
    }

    pub fn set_align_bypass_bit(&mut self, state: bool) {
        self.dword1 = if state {
            self.dword1 & !(1 << 22) | (1 << 22)
        } else {
            self.dword1 & !(1 << 22)
        };
    }

    pub fn far_end_transmit_only(&self) -> bool {
        (self.dword1 & (1 << 23)) != 0
    }

    pub fn set_far_end_transmit_only_bit(&mut self, state: bool) {
        self.dword1 = if state {
            self.dword1 & !(1 << 23) | (1 << 23)
        } else {
            self.dword1 & !(1 << 23)
        };
    }
}

/// `PIO Setup - Device to Host FIS`
///
/// Used by the device to provide the host adapter with sufficient information regarding a
/// Programmed Input/Output (PIO) data phase to allow the host adapter to efficiently handle PIO
/// data transfers. For PIO data transfers, the device shall send to the host a PIO Setup - Device
/// to Host FIS just before each and every data transfer FIS that is required to complete the data
/// transfer.
#[derive(Debug)]
pub struct PIOSetupFIS {
    dword1: u32,
    dword2: u32,
    dword3: u32,
    dword4: u32,
    dword5: u32,
}

impl PIOSetupFIS {
    /// Returns a new `PIOSetupFIS`, with only the `FIS type` defined.
    pub fn new_empty() -> Self {
        let dword1 = Into::<u8>::into(FISType::PIOSetupFIS) as u32;

        Self {
            dword1,
            dword2: 0,
            dword3: 0,
            dword4: 0,
            dword5: 0,
        }
    }

    /// Returns the device port address that the `FIS` should be delivered to or is received from, if
    /// an endpoint is attached via a `Port Multiplier`.
    ///
    /// This field is set by the host for Host to Device transmission and is set by the Port
    /// Multiplier for Device to Host transmission.
    pub fn pm_port(&self) -> u8 {
        ((self.dword1 >> 8) & 0xf) as u8
    }

    /// Sets the device port address that the `FIS` should be delivered to or is received from, if
    /// an endpoint is attached via a `Port Multiplier`.
    ///
    /// This field is set by the host for Host to Device transmission and is set by the Port
    /// Multiplier for Device to Host transmission.
    pub fn set_pm_port(&mut self, port: u8) {
        self.dword1 = (self.dword1 & !0xf00) | ((port as u32) << 8);
    }

    /// Returns the content of the `Device` register of the Command Block.
    pub fn device(&self) -> u8 {
        ((self.dword2 >> 24) & 0xff) as u8
    }

    /// Sets the content of the `Device` register of the Command Block.
    pub fn set_device(&mut self, device: u8) {
        self.dword2 = (self.dword2 & !(0xff000000)) | ((device as u32) << 24);
    }

    /// Contains the contents of the `LBA` register of the `Shadow Register Block`.
    pub fn lba(&self) -> u64 {
        let lba_1 = (self.dword2 & 0xff) as u64;
        let lba_2 = ((self.dword2 >> 8) & 0xff) as u64;
        let lba_3 = ((self.dword2 >> 16) & 0xff) as u64;
        let lba_4 = (self.dword3 & 0xff) as u64;
        let lba_5 = ((self.dword3 >> 8) & 0xff) as u64;
        let lba_6 = ((self.dword3 >> 16) & 0xff) as u64;

        lba_1 | lba_2 << 8 | lba_3 << 16 | lba_4 << 24 | lba_5 << 32 | lba_6 << 40
    }

    /// Sets the content of the `LBA` register of the Command Block.
    pub fn set_lba(&mut self, lba: u64) {
        let lba_1 = (lba & 0xff) as u32;
        let lba_2 = ((lba >> 8) & 0xff) as u32;
        let lba_3 = ((lba >> 16) & 0xff) as u32;
        let lba_4 = ((lba >> 24) & 0xff) as u32;
        let lba_5 = ((lba >> 32) & 0xff) as u32;
        let lba_6 = ((lba >> 40) & 0xff) as u32;

        self.dword2 = (self.dword2 & !0xffffff) | lba_1 | (lba_2 << 8) | (lba_3 << 16);
        self.dword3 = (self.dword3 & !0xffffff) | lba_4 | (lba_5 << 8) | (lba_6 << 16);
    }

    /// Returns the new value of the `Status` register of the Command Block for initiation of host
    /// data transfer.
    pub fn status(&self) -> u8 {
        ((self.dword1 >> 16) & 0xff) as u8
    }

    /// Sets the new value of the `Status` register of the Command Block for initiation of host
    /// data transfer.
    pub fn set_status(&mut self, cmd: u8) {
        self.dword1 = (self.dword1 & !(0x00ff0000)) | ((cmd as u32) << 16);
    }

    /// Returns the new value of the `Error` register of the Command Block at the conclusion of all
    /// subsequent Data to Device frames.
    pub fn error(&self) -> u8 {
        ((self.dword1 >> 24) & 0xff) as u8
    }

    /// Sets the new value of the `Error` register of the Command Block at the conclusion of all
    /// subsequent Data to Device frames.
    pub fn set_error(&mut self, error: u8) {
        self.dword1 = (self.dword1 & !(0xff000000)) | ((error as u32) << 24);
    }

    /// Returns the content of the `Count` register of the Command Block (`7:0`), and of the Shadow
    /// Register Block (`15:8`)
    pub fn count(&self) -> u16 {
        (self.dword4 & 0xffff) as u16
    }

    /// Sets the content of the `Count` register of the Command Block (`7:0`), and of the Shadow
    /// Register Block (`15:8`)
    pub fn set_count(&mut self, count: u16) {
        self.dword4 = (self.dword4 & !0xffff) | (count as u32);
    }

    /// Returns the new value of the Status register of the Command Block at the conclusion of the
    /// subsequent [`DataFIS`].
    pub fn e_status(&self) -> u8 {
        ((self.dword4 >> 24) & 0xff) as u8
    }

    /// Sets the new value of the Status register of the Command Block at the conclusion of the
    /// subsequent [`DataFIS`].
    pub fn set_e_status(&mut self, cmd: u8) {
        self.dword4 = (self.dword4 & !(0xff000000)) | ((cmd as u32) << 24);
    }

    /// Returns the number of bytes to be transferred in the subsequent [`DataFIS`].
    ///
    /// The transfer count value shall be nonzero and the low order bit shall be zero.
    pub fn transfer_count(&self) -> u16 {
        (self.dword5 & 0xffff) as u16
    }

    /// Sets the number of bytes to be transferred in the subsequent [`DataFIS`].
    ///
    /// The transfer count value shall be nonzero and the low order bit shall be zero.
    pub fn set_transfer_count(&mut self, count: u16) {
        self.dword5 = (self.dword5 & !(0xffff)) | (count as u32);
    }
}

/// `Data (bidirectional) FIS`
///
/// The `Data - Host to Device and the Data - Device to Host` FISes are used for transporting
/// payload data, as the data read from or written to a number of sectors on a hard drive for
/// instance. The FIS may either be generated by the device to transmit data to the host or may be
/// generated by the host to transmit data to the device. This FIS is generally only one element of
/// a sequence of transactions leading up to a data transmission, and the transactions leading up
/// to and following the Data FIS establish the proper context for both the host and device.
pub struct DataFIS {
    pub(crate) dword1: u32,
}

impl DataFIS {
    /// Returns a new `DataFIS`, with only the `FIS Type` defined.
    pub fn new_empty() -> Self {
        let dword1 = Into::<u8>::into(FISType::DataFIS) as u32;

        Self { dword1 }
    }

    /// Returns the device port address that the `FIS` should be delivered to or is received from, if
    /// an endpoint is attached via a `Port Multiplier`.
    ///
    /// This field is set by the host for Host to Device transmission and is set by the Port
    /// Multiplier for Device to Host transmission.
    pub fn pm_port(&self) -> u8 {
        ((self.dword1 >> 8) & 0xf) as u8
    }

    /// Sets the device port address that the `FIS` should be delivered to or is received from, if
    /// an endpoint is attached via a `Port Multiplier`.
    ///
    /// This field is set by the host for Host to Device transmission and is set by the Port
    /// Multiplier for Device to Host transmission.
    pub fn set_pm_port(&mut self, port: u8) {
        self.dword1 = (self.dword1 & !0xf00) | ((port as u32) << 8);
    }
}

pub enum FISType {
    RegisterHostToDevice,
    RegisterDeviceToHost,
    DMAActivateFIS,
    DMASetupFIS,
    DataFIS,
    BISTActivateFIS,
    PIOSetupFIS,
    SetDeviceBitsFIS,
    Unknown,
}

impl From<u8> for FISType {
    fn from(value: u8) -> Self {
        match value {
            0x27 => Self::RegisterHostToDevice,
            0x34 => Self::RegisterDeviceToHost,
            0x39 => Self::DMAActivateFIS,
            0x41 => Self::DMASetupFIS,
            0x46 => Self::DataFIS,
            0x58 => Self::BISTActivateFIS,
            0x5F => Self::PIOSetupFIS,
            0xA1 => Self::SetDeviceBitsFIS,
            _ => Self::Unknown,
        }
    }
}

impl From<FISType> for u8 {
    fn from(value: FISType) -> Self {
        match value {
            FISType::RegisterHostToDevice => 0x27,
            FISType::RegisterDeviceToHost => 0x34,
            FISType::DMAActivateFIS => 0x39,
            FISType::DMASetupFIS => 0x41,
            FISType::DataFIS => 0x46,
            FISType::BISTActivateFIS => 0x58,
            FISType::PIOSetupFIS => 0x5F,
            FISType::SetDeviceBitsFIS => 0xA1,
            FISType::Unknown => 0xD9,
        }
    }
}
