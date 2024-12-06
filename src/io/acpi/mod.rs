//! ACPI implementation and related utils.
//!
//! Based on the following specification: <https://uefi.org/sites/default/files/resources/ACPI_Spec_6_5_Aug29.pdf>
use acpi::hpet::HpetTable;
use acpi::{AcpiHandler, AcpiTables, PhysicalMapping};
use core::ops::Deref;
use core::ptr::NonNull;
use core::{mem, ptr, slice};

use conquer_once::spin::OnceCell;

use crate::{error, info, println};

pub mod hpet;
pub mod sdt;

/// Shared [`RSDPDescriptor`] initialized during ACPI setup.
pub static RSDP: OnceCell<RSDPDescriptor> = OnceCell::uninit();

/// ACPI Generic Address Structure.
///
/// Used to express register addresses within ACPI-defined tables.
#[repr(C, packed)]
#[derive(Debug)]
pub struct ACPIAddress {
    // Address space to which the data structure / register belongs
    // to.
    address_space_id: u8,

    // Size in bits of the register.
    // Must be 0 when addressing a data structure.
    register_bit_width: u8,

    // Bit offset of the given register at the given address.
    // Must be 0 when addressing a data structure.
    register_bit_offset: u8,

    // Specifies access size
    access_size: ACPIAddressAccessSize,

    // 64-bit address of the data structure / register in the
    // address space.
    pub address: u64,
}

/// ACPI Generic Address Structure access size.
#[repr(u8)]
#[derive(Clone, Copy, Debug)]
pub enum ACPIAddressAccessSize {
    Undefined = 0,
    Byte = 1,
    Word = 2,
    Dword = 3,
    Qword = 4,
}

/// Root System Description Pointer structure.
///
/// This contains the address of the RSDT / XSDT table that contains
/// pointers to the other tables in memory.
///
/// It must be located and loaded before using any ACPI-related functionnality.
///
/// Depending on the ACPI revision, the `RSDPDescriptor` contains different fields.
/// When revision value is over 2, it contains 4 additional fields.
pub enum RSDPDescriptor {
    V1(RSDPDescriptorV1),
    V2(RSDPDescriptorV2),
}

/// [`RSDPDescriptor`] for ACPI revision lower than 2.
///
/// Does not contain the XSDT address.
#[repr(C, packed)]
pub struct RSDPDescriptorV1 {
    // 8 chars ASCII string that serves as an identifier.
    // Should equal "RSD PTR ".
    signature: [u8; 8],

    // Checksum for the first 20 bytes of the table.
    // For revision lower than 2, that is the full table
    // checksum.
    checksum: u8,

    // OEM-supplied string that identifies the OEM.
    oem_id: [u8; 6],

    // Revision of the structure. ACPI 1.0 revision number is 0, and
    // the corresponding RSDP structure is the `RSDPDescriptorV1`.
    pub revision: u8,

    // 32-bit physical address of the Root System Descriptor Table.
    pub rsdt_addr: u32,
}

/// [`RSDPDescriptor`] for ACPI revision 2 or above.
///
/// It contains 4 fields that are not present in [`RSDPDescriptorV1`], and
/// the `xsdt_addr` should be used instead of the `rsdt_addr`.
#[repr(C, packed)]
pub struct RSDPDescriptorV2 {
    // 8 chars ASCII string that serves as an identifier.
    // Should equal "RSD PTR ".
    signature: [u8; 8],

    // Checksum for the first 20 bytes of the table.
    // For revision lower than 2, that is the full table
    // checksum.
    checksum: u8,

    // OEM-supplied string that identifies the OEM.
    oem_id: [u8; 6],

    // Revision of the structure. ACPI 1.0 revision number is 0, and
    // the corresponding RSDP structure is the `RSDPDescriptorV1`.
    pub revision: u8,

    // 32-bit physical address of the Root System Descriptor Table.
    pub rsdt_addr: u32,

    // Length of the table in bytes, including the header.
    length: u32,

    // 64-bit physical address of the
    pub xsdt_addr: u64,

    // Checksum for the entire `RSDPDescriptorV2` structure.
    ext_checksum: u8,

    // Reserved fields
    reserved: [u8; 3],
}

pub fn acpi_init() {
    __load_rsdp();
}

/// Initialize the `RSDPDescriptor`.
///
/// The structure must be located in the physical memory.
fn __load_rsdp() {
    // We expect the structure to be in the [0xe0000 - 0xfffff] section of the
    // physical memory.
    let mut address = 0xe0000;

    while address < 0xfffff {
        let sig: &[u8];
        unsafe {
            sig = slice::from_raw_parts(address as *mut u8, 8);
        }

        // We look for the signature of the [`RSDPDescriptor`]
        if sig == "RSD PTR ".as_bytes() {
            info!("acpi", "found RSDP descriptor at {:#010x}", address);
            break;
        }

        address += 8;
    }

    // We went through the whole segment without locating the signature.
    if address >= 0xfffff {
        panic!("failed to locate RSDP descriptor");
    }

    // The first fields of the [`RSDPDescriptor`] are identical regardless of the revision.
    // So we cast it early as a V1 descriptor to check the revision first.
    let rsdp: RSDPDescriptorV1 = unsafe { ptr::read(address as *const RSDPDescriptorV1) };

    info!("acpi", "root system descriptor table at {:#010x}", unsafe {
        ptr::read_unaligned(ptr::addr_of!(rsdp.rsdt_addr))
    });

    // We now need to distinguish between revision numbers.
    match rsdp.revision {
        0 => {
            info!("acpi", "ACPI version 1.0");

            let check_checksum = __validate_checksum(&rsdp);
            if !check_checksum {
                error!("acpi", "invalid RSDP checksum");
            }

            RSDP.init_once(|| RSDPDescriptor::V1(rsdp));
        }
        2 => {
            info!("acpi", "ACPI version > 1.0");
            let rsdp: RSDPDescriptorV2 = unsafe { ptr::read(address as *const RSDPDescriptorV2) };

            let check_checksum = __validate_checksum(&rsdp);
            if !check_checksum {
                error!("acpi", "invalid RSDP checksum");
            }

            RSDP.init_once(|| RSDPDescriptor::V2(rsdp));
        }
        _ => {
            error!("acpi", "Invalid ACPI revision number");
        }
    }
}

/// Validate the checksum of a [`RSDPDescriptor`] header, regardless
/// of the ACPI revision.
fn __validate_checksum<T>(header: &T) -> bool {
    let ptr = header as *const T as usize;
    let mut checksum: u8 = 0;

    for i in 0..mem::size_of::<T>() {
        let c_byte = unsafe { ptr::read((ptr + i) as *const u8) };

        checksum = checksum.wrapping_add(c_byte);
    }

    if checksum != 0 {
        return false;
    }

    true
}
