use crate::{
    io::outb,
    x86::idt::{GateDescriptor, GateType, SegmentSelector, Table},
};

#[cfg(feature = "alloc")]
#[fzproc_macros::interrupt_descriptor_table(0x8)]
pub mod handlers;

#[no_mangle]
pub fn _pic_eoi() {
    outb(0x20, 0x20);
    outb(0xA0, 0x20);
}
