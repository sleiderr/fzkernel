use acpi::{AcpiHandler, AcpiTables, HpetInfo, PhysicalMapping};
use conquer_once::spin::OnceCell;
use core::ops::Deref;
use core::ptr::NonNull;
use spin::Mutex;

pub static ACPI_TABLES: OnceCell<Mutex<AcpiTables<AcpiMemoryIdentityMapper>>> = OnceCell::uninit();

pub fn acpi_tables() -> Option<&'static Mutex<AcpiTables<AcpiMemoryIdentityMapper>>> {
    unsafe {
        ACPI_TABLES
            .try_get_or_init(|| {
                Mutex::new(AcpiTables::search_for_rsdp_bios(AcpiMemoryIdentityMapper {}).unwrap())
            })
            .ok()
    }
}

#[must_use]
pub fn hpet_info() -> Option<HpetInfo> {
    HpetInfo::new(acpi_tables()?.lock().deref()).ok()
}

#[derive(Clone, Copy, Debug)]
pub struct AcpiMemoryIdentityMapper {}

impl AcpiHandler for AcpiMemoryIdentityMapper {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> PhysicalMapping<Self, T> {
        PhysicalMapping::new(
            physical_address,
            NonNull::new(physical_address as *mut T).unwrap(),
            size,
            size,
            *self,
        )
    }

    fn unmap_physical_region<T>(region: &PhysicalMapping<Self, T>) {}
}
