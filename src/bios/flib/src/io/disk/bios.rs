//! BIOS based utilities to read from disk.
//!
//! Uses either LBA addressing or CHS addressing.
//! Used for early initialization of the bootloader from disk.
use core::arch::asm;

/// Checks if INT13h extensions are supported by the bios.
#[inline]
pub fn edd_ext_check(drive_number: u8) -> bool {
    let mut cflag: u16;

    // INT 13h
    // 41h call: Check Extensions Present
    //
    // Input:  AH = Function number for extensions check
    //         DL = drive index
    //         BX = 0x55aa
    //
    // Output: CF = Set if not present
    //         AH = Error code or major version number
    //         BX = 0x55aa
    //         CX = Interface support bitmask
    unsafe {
        asm!(
        "push bx",
        "mov ah, 0x41",
        "mov bx, 0x55aa",
        "int 0x13",
        "pop bx",
        "sbb ax, ax",
        in("dl") drive_number,
        out("ax") cflag
        );
    }

    if cflag == 0x00 {
        return true;
    }

    false
}

/// Resets the drive `drive_number`.
/// You can choose which drive to reset from by indicating its drive number.
///
/// Drive 0 is usually 0x80, drive 1 is 0x81 and so on.
#[inline]
pub fn drive_reset(driver_number: u8) {
    // INT 13h
    // 00h call: Reset Disk System
    //
    // Input:  AH = 0x00
    //         DL = drive index
    //
    // Output:  CF = Set on error
    //          AH = Return code
    unsafe {
        asm!(
        "xor ah, ah",
        "int 0x13",
        in("dl") driver_number
        )
    }
}

/// Converts a LBA address to a CHS one.
///
/// Returns an array, with in order: cylinder, head, sector.
/// It requires the number of sectors per track and the head counts,
/// which can be acquired from [`disk_geometry`].
#[inline]
pub fn lba_to_chs(lba: u32, sectors_per_track: u8, heads_count: u8) -> [u32; 3] {
    let temp = lba / sectors_per_track as u32;
    let sector = (lba % sectors_per_track as u32) + 1;
    let head = temp % heads_count as u32;
    let cylinder = temp / heads_count as u32;

    [cylinder, head, sector]
}

/// Reads sectors from a disk, through CHS addressing.
#[inline]
pub fn read_sectors_from_drive(
    sectors_count: u8,
    cylinder: u8,
    sector: u8,
    head: u8,
    segment: u16,
    offset: u16,
    drive_number: u8,
) {
    unsafe {
        // INT 13h
        // 02h call: Read Sectors From Drive
        //
        // Input:  AH = 0x02
        //         AL = Sectors To Read Count
        //         CH = Cylinder
        //         CL = Sector
        //         DH = Head
        //         DL = Drive
        //         ES:BX = Buffer Address Pointer
        //
        //  Output:  CF = Set on Error
        //           AH = Return code
        //           AL = Actual Sectors Read Count
        asm!(
        "mov ah, 0x2",
        "mov es, di",
        "int 0x13",
        in("di") segment,
        in("al") sectors_count,
        in("ch") cylinder,
        in("cl") (sector | ((cylinder >> 2) & 0xc0)),
        in("dh") head,
        in("dl") drive_number,
        in("bx") offset,
        );
    }
}

/// Returns information about the disk geometry, in an array with in order:
///      - Number of heads
///      - Sectors per track
#[inline]
pub fn disk_geometry(driver_number: u8) -> [u8; 2] {
    let dh: u8;
    let cl: u8;
    unsafe {
        // INT 13h
        // 08h call: Read Drive Parameters
        //
        // Input:  AH = 0x08
        //         DL = drive index
        //
        // Output: CF = Set on Error
        //         AH = Return code
        //         DL = Number of hard disk drives
        //         DH = Number of heads - 1
        //         CX[6:15] = Number of cylinders - 1
        //         CX[0:5] = Sectors per track
        //         BL = Drive type
        //         ES:DI = pointer to drive parameter table (for floppy)
        asm!("mov ah, 0x8", "int 0x13", out("dh") dh, out("cl") cl, in("dl") driver_number, options(nomem, nostack));
    }

    [dh + 1, cl & 0x3f]
}

/// A Disk Address Packet.
///
/// They provide a way to communicate with int13h extended capabilities,
/// and they use LBA 'Logical Block Addressing' instead of the older, less
/// friendly CHS 'Cylinder-Head-Sector' addressing.
///
/// Before using any of the extended capabilities, you need to make sure
/// these extensions are supported, which can be done by checking the result
/// of [`edd_ext_check`] beforehand.
///
/// It uses real mode segment:offset addressing.
/// You need to make sure that the entire buffer will fit inside the given segment.
///
/// As a reminder, the linear address corresponding to a real mode offset:segment
/// address can be obtained using the following formula:
///
///     segment * 16 + offset
///
/// You need to be careful about the fact that segments do overlap.
/// Finally, the buffer needs to be 16-bit aligned.
#[repr(C, align(128))]
pub struct AddressPacket {
    size: u8,
    zero: u8,
    sectors_count: u16,
    buffer: u32,
    s_lba: u64,
}

impl AddressPacket {
    /// Creates a new `AddressPacket` from the required information:
    #[inline]
    pub fn new(sectors_count: u16, segment: u16, offset: u16, s_lba: u64) -> Self {
        AddressPacket {
            size: 0x10,
            zero: 0x00,
            sectors_count,
            buffer: ((segment as u32) << 16) | offset as u32,
            s_lba,
        }
    }

    /// Reads sectors from the drive, based on the information provided
    /// in the `AddressPacket`.
    ///
    /// You can choose which drive to read from by indicating its drive number.
    ///
    /// Drive 0 is usually 0x80, drive 1 is 0x81 and so on.
    #[inline]
    pub fn disk_read(&self, drive_number: u8) {
        let result: u8;
        let dap_addr: *const AddressPacket = self;

        // INT 13h
        // 42h call: Extended Read Sectors From Drive
        //
        // Input:  AX = 0x42
        //         DL = Drive index
        //         DS:SI: Pointer to a [`AddressPacket`]
        //                structure
        //
        // Output:  AH = Return code
        //          CF = Set on error
        unsafe {
            asm!(
                "push si",
                "push ds",
                "mov ah, 0x42",
                "mov si, cx",
                "xor bx, bx",
                "mov ds, bx",
                "int 0x13",
                "pop ds",
                "pop si",
                in("dl") drive_number,
                in("cx") dap_addr,
                out("ah") result
            )
        }

        // The call was unsucessful.
        //
        // Usually, a failed disk read at that level is fatal, or at least
        // we will assume it is.
        //
        // Possible error codes: <http://www.ctyme.com/intr/rb-0606.htm#Table234>
        if result != 0x00 {
            loop {}
        }
    }
}
