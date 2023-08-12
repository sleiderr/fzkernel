use crate::x86::idt::{GateDescriptor, GateType, SegmentSelector, Table};

#[cfg(feature = "alloc")]
#[fzproc_macros::interrupt_descriptor_table(0x8)]
pub mod handlers;
