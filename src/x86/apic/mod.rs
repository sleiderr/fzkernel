//! `APIC` (_Advanced Programmable Interrupt Controller_) implementation.
//!
//! The _APIC_ is an evolution of the older _Intel 8259 PIC_ (programmable interrupt controller).
//! Most notably, it enables the usage of multiprocessor systems.
//!
//! It usually consists of two components:
//!
//! - _Local APIC_: one for each CPU on the system, manages external interrupts for a specific CPU, and are able to
//! accept and generate _IPI_ (interprocessor interrupts).
//!
//! - _I/O APIC_: one or more for each system, it contains a redirection table to route the interrupts received from
//! external buses (_ISA_, _PCI_) to one or more _Local APICs_

pub(crate) mod io_apic;
pub(crate) mod local_apic;
pub(crate) mod mp_table;

pub use local_apic::local_apic;
