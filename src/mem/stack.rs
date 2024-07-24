//! Kernel stack management code
//!
//! Contains the memory allocator for the kernel stack address space.

use alloc::vec::Vec;
use conquer_once::spin::OnceCell;
use spin::Mutex;

use crate::{
    kernel_syms::{KERNEL_STACK_MAPPING_BASE, KERNEL_STACK_SIZE},
    x86::paging::{get_memory_mapper, page_alloc::frame_alloc::alloc_page, PageTableFlags},
};

use super::{kernel_sec::nx_prot_enabled, Alignment, VirtAddr};

#[repr(C)]
pub struct KernelStack {
    stack: [u8; KERNEL_STACK_SIZE],
}

static MAIN_KERNEL_STACK_ALLOCATOR: OnceCell<Mutex<VirtualKernelStackAllocator>> =
    OnceCell::uninit();

/// Returns the Kernel stack allocator for this system.
///
/// Must be used when creating a new kernel thread, which uses a different stack every time.
/// The allocator manages the allocation and freeing of those stacks, as well as the mapping to physical memory.
pub fn get_kernel_stack_allocator() -> &'static Mutex<VirtualKernelStackAllocator> {
    MAIN_KERNEL_STACK_ALLOCATOR
        .get_or_init(|| Mutex::new(VirtualKernelStackAllocator::new(KERNEL_STACK_MAPPING_BASE)))
}

/// Kernel stack allocator.
///
/// It manages the virtual memory space dedicated to the kernel stack ([`KERNEL_STACK_MAPPING_BASE`]), and maps
/// the kernel stack's virtual address space to physical memory with the appropriate flags.
pub struct VirtualKernelStackAllocator {
    running_ptr: VirtAddr,
    free_stacks: Vec<VirtAddr>,
}

impl VirtualKernelStackAllocator {
    pub fn new(base: VirtAddr) -> Self {
        Self {
            running_ptr: base,
            free_stacks: Vec::new(),
        }
    }

    /// Allocates a new kernel stack.
    ///
    /// Used when creating a new kernel thread, as each one relies on a different stack.
    /// It also allocates physical memory to support the kernel stack, and takes care of mapping
    /// memory with the appropriate flags and permissions.
    pub fn alloc_stack(&mut self) -> VirtAddr {
        if let Some(stack) = self.free_stacks.pop() {
            stack + KERNEL_STACK_SIZE
        } else {
            let stack = self.running_ptr;

            let stack_page = alloc_page(KERNEL_STACK_SIZE).unwrap();

            let mut stack_page_flags = PageTableFlags::new().with_write(true);

            if nx_prot_enabled() {
                stack_page_flags.set_nxe(true);
            }

            unsafe {
                get_memory_mapper().lock().map_physical_memory(
                    stack_page.start,
                    stack,
                    stack_page_flags,
                    PageTableFlags::new().with_write(true),
                    KERNEL_STACK_SIZE,
                );
            }

            self.running_ptr = self.running_ptr + KERNEL_STACK_SIZE;

            stack + KERNEL_STACK_SIZE - 0x1_usize
        }
    }

    pub fn free_stack(&mut self, stack: VirtAddr) {
        // TODO: add a MAX_MAPPED_KERNEL_STACK constant and free the phyiscal memory when too many unused kernel stacks are still mapped
        // to physical memory
        if !stack.is_aligned_with(Alignment::ALIGN_4KB) {
            panic!("attempted to free stack with invalid alignment");
        }

        self.free_stacks.push(stack)
    }
}
