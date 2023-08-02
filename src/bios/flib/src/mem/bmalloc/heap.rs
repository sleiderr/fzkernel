use core::{
    alloc::{GlobalAlloc, Layout},
    cmp, mem,
    ptr::{self, NonNull},
};

const MIN_HEAP_ALIGN: usize = 4096;

pub struct LockedBuddyAllocator<const N: usize> {
    alloc: spin::Mutex<BuddyAllocator<N>>,
}

impl<const N: usize> LockedBuddyAllocator<N> {
    pub fn new(base_addr: NonNull<u8>, max_blk_size: usize) -> Self {
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
/// Stored at the start of the block.
struct FreeBlock {
    /// Pointer to the next free block of identical size,
    /// can be null ptr if it is the last block. No need
    /// to keep track of the block size because we use
    /// a different list for each size.
    next_blk: NullLock<*mut FreeBlock>,
}

impl FreeBlock {
    /// Return a new `FreeBlock`
    pub fn new(next_blk: *mut FreeBlock) -> Self {
        Self {
            next_blk: NullLock::new(next_blk),
        }
    }
}

pub(crate) struct BuddyAllocator<const N: usize> {
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
    pub fn new(base_addr: NonNull<u8>, max_blk_size: usize) -> Self {
        // We can compute the min block size using the
        // given max block size and levels count.
        let min_blk_size = max_blk_size >> (N - 1);

        // We need to make sure that the smallest possible
        // block can still contain our `FreeBlock` header
        assert!(min_blk_size >= mem::size_of::<FreeBlock>());

        // Heap has to be aligned on `MIN_HEAP_ALIGN` (4K)
        // boundaries. This also ensures that `max_blk_size`
        // is a multiple of the alignement (and therefore a
        // multiple of 2 as well, which is also required).
        assert_eq!(base_addr.as_ptr() as usize & (MIN_HEAP_ALIGN - 1), 0);
        assert_eq!(max_blk_size & (MIN_HEAP_ALIGN - 1), 0);

        unsafe { Self::from_base_unchecked(base_addr, max_blk_size) }
    }

    pub unsafe fn from_base_unchecked(base_addr: NonNull<u8>, max_blk_size: usize) -> Self {
        let min_blk_size = max_blk_size >> (N - 1);
        let base_addr_ptr = base_addr.as_ptr();

        // Initialize the linked lists with null head.
        let mut free_lists = [NullLock::new(ptr::null_mut() as *mut FreeBlock); N];

        // Except for the last one, which initially contains
        // the entire heap.
        free_lists[N - 1] = NullLock::new(base_addr_ptr as *mut FreeBlock);

        let log2_min_blk_size = log2(min_blk_size);

        Self {
            base_addr: NullLock::new(base_addr_ptr),
            max_blk_size,
            min_blk_size,
            free_lists,
            log2_min_blk_size,
        }
    }

    unsafe fn allocate(&mut self, layout: Layout) -> *mut u8 {
        let level_req = self.allocation_level(layout.size(), layout.align());
        let mut level = level_req;

        while (level as usize) < self.free_lists.len() {
            if let Some(blk) = self.pop_blk_with_level(level) {
                if level > level_req {
                    self.split_blk(blk, level, level_req);
                }
                return blk;
            }
        }

        ptr::null_mut()
    }

    unsafe fn deallocate(&mut self, block: *mut u8, layout: Layout) {
        let alloc_level = self.allocation_level(layout.size(), layout.align()) as usize;

        let mut full_block = block;
        for level in alloc_level..self.free_lists.len() {
            if let Some(buddy) = self.buddy(block, level as u8) {
                if self.remove_blk(buddy, level as u8) {
                    full_block = cmp::min(buddy, block);
                    continue;
                }

                self.free_blk(full_block, level as u8);
                return;
            }
        }
    }

    pub fn allocation_level(&self, mut size: usize, align: usize) -> u8 {
        // We cannot allocate a block larger than our
        // heap.
        assert!(size < self.max_blk_size);

        // Make sure the alignement is somewhat standard
        assert!(align.is_power_of_two());

        // We cannot align more precisely than the initial
        // heap alignement.
        assert!(align < MIN_HEAP_ALIGN);

        size = cmp::max(size, align);

        // Make sure we allocate at least the minimum
        // block size, and round the final allocation size
        // to the next power of two to match a block size.
        size = cmp::max(size, self.min_blk_size);
        size.next_power_of_two();

        (log2(size) - self.log2_min_blk_size) as u8
    }

    pub fn level_size(&self, level: u8) -> usize {
        self.min_blk_size << level
    }

    pub fn remove_blk(&mut self, block: *mut u8, level: u8) -> bool {
        let blk_header = block as *mut FreeBlock;

        let mut curr_blk = &mut self.free_lists[level as usize].inner;

        while !(*curr_blk).is_null() {
            if *(curr_blk) == blk_header {
                unsafe { (*(*curr_blk)).next_blk = (*blk_header).next_blk }
                return true;
            }

            curr_blk = unsafe { &mut (*(*curr_blk)).next_blk.inner };
        }
        false
    }

    pub unsafe fn pop_blk_with_level(&mut self, level: u8) -> Option<*mut u8> {
        let head = self.free_lists[level as usize];

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

    pub unsafe fn free_blk(&mut self, block: *mut u8, level: u8) {
        // Initialize / update the header of the block
        let blk_header = block as *mut FreeBlock;
        *blk_header = FreeBlock::new(self.free_lists[level as usize].inner);

        // Update the head of the list of level k
        self.free_lists[level as usize] = NullLock::new(blk_header);
    }

    pub unsafe fn split_blk(&mut self, block: *mut u8, mut level: u8, new_level: u8) {
        // We can't make a block larger by splitting it...
        assert!(level >= new_level);

        // To split the initial block, of level k, we view it
        // as a block of level k - 1, and mark his buddy as free.
        // The buddy was the second half of the level k block, so
        // we effectively splitted the initial block.
        while level > new_level {
            level -= 1;
            let buddy = self.buddy(block, level).unwrap();
            self.free_blk(buddy, level);
        }
    }

    pub fn buddy(&self, block: *mut u8, level: u8) -> Option<*mut u8> {
        // Make sure the block is in our bounds.
        assert!(block >= self.base_addr.inner);
        assert!(unsafe { block <= self.base_addr.inner.add(self.max_blk_size) });

        if self.level_size(level) == self.max_blk_size {
            return None;
        }

        // To find the address, given a block, of its matching
        // buddy, we want to flip the bit corresponding to
        // the level of the block.
        Some(((block as usize) ^ (1 << level)) as *mut u8)
    }
}

#[derive(Clone, Copy)]
struct NullLock<T: Clone + Copy> {
    inner: T,
}

impl<T: Copy> NullLock<T> {
    pub fn new(obj: T) -> Self {
        Self { inner: obj }
    }
}

unsafe impl<T: Copy> Sync for NullLock<T> {}
unsafe impl<T: Copy> Send for NullLock<T> {}

fn log2(mut a: usize) -> usize {
    let mut power = 0;

    while a != 0 {
        a >>= 1;
        power += 1;
    }

    power
}
