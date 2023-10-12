#![feature(allow_internal_unstable)]
#![feature(proc_macro_hygiene)]
#![feature(naked_functions)]
#![feature(pointer_byte_offsets)]
#![no_std]
#[macro_use]

pub mod video;
pub mod bios;
pub mod drivers;
pub mod fs;
pub mod fzboot;
pub mod io;
pub mod mem;
pub mod x86;

pub use crate::fzboot::*;
pub use numtoa;

#[cfg(feature = "alloc")]
extern crate alloc;

extern crate rlibc;
