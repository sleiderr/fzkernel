use core::mem::size_of;

use crate::mem::{vmalloc::rbtree::Node, Alignment, VirtAddr};

use super::rbtree::{NodeColor, NodePayload, RbTree};

const MIN_HEAP_SIZE: usize = 0x1_000_000;

pub struct KernelHeapAllocator {
    start: VirtAddr,
    end: VirtAddr,
    size: usize,
    alloc_tree: RbTree<AllocHeader>,
}

#[repr(transparent)]
pub(crate) struct AllocHeader {
    inner: u64,
}

impl AllocHeader {
    pub(crate) fn allocate(&mut self) {
        self.inner |= 0x1;
    }

    pub(crate) fn free(&mut self) {
        self.inner &= !0x1;
    }
}

impl NodePayload for AllocHeader {
    const NULL: Self = Self { inner: 0 };

    fn get_color(&self) -> super::rbtree::NodeColor {
        if (self.inner >> 2) & 0x1 == 0 {
            NodeColor::Black
        } else {
            NodeColor::Red
        }
    }

    fn set_color(&mut self, color: super::rbtree::NodeColor) {
        match color {
            NodeColor::Black => {
                self.inner &= !0xb100;
            }
            NodeColor::Red => {
                self.inner |= 0b100;
            }
        }
    }
}

impl KernelHeapAllocator {
    pub(crate) unsafe fn init(heap_start: VirtAddr, heap_size: usize) -> Self {
        assert!(
            heap_start.is_aligned_with(Alignment::ALIGN_4KB),
            "invalid alignment for the kernel heap"
        );

        assert!(
            heap_size > MIN_HEAP_SIZE,
            "not enough memory for the kernel heap"
        );

        let heap_end = heap_start + heap_size - size_of::<Node<AllocHeader>>();

        let mut alloc_tree: RbTree<AllocHeader> = RbTree::new_raw(heap_start, heap_end);

        alloc_tree.black_nil.get_node_mut().header.allocate();

        Self {
            start: heap_start,
            end: heap_end,
            size: heap_size,
            alloc_tree,
        }
    }
}
