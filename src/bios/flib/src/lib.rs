#![no_std]

#[macro_use]
pub mod video_io;
pub mod bios;
pub mod disk_io;
pub mod gdt;
pub mod io;
pub mod mem;
pub mod part_mbr;
pub mod ps2;
pub mod x86;

pub mod interrupts;
#[cfg(feature = "alloc")]
pub mod idt;
#[cfg(feature = "alloc")]
pub mod debug;
#[cfg(feature = "alloc")]
pub mod int;

pub use numtoa;

extern crate rlibc;

#[cfg(feature = "alloc")]
extern crate alloc;


