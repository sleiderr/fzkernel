use core::{mem, slice};

pub(crate) const AHCI_CMDH_ATAPI: u32 = 1 << 5;
pub(crate) const AHCI_CMDH_WRITE: u32 = 1 << 6;
pub(crate) const AHCI_CMDH_PREFETCHABLE: u32 = 1 << 7;
pub(crate) const AHCI_CMDH_RESET: u32 = 1 << 8;
pub(crate) const AHCI_CMDH_BIST: u32 = 1 << 9;
pub(crate) const AHCI_CMDH_CLEAR_BUSY: u32 = 1 << 10;
pub(crate) const AHCI_CMDH_PMP: u32 = 1 << 12;
pub(crate) const AHCI_CMDH_PRDTL: u32 = 1 << 16;

#[derive(Debug)]
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
    pub fn new_empty() -> Self {
        Self {
            di: 0,
            cs: 0,
            ctba: 0,
            ctba_hi: 0,
            res_1: 0,
            res_2: 0,
            res_3: 0,
            res_4: 0,
        }
    }

    pub fn build_command_table(
        &mut self,
        raw_fis: &[u8],
        raw_acmd: &[u8],
        prdt: alloc::vec::Vec<AHCIPhysicalRegionDescriptor>,
    ) {
        assert!(raw_acmd.len() < 0x11,
            "Invalid ATAPI Command header size (size is {} bytes but the maximum allowed value is 16 bytes)", raw_acmd.len());
        self.set_command_fis_length((raw_fis.len() >> 2) as u8);
        self.set_prd_table_length(prdt.len() as u16);
        let raw_prdt_len = prdt.len() * mem::size_of::<AHCIPhysicalRegionDescriptor>();

        let total_len =
            0x40 + 0x10 + 0x30 + (prdt.len() * mem::size_of::<AHCIPhysicalRegionDescriptor>());

        let mut cmd_table_bytes =
            mem::ManuallyDrop::new(alloc::vec::Vec::<u8>::with_capacity(total_len));
        unsafe {
            cmd_table_bytes.set_len(total_len);
        }
        let mut raw_fis_ext = [0u8; 0x40];
        raw_fis_ext[..raw_fis.len()].copy_from_slice(raw_fis);
        cmd_table_bytes.as_mut_slice()[..0x40].copy_from_slice(&raw_fis_ext);

        let mut raw_acmd_ext = [0u8; 0x10];
        raw_acmd_ext[..raw_acmd.len()].copy_from_slice(raw_acmd);
        cmd_table_bytes.as_mut_slice()[0x40..0x50].copy_from_slice(&raw_acmd_ext);

        let reserved = [0u8; 0x30];
        cmd_table_bytes.as_mut_slice()[0x50..0x80].copy_from_slice(&reserved);

        unsafe {
            let raw_prdt = slice::from_raw_parts(prdt.as_ptr() as *const u8, raw_prdt_len);
            cmd_table_bytes.as_mut_slice()[0x80..0x80 + raw_prdt.len()].copy_from_slice(raw_prdt);
        }

        self.set_cmd_table_base_addr64(cmd_table_bytes.as_ptr());
    }

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

    /// Indicates that the direction is a device write, or a device read if it returns false.
    pub fn is_write(&self) -> bool {
        (self.di & AHCI_CMDH_WRITE) != 0
    }

    /// Changes the direction of the transaction.
    ///
    /// Set to true for a device write, to false for a device read.
    pub fn set_write(&mut self, state: bool) {
        self.di = if state {
            (self.di & !AHCI_CMDH_WRITE) | AHCI_CMDH_WRITE
        } else {
            self.di & !AHCI_CMDH_WRITE
        };
    }

    /// Indicates if the HBA may prefetch PRDs, if this bit is set and `PRDTL` is non-zero.
    pub fn is_prefetchable(&self) -> bool {
        (self.di & AHCI_CMDH_PREFETCHABLE) != 0
    }

    /// Set if the HBA may prefetch PRDs, if this bit is set and `PRDTL` is non-zero.
    pub fn set_prefetchable(&mut self, state: bool) {
        self.di = if state {
            (self.di & !AHCI_CMDH_PREFETCHABLE) | AHCI_CMDH_PREFETCHABLE
        } else {
            self.di & !AHCI_CMDH_PREFETCHABLE
        };
    }

    /// This command is part of a software reset sequence that manipulates the `SRST` bit in the
    /// `Device Control` register.
    pub fn in_reset_sequence(&self) -> bool {
        (self.di & AHCI_CMDH_RESET) != 0
    }

    /// Set if this command is part of a software reset sequence that manipulates the `SRST` bit in the
    /// `Device Control` register.
    pub fn set_in_reset_sequence(&mut self, state: bool) {
        self.di = if state {
            (self.di & !AHCI_CMDH_RESET) | AHCI_CMDH_RESET
        } else {
            self.di & !AHCI_CMDH_RESET
        };
    }

    /// This command is for sending a BIST FIS.
    ///
    /// The HBA shall send the FIS and enter a test mode.
    pub fn is_bist(&self) -> bool {
        (self.di & AHCI_CMDH_BIST) != 0
    }

    /// Set if this command is for sending a BIST FIS.
    ///
    /// The HBA shall send the FIS and enter a test mode.
    pub fn set_bist(&mut self, state: bool) {
        self.di = if state {
            (self.di & !AHCI_CMDH_BIST) | AHCI_CMDH_BIST
        } else {
            self.di & !AHCI_CMDH_BIST
        };
    }

    /// When set, the HBA shall clear `BSY` after transmitting this `FIS` and receiving `R_OK`.
    pub fn should_clear_busy(&self) -> bool {
        (self.di & AHCI_CMDH_CLEAR_BUSY) != 0
    }

    /// Sets if the HBA shall clear `BSY` after transmitting this `FIS` and receiving `R_OK`.
    pub fn set_should_clear_busy(&mut self, state: bool) {
        self.di = if state {
            (self.di & !AHCI_CMDH_CLEAR_BUSY) | AHCI_CMDH_CLEAR_BUSY
        } else {
            self.di & !AHCI_CMDH_CLEAR_BUSY
        };
    }

    /// Indicates the port number that should be used when constructing Data FISes on transmit, and
    /// to check against all FISes received for this command.
    pub fn port_multiplier_port(&self) -> u8 {
        ((self.di >> 12) & 0xf) as u8
    }

    /// Sets the port number that should be used when constructing Data FISes on transmit, and
    /// to check against all FISes received for this command.
    pub fn set_port_multiplier_port(&mut self, port: u8) {
        self.di = (self.di & !(0b1111 << 12)) | (port as u32);
    }

    /// Length of the `Physical Region Descriptor Table` in entries.
    pub fn prd_table_length(&self) -> u16 {
        (self.di >> 16) as u16
    }

    /// Sets the length of the `Physical Region Descriptor Table` in entries.
    pub fn set_prd_table_length(&mut self, length: u16) {
        self.di = (self.di & !(0xff << 16)) | ((length as u32) << 16);
    }

    /// Indicates the current byte count that has transferred on device writes or device reads.
    pub fn bytes_count_transferred(&self) -> u32 {
        self.cs
    }

    /// Indicates the physical address of the `command table`.
    pub fn cmd_table_base_addr(&self) -> *mut u8 {
        self.ctba as *mut u8
    }

    /// Sets the physical address of the `command table`.
    pub fn set_cmd_table_base_addr(&mut self, addr: *const u8) {
        self.ctba = addr as u32;
    }

    /// Indicates the physical address of the `command table`, if 64-bit addressing is supported.
    pub fn cmd_table_base_addr64(&self) -> *mut u8 {
        (((self.ctba_hi as u64) << 32) | (self.ctba as u64)) as *mut u8
    }

    /// Sets the physical address of the `command table`, if 64-bit addressing is supported..
    pub fn set_cmd_table_base_addr64(&mut self, addr: *const u8) {
        self.ctba_hi = ((addr as u64) >> 32) as u32;
        self.ctba = ((addr as u64) & 0xffffffff) as u32;
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
