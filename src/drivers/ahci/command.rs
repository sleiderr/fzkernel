use core::mem;

pub(crate) const AHCI_CMDH_ATAPI: u32 = 1 << 5;
pub(crate) const AHCI_CMDH_WRITE: u32 = 1 << 6;
pub(crate) const AHCI_CMDH_PREFETCHABLE: u32 = 1 << 7;
pub(crate) const AHCI_CMDH_RESET: u32 = 1 << 8;
pub(crate) const AHCI_CMDH_BIST: u32 = 1 << 9;
pub(crate) const AHCI_CMDH_CLEAR_BUSY: u32 = 1 << 10;
pub(crate) const AHCI_CMDH_PMP: u32 = 1 << 12;
pub(crate) const AHCI_CMDH_PRDTL: u32 = 1 << 16;

pub struct AHCICommandHeader {
    /// Description Information
    pub di: u32,

    /// Command Status
    pub cs: u32,

    /// Command Table Base Address
    pub ctba: u32,

    /// Command Table Base Addres Upper
    pub ctba_hi: u32,

    /// Reserved 1
    pub res_1: u32,

    /// Reserved 2
    pub res_2: u32,

    /// Reserved 3
    pub res_3: u32,

    /// Reserved 4
    pub res_4: u32,
}

impl AHCICommandHeader {
    /// Length of the Command FIS, in DWORDs.
    pub fn command_fis_length(&self) -> u8 {
        (self.di & 0xf) as u8
    }

    /// Set the length of the Command FIS, in DWORDs.
    ///
    /// The maximum length allowed is 0x10, or 16 DWORDs.
    /// A value of 0 or 1 is not allowed.
    pub fn set_command_fis_length(&mut self, length: u8) {
        assert!(
            length > 1,
            "Invalid AHCI Command FIS length (length is {} bytes)",
            4 * length
        );
        assert!(
            length < 17,
            "Invalid AHCI Command FIS length (length is {} bytes but the maximum allowed value is 64)",
            4*length
        );

        self.di = (self.di & !0xf) | (length as u32);
    }

    /// When set, indicates that a PIO setup FIS shall be sent by the device indicating a transfer
    /// for the ATAPI command.
    pub fn atapi(&self) -> bool {
        (self.di & AHCI_CMDH_ATAPI) != 0
    }

    /// When set, indicates that a PIO setup FIS shall be sent by the device indicating a transfer
    /// for the ATAPI command.
    pub fn set_atapi(&mut self, state: bool) {
        self.di = if state {
            (self.di & !AHCI_CMDH_ATAPI) | AHCI_CMDH_ATAPI
        } else {
            self.di & !AHCI_CMDH_ATAPI
        };
    }

    pub fn is_write(&self) -> bool {
        (self.di & AHCI_CMDH_WRITE) != 0
    }

    pub fn set_write(&mut self, state: bool) {
        self.di = if state {
            (self.di & !AHCI_CMDH_WRITE) | AHCI_CMDH_WRITE
        } else {
            self.di & !AHCI_CMDH_WRITE
        };
    }

    pub fn is_prefetchable(&self) -> bool {
        (self.di & AHCI_CMDH_PREFETCHABLE) != 0
    }

    pub fn set_prefetchable(&mut self, state: bool) {
        self.di = if state {
            (self.di & !AHCI_CMDH_PREFETCHABLE) | AHCI_CMDH_PREFETCHABLE
        } else {
            self.di & !AHCI_CMDH_PREFETCHABLE
        };
    }

    pub fn in_reset_sequence(&self) -> bool {
        (self.di & AHCI_CMDH_RESET) != 0
    }

    pub fn set_in_reset_sequence(&mut self, state: bool) {
        self.di = if state {
            (self.di & !AHCI_CMDH_RESET) | AHCI_CMDH_RESET
        } else {
            self.di & !AHCI_CMDH_RESET
        };
    }

    pub fn is_bist(&self) -> bool {
        (self.di & AHCI_CMDH_BIST) != 0
    }

    pub fn set_bist(&mut self, state: bool) {
        self.di = if state {
            (self.di & !AHCI_CMDH_BIST) | AHCI_CMDH_BIST
        } else {
            self.di & !AHCI_CMDH_BIST
        };
    }

    pub fn should_clear_busy(&self) -> bool {
        (self.di & AHCI_CMDH_CLEAR_BUSY) != 0
    }

    pub fn set_should_clear_busy(&mut self, state: bool) {
        self.di = if state {
            (self.di & !AHCI_CMDH_CLEAR_BUSY) | AHCI_CMDH_CLEAR_BUSY
        } else {
            self.di & !AHCI_CMDH_CLEAR_BUSY
        };
    }

    pub fn port_multiplier_port(&self) -> u8 {
        ((self.di >> 12) & 0xf) as u8
    }

    pub fn set_port_multiplier_port(&mut self, port: u8) {
        self.di = (self.di & !(0b1111 << 12)) | (port as u32);
    }

    pub fn prd_table_length(&self) -> u16 {
        (self.di >> 16) as u16
    }

    pub fn set_prd_table_length(&mut self, length: u16) {
        self.di = (self.di & !(0xff << 16)) | ((length as u32) << 16);
    }

    pub fn bytes_count_transferred(&self) -> u32 {
        self.cs
    }

    pub fn cmd_table_base_addr(&self) -> *mut u8 {
        self.ctba as *mut u8
    }

    pub fn set_cmd_table_base_addr(&mut self, addr: *mut u8) {
        self.ctba = addr as u32;
    }

    pub fn cmd_table_base_addr64(&self) -> *mut u8 {
        (((self.ctba_hi as u64) << 32) | (self.ctba as u64)) as *mut u8
    }

    pub fn set_cmd_table_base_addr64(&mut self, addr: *mut u8) {
        self.ctba_hi = ((addr as u64) >> 32) as u32;
        self.ctba = ((addr as u64) & 0xffff) as u32;
    }
}

pub struct AHCIPhysicalRegionDescriptor {
    /// Data Base Address
    pub dba: u32,

    /// Data Base Address Upper
    pub dbau: u32,

    /// Reserved
    pub reserved: u32,

    /// Description Information
    pub di: u32,
}

impl AHCIPhysicalRegionDescriptor {
    pub fn base_address(&self) -> *mut u8 {
        if self.dbau != 0 {
            (((self.dbau as u64) << 32) | (self.dba as u64)) as *mut u8
        } else {
            self.dba as *mut u8
        }
    }

    pub fn set_base_address(&mut self, addr: *mut u8) {
        if mem::size_of::<*mut u8>() == 8 {
            self.dbau = ((addr as u64) >> 32) as u32;
            self.dba = ((addr as u64) & 0xffffffff) as u32;
        } else {
            self.dba = addr as u32;
        }
    }

    pub fn interrupt_on_completion(&self) -> bool {
        (self.di & (1 << 31)) != 0
    }

    pub fn set_interrupt_on_completion(&mut self, state: bool) {
        self.di = if state {
            (self.di & !(1 << 31)) | (1 << 31)
        } else {
            self.di & !(1 << 31)
        };
    }

    pub fn data_bytes_count(&self) -> u32 {
        (self.di & ((1 << 22) - 1)) + 1
    }

    pub fn set_data_bytes_count(&mut self, count: u32) {
        assert_eq!(
            count & 1,
            0,
            "Odd AHCI Physical Region Descriptor bytes count (count = {count} bytes)"
        );
        self.di = (self.di & !((1 << 22) - 1)) | (count + 1) & ((1 << 22) - 1);
    }
}
