#![feature(allow_internal_unstable)]
#![no_std]
#[macro_use]

pub mod video;
pub mod bios;
pub mod fs;
pub mod io;
pub mod mem;
pub mod time;
pub mod x86;


#[cfg(feature = "alloc")]
pub mod idt;

pub mod int;

pub use numtoa;

#[cfg(feature = "alloc")]
extern crate alloc;

extern crate rlibc;