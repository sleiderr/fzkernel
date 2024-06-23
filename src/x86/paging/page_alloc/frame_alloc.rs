//! Main physical memory allocator.
//!
//! Allocates contiguous areas of physical memory, using a buddy memory allocator. Only uses a single
//! buddy allocator, and therefore does not differentiate between different areas / types of physical
//! memory. It may be required to implement another buddy allocator to manage DMA-compatible memory
//! for instance.
//!
//! Frames allocated using these allocators are not mapped to any virtual address space, thus paging requires
//! a page mapper to map frames in a virtual address space.

use crate::mem::{MemoryAddress, PhyAddr};
use crate::x86::paging::page_table::mapper::MemoryMapping;
use core::cmp::{max, min};
use core::ptr::null_mut;

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
    next_blk: *mut FreePageBlock,
}

impl FreePageBlock {
    fn new(next_blk: *mut FreePageBlock) -> Self {
        Self { next_blk: next_blk }
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

    free_lists: [*mut FreePageBlock; N],
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
        unsafe { Self::from_base_unchecked(base_addr, mapping, max_blk_size) }
    }

    const unsafe fn from_base_unchecked(
        base_addr: PhyAddr,
        mapping: M,
        max_blk_size: usize,
    ) -> Self {
        let min_blk_size = max_blk_size >> (N - 1);

        let mut free_lists = [null_mut(); N];

        free_lists[N - 1] = base_addr.const_mut_convert::<FreePageBlock>();

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
            if let Some(buddy) = self.buddy(block, level as u8) {
                if self.remove_blk(buddy, level as u8) {
                    full_block = min(buddy, block);
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

        let mut curr_blk = self.free_lists[level as usize];

        while !curr_blk.is_null() {
            let mut virt_curr_blk = &mut self
                .mapping
                .convert(PhyAddr::from(curr_blk))
                .as_mut_ptr::<FreePageBlock>();

            if curr_blk == blk_header {
                unsafe { *virt_curr_blk = (*blk_header).next_blk }
                return true;
            }

            curr_blk = unsafe { (*(*virt_curr_blk)).next_blk };
        }

        false
    }

    unsafe fn pop_blk_with_level(&mut self, level: u8) -> Option<*mut u8> {
        let head = self.free_lists[level as usize];
        let virt_head = self
            .mapping
            .convert(PhyAddr::from(head))
            .as_mut_ptr::<FreePageBlock>();

        if !head.is_null() {
            if level as usize == N - 1 {
                self.free_lists[level as usize] = null_mut();
            } else {
                self.free_lists[level as usize] = (*virt_head).next_blk;
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
        *virt_blk_header = FreePageBlock::new(self.free_lists[level as usize]);

        self.free_lists[level as usize] = blk_header;
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
