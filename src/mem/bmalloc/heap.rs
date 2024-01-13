//! Main memory allocator
//!
//! It defines the [`BuddyAllocator`] which can then
//! be used with the `#[global_allocator]` attribute
//! to serve as a general purpose memory allocator
//! for the bootloader.
//!
//! It manages the heap both in real and protected
//! mode.

use core::{
    alloc::{GlobalAlloc, Layout},
    cmp,
    ptr::{self, NonNull},
};

const MIN_HEAP_ALIGN: usize = 8192;

/// Locked version of the [`BuddyAllocator`].
///
/// It uses a spinlock-based Mutex to ensure interior
/// mutability.
pub struct LockedBuddyAllocator<const N: usize> {
    pub alloc: spin::Mutex<BuddyAllocator<N>>,
}

impl<const N: usize> LockedBuddyAllocator<N> {
    pub const fn new(base_addr: NonNull<u8>, max_blk_size: usize) -> Self {
        let allocator = BuddyAllocator::new(base_addr, max_blk_size);
        Self {
            alloc: spin::Mutex::new(allocator),
        }
    }
}

unsafe impl<const N: usize> GlobalAlloc for LockedBuddyAllocator<N> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut allocator = self.alloc.lock();
        allocator.allocate(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut allocator = self.alloc.lock();
        allocator.deallocate(ptr, layout);
    }
}

/// Memory block header for blocks that have been freed.
/// Stored in the first bytes of the block.
struct FreeBlock {
    /// Pointer to the next free block of identical size,
    /// can be null ptr if it is the last block.
    /// No need to keep track of the block size because we
    /// use a different list for each size.
    next_blk: NullLock<*mut FreeBlock>,
}

impl FreeBlock {
    /// Returns a new `FreeBlock`
    pub fn new(next_blk: *mut FreeBlock) -> Self {
        Self {
            next_blk: NullLock::new(next_blk),
        }
    }
}

/// Default memory allocator for the bootloader.
///
/// It is based on the "buddy" memory allocation technique,
/// and thus uses blocks of fixed size for its allocations.
/// It offers a good balance between a decent speed and an
/// easy implementation, as well as low external fragmentation.
///
/// It can be used in real mode or later on, as long
/// as the underlying physical memory is valid and
/// remains consistent (no external writes).
///
/// The parameter `N` defines the number of block sizes
/// available for use.
pub struct BuddyAllocator<const N: usize> {
    /// Base memory address of the underlying physical
    /// memory that the allocator can use.
    base_addr: NullLock<*mut u8>,

    /// Size of the largest block we can allocate.
    /// Must be a multiple of 2.
    ///
    /// The size of the allocation pool is given by:
    /// total_size = base_addr + max_blk_size
    max_blk_size: usize,

    /// Size of the smallest block we can allocate.
    /// Must be a multiple of 2.
    min_blk_size: usize,

    /// log2 of the minimum block size to avoid
    /// redundant calculation.
    log2_min_blk_size: usize,

    /// Array of pointers to the `FreeBlock` that are
    /// the top of the linked lists keeping track of
    /// free blocks for each size
    free_lists: [NullLock<*mut FreeBlock>; N],
}

impl<const N: usize> BuddyAllocator<N> {
    /// Creates a new `BuddyAllocator` from a base
    /// physical address and a size.
    ///
    /// The number of blocks `N` and the heap size must
    /// be sufficient so that the smallest possible blocks
    /// can still contain the `FreeBlock` header.
    pub const fn new(base_addr: NonNull<u8>, max_blk_size: usize) -> Self {
        // We can compute the min block size using the
        // given max block size and levels count.
        let min_blk_size = max_blk_size >> (N - 1);

        // We need to make sure that the smallest possible
        // block can still contain our `FreeBlock` header
        assert!(min_blk_size >= core::mem::size_of::<FreeBlock>());

        // Heap has to be aligned on `MIN_HEAP_ALIGN` (4K)
        // boundaries. This ensures that `max_blk_size` is
        // a multiple of the alignment (and therefore a
        // multiple of 2 as well, which is also required).
        assert!(max_blk_size & (MIN_HEAP_ALIGN - 1) == 0);

        unsafe { Self::from_base_unchecked(base_addr, max_blk_size) }
    }

    /// Resizes or translates the heap.
    ///
    /// Can be used to dynamically set up the heap depending on available physical memory.
    pub fn resize(&mut self, base_addr: NonNull<u8>, max_blk_size: usize) {
        let min_blk_size = max_blk_size >> (N - 1);

        assert!(min_blk_size >= core::mem::size_of::<FreeBlock>());

        assert!(max_blk_size & (MIN_HEAP_ALIGN - 1) == 0);

        let base_addr_ptr = base_addr.as_ptr();
        let base_addr = NullLock::new(base_addr_ptr);

        self.base_addr = base_addr;
        self.max_blk_size = max_blk_size;
        self.min_blk_size = min_blk_size;

        // Initialize the linked lists with null head.
        let mut free_lists = [NullLock::new(ptr::null_mut()); N];

        // Except for the last one, which initially contains
        // the entire heap.
        free_lists[N - 1] = NullLock::new(base_addr_ptr as *mut FreeBlock);
        self.free_lists = free_lists;

        let log2_min_blk_size = log2(min_blk_size);
        self.log2_min_blk_size = log2_min_blk_size;
    }

    const unsafe fn from_base_unchecked(base_addr: NonNull<u8>, max_blk_size: usize) -> Self {
        let min_blk_size = max_blk_size >> (N - 1);
        let base_addr_ptr = base_addr.as_ptr();
        let base_addr = NullLock::new(base_addr_ptr);

        // Initialize the linked lists with null head.
        let mut free_lists = [NullLock::new(ptr::null_mut()); N];

        // Except for the last one, which initially contains
        // the entire heap.
        free_lists[N - 1] = NullLock::new(base_addr_ptr as *mut FreeBlock);

        let log2_min_blk_size = log2(min_blk_size);

        Self {
            base_addr,
            max_blk_size,
            min_blk_size,
            free_lists,
            log2_min_blk_size,
        }
    }

    unsafe fn allocate(&mut self, layout: Layout) -> *mut u8 {
        let level_req = self.allocation_level(layout.size(), layout.align());
        let mut level = level_req;

        // We iterate over the possible block sizes, until
        // we get an available block. If the block size is
        // bigger than the minimal size, we split it.
        while (level as usize) < self.free_lists.len() {
            if let Some(blk) = self.pop_blk_with_level(level) {
                if level > level_req {
                    self.split_blk(blk, level, level_req);
                }
                return blk;
            }
            level += 1;
        }

        // We have no available blocks
        ptr::null_mut()
    }

    unsafe fn deallocate(&mut self, block: *mut u8, layout: Layout) {
        let alloc_level = self.allocation_level(layout.size(), layout.align()) as usize;

        let mut full_block = block;
        // We merge our newly freed block with its buddy if
        // it's also free. We keep doing that for each level
        // until the buddy is in use.
        for level in alloc_level..self.free_lists.len() {
            if let Some(buddy) = self.buddy(block, level as u8) {
                if self.remove_blk(buddy, level as u8) {
                    full_block = cmp::min(buddy, block);
                    continue;
                }
            }

            // We cannot remove the buddy from the free
            // list, which means it is currently in used,
            // we can stop merging here.
            self.free_blk(full_block, level as u8);
            return;
        }
    }

    /// Returns the minimum level for the allocation.
    /// It looks for the smallest block size that can fit
    /// the [`Layout`] of the required allocation.
    ///
    /// It expects a valid alignment (that is a power of 2
    /// and less precise than the `MIN_HEAP_ALIGN`)
    pub fn allocation_level(&self, mut size: usize, align: usize) -> u8 {
        // We cannot allocate a block larger than our
        // heap.
        assert!(size < self.max_blk_size);

        // Make sure the alignment is somewhat standard
        assert!(align.is_power_of_two());

        // We cannot align more precisely than the initial
        // heap alignment.
        assert!(align < MIN_HEAP_ALIGN);

        // If size < align, we still have to allocate
        // at least `align` bytes.
        size = cmp::max(size, align);

        // Make sure we allocate at least the minimum
        // block size, and round the final allocation size
        // to the next power of two to match a block size.
        size = cmp::max(size, self.min_blk_size);
        size = size.next_power_of_two();

        (log2(size) - self.log2_min_blk_size) as u8
    }

    /// Returns the block size in bytes corresponding
    /// to a given level.
    pub fn level_size(&self, level: u8) -> usize {
        self.min_blk_size << level
    }

    /// Remove a given `FreeBlock` from free lists.
    ///
    /// Returns `false` if the operation was unsuccessful,
    /// it usually means that the `FreeBlock is in use`.
    pub fn remove_blk(&mut self, block: *mut u8, level: u8) -> bool {
        let blk_header = block as *mut FreeBlock;

        // We find the predecessor of the block to be
        // removed in the linked list of the corresponding
        // level.
        let mut curr_blk = &mut self.free_lists[level as usize].inner;

        while !(*curr_blk).is_null() {
            if *curr_blk == blk_header {
                unsafe { *curr_blk = (*blk_header).next_blk.inner }
                return true;
            }

            curr_blk = unsafe { &mut (*(*curr_blk)).next_blk.inner };
        }

        // The block did not appear in the list
        false
    }

    /// Pop a [`FreeBlock`] of a given level from the
    /// corresponding free list.
    ///
    /// Returns `None` if there is no available block for
    /// that size.
    pub unsafe fn pop_blk_with_level(&mut self, level: u8) -> Option<*mut u8> {
        let head = self.free_lists[level as usize];

        // We have an available block
        if !head.inner.is_null() {
            // If the requested level corresponds to the entire
            // heap, `next_blk` might be uninitialized data,
            // this takes care of that special case.
            if level as usize == N - 1 {
                self.free_lists[level as usize] = NullLock::new(ptr::null_mut());
            } else {
                self.free_lists[level as usize] = (*head.inner).next_blk;
            }

            return Some(head.inner as *mut u8);
        }

        None
    }

    /// Free a memory block of a given level.
    ///
    /// It adds the [`FreeBlock`] header and adds it to
    /// the free list.
    pub unsafe fn free_blk(&mut self, block: *mut u8, level: u8) {
        // Initialize / update the header of the block
        let blk_header = block as *mut FreeBlock;
        *blk_header = FreeBlock::new(self.free_lists[level as usize].inner);

        // Update the head of the list of level k
        self.free_lists[level as usize] = NullLock::new(blk_header);
    }

    /// Split a [`FreeBlock`] until it reaches `new_level`
    ///
    /// It finds the corresponding buddy at each level and
    /// mark it as free.
    pub unsafe fn split_blk(&mut self, block: *mut u8, mut level: u8, new_level: u8) {
        // We can't make a block larger by splitting it...
        assert!(level >= new_level);

        // To split the initial block, of level k, we view it
        // as a block of level k - 1, and mark his buddy as free.
        // The buddy was the second half of the level k block, so
        // we effectively split the initial block.
        while level > new_level {
            level -= 1;
            let buddy = self.buddy(block, level).unwrap();
            self.free_blk(buddy, level);
        }
    }

    /// Find the buddy of a given block and level.
    ///
    /// If the block has a k level, it flips the
    /// (k + `log2_min_blk_size`)-bit in the binary
    /// representation of the memory address of the
    /// block.
    ///
    /// If the minimum block size is 2 = 2^1, and
    /// block A has a level of 1:
    ///
    /// Block A: 1000
    /// Buddy: 1100
    pub fn buddy(&self, block: *mut u8, level: u8) -> Option<*mut u8> {
        // Make sure the block is in our bounds.
        assert!(
            block >= self.base_addr.inner,
            "{:#x} : {:#x}",
            block as u64,
            self.base_addr.inner as u64
        );
        assert!(unsafe { block <= self.base_addr.inner.add(self.max_blk_size) });

        // The entire heap does not have a buddy
        if self.level_size(level) == self.max_blk_size {
            return None;
        }

        // To find the address, given a block, of its matching
        // buddy, we want to flip the bit corresponding to
        // the level of the block.
        Some(((block as usize) ^ (1 << (level - 1 + self.log2_min_blk_size as u8))) as *mut u8)
    }
}

/// `NullLock` is used to encapsulate types that
/// are not [`Send`] or [`Sync`].
/// It does nothing but simulate the implementation
/// of `Send` and `Sync`
#[derive(Clone, Copy)]
struct NullLock<T: Clone + Copy> {
    inner: T,
}

impl<T: Copy> NullLock<T> {
    pub const fn new(obj: T) -> Self {
        Self { inner: obj }
    }
}

unsafe impl<T: Copy> Sync for NullLock<T> {}
unsafe impl<T: Copy> Send for NullLock<T> {}

const fn log2(mut a: usize) -> usize {
    let mut power = 0;

    while a != 0 {
        a >>= 1;
        power += 1;
    }

    power
}
