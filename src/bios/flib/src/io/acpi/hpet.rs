use crate::{
    io::acpi::{sdt::ACPISDTHeader, ACPIAddress},
    sdt_getter,
};

#[repr(C, packed)]
pub struct HPETDescriptionTable {
    pub header: ACPISDTHeader,
    pub event_time_block_id: u32,
    pub base_addr: ACPIAddress,
    pub hpet_number: u8,
    pub min_clock_tick_periodic: u16,
    pub page_prot_oem_attr: u8,
}

impl HPETDescriptionTable {
    sdt_getter!("HPET");
}
