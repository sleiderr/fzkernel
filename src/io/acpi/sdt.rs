//! ACPI System Description Table.

use core::ptr;

/// System Description Table header.
///
/// Every static ACPI system description table begins with this
/// header, that can be used to identify which table we are dealing
/// with.
#[repr(C, packed)]
pub struct ACPISDTHeader {
    // The `signature` is 4 chars ASCII string that serves as a
    // table identifier.
    pub signature: [u8; 4],

    // Length of full table (including the header) in bytes.
    pub length: u32,

    // Revision of the structure.
    // Larger revision numbers are backward compatible.
    pub revision: u8,

    // The entire table (including checksum) must add to 0, otherwise
    // the table should be considered invalid.
    pub checksum: u8,

    // OEM-supplied string for identification purposes.
    pub oem_id: [u8; 6],

    // OEM-supplied string used by the OEM to identify the data table.
    pub oem_table_id: [u8; 8],

    // OEM-supplied revision number.
    pub oem_revision: u32,

    // Vendor ID of the utility that created the table.
    pub creator_id: u32,

    // Revision of the utility that created the table.
    pub creator_revision: u32,
}

/// Implement a getter method for a System Description Table.
///
/// Requires only the `signature` of the table as argument.
///
/// # Panics
///
/// Panics if the getter is called before loading the [`RSDPDescriptor`].
#[macro_export]
macro_rules! sdt_getter {
    ($sig: literal) => {
        pub fn load() -> Option<&'static mut Self> {
            let rsdp = $crate::io::acpi::RSDP
                .get()
                .expect("ACPI failure: tried to load description table before the main descriptor");

            let (rsdt_addr, entry_count, entry_size) = match rsdp {
                $crate::io::acpi::RSDPDescriptor::V1(rsdp) => {
                    let rsdt_header =
                        unsafe { core::ptr::read(rsdp.rsdt_addr as *const ACPISDTHeader) };
                    (
                        rsdp.rsdt_addr,
                        ((rsdt_header.length - core::mem::size_of::<ACPISDTHeader>() as u32) >> 2),
                        4,
                    )
                }
                $crate::io::acpi::RSDPDescriptor::V2(rsdp) => {
                    let rsdt_header =
                        unsafe { core::ptr::read(rsdp.rsdt_addr as *const ACPISDTHeader) };
                    (
                        rsdp.xsdt_addr as u32,
                        ((rsdt_header.length - core::mem::size_of::<ACPISDTHeader>() as u32) >> 3),
                        8,
                    )
                }
            };
            let mut base_address = rsdt_addr + core::mem::size_of::<ACPISDTHeader>() as u32;
            let mut address =
                unsafe { core::ptr::read(base_address as *const u32) } as *const ACPISDTHeader;
            if !(0..entry_count)
                .map(|_| {
                    base_address += entry_size;
                    address = unsafe { core::ptr::read(base_address as *const u32) }
                        as *const ACPISDTHeader;
                    let entry = unsafe { core::ptr::read(address) };
                    entry.signature
                })
                .any(|sig| sig == $sig.as_bytes())
            {
                return None;
            }

            if !$crate::io::acpi::sdt::table_checksum(unsafe { &*address }) {
                return None;
            }

            $crate::info!("acpi", "{} located at {:#010x}", $sig, address as u32);

            Some(unsafe { &mut *(address as *mut Self) })
        }
    };
}

/// Verifies the checksum of a System Description Table.
///
/// Returns true if the table fields add to 0.
pub(super) fn table_checksum(header: &ACPISDTHeader) -> bool {
    let mut checksum: u8 = 0;
    let header_addr = header as *const ACPISDTHeader as usize;
    for i in 0..header.checksum {
        let c_byte = unsafe { ptr::read((header_addr + i as usize) as *mut u8) };
        checksum = checksum.wrapping_add(c_byte);
    }

    if checksum != 0 {
        return false;
    }

    true
}
