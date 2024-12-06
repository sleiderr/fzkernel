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
use fzboot::irq::manager::{get_interrupt_manager, get_prot_interrupt_manager};
use fzboot::mem::e820::{e820_entries_bootloader, E820_MAP_ADDR};
use fzboot::mem::{MemoryAddress, PhyAddr, VirtAddr};
use fzboot::video::vesa::{init_text_buffer_from_vesa, text_buffer};
use fzboot::x86::apic::InterruptVector;
use fzboot::x86::descriptors::gdt::{long_init_gdt, LONG_GDT_ADDR};
use fzboot::x86::int::enable_interrupts;
use fzboot::x86::paging::bootinit_paging;
use fzboot::{
    drivers::pci::pci_devices_init,
    mem::{
        e820::{AddressRangeDescriptor, E820MemType, E820MemoryMap},
        MemoryStructure, MEM_STRUCTURE,
    },
};
use fzboot::{drivers::pci::pci_enumerate, io::pic::PIC};
use fzboot::{error, println};
use fzboot::{
    info,
    io::acpi::{acpi_init, hpet::hpet_clk_init},
    mem::bmalloc::heap::LockedBuddyAllocator,
    time,
    x86::tsc::TSCClock,
};
use fzproc_macros::interrupt_handler;

static mut DEFAULT_HEAP_ADDR: usize = 0x5000000;
/// Default heap size: 512KiB
const DEFAULT_HEAP_SIZE: usize = 0x1000000;

/// Minimum heap size: 4KiB
const MIN_HEAP_SIZE: usize = 0x1000;

const MAX_HEAP_SIZE: usize = 0x1000000;

/// Default stack size, if enough RAM is available: 32KiB
const STACK_SIZE: usize = 0x8000;

#[global_allocator]
pub static BUDDY_ALLOCATOR: LockedBuddyAllocator<14> = LockedBuddyAllocator::new(
    NonNull::new(unsafe { DEFAULT_HEAP_ADDR as *mut u8 }).unwrap(),
    DEFAULT_HEAP_SIZE,
);

#[no_mangle]
#[link_section = ".start"]
pub extern "C" fn _start() -> ! {
    boot_main();
}

pub fn boot_main() -> ! {
    init_text_buffer_from_vesa();
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

    info!("kernel", "jumping to kernel main (addr = 0x80000)");

    unsafe {
        long_init_gdt(PhyAddr::new(LONG_GDT_ADDR));
        asm!("mov ecx, {}", in(reg) mb_information_hdr_addr);
        asm!("mov ebp, 0", "push 0x10", "push 0x800000", "retf");
        core::unreachable!();
    }
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
    let pic = PIC::default();
    pic.remap(0x20, 0x28);

    let int_mgr = get_interrupt_manager();

    unsafe {
        int_mgr.load_idt();
    }
    enable_interrupts();
}

pub fn heap_init() {
    let e820_map = E820MemoryMap::new(E820_MAP_ADDR as *mut u8);
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
    error!("fatal: {info}");
    loop {}
}
