#![feature(start)]
#![feature(const_nonnull_new)]
#![feature(const_option)]
#![feature(naked_functions)]
#![feature(panic_info_message)]
#![no_std]
#![no_main]

extern crate alloc;

use core::{arch::asm, mem::size_of, panic::PanicInfo, ptr::NonNull};

use fzboot::{
    boot::multiboot::mb_information,
    exceptions::{panic::panic_entry_no_exception, register_exception_handlers},
    irq::manager::get_interrupt_manager,
    mem::{
        e820::E820MemoryMap,
        kernel_sec::enable_kernel_mem_sec,
        stack::get_kernel_stack_allocator,
        vmalloc::{init_kernel_heap, SyncKernelHeapAllocator},
        MemoryAddress, PhyAddr, VirtAddr,
    },
    video,
    x86::{
        descriptors::gdt::{long_init_gdt, LONG_GDT_ADDR},
        int::enable_interrupts,
        paging::{
            get_memory_mapper, init_global_mapper,
            page_alloc::frame_alloc::init_phys_memory_pool,
            page_table::mapper::{MemoryMapping, PhysicalMemoryMapping},
        },
    },
};

#[global_allocator]
pub static KERNEL_HEAP_ALLOCATOR: SyncKernelHeapAllocator = SyncKernelHeapAllocator::new();

#[no_mangle]
#[link_section = ".start"]
pub extern "C" fn _start() -> ! {
    let mut mb_information_ptr: u64 = 0;
    unsafe {
        asm!("", out("rcx") mb_information_ptr);
    }

    let mb_information: mb_information::MultibootInformation = unsafe {
        core::ptr::read(mb_information_ptr as *const mb_information::MultibootInformation)
    };

    unsafe {
        mem_init(&mb_information);
    }

    video::vesa::init_text_buffer_from_multiboot(mb_information.framebuffer().unwrap());
    let kernel_stack = get_kernel_stack_allocator().lock().alloc_stack();

    unsafe {
        asm!("
            mov rsp, {}
            mov rbp, rsp
        ", in(reg) kernel_stack.as_mut_ptr::<u8>());
    }

    _kmain();
}

#[no_mangle]
#[inline(never)]
extern "C" fn _kmain() -> ! {
    unsafe {
        get_memory_mapper()
            .lock()
            .unmap_physical_memory(VirtAddr::new(0), 0x1_000_000);
    }

    enable_kernel_mem_sec();

    unsafe {
        get_interrupt_manager().load_idt();
    }
    register_exception_handlers();

    enable_interrupts();

    loop {}
}

unsafe fn mem_init(mb_information: &mb_information::MultibootInformation) {
    let memory_map = E820MemoryMap::new(
        PhysicalMemoryMapping::KERNEL_DEFAULT_MAPPING
            .convert(PhyAddr::from(mb_information.get_mmap_addr()))
            .as_mut_ptr(),
    );

    long_init_gdt(
        PhysicalMemoryMapping::KERNEL_DEFAULT_MAPPING.convert(PhyAddr::new(LONG_GDT_ADDR)),
    );

    init_phys_memory_pool(memory_map);
    init_global_mapper(PhyAddr::new(0x200_000));
    init_kernel_heap();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(panic_msg) = info.message() {
        panic_entry_no_exception(panic_msg.as_str().unwrap());
    }

    if let Some(panic_msg) = info.payload().downcast_ref::<&str>() {
        panic_entry_no_exception(panic_msg);
    } else {
        panic_entry_no_exception("Unknown exception");
    }
}
