#![no_std]
#![no_main]
#![feature(const_nonnull_new)]
#![feature(const_option)]
#![feature(proc_macro_hygiene)]
#![feature(naked_functions)]

extern crate alloc;

use core::{arch::global_asm, ptr};
use core::{panic::PanicInfo, ptr::NonNull};
use fzboot::io::pic::PIC;
use fzboot::x86::idt::{load_idt, GateDescriptor, GateType, IDTDescriptor, SegmentSelector, Table};
use fzboot::{
    info,
    io::acpi::{acpi_init, hpet::hpet_clk_init},
    mem::bmalloc::heap::LockedBuddyAllocator,
    time,
    video::vesa::video_mode::ModeInfoBlock,
    x86::tsc::TSCClock,
};
use fzboot::{
    println,
    video::vesa::{
        framebuffer::{LockedTextFrameBuffer, TextFrameBuffer},
        video_mode::VESA_MODE_BUFFER,
        TEXT_BUFFER,
    },
};

global_asm!(include_str!("arch/x86/setup.S"));

const HEAP_ADDR: usize = 0x5000000;
const HEAP_SIZE: usize = 0x1000000;

#[global_allocator]
static BUDDY_ALLOCATOR: LockedBuddyAllocator<16> =
    LockedBuddyAllocator::new(NonNull::new(HEAP_ADDR as *mut u8).unwrap(), HEAP_SIZE);

#[no_mangle]
#[link_section = ".start"]
pub extern "C" fn _start() -> ! {
    boot_main();
}

pub fn boot_main() -> ! {
    fzboot::mem::zero_bss();

    init_framebuffer();
    acpi_init();
    clock_init();
    interrupts_init();

    loop {}
}

pub fn init_framebuffer() {
    let vesamode_info_ptr = VESA_MODE_BUFFER as *mut ModeInfoBlock;
    let vesamode_info = unsafe { ptr::read(vesamode_info_ptr) };
    let mut framebuffer = TextFrameBuffer::from_vesamode_info(&vesamode_info);
    TEXT_BUFFER.init_once(|| LockedTextFrameBuffer::new(framebuffer));
}

pub fn clock_init() {
    hpet_clk_init();
    TSCClock::init();

    let curr_time = time::now();

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

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("fatal: {info}");
    loop {}
}
