use core::{
    alloc::Layout,
    mem::size_of,
    ops::{Add, Sub},
};

use crate::{
    kernel_syms::PAGE_SIZE,
    mem::{vmalloc::rbtree::Node, Alignment, MemoryAddress, VirtAddr},
    x86::paging::page_alloc::frame_alloc::alloc_page,
};

use super::rbtree::{NodeColor, NodeLink, NodePayload, RbTree};

const MIN_HEAP_SIZE: usize = 0x1_000;

pub struct KernelHeapAllocator {
    start: VirtAddr,
    end: VirtAddr,
    size: usize,
    mapped_alloc_tree: RbTree<AllocHeader>,
    unmapped_alloc_tree: RbTree<AllocHeader>,
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
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

    pub(crate) fn is_allocated(&self) -> bool {
        self.inner & 0x1 != 0
    }

    pub(crate) fn get_size(&self) -> u64 {
        self.inner & !0b111
    }

    pub(crate) fn set_size(&mut self, size: u64) {
        let header_attr = self.inner & 0b111;
        self.inner = size | header_attr;
    }

    pub(crate) fn left_allocated(&self) -> bool {
        self.inner & 0b10 != 0
    }

    pub(crate) fn set_left_allocated(&mut self, allocated: bool) {
        if allocated {
            self.inner |= 0b10;
        } else {
            self.inner &= !0b10;
        }
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
                self.inner &= !0b100;
            }
            NodeColor::Red => {
                self.inner |= 0b100;
            }
        }
    }

    fn value(&self) -> u64 {
        self.get_size()
    }

    fn set_value(&mut self, new_val: u64) {
        self.set_size(new_val);
    }
}

#[derive(Debug)]
struct BlockMergeResult {
    current: NodeLink<AllocHeader>,
    left: NodeLink<AllocHeader>,
    right: NodeLink<AllocHeader>,
    new_size: u64,
}

impl KernelHeapAllocator {
    const MIN_KHEAP_ALIGN: usize = 0b1000;
    const MIN_BLOCK_SIZE: u64 = 0x40;

    pub(crate) unsafe fn init(heap_start: VirtAddr, heap_size: usize) -> Self {
        assert!(
            heap_start.is_aligned_with(Alignment::ALIGN_4KB),
            "invalid alignment for the kernel heap"
        );

        assert!(
            heap_size > MIN_HEAP_SIZE,
            "not enough memory for the kernel heap"
        );

        let heap_end = heap_start + heap_size - 2 * size_of::<Node<AllocHeader>>();

        let mut unmapped_alloc_tree: RbTree<AllocHeader> =
            RbTree::new_raw(heap_start + size_of::<Node<AllocHeader>>(), heap_end);

        let mut mapped_alloc_tree: RbTree<AllocHeader> =
            RbTree::new_raw(heap_start, heap_end + size_of::<Node<AllocHeader>>());

        unmapped_alloc_tree
            .black_nil
            .get_node_mut()
            .header
            .allocate();

        let mut heap = Self {
            start: heap_start,
            end: heap_end,
            size: heap_size,
            mapped_alloc_tree,
            unmapped_alloc_tree,
        };

        heap.init_node_header(
            heap.unmapped_alloc_tree.root,
            u64::try_from(heap_size - (size_of::<Node<AllocHeader>>() + size_of::<AllocHeader>()))
                .expect("infallible conversion"),
        );

        heap.init_node_header(heap.mapped_alloc_tree.root, 0);

        heap.init_node_end(
            heap.unmapped_alloc_tree.root,
            u64::try_from(heap_size - (size_of::<Node<AllocHeader>>() + size_of::<AllocHeader>()))
                .expect("infallible conversion"),
        );

        heap.init_node_end(heap.mapped_alloc_tree.root, 0);

        heap
    }

    pub(crate) unsafe fn kalloc_layout(&mut self, alloc_layout: Layout) -> VirtAddr {
        let (alloc_size, alloc_align) = (alloc_layout.size(), alloc_layout.align());

        if alloc_size == 0 {
            return VirtAddr::NULL_PTR;
        }

        let alloc_size64 = u64::try_from(alloc_size).expect("infallible conversion");

        match self.mapped_alloc_tree.find_best_node_fit(alloc_size64) {
            Some(node) => self.split_alloc(node, alloc_size),
            None => {
                let page_aligned_alloc_size = if alloc_size % PAGE_SIZE != 0 {
                    alloc_size / PAGE_SIZE + 1
                } else {
                    alloc_size / PAGE_SIZE
                };

                let page_aligned_alloc_size64 =
                    u64::try_from(page_aligned_alloc_size).expect("infallible conversion");

                match self
                    .unmapped_alloc_tree
                    .find_best_node_fit(page_aligned_alloc_size64)
                {
                    Some(node) => self.split_alloc_and_map(node, alloc_size),
                    None => return VirtAddr::NULL_PTR,
                }
            }
        }
    }

    pub(crate) unsafe fn kfree(&mut self, block: VirtAddr) {
        if block == VirtAddr::NULL_PTR {
            return;
        }

        let mut merge_result = self.merge_scan_neighbors(self.get_node_from_block_addr(block));

        self.merge(&mut merge_result, true);
        self.init_free_node(
            merge_result.current,
            merge_result.current.get_node().header.get_size(),
            true,
        );
    }

    unsafe fn split_alloc_and_map(
        &mut self,
        free_block: NodeLink<AllocHeader>,
        size_req: usize,
    ) -> VirtAddr {
        let block_size = free_block.get_node().header.get_size();
        let size_req_64 = u64::try_from(size_req).expect("infallible conversion");

        let page_aligned_alloc_size = if size_req % PAGE_SIZE != 0 {
            size_req / PAGE_SIZE + 1
        } else {
            size_req / PAGE_SIZE
        } * PAGE_SIZE;

        let pages = match alloc_page(page_aligned_alloc_size) {
            Ok(ptr) => ptr,
            Err(_) => return VirtAddr::NULL_PTR,
        };

        // TODO: map physical pages to memory block

        if block_size >= size_req_64 + Self::MIN_BLOCK_SIZE {
            self.init_free_node(
                self.get_block_right_neighbor(free_block, size_req_64),
                block_size
                    - size_req_64
                    - u64::try_from(size_of::<AllocHeader>()).expect("infallible conversion"),
                false,
            );
            self.init_node_header(free_block, size_req_64);
            free_block.get_node_mut().header.allocate();

            return self.get_block_start_addr(free_block);
        }

        let right_neigh = self.get_block_right_neighbor(free_block, block_size);

        right_neigh.get_node_mut().header.set_left_allocated(true);
        self.init_node_header(free_block, block_size);
        free_block.get_node_mut().header.allocate();

        self.get_block_start_addr(free_block)
    }

    unsafe fn split_alloc(
        &mut self,
        free_block: NodeLink<AllocHeader>,
        size_req: usize,
    ) -> VirtAddr {
        let block_size = free_block.get_node().header.get_size();
        let size_req_64 = u64::try_from(size_req).expect("infallible conversion");

        if block_size >= size_req_64 + Self::MIN_BLOCK_SIZE {
            self.init_free_node(
                self.get_block_right_neighbor(free_block, size_req_64),
                block_size
                    - size_req_64
                    - u64::try_from(size_of::<AllocHeader>()).expect("infallible conversion"),
                true,
            );
            self.init_node_header(free_block, size_req_64);
            free_block.get_node_mut().header.allocate();

            return self.get_block_start_addr(free_block);
        }

        let right_neigh = self.get_block_right_neighbor(free_block, block_size);

        right_neigh.get_node_mut().header.set_left_allocated(true);
        self.init_node_header(free_block, block_size);
        free_block.get_node_mut().header.allocate();

        self.get_block_start_addr(free_block)
    }

    fn merge_scan_neighbors(&self, node: NodeLink<AllocHeader>) -> BlockMergeResult {
        let start_size = node.get_node().header.get_size();
        let mut merge_result = BlockMergeResult {
            current: node,
            left: NodeLink::NULL_LINK,
            right: NodeLink::NULL_LINK,
            new_size: start_size,
        };

        let right_node = unsafe { self.get_block_right_neighbor(node, start_size) };

        if !right_node.get_node().header.is_allocated() {
            merge_result.new_size += right_node.get_node().header.get_size()
                + u64::try_from(size_of::<AllocHeader>()).expect("infallible conversion");

            merge_result.right = right_node;
        }

        if node.addr() != self.start && !node.get_node().header.left_allocated() {
            merge_result.left = unsafe { self.get_block_left_neighbor(node) };
            merge_result.new_size += merge_result.left.get_node().header.get_size()
                + u64::try_from(size_of::<AllocHeader>()).expect("infallible conversion");
        }

        merge_result
    }

    #[inline(always)]
    unsafe fn merge(&mut self, scan_result: &mut BlockMergeResult, mapped: bool) {
        let tree = if mapped {
            &mut self.mapped_alloc_tree
        } else {
            &mut self.unmapped_alloc_tree
        };

        if scan_result.left != NodeLink::NULL_LINK {
            scan_result.current = tree.remove_node(scan_result.left);
        }
        if scan_result.right != NodeLink::NULL_LINK {
            scan_result.right = tree.remove_node(scan_result.right);
        }

        self.init_node_header(scan_result.current, scan_result.new_size);
    }

    unsafe fn init_free_node(&mut self, node: NodeLink<AllocHeader>, size: u64, mapped: bool) {
        node.get_node_mut().header.set_size(size);
        node.get_node_mut().header.set_color(NodeColor::Red);
        node.get_node_mut().header.set_left_allocated(true);

        self.init_node_end(node, size);
        self.get_block_right_neighbor(node, size)
            .get_node_mut()
            .header
            .set_left_allocated(false);

        if mapped {
            self.mapped_alloc_tree.insert_node(node);
        } else {
            self.unmapped_alloc_tree.insert_node(node);
        }
    }

    unsafe fn init_node_end(&self, node: NodeLink<AllocHeader>, size: u64) {
        let footer_ptr: *mut AllocHeader = node.addr().add(size).as_mut_ptr();
        *footer_ptr = node.get_node().header;
    }

    unsafe fn init_node_header(&self, node: NodeLink<AllocHeader>, size: u64) {
        node.get_node_mut().header.set_size(size);
        node.get_node_mut().header.set_left_allocated(true);
    }

    unsafe fn get_block_right_neighbor(
        &self,
        node: NodeLink<AllocHeader>,
        size: u64,
    ) -> NodeLink<AllocHeader> {
        let right_ptr = node.addr().add(size).add(size_of::<AllocHeader>());

        NodeLink::link_from_raw_ptr(right_ptr.as_mut_ptr())
    }

    unsafe fn get_block_left_neighbor(&self, node: NodeLink<AllocHeader>) -> NodeLink<AllocHeader> {
        let left_footer_ptr: *const AllocHeader =
            node.addr().sub(size_of::<AllocHeader>()).as_ptr();
        let left_footer = &*left_footer_ptr;

        let left_node_ptr = node
            .addr()
            .sub(left_footer.get_size())
            .sub(size_of::<AllocHeader>());

        NodeLink::link_from_raw_ptr(left_node_ptr.as_mut_ptr())
    }

    unsafe fn get_block_start_addr(&self, node: NodeLink<AllocHeader>) -> VirtAddr {
        node.addr().add(size_of::<AllocHeader>())
    }

    unsafe fn get_node_from_block_addr(&self, block: VirtAddr) -> NodeLink<AllocHeader> {
        NodeLink::link_from_raw_ptr(block.sub(size_of::<AllocHeader>()).as_mut_ptr())
    }
}
