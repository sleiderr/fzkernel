//! Red-black tree implementation.
//!
//! Used for the `vmalloc` kernel memory allocator to manage the heap's virtual address space.

use core::ptr::{self, null_mut};

use crate::mem::{MemoryAddress, VirtAddr};

/// A red-black tree implementation.
///
/// The payload contained in the nodes can be set for each tree as a generic parameter.
pub(crate) struct RbTree<P: NodePayload> {
    /// Link to the root node of the tree.
    pub(super) root: NodeLink<P>,

    /// Link to the leaf node, considered black by default.
    pub(super) black_nil: NodeLink<P>,

    /// Number of nodes currently in the tree.
    pub(super) count: usize,
}

/// Available colors for the [`RbTree`] nodes.
pub(super) enum NodeColor {
    Black,
    Red,
}

/// Wrapper around the pointers in between nodes, for the direct children / parent of a node.
pub(super) struct NodeLink<P: NodePayload> {
    linked_node: *mut Node<P>,
}

impl<P: NodePayload> NodeLink<P> {
    /// Null pointer, used when a node does not have a left or right child.
    pub(super) const NULL_LINK: Self = Self {
        linked_node: null_mut(),
    };

    /// Turns a raw pointer to a node in tree into a `NodeLink` wrapper.
    pub(super) fn link_from_raw_ptr(raw_ptr: *mut Node<P>) -> Self {
        Self {
            linked_node: raw_ptr,
        }
    }

    /// Returns a reference to the [`Node`] to which this `NodeLink` links to.
    ///
    /// # Safety
    ///
    /// This is unsafe as it dereferences the raw pointer contained in this link. You should make sure to
    /// respect the usual borrow checking rules, even if they can be avoided here.
    pub(super) unsafe fn get_node(&self) -> &Node<P> {
        &*self.linked_node
    }

    /// Returns a mutable reference to the [`Node`] to which this `NodeLink` links to.
    ///
    /// # Safety
    ///
    /// This is unsafe as it dereferences the raw pointer contained in this link. You should make sure to
    /// respect the usual borrow checking rules, even if they can be avoided here.
    pub(super) unsafe fn get_node_mut(&self) -> &mut Node<P> {
        &mut *self.linked_node
    }
}

/// Trait that must be implemented in order for a structure to be used as a payload associated to the node in a
/// [`RbTree`].
///
/// Node color information have to be contained in the payload, so the structure must implement methods to retrieve
/// and update the color of the node.
pub(super) trait NodePayload {
    /// Empty payload, used as a default when creating new nodes.
    const NULL: Self;

    /// Returns the color of the [`Node`] associated to this payload.
    fn get_color(&self) -> NodeColor;

    /// Updates the color of the [`Node`] associated to this payload.
    fn set_color(&mut self, color: NodeColor);
}

/// Represents a Node in a [`RbTree`].
///
/// The node can contain any type of payload, but all nodes in a given tree have to use the same type of payload. The actual
/// form of the payload is the structure given as a generic parameter.
pub(super) struct Node<P: NodePayload> {
    /// Payload associated with the node.
    pub(super) header: P,

    /// Link to the parent of this node.
    pub(super) parent: NodeLink<P>,

    /// Link to the left child of this node.
    pub(super) left: NodeLink<P>,

    /// Link to the right child of this node.
    pub(super) right: NodeLink<P>,
}

impl<P: NodePayload> RbTree<P> {
    /// Creates a new `RbTree` (_Red-black Tree_), and creates the tree's root node at the given memory address (`root_addr`).
    ///
    /// Also creates the sentinel node used as leaf node for this tree at the `black_nil_addr`.
    pub(super) unsafe fn new_raw(mut root_addr: VirtAddr, mut black_nil_addr: VirtAddr) -> Self {
        ptr::write(
            root_addr.as_mut_ptr(),
            Node {
                header: P::NULL,
                parent: NodeLink::link_from_raw_ptr(black_nil_addr.as_mut_ptr()),
                left: NodeLink::link_from_raw_ptr(black_nil_addr.as_mut_ptr()),
                right: NodeLink::link_from_raw_ptr(black_nil_addr.as_mut_ptr()),
            },
        );

        ptr::write(
            black_nil_addr.as_mut_ptr(),
            Node {
                header: P::NULL,
                parent: NodeLink::NULL_LINK,
                left: NodeLink::NULL_LINK,
                right: NodeLink::NULL_LINK,
            },
        );

        Self {
            root: NodeLink::link_from_raw_ptr(root_addr.as_mut_ptr()),
            black_nil: NodeLink::link_from_raw_ptr(black_nil_addr.as_mut_ptr()),
            count: 1,
        }
    }
}
