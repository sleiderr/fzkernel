//! Main physical memory allocator.
//!
//! Allocates contiguous areas of physical memory, using a buddy memory allocator. Only uses a single
//! buddy allocator, and therefore does not differentiate between different areas / types of physical
//! memory. It may be required to implement another buddy allocator to manage DMA-compatible memory
//! for instance.
//!
//! Frames allocated using these allocators are not mapped to any virtual address space, thus paging requires
//! a page mapper to map frames in a virtual address space.

use conquer_once::spin::OnceCell;
use spin::Mutex;

use crate::kernel_syms;
use crate::mem::e820::{AddressRangeDescriptor, E820MemType, E820MemoryMap};
use crate::mem::{MemoryAddress, PhyAddr};
use crate::x86::paging::page_table::mapper::{MemoryMapping, PhysicalMemoryMapping};
use core::cmp::{max, min};
use core::mem::MaybeUninit;
use core::ptr::null_mut;
use core::sync::atomic::{AtomicPtr, Ordering};

pub const MAX_PHYSICAL_MEM_BLK_SIZE: usize = 0x20000000;

/// Defines the basic set of operations that should be offered by a physical memory allocator (_Frame_ allocator)
pub trait FrameAllocator {
    /// Allocates a `Frame` (contiguous area of physical memory) from the physical memory pool associated with this
    /// allocator.
    ///
    /// Returns a structure that contains all information about the physical _Frame_ that has been allocated.
    ///
    /// # Errors
    ///
    /// Returns a variant of [`FrameAllocationError`] if any step of the allocation process failed. Usually, it means that
    /// the allocator is running out of memory, in which case the [`FrameAllocationError::NoAvailableFrame`] variant is returned.
    fn allocate(&mut self, size: usize) -> Result<FrameAllocation, FrameAllocationError>;

    /// Deallocates a `Frame` (contiguous area of physical memory) from the physical memory pool associated with this allocator.
    fn deallocate(&mut self, alloc: FrameAllocation);
}

/// Contains information about a physical memory `Frame` after it has been allocated by a [`FrameAllocator`].
#[derive(Debug)]
pub struct FrameAllocation {
    start: PhyAddr,
    length: usize,
}

/// Errors that may happen during the physical memory `Frame` allocation process.
#[derive(Debug)]
pub enum FrameAllocationError {
    /// The allocator ran out of `Frame` of appropriate size. Usually means that the allocator is running out of memory.
    NoAvailableFrame,
}

struct FreePageBlock {
    next_blk: AtomicPtr<FreePageBlock>,
}

impl FreePageBlock {
    fn new(next_blk: AtomicPtr<FreePageBlock>) -> Self {
        Self { next_blk: next_blk }
    }
}

static PHYSICAL_MEMORY_POOL: OnceCell<Mutex<BuddyFrameAllocator<18, PhysicalMemoryMapping>>> =
    OnceCell::uninit();

#[no_mangle]
pub unsafe extern "C" fn pm_alloc(alloc_size: usize) -> *mut u8 {
    if let Some(mem_pool) = PHYSICAL_MEMORY_POOL.get() {
        let allocation_attempt = mem_pool.lock().allocate(alloc_size);

        if let Ok(alloc) = allocation_attempt {
            alloc.start.as_mut_ptr()
        } else {
            null_mut()
        }
    } else {
        null_mut()
    }
}

#[no_mangle]
pub unsafe extern "C" fn pm_free(alloc_base: *mut u8, alloc_size: usize) {
    if let Some(mem_pool) = PHYSICAL_MEMORY_POOL.get() {
        mem_pool.lock().deallocate(FrameAllocation {
            start: PhyAddr::from(alloc_base),
            length: alloc_size,
        })
    }
}

#[no_mangle]
pub unsafe extern "C" fn init_phys_memory_pool(memory_map: E820MemoryMap) {
    let mut largest_ram_segment = AddressRangeDescriptor::default();

    for entry in memory_map {
        if matches!(entry.addr_type, E820MemType::RAM)
            && entry.length() > largest_ram_segment.length()
        {
            largest_ram_segment = entry;
        }
    }

    let mut segment_base = PhyAddr::from(largest_ram_segment.base_addr());

    // check if the kernel mapping is located inside the largest ram segment
    if kernel_syms::KERNEL_LOAD_ADDR > segment_base
        && kernel_syms::KERNEL_LOAD_ADDR < segment_base + largest_ram_segment.length()
    {
        segment_base = kernel_syms::KERNEL_LOAD_ADDR + kernel_syms::KERNEL_SECTOR_SZ * 0x200;
    }

    assert!(
        !PHYSICAL_MEMORY_POOL.is_initialized(),
        "attempted to initialize physical memory twice"
    );

    PHYSICAL_MEMORY_POOL.init_once(|| unsafe {
        Mutex::new(BuddyFrameAllocator::from_base_unchecked(
            segment_base,
            PhysicalMemoryMapping::KERNEL_DEFAULT_MAPPING,
            MAX_PHYSICAL_MEM_BLK_SIZE,
        ))
    });
}

// TODO: add allocation flags (urgent allocation that panic if lock is held, ...)
pub fn alloc_page(alloc_size: usize) -> Result<FrameAllocation, FrameAllocationError> {
    if let Some(mem_pool) = PHYSICAL_MEMORY_POOL.get() {
        mem_pool.lock().allocate(alloc_size)
    } else {
        Err(FrameAllocationError::NoAvailableFrame)
    }
}

pub fn free_page(alloc: FrameAllocation) {
    if let Some(mem_pool) = PHYSICAL_MEMORY_POOL.get() {
        mem_pool.lock().deallocate(alloc)
    }
}

/// Main physical memory allocator used by the kernel.
///
/// Allocates contiguous areas of physical memory, using a buddy memory allocator.
///
/// Frames allocated by this allocator are not mapped to the virtual address space, they are _raw_ frames.
#[derive(Debug)]
pub struct BuddyFrameAllocator<const N: usize, M: MemoryMapping> {
    base_addr: PhyAddr,

    max_blk_size: usize,

    min_blk_size: usize,

    log2_min_blk_size: usize,

    mapping: M,

    free_lists: [AtomicPtr<FreePageBlock>; N],
}

impl<const N: usize, M: MemoryMapping> FrameAllocator for BuddyFrameAllocator<N, M> {
    fn allocate(&mut self, size: usize) -> Result<FrameAllocation, FrameAllocationError> {
        let alloc_ptr = unsafe { self.alloc(size) };

        if alloc_ptr.is_null() {
            return Err(FrameAllocationError::NoAvailableFrame);
        }

        Ok(FrameAllocation {
            start: PhyAddr::from(alloc_ptr),
            length: size,
        })
    }

    fn deallocate(&mut self, alloc: FrameAllocation) {
        unsafe {
            self.dealloc(alloc.start.as_mut_ptr(), alloc.length);
        }
    }
}

impl<const N: usize, M: MemoryMapping> BuddyFrameAllocator<N, M> {
    pub const fn new(base_addr: PhyAddr, mapping: M, max_blk_size: usize) -> Self {
        let min_blk_size = max_blk_size >> (N - 1);
        assert!(min_blk_size == 4096);

        unsafe { Self::from_base_unchecked(base_addr, mapping, max_blk_size) }
    }

    const unsafe fn from_base_unchecked(
        base_addr: PhyAddr,
        mapping: M,
        max_blk_size: usize,
    ) -> Self {
        let min_blk_size = max_blk_size >> (N - 1);

        let mut init_free_list = MaybeUninit::uninit_array::<N>();
        let mut i = 0;
        loop {
            init_free_list[i].write(AtomicPtr::new(null_mut()));
            i += 1;
            if i == N {
                break;
            }
        }
        let mut free_lists = unsafe { MaybeUninit::array_assume_init(init_free_list) };

        free_lists[N - 1] = AtomicPtr::new(base_addr.const_mut_convert::<FreePageBlock>());

        let log2_min_blk_size = log2(min_blk_size);

        Self {
            base_addr,
            max_blk_size,
            min_blk_size,
            free_lists,
            mapping,
            log2_min_blk_size,
        }
    }

    unsafe fn alloc(&mut self, size: usize) -> *mut u8 {
        let level_req = self.allocation_level(size);
        let mut level = level_req;

        while (level as usize) < self.free_lists.len() {
            if let Some(blk) = self.pop_blk_with_level(level) {
                if level > level_req {
                    self.split_blk(blk, level, level_req);
                }
                return blk;
            }
            level += 1;
        }

        null_mut()
    }

    unsafe fn dealloc(&mut self, block: *mut u8, size: usize) {
        let alloc_level = self.allocation_level(size) as usize;

        let mut full_block = block;
        for level in alloc_level..self.free_lists.len() {
            if let Some(buddy) = self.buddy(full_block, level as u8) {
                if self.remove_blk(buddy, level as u8) {
                    full_block = min(buddy, full_block);
                    continue;
                }
            }

            self.free_blk(full_block, level as u8);
            return;
        }
    }

    fn allocation_level(&self, mut size: usize) -> u8 {
        assert!(size < self.max_blk_size);

        size = max(size, self.min_blk_size);
        size = size.next_power_of_two();

        (log2(size) - self.log2_min_blk_size) as u8
    }

    fn level_size(&self, level: u8) -> usize {
        self.min_blk_size << level
    }

    fn remove_blk(&mut self, block: *mut u8, level: u8) -> bool {
        let blk_header = block as *mut FreePageBlock;
        let virt_blk_header = self
            .mapping
            .convert(PhyAddr::from(blk_header))
            .as_mut_ptr::<FreePageBlock>();

        let mut curr_blk = &mut self.free_lists[level as usize];
        let mut curr_blk_ptr = curr_blk.load(Ordering::Acquire);

        while !curr_blk_ptr.is_null() {
            let mut virt_curr_blk = self
                .mapping
                .convert(PhyAddr::from(curr_blk_ptr))
                .as_mut_ptr::<FreePageBlock>();

            if curr_blk_ptr == blk_header {
                unsafe {
                    *curr_blk = AtomicPtr::new((*blk_header).next_blk.load(Ordering::Relaxed))
                }
                return true;
            }

            curr_blk = unsafe { &mut (*virt_curr_blk).next_blk };
        }

        false
    }

    unsafe fn pop_blk_with_level(&mut self, level: u8) -> Option<*mut u8> {
        let head = self.free_lists[level as usize].load(Ordering::Acquire);
        let virt_head = self
            .mapping
            .convert(PhyAddr::from(head))
            .as_mut_ptr::<FreePageBlock>();

        if !head.is_null() {
            if level as usize == N - 1 {
                self.free_lists[level as usize] = AtomicPtr::new(null_mut());
            } else {
                self.free_lists[level as usize] =
                    AtomicPtr::new((*virt_head).next_blk.load(Ordering::Relaxed));
            }

            return Some(head as *mut u8);
        }

        None
    }

    unsafe fn free_blk(&mut self, block: *mut u8, level: u8) {
        let blk_header = block as *mut FreePageBlock;
        let virt_blk_header = self
            .mapping
            .convert(PhyAddr::from(block))
            .as_mut_ptr::<FreePageBlock>();
        *virt_blk_header = FreePageBlock::new(AtomicPtr::new(
            self.free_lists[level as usize].load(Ordering::Relaxed),
        ));

        self.free_lists[level as usize] = AtomicPtr::new(blk_header);
    }

    unsafe fn split_blk(&mut self, block: *mut u8, mut level: u8, new_level: u8) {
        assert!(level >= new_level);

        while level > new_level {
            level -= 1;
            let buddy = self.buddy(block, level).unwrap();
            self.free_blk(buddy, level);
        }
    }

    fn buddy(&self, block: *mut u8, level: u8) -> Option<*mut u8> {
        assert!(
            block >= self.base_addr.as_mut_ptr(),
            "{:#x} : {:#x}",
            block as u64,
            self.base_addr.as_mut_ptr::<u8>() as u64
        );
        assert!(unsafe { block <= self.base_addr.as_mut_ptr::<u8>().add(self.max_blk_size) });

        if self.level_size(level) == self.max_blk_size {
            return None;
        }

        Some(((block as usize) ^ (1 << (level - 1 + self.log2_min_blk_size as u8))) as *mut u8)
    }
}

const fn log2(mut a: usize) -> usize {
    let mut power = 0;

    while a != 0 {
        a >>= 1;
        power += 1;
    }

    power
}
