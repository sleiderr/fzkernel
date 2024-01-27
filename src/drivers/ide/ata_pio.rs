use crate::drivers::ahci::device::{ATAMediaRotationRate, SizeFormat};
use crate::drivers::generics::dev_disk::DiskDevice;
use crate::drivers::ide::ata_command::AtaCommand;
use crate::errors::CanFail;
use crate::io::{inb, inw, outb, IOPort};
use crate::mem::utils::Convertible;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use conquer_once::spin::OnceCell;
use core::cell::RefCell;
use core::hint;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use modular_bitfield::bitfield;
use spin::{Mutex, RwLock};

static LAST_ATA_DEVICE: AtomicU8 = AtomicU8::new(0);

pub fn ata_devices() -> &'static RwLock<Vec<AtaDevice>> {
    static ATA_DEVICES: OnceCell<RwLock<Vec<AtaDevice>>> = OnceCell::uninit();

    ATA_DEVICES
        .try_get_or_init(|| RwLock::new(Vec::<AtaDevice>::new()))
        .unwrap()
}

pub(super) struct AtaDevice {
    io_base: IOPort,
    ctrl_base: IOPort,
    is_slave: bool,
    busy: AtomicBool,
    command_queue: RefCell<Option<AtaCommandRequest>>,
    identify_data: RefCell<AtaIdentify>,
}

#[derive(Debug)]
pub(super) struct AtaIoResult {
    result: AtaResult,
    command: AtaCommand,
    data: Option<Vec<u8>>,
}

pub struct AtaIoRequest {
    inner: Arc<AtaIoRequestInner>,
}

struct AtaIoRequestInner {
    has_completed: AtomicBool,
    result: Mutex<Option<AtaIoResult>>,
}

impl AtaIoRequest {
    pub(super) fn new(has_completed: AtomicBool) -> Self {
        AtaIoRequest {
            inner: Arc::new(AtaIoRequestInner {
                has_completed,
                result: Mutex::new(None),
            }),
        }
    }

    pub(super) fn finish(self) -> AtaIoResult {
        while !self.inner.has_completed.load(Ordering::Relaxed) {
            hint::spin_loop();
        }

        let request_inner =
            Arc::into_inner(self.inner).expect("too many references to I/O request");
        let may_result = request_inner.result.into_inner();
        if let Some(result) = may_result {
            return result;
        }
        core::unreachable!();
    }
}

impl DiskDevice for AtaDevice {
    fn read(&self, start_lba: u64, sectors_count: u16) -> AtaIoRequest {
        self.set_lba(start_lba);
        self.set_sectors_count(sectors_count);
        self.send_ata_command(
            AtaCommandRequest::new(
                AtaCommand::AtaReadSectorsExt,
                u64::from(sectors_count)
                    * u64::from(self.identify_data.borrow().logical_sector_size()),
            )
            .with_data_buffer(alloc::vec![]),
        )
    }
}

impl AtaDevice {
    pub(super) fn init(io_base: IOPort, ctrl_base: IOPort, is_slave: bool) -> CanFail<AtaError> {
        let status = StatusRegister::read_byte(io_base);
        if status == 0xFF || status == 0 {
            return Err(AtaError::DriveNotPresent);
        }

        let device = AtaDevice {
            io_base,
            ctrl_base,
            is_slave,
            busy: AtomicBool::default(),
            command_queue: RefCell::new(None),
            identify_data: RefCell::new(AtaIdentify([0u16; 256])),
        };
        ata_devices().write().push(device);
        let dev_list = ata_devices().read();

        let dev = dev_list.last().ok_or(AtaError::DriveNotPresent)?;
        dev.identify();

        Ok(())
    }

    pub(super) fn handle_irq(&self) {
        let mut has_cmd_queued = false;
        StatusRegister::read(self.io_base);
        if let Some(queued_cmd) = self.command_queue.borrow_mut().as_mut() {
            has_cmd_queued = true;
            if let Some(callback) = &queued_cmd.callback {
                callback(self, queued_cmd.buffer.as_mut());
            }
            let transfer_size = queued_cmd.data_size.min(0x200);
            queued_cmd.data_size -= transfer_size;
            match queued_cmd.direction {
                AtaTransferDirection::Read => {
                    if let Some(buffer) = &mut queued_cmd.buffer {
                        buffer.reserve_exact(
                            usize::try_from(transfer_size).expect("invalid transfer size"),
                        );
                        for _ in 0..(transfer_size >> 1) {
                            let w = self.read_data_port();
                            buffer.push(w.low_bits());
                            buffer.push(w.high_bits());
                        }
                    }
                }
                AtaTransferDirection::Write => {}
            }
        }

        if has_cmd_queued {
            let status = StatusRegister::read_alternate(self.ctrl_base);
            if !status.bsy() && !status.drq() {
                let mut queued_cmd = self.command_queue.replace(None).expect("an ATA command should be queued");
                let mut ata_result = AtaIoResult {
                    result: AtaResult::Success,
                    command: queued_cmd.command,
                    data: None,
                };
                if let Some(on_completion) = &queued_cmd.on_completion {
                    on_completion(self, queued_cmd.buffer.as_mut());
                }

                ata_result.data = queued_cmd.buffer;
                if let Some(io_req) = &queued_cmd.io_req {
                    while io_req
                        .has_completed
                        .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
                        .is_err()
                    {}
                    *io_req.result.lock() = Some(ata_result);
                }

                while self
                    .busy
                    .compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed)
                    .is_err()
                {}
            }
        }
    }

    pub(super) fn may_expect_irq(&self) -> bool {
        self.busy.load(Ordering::Relaxed)
    }

    fn identify(&self) {
        self.send_ata_command(
            AtaCommandRequest::new(AtaCommand::AtaIdentifyDevice, 512)
                .with_data_buffer(alloc::vec![])
                .on_completion(Box::new(|dev, buffer| {
                    let mut identify_data = [0u16; 256];
                    let buffer = buffer.ok_or(AtaError::InvalidCommand)?;
                    for w in &mut identify_data {
                        let low_byte = buffer.remove(0);
                        let high_byte = buffer.remove(0);
                        *w = u16::from_le_bytes([low_byte, high_byte]);
                    }
                    dev.identify_data.borrow_mut().0 = identify_data;

                    Ok(())
                })),
        );
    }

    fn read_data_port(&self) -> u16 {
        inw(self.io_base)
    }

    fn send_ata_command(&self, command: AtaCommandRequest) -> AtaIoRequest {
        while self
            .busy
            .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
        {
            hint::spin_loop();
        }
        let io_req = AtaIoRequest::new(AtomicBool::default());
        let command_byte = command.command.discriminant();
        *self.command_queue.borrow_mut() = Some(command.link_to_ioreq(io_req.inner.clone()));
        outb(self.io_base + 0x7, command_byte);

        io_req
    }

    fn set_sectors_count(&self, count: u16) {
        match self.identify_data.borrow().addressing_mode() {
            AtaAddressingMode::Lba24 => outb(self.io_base + 0x2, count.low_bits()),
            AtaAddressingMode::Lba48 => {
                outb(self.io_base + 0x2, count.high_bits());
                outb(self.io_base + 0x2, count.low_bits());
            }
        }
    }

    fn set_lba(&self, lba: u64) {
        match self.identify_data.borrow().addressing_mode() {
            AtaAddressingMode::Lba24 => {
                outb(self.io_base + 0x3, lba.low_bits());
                outb(self.io_base + 0x4, (lba >> 8).low_bits());
                outb(self.io_base + 0x5, (lba >> 16).low_bits());
            }
            AtaAddressingMode::Lba48 => {
                outb(self.io_base + 0x6, 0x40);
                outb(self.io_base + 0x3, (lba >> 24).low_bits());
                outb(self.io_base + 0x4, (lba >> 32).low_bits());
                outb(self.io_base + 0x5, (lba >> 40).low_bits());
                outb(self.io_base + 0x3, lba.low_bits());
                outb(self.io_base + 0x4, (lba >> 8).low_bits());
                outb(self.io_base + 0x5, (lba >> 16).low_bits());
            }
        }
    }
}

unsafe impl Sync for AtaDevice {}

type AtaCommandCallback =
    Box<dyn Fn(&AtaDevice, Option<&mut Vec<u8>>) -> CanFail<AtaError> + Send + Sync>;

pub(crate) struct AtaIdentify([u16; 256]);

impl AtaIdentify {
    pub(crate) fn from_bytes(p0: [u16; 256]) -> AtaIdentify {
        Self(p0)
    }
    pub(crate) fn addressing_mode(&self) -> AtaAddressingMode {
        if ((self.0[83] >> 10) & 1) == 0 {
            AtaAddressingMode::Lba24
        } else {
            AtaAddressingMode::Lba48
        }
    }

    /// Returns the `maximum queue depth` supported by the device.
    ///
    /// The queue depth includes all command for which acceptance has occurred but not completion.
    /// Should be 0 if the `NCQ` feature set is not supported.
    pub fn queue_depth(&self) -> u8 {
        (self.0[75] & 0b11111) as u8 + 1
    }

    /// Returns the `Minimum Multiword DMA transfer cycle time per word`
    ///
    /// Defines, in nanoseconds, the minimum cycle time that the device supports when performing
    /// Multiword DMA transfer on a per word basis.
    ///
    /// Shall be set to `0x78` ns for `SATA` devices.
    pub fn minimum_multiword_dma_transfer_cycle_time_per_word(&self) -> u16 {
        self.0[65]
    }

    /// Returns the maximum number of logical sectors per `DRQ` data block that the device supports
    /// for `READ MULTIPLE`, `WRITE MULTIPLE`, ... commands.
    pub fn maximum_count_logical_sectors_per_drq(&self) -> u8 {
        (self.0[47] & 0xff) as u8
    }

    /// Indicates if:
    ///
    /// - the device has more than one logical sector per physical sector
    /// - the `Logical to Physical sector relationship` field is supported
    fn logical_physical_relationship_supported(&self) -> bool {
        self.0[106] & (1 << 13) != 0
    }

    /// Returns the size of a physical sector in number of logical sectors.
    pub fn logical_sectors_per_physical_sector(&self) -> u8 {
        if !self.logical_physical_relationship_supported() {
            return 1;
        }

        1 << ((self.0[106] & (0b1111)) as u8)
    }

    /// Indicates the nominal media rotation rate of the device in rpm, if available.
    pub fn nominal_rotation_rate(&self) -> ATAMediaRotationRate {
        match self.0[217] {
            0x0001 => ATAMediaRotationRate::NonRotating,
            speed if (0x0401..0xFFFE).contains(&speed) => {
                ATAMediaRotationRate::Rotating(speed as usize)
            }
            _ => ATAMediaRotationRate::NotReported,
        }
    }

    /// Returns the physical device size, in bytes.
    pub fn device_size(&self, format: SizeFormat) -> u64 {
        let bytes_size = self.maximum_addressable_lba() as u64 * self.logical_sector_size() as u64;
        match format {
            SizeFormat::Bytes => bytes_size,
            SizeFormat::Kilobytes => bytes_size >> 10,
            SizeFormat::Megabytes => bytes_size >> 20,
            SizeFormat::Gigabytes => bytes_size >> 30,
            SizeFormat::Terabytes => bytes_size >> 40,
        }
    }

    /// Returns the number of bytes per logical sector.
    pub fn logical_sector_size(&self) -> u32 {
        // if the logical_sector_size bit is set, the sector size is higher than 512 bytes, and the
        // value is contained is the `Logical sector size` (117..118) field.
        let logical_sector_size_supported = self.0[106] & (1 << 12) != 0;

        if logical_sector_size_supported {
            return ((self.0[118] as u32) << 16) | (self.0[117] as u32);
        }

        0x200
    }

    /// Returns the maximum LBA in user accessible space.
    pub fn maximum_addressable_lba(&self) -> usize {
        let max_lba = ((self.0[61] as u32) << 16) | (self.0[60] as u32);

        if max_lba == 0x0fff_ffff && (self.0[69] & 0b1000) != 0 {
            // use extended number instead
            return (((self.0[233] as u64) << 48)
                | ((self.0[232] as u64) << 32)
                | ((self.0[231] as u64) << 16)
                | (self.0[230] as u64)) as usize;
        }

        max_lba as usize
    }

    /// Returns the current `media serial number`.
    ///
    /// `Media serial number` is a 60-bytes string, the first 40 bytes indicate the media serial
    /// number, and the last 20 indicate the media manufacturer.
    pub fn media_serial_number(&self) -> String {
        let serial_words = &self.0[176..206];
        let mut serial_bytes: Vec<u8> = alloc::vec![];
        for word in serial_words {
            let word_lo = (word & 0xff) as u8;
            let word_hi = ((word >> 8) & 0xff) as u8;
            serial_bytes.push(word_hi);
            serial_bytes.push(word_lo);
        }

        unsafe { String::from_utf8_unchecked(serial_bytes) }
    }

    /// Returns the device's `Model Number`.
    ///
    /// It is a 40-bytes ATA string.
    pub fn model_number(&self) -> String {
        let model_words = &self.0[27..47];
        let mut model_bytes: Vec<u8> = alloc::vec![];
        for word in model_words {
            let word_lo = (word & 0xff) as u8;
            let word_hi = ((word >> 8) & 0xff) as u8;
            model_bytes.push(word_hi);
            model_bytes.push(word_lo);
        }

        unsafe { String::from_utf8_unchecked(model_bytes) }
    }

    /// Returns the device's `Serial Number`.
    pub fn serial_number(&self) -> String {
        let serial_words = &self.0[10..20];
        let mut serial_bytes: Vec<u8> = alloc::vec![];
        for word in serial_words {
            let word_lo = (word & 0xff) as u8;
            let word_hi = ((word >> 8) & 0xff) as u8;
            serial_bytes.push(word_hi);
            serial_bytes.push(word_lo);
        }

        unsafe { String::from_utf8_unchecked(serial_bytes) }
    }

    /// Returns the device's `Firmware Revision`
    pub fn firmware_revision(&self) -> String {
        let fw_words = &self.0[23..27];
        let mut fw_bytes: Vec<u8> = alloc::vec![];
        for word in fw_words {
            let word_lo = (word & 0xff) as u8;
            let word_hi = ((word >> 8) & 0xff) as u8;
            fw_bytes.push(word_hi);
            fw_bytes.push(word_lo);
        }

        unsafe { String::from_utf8_unchecked(fw_bytes) }
    }
}

#[derive(Debug)]
pub(super) enum AtaResult {
    Success,
    Error,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum AtaAddressingMode {
    /// 24-bit LBA addresses
    Lba24,

    /// 48-bit LBA addresses
    Lba48,
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(u8)]
pub(crate) enum AtaTransferDirection {
    #[default]
    Read,

    Write,
}

pub(super) struct AtaCommandRequest {
    command: AtaCommand,
    data_size: u64,
    direction: AtaTransferDirection,
    callback: Option<AtaCommandCallback>,
    on_completion: Option<AtaCommandCallback>,
    buffer: Option<Vec<u8>>,
    io_req: Option<Arc<AtaIoRequestInner>>,
}

impl AtaCommandRequest {
    pub(super) fn new(command: AtaCommand, data_size: u64) -> Self {
        Self {
            command,
            data_size,
            direction: AtaTransferDirection::default(),
            callback: None,
            on_completion: None,
            buffer: None,
            io_req: None,
        }
    }

    pub(super) fn with_direction(self, direction: AtaTransferDirection) -> Self {
        Self {
            command: self.command,
            data_size: self.data_size,
            direction,
            callback: self.callback,
            on_completion: self.on_completion,
            buffer: self.buffer,
            io_req: None,
        }
    }

    pub(super) fn with_data_buffer(self, buffer: Vec<u8>) -> Self {
        Self {
            command: self.command,
            data_size: self.data_size,
            direction: self.direction,
            callback: self.callback,
            on_completion: self.on_completion,
            buffer: Some(buffer),
            io_req: None,
        }
    }

    pub(super) fn with_callback(self, callback: AtaCommandCallback) -> Self {
        Self {
            command: self.command,
            data_size: self.data_size,
            direction: self.direction,
            callback: Some(callback),
            on_completion: self.on_completion,
            buffer: self.buffer,
            io_req: None,
        }
    }

    pub(super) fn on_completion(self, callback: AtaCommandCallback) -> Self {
        Self {
            command: self.command,
            data_size: self.data_size,
            direction: self.direction,
            callback: self.callback,
            on_completion: Some(callback),
            buffer: self.buffer,
            io_req: None,
        }
    }

    pub(super) fn link_to_ioreq(self, io_req: Arc<AtaIoRequestInner>) -> Self {
        Self {
            command: self.command,
            data_size: self.data_size,
            direction: self.direction,
            callback: self.callback,
            on_completion: self.on_completion,
            buffer: self.buffer,
            io_req: Some(io_req),
        }
    }
}

#[bitfield]
#[repr(u8)]
#[derive(Debug)]
pub(super) struct StatusRegister {
    err: bool,
    idx: bool,
    corr: bool,
    drq: bool,
    srv: bool,
    drive_fault: bool,
    rdy: bool,
    bsy: bool,
}

impl AtaRegister for StatusRegister {
    const BASE_OFFSET: u16 = 7;
}

pub(super) trait AtaRegister: From<u8> {
    const BASE_OFFSET: u16;

    #[inline(always)]
    fn read(io_base: IOPort) -> Self {
        Self::read_byte(io_base).into()
    }

    #[inline(always)]
    fn read_alternate(ctrl_base: IOPort) -> Self {
        Self::read_alternate_byte(ctrl_base).into()
    }

    #[inline(always)]
    fn read_alternate_byte(ctrl_base: IOPort) -> u8 {
        inb(ctrl_base)
    }

    #[inline(always)]
    fn read_byte(io_base: IOPort) -> u8 {
        inb(io_base + Self::BASE_OFFSET)
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum AtaError {
    DriveNotPresent,
    InvalidBufferSize,
    InvalidCommand,
}
