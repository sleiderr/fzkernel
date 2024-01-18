#![feature(allow_internal_unstable)]
#![feature(proc_macro_hygiene)]
#![feature(naked_functions)]
#![feature(type_alias_impl_trait)]
#![feature(allocator_api)]
#![feature(const_nonnull_new)]
#![feature(const_option)]
#![feature(strict_provenance)]
#![feature(adt_const_params)]
#![feature(non_null_convenience)]
#![warn(missing_copy_implementations)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(trivial_casts)]
#![warn(trivial_numeric_casts)]
#![warn(unreachable_pub)]
#![warn(unused_crate_dependencies)]
#![warn(clippy::pedantic)]
#![warn(clippy::as_conversions)]
#![allow(dead_code)]
#![allow(clippy::mut_from_ref)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::format_in_format_args)]
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

pub use crate::fzboot::*;
pub use crate::mem::utils::*;
pub use numtoa;

#[cfg(feature = "alloc")]
extern crate alloc;

extern crate rlibc;
