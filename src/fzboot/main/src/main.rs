#![no_std]
#![no_main]
#![feature(const_nonnull_new)]
#![feature(const_option)]
#![feature(proc_macro_hygiene)]
#![feature(naked_functions)]

mod boot;

extern crate alloc;

use boot::fzkernel;
use core::arch::asm;
use core::{panic::PanicInfo, ptr::NonNull};
use fzboot::boot::multiboot;
use fzboot::drivers::generics::dev_disk::{sata_drives, DiskDevice};
use fzboot::drivers::ide::AtaDeviceIdentifier;
use fzboot::fs::partitions::mbr;
use fzboot::mem::MemoryAddress;
use fzboot::println;
use fzboot::video::vesa::text_buffer;
use fzboot::x86::paging::bootinit_paging;
use fzboot::{
    drivers::pci::pci_devices_init,
    mem::{
        e820::{AddressRangeDescriptor, E820MemType, E820MemoryMap},
        MemoryStructure, MEM_STRUCTURE,
    },
    x86::idt::{load_idt, IDTDescriptor},
};
use fzboot::{drivers::pci::pci_enumerate, io::pic::PIC};
use fzboot::{
    info,
    io::acpi::{acpi_init, hpet::hpet_clk_init},
    mem::bmalloc::heap::LockedBuddyAllocator,
    time,
    x86::tsc::TSCClock,
};

static mut DEFAULT_HEAP_ADDR: usize = 0x5000000;
const DEFAULT_HEAP_SIZE: usize = 0x1000000;

/// Minimum heap size: 16KiB
const MIN_HEAP_SIZE: usize = 0x4000;

/// Maximum heap size: 2GiB
const MAX_HEAP_SIZE: usize = 0x80000000;

/// Default stack size, if enough RAM is available: 8 MiB
const STACK_SIZE: usize = 0x800000;

#[global_allocator]
pub static BUDDY_ALLOCATOR: LockedBuddyAllocator<16> = LockedBuddyAllocator::new(
    NonNull::new(unsafe { DEFAULT_HEAP_ADDR as *mut u8 }).unwrap(),
    DEFAULT_HEAP_SIZE,
);

#[no_mangle]
#[link_section = ".start"]
pub extern "C" fn _start() -> ! {
    boot_main();
}

pub fn boot_main() -> ! {
    fzboot::mem::zero_bss();
    heap_init();
    acpi_init();
    clock_init();
    interrupts_init();
    pci_enumerate();
    pci_devices_init();

    let kernel_part = boot::fzkernel::locate_kernel_partition();
    boot::fzkernel::load_kernel(kernel_part.0, kernel_part.1);

    let mb_information_hdr_addr = boot::headers::dump_multiboot_information_header();
    bootinit_paging::init_paging();

    let kernel_entry_ptr = boot::fzkernel::KERNEL_LOAD_ADDR.as_ptr::<()>();
    let kernel_entry: fn(*mut u8) -> ! = unsafe { core::mem::transmute(kernel_entry_ptr) };

    info!(
        "kernel",
        "jumping to kernel main (addr = {:#x})", kernel_entry_ptr as usize
    );

    kernel_entry(mb_information_hdr_addr);

    loop {}
}

pub fn clock_init() {
    hpet_clk_init();
    TSCClock::init();

    let curr_time = time::date();

    info!("rtc_clock", "Standard UTC time {curr_time}");
    info!(
        "rtc_clock",
        "time: {} date: {}",
        curr_time.format_shorttime(),
        curr_time.format_shortdate()
    );
}

pub fn interrupts_init() {
    let mut idtr = IDTDescriptor::new();
    idtr.set_offset(0x8);
    idtr.store(0x0);
    let pic = PIC::default();
    pic.remap(0x20, 0x28);
    fzboot::irq::generate_idt();
    load_idt(0x0);
}

pub fn heap_init() {
    let e820_map = E820MemoryMap::new();
    let mut best_entry = AddressRangeDescriptor::default();

    for entry in e820_map {
        if matches!(entry.addr_type, E820MemType::RAM) && entry.length() > best_entry.length() {
            best_entry = entry;
        }
    }

    assert!(best_entry.length() >= MIN_HEAP_SIZE as u64);

    if best_entry.length() > MAX_HEAP_SIZE as u64 {
        // No 64-bit support for now
        best_entry.length_high = 0;
        best_entry.length_low = MAX_HEAP_SIZE as u32;
    }

    let stack_size_min = (best_entry.length() >> 3) as usize;
    let stack_size = if stack_size_min < STACK_SIZE {
        stack_size_min as usize
    } else {
        STACK_SIZE
    };
    let heap_addr = best_entry.base_addr();
    let stack_addr = unsafe { heap_addr.add(best_entry.length() as usize) } as usize;

    let heap_size = (best_entry.length() as usize) - stack_size;

    let mem_struct = MemoryStructure {
        heap_addr: heap_addr as usize,
        heap_size,
    };
    info!(
        "mem",
        "relocated heap (addr = {:#x}    size = {:#x})", heap_addr as u64, heap_size
    );

    MEM_STRUCTURE.init_once(|| mem_struct);

    unsafe {
        BUDDY_ALLOCATOR.alloc.lock().resize(
            NonNull::new(best_entry.base_addr()).unwrap(),
            heap_size as usize,
        )
    };

    unsafe {
        asm!("mov esp, eax", in("eax") stack_addr);
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe {
        text_buffer().buffer.force_unlock();
    }
    println!("fatal: {info}");
    loop {}
}
