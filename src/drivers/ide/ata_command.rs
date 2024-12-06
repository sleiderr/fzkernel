use modular_bitfield::BitfieldSpecifier;
macro_rules! define_ata_cmd {
    ($name: tt, $code: literal) => {
        pub const $name: u8 = $code;
    };
}

#[derive(Clone, Copy, Debug, BitfieldSpecifier)]
#[bits = 8]
#[repr(u8)]
pub(crate) enum AtaCommand {
    AtaCheckPowerMode = 0xE5,
    AtaConfigureStream = 0x51,
    AtaDataSetMgmt = 0x06,
    AtaDeviceReset = 0x08,
    AtaDownloadMicrocode = 0x92,
    AtaDownloadMicrocodeDma = 0x93,
    AtaExecuteDeviceDiagnostic = 0x90,
    AtaFlushCache = 0xE7,
    AtaFlushCacheExt = 0xEA,
    AtaIdentifyDevice = 0xEC,
    AtaIdentifyPacket = 0xA1,
    AtaIdle = 0xE3,
    AtaIdleImmediate = 0xE1,
    AtaNcqQueueMgmt = 0x63,
    AtaNop = 0x00,
    AtaPacket = 0xA0,
    AtaReadBuffer = 0xE4,
    AtaReadBufferDma = 0xE9,
    AtaReadDma = 0xC8,
    AtaReadDmaExt = 0x25,
    AtaReadFpdmaQueued = 0x60,
    AtaReadLogExt = 0x2F,
    AtaReadLogDmaExt = 0x47,
    AtaReadMultiple = 0xC4,
    AtaReadMultipleExt = 0x29,
    AtaReadSectors = 0x20,
    AtaReadSectorsExt = 0x24,
    AtaWriteSectors = 0x30,
    AtaWriteSectorsExt = 0x34,
    AtaWriteMultipleExt = 0x39,
    AtaSetMultipleMode = 0xC6,
}

impl AtaCommand {
    pub(super) fn discriminant(&self) -> u8 {
        unsafe { *<*const _>::from(self).cast::<u8>() }
    }
}

define_ata_cmd!(ATA_CHECK_POWER_MODE, 0xE5);
define_ata_cmd!(ATA_CONFIGURE_STREAM, 0x51);
define_ata_cmd!(ATA_DATA_SET_MGMT, 0x06);
define_ata_cmd!(ATA_DEVICE_RESET, 0x08);
define_ata_cmd!(ATA_DOWNLOAD_MICROCODE, 0x92);
define_ata_cmd!(ATA_DOWNLOAD_MICROCODE_DMA, 0x93);
define_ata_cmd!(ATA_EXECUTE_DEVICE_DIAGNOSTIC, 0x90);
define_ata_cmd!(ATA_FLUSH_CACHE, 0xE7);
define_ata_cmd!(ATA_FLUSH_CACHE_EXT, 0xEA);
define_ata_cmd!(ATA_IDENTIFY_DEVICE, 0xEC);
define_ata_cmd!(ATA_IDENTIFY_PACKET, 0xA1);
define_ata_cmd!(ATA_IDLE, 0xE3);
define_ata_cmd!(ATA_IDLE_IMMEDIATE, 0xE1);
define_ata_cmd!(ATA_NCQ_QUEUE_MGMT, 0x63);
define_ata_cmd!(ATA_ABORT_NCQ_QUEUE, 0x63);
define_ata_cmd!(ATA_DEADLINE_HANDLING, 0x63);
define_ata_cmd!(ATA_NOP, 0x00);
define_ata_cmd!(ATA_PACKET, 0xA0);
define_ata_cmd!(ATA_READ_BUFFER, 0xE4);
define_ata_cmd!(ATA_READ_BUFFER_DMA, 0xE9);
define_ata_cmd!(ATA_READ_DMA, 0xC8);
define_ata_cmd!(ATA_READ_DMA_EXT, 0x25);
define_ata_cmd!(ATA_READ_FPDMA_QUEUED, 0x60);
define_ata_cmd!(ATA_READ_LOG_EXT, 0x2F);
define_ata_cmd!(ATA_READ_LOG_DMA_EXT, 0x47);
define_ata_cmd!(ATA_READ_MULTIPLE, 0xC4);
define_ata_cmd!(ATA_READ_MULTIPLE_EXT, 0x29);
define_ata_cmd!(ATA_READ_SECTORS, 0x20);
define_ata_cmd!(ATA_READ_SECTORS_EXT, 0x24);
define_ata_cmd!(ATA_READ_STREAM_DMA_EXT, 0x2A);
define_ata_cmd!(ATA_READ_STREAM_EXT, 0x2B);
define_ata_cmd!(ATA_READ_VERIFY_SECTORS, 0x40);
define_ata_cmd!(ATA_READ_VERIFY_SECTORS_EXT, 0x42);
define_ata_cmd!(ATA_RECEIVE_FPDMA_QUEUED, 0x65);
define_ata_cmd!(ATA_REQUEST_SENSE_DATA_EXT, 0x0B);
define_ata_cmd!(ATA_BLOCK_ERASE, 0xB4);
define_ata_cmd!(ATA_CRYPTO_SCRAMBLE, 0xB4);
define_ata_cmd!(ATA_OVERWRITE_EXT, 0xB4);
define_ata_cmd!(ATA_SANITIZE_ANTIFREEZE_LOCK_EXT, 0xB4);
define_ata_cmd!(ATA_SANITIZE_FREEZE_LOCK_EXT, 0xB4);
define_ata_cmd!(ATA_SANITIZE_STATUS_EXT, 0xB4);
define_ata_cmd!(ATA_SECURITY_DISABLE_PASSWORD, 0xF6);
define_ata_cmd!(ATA_SECURITY_ERASE_PREPARE, 0xF3);
define_ata_cmd!(ATA_SECURITY_ERASE_UNIT, 0xF4);
define_ata_cmd!(ATA_SECURITY_FREEZE_LOCK, 0xF5);
define_ata_cmd!(ATA_SECURITY_SET_PASSWORD, 0xF1);
define_ata_cmd!(ATA_SECURITY_UNLOCK, 0xF2);
define_ata_cmd!(ATA_SEND_FPDMA_QUEUED, 0x64);
define_ata_cmd!(ATA_SFQ_DATA_SET_MGMT, 0x64);
define_ata_cmd!(ATA_SET_DATE_TIME_EXT, 0x77);
define_ata_cmd!(ATA_SET_FEATURES, 0xEF);
define_ata_cmd!(ATA_SET_MULTIPLE_MODE, 0xC6);
define_ata_cmd!(ATA_SLEEP, 0xE6);
define_ata_cmd!(ATA_STANDBY, 0xE2);
define_ata_cmd!(ATA_TRUSTED_NONDATA, 0x5B);
define_ata_cmd!(ATA_TRUSTED_RECEIVE, 0x5C);
define_ata_cmd!(ATA_TRUSTED_RECEIVE_DMA, 0x5D);
define_ata_cmd!(ATA_TRUSTED_SEND, 0x5E);
define_ata_cmd!(ATA_TRUSTED_SEND_DMA, 0x5F);
define_ata_cmd!(ATA_WRITE_BUFFER, 0xE8);
define_ata_cmd!(ATA_WRITE_BUFFER_DMA, 0xEB);
define_ata_cmd!(ATA_WRITE_DMA, 0xCA);
define_ata_cmd!(ATA_WRITE_DMA_EXT, 0x35);
define_ata_cmd!(ATA_WRITE_DMA_FUA_EXT, 0x3D);
define_ata_cmd!(ATA_WRITE_FPDMA_QUEUED, 0x61);
define_ata_cmd!(ATA_WRITE_LOG_EXT, 0x3F);
define_ata_cmd!(ATA_WRITE_LOG_DMA_EXT, 0x57);
define_ata_cmd!(ATA_WRITE_MULTIPLE, 0xC5);
define_ata_cmd!(ATA_WRITE_MULTIPLE_EXT, 0x39);
define_ata_cmd!(ATA_WRITE_MULTIPLE_FUA_EXT, 0xCE);
define_ata_cmd!(ATA_WRITE_SECTORS, 0x30);
define_ata_cmd!(ATA_WRITE_SECTORS_EXT, 0x34);
define_ata_cmd!(ATA_WRITE_STREAM_DMA_EXT, 0x3A);
define_ata_cmd!(ATA_WRITE_STREAM_EXT, 0x3B);
define_ata_cmd!(ATA_WRITE_UNCORRECTABLE_EXT, 0x45);
define_ata_cmd!(ATA_SMART, 0xB0);
