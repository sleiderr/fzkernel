#![feature(allow_internal_unstable)]
#![feature(proc_macro_hygiene)]
#![feature(noop_waker)]
#![feature(naked_functions)]
#![feature(type_alias_impl_trait)]
#![feature(allocator_api)]
#![feature(const_nonnull_new)]
#![feature(const_option)]
#![feature(strict_provenance)]
#![feature(adt_const_params)]
#![feature(
    maybe_uninit_array_assume_init,
    maybe_uninit_uninit_array,
    const_maybe_uninit_array_assume_init,
    const_maybe_uninit_write,
    const_mut_refs,
    const_maybe_uninit_uninit_array
)]
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
pub mod boot;
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

/// Contains various symbols and constants often reused in the Kernel and bootloader code.
pub mod kernel_syms {
    use crate::mem::{PhyAddr, VirtAddr};

    /// Starting physical address to which the Kernel is loaded.
    pub const KERNEL_LOAD_ADDR: PhyAddr = PhyAddr::new(0x800_000);

    /// Size of the Kernel in sectors (512 bytes chunks).
    pub const KERNEL_SECTOR_SZ: usize = 0x20 * 0x800;

    /// Standard size for every Kernel stack.
    pub const KERNEL_STACK_SIZE: usize = 0x800_000;

    /// Base virtual address for the mapping of the Kernel code.
    pub const KERNEL_CODE_MAPPING_BASE: VirtAddr = VirtAddr::new(0xFFFF_8C00_0000_0000);

    /// Base virtual address for the physical memory mapping.
    pub const KERNEL_PHYS_MAPPING_BASE: VirtAddr = VirtAddr::new(0xFFFF_CF80_0000_0000);

    /// Base virtual address of the segment dedicated to Kernel stacks.
    pub const KERNEL_STACK_MAPPING_BASE: VirtAddr = VirtAddr::new(0xFFFF_9000_0000_0000);

    /// Base virtual address of the Kernel heap.
    pub const KERNEL_HEAP_BASE: VirtAddr = VirtAddr::new(0xFFFF_B000_0000_0000);

    /// Default size for the Kernel heap.
    ///
    /// This is the size of the virtual memory segment, and does not correspond to a standard physical memory size for the Kernel heap.
    #[cfg(target_pointer_width = "64")]
    pub const KERNEL_HEAP_SIZE: usize = 0xBAB_0000_0000;

    /// Smallest size available for memory pages.
    ///
    /// That may depend on the architure of the system, but for now we are using the standard size of 4KB for virtual memory pages.
    pub const PAGE_SIZE: usize = 0x1000;

    /// Size of large memory pages.
    ///
    /// Multiple level paging with huge page capacities can offer several virtual memory page sizes.
    pub const LARGE_PAGE_SIZE: usize = 0x200_000;

    /// Highest size available for memory pages.
    pub const HUGE_PAGE_SIZE: usize = 0x40_000_000;
}
