use crate::{io::acpi::sdt::ACPISDTHeader, sdt_getter};

pub struct MCFGTable {
    header: ACPISDTHeader,
}

impl MCFGTable {
    sdt_getter!("MCFG");
}
