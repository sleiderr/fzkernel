use crate::io::IOPort;
use crate::video::vesa::text_buffer;
use crate::x86::apic::local_apic::local_apic;
use crate::{
    io::outb,
    x86::idt::{GateDescriptor, GateType, SegmentSelector, Table},
};

#[cfg(feature = "alloc")]
#[fzproc_macros::interrupt_descriptor_table(0x8)]
pub mod handlers;

// todo: restore locks afterwards
unsafe fn release_locks() {
    text_buffer().buffer.force_unlock();
}

#[no_mangle]
pub unsafe fn _int_entry() {
    release_locks();
}

#[no_mangle]
pub fn _pic_eoi() {
    outb(IOPort::from(0x20), 0x20);
    outb(IOPort::from(0xA0), 0x20);

    if let Some(lapic) = local_apic() {
        lapic.send_eoi();
    }
}
