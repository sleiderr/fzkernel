use crate::drivers::ahci::device::{ATAMediaRotationRate, SizeFormat};
use crate::drivers::generics::dev_disk::{DiskDevice, SataDeviceType};
use crate::drivers::ide::ata_command::AtaCommand;
use crate::drivers::ide::AtaDeviceIdentifier;
use crate::errors::CanFail;
use crate::fs::partitions::gpt::load_drive_gpt;
use crate::fs::partitions::mbr::{load_drive_mbr, PartitionType};
use crate::fs::partitions::{Partition, PartitionMetadata, PartitionTable};
use crate::io::{inb, inw, outb, outw, IOPort};
use crate::mem::utils::Convertible;
use crate::wait;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use conquer_once::spin::OnceCell;
use core::cell::{RefCell, UnsafeCell};
use core::hint;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use modular_bitfield::bitfield;
use modular_bitfield::specifiers::B4;
use spin::{Mutex, RwLock};

static LAST_ATA_DEVICE: AtomicU8 = AtomicU8::new(0);

pub fn ata_devices() -> &'static RwLock<BTreeMap<AtaDeviceIdentifier, Arc<AtaDevice>>> {
    static ATA_DEVICES: OnceCell<RwLock<BTreeMap<AtaDeviceIdentifier, Arc<AtaDevice>>>> =
        OnceCell::uninit();

    ATA_DEVICES
        .try_get_or_init(|| RwLock::new(BTreeMap::<AtaDeviceIdentifier, Arc<AtaDevice>>::new()))
        .unwrap()
}

pub(crate) struct AtaDevice {
    id: AtaDeviceIdentifier,
    io_base: IOPort,
    ctrl_base: IOPort,
    is_slave: bool,
    busy: AtomicBool,
    sector_sz: UnsafeCell<usize>,
    command_queue: RefCell<Option<AtaCommandRequest>>,
    identify_data: UnsafeCell<AtaIdentify>,
    sectors_per_drq: UnsafeCell<u16>,
    partition_table: UnsafeCell<PartitionTable>,
    partitions: UnsafeCell<Vec<Partition>>,
}

#[derive(Debug)]
pub struct AtaIoResult {
    pub result: AtaResult,
    pub command: AtaCommand,
    pub data: Option<Vec<u8>>,
}

pub struct AtaIoRequest {
    pub(in crate::drivers) inner: Arc<AtaIoRequestInner>,
}

pub(in crate::drivers) struct AtaIoRequestInner {
    pub(in crate::drivers) has_completed: AtomicBool,
    pub(in crate::drivers) result: Mutex<Option<AtaIoResult>>,
}

impl AtaIoRequest {
    /// Creates a new `I/O` request.
    ///
    /// It is associated with a [`AtaCommandRequest`], and indicates the current state of that
    /// command request, and contains its result when it has been fully processed.
    pub(crate) fn new(has_completed: AtomicBool) -> Self {
        AtaIoRequest {
            inner: Arc::new(AtaIoRequestInner {
                has_completed,
                result: Mutex::new(None),
            }),
        }
    }

    /// Waits until the `I/O` operation completes, and returns its result.
    ///
    /// `I/O` operations are processed as soon as the [`AtaCommandRequest`] is dispatched to the
    /// device. That process is asynchronous, and therefore to get the result of the operation you
    /// must make sure it has been fully processed by the device.
    pub fn complete(self) -> AtaIoResult {
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

        match self.identify_data().addressing_mode() {
            // todo: implement ReadMultiple support, but that addressing really shouldn't be used
            AtaAddressingMode::Lba24 => {
                let mut remaining_sectors = sectors_count;
                let mut buffer = alloc::vec![];
                let mut read_err = None;

                while remaining_sectors != 0 {
                    let sectors_to_read = u16::min(0xff, remaining_sectors);
                    self.set_lba(start_lba + u64::from(sectors_count - remaining_sectors));
                    self.set_sectors_count(sectors_to_read);
                    let cmd_result = self
                        .send_ata_command(
                            AtaCommandRequest::new(
                                AtaCommand::AtaReadSectors,
                                u64::from(sectors_to_read)
                                    * u64::try_from(self.sector_size())
                                        .expect("invalid sector size"),
                            )
                            .with_data_buffer(alloc::vec![]),
                        )
                        .complete();

                    if let AtaResult::Error(err) = cmd_result.result {
                        read_err = Some(err);
                        break;
                    }

                    if let Some(req_buf) = cmd_result.data {
                        buffer.extend(&req_buf);
                    } else {
                        read_err = Some(AtaError::new(
                            AtaErrorCode::InvalidBufferSize,
                            self.read_lba(),
                        ));
                        break;
                    }

                    remaining_sectors -= sectors_to_read;
                }

                let io_req = AtaIoRequest::new(AtomicBool::new(true));
                let mut io_res = io_req.inner.result.lock();
                let io_res_code = match read_err {
                    Some(err) => AtaResult::Error(err),
                    None => AtaResult::Success,
                };

                *io_res = Some(AtaIoResult {
                    result: io_res_code,
                    command: AtaCommand::AtaReadSectors,
                    data: Some(buffer),
                });

                drop(io_res);

                io_req
            }

            AtaAddressingMode::Lba48 => {
                let ata_cmd = if self.sectors_per_drq() == 0 {
                    AtaCommand::AtaReadSectorsExt
                } else {
                    AtaCommand::AtaReadMultipleExt
                };

                let transfer_blk_size = u16::max(
                    0x200,
                    self.sectors_per_drq()
                        * u16::try_from(self.sector_size()).expect("invalid sector size"),
                );
                self.send_ata_command(
                    AtaCommandRequest::new(
                        ata_cmd,
                        u64::from(sectors_count)
                            * u64::try_from(self.sector_size()).expect("invalid sector size"),
                    )
                    .with_transfer_blk_size(transfer_blk_size)
                    .with_data_buffer(alloc::vec![]),
                )
            }
        }
    }

    fn write(&self, start_lba: u64, sectors_count: u16, mut data: Vec<u8>) -> AtaIoRequest {
        self.set_lba(start_lba);
        self.set_sectors_count(sectors_count);

        let initial_write_sz = data.len().min(self.sector_size());
        let mut initial_sector: Vec<u8> = data.drain(..initial_write_sz).collect();
        initial_sector.resize(self.sector_size(), 0);
        let mut initial_sector_iter = initial_sector.iter();

        let request = match self.identify_data().addressing_mode() {
            AtaAddressingMode::Lba24 => {
                let mut remaining_sectors = sectors_count;
                let buffer = alloc::vec![];
                let mut read_err = None;

                while remaining_sectors != 0 {
                    let sectors_to_read = u16::min(0xff, remaining_sectors);
                    self.set_lba(start_lba + u64::from(sectors_count - remaining_sectors));
                    self.set_sectors_count(sectors_to_read);
                    let data_to_write: Vec<u8> = data
                        .drain(
                            (usize::from(sectors_count - remaining_sectors) * self.sector_size())
                                ..(usize::from(sectors_count + 1 - remaining_sectors)
                                    * self.sector_size()),
                        )
                        .collect();
                    let cmd_result = self
                        .send_ata_command(
                            AtaCommandRequest::new(
                                AtaCommand::AtaWriteSectors,
                                u64::from(sectors_to_read)
                                    * u64::try_from(self.sector_size())
                                        .expect("invalid sector size"),
                            )
                            .with_data_buffer(data_to_write)
                            .with_direction(AtaTransferDirection::Write),
                        )
                        .complete();

                    if let AtaResult::Error(err) = cmd_result.result {
                        read_err = Some(err);
                        break;
                    }

                    remaining_sectors -= sectors_to_read;
                }

                let io_req = AtaIoRequest::new(AtomicBool::new(true));
                let mut io_res = io_req.inner.result.lock();
                let io_res_code = match read_err {
                    Some(err) => AtaResult::Error(err),
                    None => AtaResult::Success,
                };

                *io_res = Some(AtaIoResult {
                    result: io_res_code,
                    command: AtaCommand::AtaWriteSectors,
                    data: Some(buffer),
                });
                drop(io_res);

                io_req
            }
            AtaAddressingMode::Lba48 => {
                let ata_cmd = if self.sectors_per_drq() == 0 {
                    AtaCommand::AtaWriteSectorsExt
                } else {
                    AtaCommand::AtaWriteMultipleExt
                };

                let transfer_blk_size = u16::max(
                    0x200,
                    self.sectors_per_drq()
                        * u16::try_from(self.sector_size()).expect("invalid sector size"),
                );
                self.send_ata_command(
                    AtaCommandRequest::new(
                        ata_cmd,
                        u64::from(sectors_count)
                            * u64::try_from(self.sector_size()).expect("invalid sector size"),
                    )
                    .with_transfer_blk_size(transfer_blk_size)
                    .with_data_buffer(data)
                    .with_direction(AtaTransferDirection::Write),
                )
            }
        };

        for _ in 0..(self.sector_size() >> 1) {
            let wd_to_wr = u16::from_le_bytes([
                *initial_sector_iter.next().expect("invalid buffer size"),
                *initial_sector_iter.next().expect("invalid buffer size"),
            ]);
            self.write_data_port(wd_to_wr);
        }

        request
    }

    fn partitions(&self) -> &Vec<Partition> {
        unsafe { &(*self.partitions.get()) }
    }

    fn identifier(&self) -> AtaDeviceIdentifier {
        return self.id;
    }

    fn max_sector(&self) -> usize {
        self.identify_data().maximum_addressable_lba()
    }

    fn logical_sector_size(&self) -> u64 {
        u64::from(self.identify_data().logical_sector_size())
    }
}

impl AtaDevice {
    pub(super) fn init(
        id: AtaDeviceIdentifier,
        io_base: IOPort,
        ctrl_base: IOPort,
        is_slave: bool,
        ctrl_id: usize,
        is_prim: bool,
    ) -> Result<AtaDeviceIdentifier, AtaErrorCode> {
        if is_slave {
            outb(io_base + 0x6, 1 << 4);
        }
        let status = StatusRegister::read_byte(io_base);
        if status == 0xFF || status == 0 {
            return Err(AtaErrorCode::DriveNotPresent);
        }

        let device = AtaDevice {
            id,
            io_base,
            ctrl_base,
            is_slave,
            busy: AtomicBool::default(),
            command_queue: RefCell::new(None),
            identify_data: UnsafeCell::new(AtaIdentify([0u16; 256])),
            sector_sz: UnsafeCell::new(0),
            sectors_per_drq: UnsafeCell::new(0),
            partition_table: UnsafeCell::new(PartitionTable::Unknown),
            partitions: UnsafeCell::new(alloc::vec![]),
        };
        let ctlr_dev_id = match (is_slave, is_prim) {
            (false, true) => 0,
            (false, false) => 1,
            (true, true) => 2,
            (true, false) => 3,
        };
        let device_id = AtaDeviceIdentifier::new(SataDeviceType::IDE, ctrl_id, ctlr_dev_id);
        ata_devices().write().insert(device_id, Arc::new(device));
        let dev_list = ata_devices().read();

        let dev = dev_list
            .get(&device_id)
            .ok_or(AtaErrorCode::DriveNotPresent)?;
        dev.enable_irq();
        dev.identify();

        dev.load_partition_table();

        Ok(device_id)
    }

    pub(super) fn enable_irq(&self) {
        ControlRegister::new().write(self.ctrl_base);
    }

    pub(super) fn disable_irq(&self) {
        ControlRegister::new()
            .with_int_disabled(true)
            .write(self.ctrl_base);
    }

    pub(super) fn soft_reset(&self) {
        // todo: also check the busy flag of the other atadevice on that bus
        while self
            .busy
            .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
            .is_err()
        {
            hint::spin_loop();
        }
        ControlRegister::new()
            .with_soft_reset(true)
            .write(self.ctrl_base);
        wait!(0.005);
        ControlRegister::new()
            .with_soft_reset(false)
            .write(self.ctrl_base);
        self.busy.store(false, Ordering::Release);
    }

    pub(super) fn sector_size(&self) -> usize {
        unsafe { *(self.sector_sz.get() as *const usize) }
    }

    pub(super) fn sectors_per_drq(&self) -> u16 {
        unsafe { *(self.sectors_per_drq.get() as *const u16) }
    }

    pub(super) fn identify_data(&self) -> &AtaIdentify {
        unsafe { &*(self.identify_data.get() as *const AtaIdentify) }
    }

    pub(super) fn set_sectors_per_drq(&self, sectors_per_drq: u8) {
        self.set_sectors_count(sectors_per_drq.into());
        self.send_ata_command(
            AtaCommandRequest::new(AtaCommand::AtaSetMultipleMode, 0).on_completion(Box::new(
                move |dev, _| {
                    unsafe {
                        (*dev.sectors_per_drq.get()) = u16::from(sectors_per_drq);
                    }
                    Ok(())
                },
            )),
        )
        .complete();
    }

    pub(super) fn handle_irq(&self) {
        let mut has_cmd_queued = false;
        StatusRegister::read(self.io_base);
        if let Some(queued_cmd) = self.command_queue.borrow_mut().as_mut() {
            has_cmd_queued = true;
            if let Some(callback) = &queued_cmd.callback {
                match callback(self, queued_cmd.buffer.as_mut()) {
                    Ok(_) => (),
                    Err(err) => queued_cmd.err = Some(AtaError::new(err, self.read_lba())),
                }
            }
            let transfer_size = queued_cmd
                .data_size
                .min(u64::from(queued_cmd.transfer_blk_size));
            queued_cmd.data_size -= transfer_size;
            let status = StatusRegister::read_alternate(self.ctrl_base);
            if status.drq() {
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
                    AtaTransferDirection::Write => {
                        if let Some(buffer) = &mut queued_cmd.buffer {
                            // todo: replace asserts with better error handling
                            assert!(
                                u64::try_from(buffer.len()).expect("invalid buffer length")
                                    >= transfer_size
                            );
                            let mut data_to_write = buffer.drain(
                                0..usize::try_from(transfer_size).expect("invalid transfer size"),
                            );
                            for _ in 0..(transfer_size >> 1) {
                                let wd_to_wr = u16::from_le_bytes([
                                    data_to_write.next().expect("invalid buffer size"),
                                    data_to_write.next().expect("invalid buffer size"),
                                ]);
                                self.write_data_port(wd_to_wr);
                            }
                        }
                    }
                }
            }
        }

        if has_cmd_queued {
            let status = StatusRegister::read_alternate(self.ctrl_base);
            if !status.bsy() && !status.drq() {
                let mut queued_cmd = self
                    .command_queue
                    .replace(None)
                    .expect("an ATA command should be queued");
                let mut ata_result = AtaIoResult {
                    result: AtaResult::Success,
                    command: queued_cmd.command,
                    data: None,
                };

                if status.err() {
                    let err_reg = ErrorRegister::read(self.io_base);
                    let lba_first_err = self.read_lba();

                    let err_code = if err_reg.abrt() {
                        AtaErrorCode::CommandAbort
                    } else if err_reg.bbk() {
                        AtaErrorCode::BadBlock
                    } else {
                        AtaErrorCode::Generic
                    };

                    queued_cmd.err = Some(AtaError::new(err_code, lba_first_err));
                }

                if status.drive_fault() {
                    queued_cmd.err = Some(AtaError::new(AtaErrorCode::DriveFault, self.read_lba()));
                }

                if let Some(on_completion) = &queued_cmd.on_completion {
                    match on_completion(self, queued_cmd.buffer.as_mut()) {
                        Ok(_) => (),
                        Err(err) => queued_cmd.err = Some(AtaError::new(err, self.read_lba())),
                    }
                }

                ata_result.result = match queued_cmd.err {
                    Some(err) => AtaResult::Error(err),
                    None => AtaResult::Success,
                };
                ata_result.data = queued_cmd.buffer;
                if let Some(io_req) = &queued_cmd.io_req {
                    while io_req
                        .has_completed
                        .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
                        .is_err()
                    {
                        hint::spin_loop();
                    }
                    *io_req.result.lock() = Some(ata_result);
                }

                while self
                    .busy
                    .compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed)
                    .is_err()
                {
                    hint::spin_loop();
                }
            }
        }
    }

    /// Loads the partitions contained on this device, whether the partition scheme is _MBR_ or
    /// _GPT_.
    pub fn load_partition_table(&self) {
        let mbr = load_drive_mbr(self, 0);

        if mbr.is_pmbr() {
            let gpt = load_drive_gpt(self);

            if let Some(gpt) = gpt {
                unsafe {
                    *self.partitions.get() = gpt.get_partitions();
                    *self.partition_table.get() = PartitionTable::GPT(gpt);

                    for partition in &mut (*self.partitions.get()) {
                        partition.load_fs().unwrap();
                    }
                }

                return;
            }
        }

        unsafe {
            *self.partitions.get() = mbr.get_partitions();
        }

        unsafe {
            for partition in (*self.partitions.get()).clone().iter() {
                if let PartitionMetadata::MBR(mut meta) = partition.metadata() {
                    // if this device uses _EPBR_, we traverse the linked list to find all partitions.
                    if matches!(meta.partition_type(), PartitionType::Extended)
                        || matches!(meta.partition_type(), PartitionType::ExtendedLBA)
                    {
                        while load_drive_mbr(self, meta.start_lba() as u64).get_partition_metadata()
                            [1]
                        .is_used()
                            || load_drive_mbr(self, meta.start_lba() as u64)
                                .get_partition_metadata()[0]
                                .is_used()
                        {
                            let partitions = load_drive_mbr(self, meta.start_lba() as u64)
                                .get_partition_metadata();

                            let mut ext_part = partitions[0];

                            ext_part.set_start_lba(ext_part.start_lba() + meta.start_lba());

                            (*self.partitions.get()).push(
                                Partition::from_metadata(
                                    0,
                                    self.id,
                                    PartitionMetadata::MBR(ext_part),
                                )
                                .unwrap(),
                            );
                            meta = partitions[1];
                        }
                    }
                }
            }

            *self.partition_table.get() = PartitionTable::MBR(mbr);
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
                    let buffer = buffer.ok_or(AtaErrorCode::InvalidCommand)?;
                    for w in &mut identify_data {
                        let low_byte = buffer.remove(0);
                        let high_byte = buffer.remove(0);
                        *w = u16::from_le_bytes([low_byte, high_byte]);
                    }
                    unsafe { (*dev.identify_data.get()).0 = identify_data }
                    unsafe {
                        *dev.sector_sz.get() =
                            usize::try_from(dev.identify_data().logical_sector_size())
                                .expect("invalid sector size")
                    }

                    Ok(())
                })),
        )
        .complete();
    }

    fn read_data_port(&self) -> u16 {
        inw(self.io_base)
    }

    fn write_data_port(&self, data: u16) {
        outw(self.io_base, data)
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
        let mut drive_reg = inb(self.io_base + 0x6);
        if self.is_slave {
            drive_reg |= 1 << 4;
        }
        outb(self.io_base + 0x6, drive_reg);
        outb(self.io_base + 0x7, command_byte);

        io_req
    }

    fn set_sectors_count(&self, count: u16) {
        match self.identify_data().addressing_mode() {
            AtaAddressingMode::Lba24 => outb(self.io_base + 0x2, count.low_bits()),
            AtaAddressingMode::Lba48 => {
                outb(self.io_base + 0x2, count.high_bits());
                outb(self.io_base + 0x2, count.low_bits());
            }
        }
    }

    fn set_lba(&self, lba: u64) {
        match self.identify_data().addressing_mode() {
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

    fn read_lba(&self) -> u64 {
        match self.identify_data().addressing_mode() {
            AtaAddressingMode::Lba24 => {
                let low_b = inb(self.io_base);
                let mid_b = inb(self.io_base);
                let high_b = inb(self.io_base);

                u64::from(low_b) | (u64::from(low_b) << 8) | (u64::from(low_b) << 16)
            }
            AtaAddressingMode::Lba48 => {
                outb(self.io_base + 0x6, 0x40);
                let b1 = inb(self.io_base + 0x3);
                let b2 = inb(self.io_base + 0x4);
                let b3 = inb(self.io_base + 0x5);
                ControlRegister::new()
                    .with_read_high(true)
                    .write(self.ctrl_base);
                let b4 = inb(self.io_base + 0x3);
                let b5 = inb(self.io_base + 0x4);
                let b6 = inb(self.io_base + 0x5);
                ControlRegister::new().write(self.ctrl_base);

                u64::from_le_bytes([b1, b2, b3, b4, b5, b6, 0, 0])
            }
        }
    }
}

unsafe impl Sync for AtaDevice {}

type AtaCommandCallback =
    Box<dyn Fn(&AtaDevice, Option<&mut Vec<u8>>) -> CanFail<AtaErrorCode> + Send + Sync>;

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

    pub(super) fn logical_sectors_per_drq(&self) -> u8 {
        self.0[59].low_bits()
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
pub enum AtaResult {
    Success,

    Error(AtaError),
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
    transfer_blk_size: u16,
    direction: AtaTransferDirection,
    callback: Option<AtaCommandCallback>,
    on_completion: Option<AtaCommandCallback>,
    buffer: Option<Vec<u8>>,
    io_req: Option<Arc<AtaIoRequestInner>>,
    err: Option<AtaError>,
}

impl AtaCommandRequest {
    pub(super) fn new(command: AtaCommand, data_size: u64) -> Self {
        Self {
            command,
            data_size,
            transfer_blk_size: 0x200,
            direction: AtaTransferDirection::default(),
            callback: None,
            on_completion: None,
            buffer: None,
            io_req: None,
            err: None,
        }
    }

    pub(super) fn with_direction(self, direction: AtaTransferDirection) -> Self {
        Self {
            command: self.command,
            data_size: self.data_size,
            transfer_blk_size: self.transfer_blk_size,
            direction,
            callback: self.callback,
            on_completion: self.on_completion,
            buffer: self.buffer,
            io_req: None,
            err: None,
        }
    }

    pub(super) fn with_data_buffer(self, buffer: Vec<u8>) -> Self {
        Self {
            command: self.command,
            data_size: self.data_size,
            transfer_blk_size: self.transfer_blk_size,
            direction: self.direction,
            callback: self.callback,
            on_completion: self.on_completion,
            buffer: Some(buffer),
            io_req: None,
            err: None,
        }
    }

    pub(super) fn with_callback(self, callback: AtaCommandCallback) -> Self {
        Self {
            command: self.command,
            data_size: self.data_size,
            transfer_blk_size: self.transfer_blk_size,
            direction: self.direction,
            callback: Some(callback),
            on_completion: self.on_completion,
            buffer: self.buffer,
            io_req: None,
            err: None,
        }
    }

    pub(super) fn with_transfer_blk_size(self, transfer_blk_size: u16) -> Self {
        Self {
            command: self.command,
            data_size: self.data_size,
            transfer_blk_size,
            direction: self.direction,
            callback: self.callback,
            on_completion: self.on_completion,
            buffer: self.buffer,
            io_req: None,
            err: None,
        }
    }

    pub(super) fn on_completion(self, callback: AtaCommandCallback) -> Self {
        Self {
            command: self.command,
            data_size: self.data_size,
            transfer_blk_size: self.transfer_blk_size,
            direction: self.direction,
            callback: self.callback,
            on_completion: Some(callback),
            buffer: self.buffer,
            io_req: None,
            err: None,
        }
    }

    pub(super) fn link_to_ioreq(self, io_req: Arc<AtaIoRequestInner>) -> Self {
        Self {
            command: self.command,
            data_size: self.data_size,
            transfer_blk_size: self.transfer_blk_size,
            direction: self.direction,
            callback: self.callback,
            on_completion: self.on_completion,
            buffer: self.buffer,
            io_req: Some(io_req),
            err: None,
        }
    }
}

#[bitfield]
#[repr(u8)]
#[derive(Debug)]
pub(super) struct ErrorRegister {
    /// Address mark not found
    am_nf: bool,

    /// Track zero not found
    tkz_nf: bool,

    /// Aborted command
    abrt: bool,

    /// Media change request
    mcr: bool,

    /// ID not found
    id_nf: bool,

    /// Media changed
    mc: bool,

    /// Uncorrectable date error
    unc: bool,

    /// Bad Block detected
    bbk: bool,
}

impl AtaRegister for ErrorRegister {
    const BASE_OFFSET: u16 = 1;
}

#[bitfield]
#[repr(u8)]
#[derive(Debug)]
pub(super) struct ControlRegister {
    #[skip]
    __: bool,
    int_disabled: bool,
    soft_reset: bool,
    #[skip]
    __: B4,
    read_high: bool,
}

impl AtaRegister for ControlRegister {
    const BASE_OFFSET: u16 = 0;
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

pub(super) trait AtaRegister: From<u8> + Into<u8> {
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

    #[inline(always)]
    fn write(self, io_base: IOPort) {
        outb(io_base + Self::BASE_OFFSET, self.into());
    }
}

#[derive(Debug)]
pub(in crate::drivers) struct AtaError {
    pub(in crate::drivers) code: AtaErrorCode,
    pub(in crate::drivers) lba: u64,
}

impl AtaError {
    pub(super) fn new(code: AtaErrorCode, lba: u64) -> Self {
        Self { code, lba }
    }
}

#[derive(Clone, Copy, Debug)]
pub(in crate::drivers) enum AtaErrorCode {
    CommandAbort,
    DriveNotPresent,
    InvalidBufferSize,
    InvalidCommand,
    BadBlock,
    Generic,
    DriveFault,
}
