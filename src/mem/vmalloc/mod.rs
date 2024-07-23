use core::alloc::GlobalAlloc;

use conquer_once::spin::OnceCell;
use kheap::KernelHeapAllocator;
use spin::Mutex;

use crate::{
    kernel_syms::{KERNEL_HEAP_BASE, KERNEL_HEAP_SIZE, PAGE_SIZE},
    x86::paging::{get_memory_mapper, page_alloc::frame_alloc::alloc_page, PageTableFlags},
};

use super::VirtAddr;

pub(crate) mod kheap;
pub(crate) mod rbtree;

static KERNEL_HEAP_ALLOCATOR: OnceCell<Mutex<KernelHeapAllocator>> = OnceCell::uninit();

pub unsafe fn init_kernel_heap() {
    let initial_heap_page = alloc_page(PAGE_SIZE).unwrap();

    get_memory_mapper().lock().map_physical_memory(
        initial_heap_page.start,
        KERNEL_HEAP_BASE,
        PageTableFlags::new().with_write(true),
        PageTableFlags::new(),
        initial_heap_page.length,
    );

    let last_heap_page = alloc_page(PAGE_SIZE).unwrap();

    get_memory_mapper().lock().map_physical_memory(
        last_heap_page.start,
        KERNEL_HEAP_BASE + KERNEL_HEAP_SIZE - PAGE_SIZE,
        PageTableFlags::new().with_write(true),
        PageTableFlags::new(),
        last_heap_page.length,
    );

    KERNEL_HEAP_ALLOCATOR.init_once(|| {
        Mutex::new(KernelHeapAllocator::init(
            KERNEL_HEAP_BASE,
            KERNEL_HEAP_SIZE,
        ))
    })
}

pub struct SyncKernelHeapAllocator {}

unsafe impl GlobalAlloc for SyncKernelHeapAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        KERNEL_HEAP_ALLOCATOR
            .get_unchecked()
            .lock()
            .kalloc_layout(layout)
            .as_mut_ptr::<u8>()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        KERNEL_HEAP_ALLOCATOR
            .get_unchecked()
            .lock()
            .kfree(VirtAddr::new(ptr as u64))
    }
}

impl SyncKernelHeapAllocator {
    pub const fn new() -> Self {
        Self {}
    }
}
