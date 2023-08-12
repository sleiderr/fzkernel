use core::arch::asm;

use fzboot::{
    hex_print,
    mem::gdt::{Gdtr, SegmentDescriptor},
    rinfo,
};
use fzboot::int::{disable_interrupts, enable_interrupts};

const GDT_START: u32 = 0x5da0;

pub fn load_gdt() -> Result<(), ()> {
    let mut __boot_ds = SegmentDescriptor::new();
    __boot_ds.set_base(0);
    __boot_ds.set_limit(0xfffff);
    __boot_ds.set_access_byte(0b10010010);
    __boot_ds.set_flags(0b1100);

    let mut __boot_cs = SegmentDescriptor::new();
    __boot_cs.set_base(0);
    __boot_cs.set_limit(0xfffff);
    __boot_cs.set_access_byte(0b10011010);
    __boot_cs.set_flags(0b1100);

    let null_entry = SegmentDescriptor::new();

    let mut gdt: Gdtr = Gdtr::new();

    gdt.set_offset(GDT_START);

    gdt.add_segment(__boot_ds);
    gdt.add_segment(__boot_cs);

    disable_interrupts();
    unsafe {
        asm!("lgdt [0x5da0]");
    }
    enable_interrupts();

    rinfo!("GDT initialized at ");
    hex_print!(GDT_START, u32);

    Ok(())
}
