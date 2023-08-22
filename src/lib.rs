#![feature(allow_internal_unstable)]
#![feature(proc_macro_hygiene)]
#![feature(naked_functions)]
#![feature(pointer_byte_offsets)]
#![allow(dead_code)]
#![allow(clippy::mut_from_ref)]
#![no_std]
#[macro_use]

pub mod video;
pub mod bios;
pub mod drivers;
#[cfg(feature = "alloc")]
pub mod fs;
pub mod fzboot;
pub mod io;
pub mod mem;
pub mod x86;
#[cfg(feature = "alloc")]
pub mod network;

pub use crate::fzboot::*;
pub use numtoa;

#[cfg(feature = "alloc")]
extern crate alloc;

extern crate rlibc;
