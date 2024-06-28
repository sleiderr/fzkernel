//! Red-black tree implementation.
//!
//! Used for the `vmalloc` kernel memory allocator to manage the heap's virtual address space.

use core::ptr::{self, null_mut};

use crate::{
    mem::{MemoryAddress, VirtAddr},
    video::io::color,
};

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
#[derive(Clone, Copy, Debug)]
pub(super) enum NodeColor {
    Black,
    Red,
}

/// Wrapper around the pointers in between nodes, for the direct children / parent of a node.
#[derive(Clone, Copy, Debug)]
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
    pub(super) fn get_node(&self) -> &Node<P> {
        unsafe { &*self.linked_node }
    }

    /// Returns a mutable reference to the [`Node`] to which this `NodeLink` links to.
    ///
    /// # Safety
    ///
    /// This is unsafe as it dereferences the raw pointer contained in this link. You should make sure to
    /// respect the usual borrow checking rules, even if they can be avoided here.
    pub(super) fn get_node_mut(&self) -> &mut Node<P> {
        unsafe { &mut *self.linked_node }
    }
}

impl<P: NodePayload> PartialEq for NodeLink<P> {
    fn eq(&self, other: &Self) -> bool {
        self.linked_node == other.linked_node
    }
}

/// Trait that must be implemented in order for a structure to be used as a payload associated to the node in a
/// [`RbTree`].
///
/// Node color information have to be contained in the payload, so the structure must implement methods to retrieve
/// and update the color of the node.
pub(super) trait NodePayload: Clone + Copy {
    /// Empty payload, used as a default when creating new nodes.
    const NULL: Self;

    /// Returns the color of the [`Node`] associated to this payload.
    fn get_color(&self) -> NodeColor;

    /// Updates the color of the [`Node`] associated to this payload.
    fn set_color(&mut self, color: NodeColor);

    fn value(&self) -> u64;

    fn set_value(&mut self, new_val: u64);
}

/// Represents a Node in a [`RbTree`].
///
/// The node can contain any type of payload, but all nodes in a given tree have to use the same type of payload. The actual
/// form of the payload is the structure given as a generic parameter.
#[derive(Debug)]
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

impl<P: NodePayload> Node<P> {
    pub(super) fn new_empty() -> Self {
        Self {
            header: P::NULL,
            parent: NodeLink::NULL_LINK,
            left: NodeLink::NULL_LINK,
            right: NodeLink::NULL_LINK,
        }
    }

    pub(super) fn new_isolated_with_value(value: u64) -> Self {
        let mut header = P::NULL;

        header.set_value(value);

        Self {
            header,
            parent: NodeLink::NULL_LINK,
            left: NodeLink::NULL_LINK,
            right: NodeLink::NULL_LINK,
        }
    }
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

    pub(super) fn insert_node(&mut self, new_node: NodeLink<P>) {
        let mut child = self.root;
        let mut parent = self.black_nil;
        let mut curr_size = new_node.get_node().header.value();

        while child != self.black_nil {
            parent = child;
            let child_size = child.get_node().header.value();

            if curr_size < child_size {
                child = child.get_node().left;
            } else {
                child = child.get_node().right;
            }
        }

        new_node.get_node_mut().parent = parent;

        if parent == self.black_nil {
            self.root = new_node;
        } else if curr_size < parent.get_node().header.value() {
            parent.get_node_mut().left = new_node;
        } else {
            parent.get_node_mut().right = new_node;
        }

        new_node.get_node_mut().left = self.black_nil;
        new_node.get_node_mut().right = self.black_nil;
        new_node.get_node_mut().header.set_color(NodeColor::Red);

        self.fix_insert_rb_violation(new_node);

        self.count += 1;
    }

    fn fix_insert_rb_violation(&mut self, mut new_node: NodeLink<P>) {
        while let NodeColor::Red = new_node.get_node().parent.get_node().header.get_color() {
            if new_node.get_node().parent
                == new_node.get_node().parent.get_node().parent.get_node().left
            {
                let uncle = new_node
                    .get_node()
                    .parent
                    .get_node()
                    .parent
                    .get_node()
                    .right;
                if matches!(uncle.get_node().header.get_color(), NodeColor::Red) {
                    new_node
                        .get_node()
                        .parent
                        .get_node_mut()
                        .header
                        .set_color(NodeColor::Black);

                    uncle.get_node_mut().header.set_color(NodeColor::Black);

                    new_node
                        .get_node()
                        .parent
                        .get_node()
                        .parent
                        .get_node_mut()
                        .header
                        .set_color(NodeColor::Red);

                    new_node = new_node.get_node().parent.get_node().parent;

                    continue;
                }

                if new_node == new_node.get_node().parent.get_node().right {
                    new_node = new_node.get_node().parent;
                    self.left_rotate(new_node);
                }

                new_node
                    .get_node()
                    .parent
                    .get_node_mut()
                    .header
                    .set_color(NodeColor::Black);

                new_node
                    .get_node()
                    .parent
                    .get_node()
                    .parent
                    .get_node_mut()
                    .header
                    .set_color(NodeColor::Red);

                self.right_rotate(new_node.get_node().parent.get_node().parent);
                continue;
            }

            let uncle = new_node.get_node().parent.get_node().parent.get_node().left;

            if matches!(uncle.get_node().header.get_color(), NodeColor::Red) {
                new_node
                    .get_node()
                    .parent
                    .get_node_mut()
                    .header
                    .set_color(NodeColor::Black);

                uncle.get_node_mut().header.set_color(NodeColor::Black);

                new_node
                    .get_node()
                    .parent
                    .get_node()
                    .parent
                    .get_node_mut()
                    .header
                    .set_color(NodeColor::Red);

                new_node = new_node.get_node().parent.get_node().parent;

                continue;
            }

            if new_node == new_node.get_node().parent.get_node().left {
                new_node = new_node.get_node().parent;
                self.right_rotate(new_node);
            }

            let parent = new_node.get_node().parent;
            parent.get_node_mut().header.set_color(NodeColor::Black);
            parent
                .get_node()
                .parent
                .get_node_mut()
                .header
                .set_color(NodeColor::Red);

            self.left_rotate(parent.get_node().parent);
        }

        self.root.get_node_mut().header.set_color(NodeColor::Black);
    }

    fn left_rotate(&mut self, center: NodeLink<P>) {
        let right_child = center.get_node().right;
        center.get_node_mut().right = right_child.get_node().left;

        if right_child.get_node().left != self.black_nil {
            right_child.get_node().left.get_node_mut().parent = center;
        }

        right_child.get_node_mut().parent = center.get_node().parent;

        if center.get_node().parent == self.black_nil {
            self.root = right_child;
        } else if center == center.get_node().parent.get_node().left {
            center.get_node().parent.get_node_mut().left = right_child;
        } else {
            center.get_node().parent.get_node_mut().right = right_child;
        }

        right_child.get_node_mut().left = center;
        center.get_node_mut().parent = right_child;
    }

    fn right_rotate(&mut self, center: NodeLink<P>) {
        let left_child = center.get_node().left;
        center.get_node_mut().left = left_child.get_node().right;

        if left_child.get_node().right != self.black_nil {
            left_child.get_node().right.get_node_mut().parent = center;
        }

        left_child.get_node_mut().parent = center.get_node().parent;

        if center.get_node().parent == self.black_nil {
            self.root = left_child;
        } else if center == center.get_node().parent.get_node().right {
            center.get_node().parent.get_node_mut().right = left_child;
        } else {
            center.get_node().parent.get_node_mut().left = left_child;
        }

        left_child.get_node_mut().right = center;
        center.get_node_mut().parent = left_child;
    }

    pub(super) fn remove_node(&mut self, mut node_to_remove: NodeLink<P>) -> NodeLink<P> {
        let mut color_check = node_to_remove.get_node().header.get_color();
        let mut extra_black: NodeLink<P> = NodeLink::NULL_LINK;

        if node_to_remove.get_node().left == self.black_nil {
            extra_black = node_to_remove.get_node().right;
            self.swap_nodes(node_to_remove, extra_black);
        } else if node_to_remove.get_node().right == self.black_nil {
            extra_black = node_to_remove.get_node().left;
            self.swap_nodes(node_to_remove, extra_black);
        } else {
            let right_min: NodeLink<P> = self.get_subtree_min(node_to_remove.get_node().right);
            color_check = right_min.get_node().header.get_color();

            extra_black = right_min.get_node().right;

            if right_min != node_to_remove.get_node().right {
                self.swap_nodes(right_min, right_min.get_node().right);
                right_min.get_node_mut().right = node_to_remove.get_node().right;
                right_min.get_node().right.get_node_mut().parent = right_min;
            } else {
                extra_black.get_node_mut().parent = right_min;
            }

            self.swap_nodes(node_to_remove, right_min);
            right_min.get_node_mut().left = node_to_remove.get_node().left;
            right_min.get_node().left.get_node_mut().parent = right_min;
            right_min
                .get_node_mut()
                .header
                .set_color(node_to_remove.get_node().header.get_color());
        }

        if let NodeColor::Black = color_check {
            self.fix_remove_rb_violation(extra_black);
        }

        self.count -= 1;

        node_to_remove
    }

    fn fix_remove_rb_violation(&mut self, mut extra_black_node: NodeLink<P>) {
        while extra_black_node != self.root
            && !matches!(
                extra_black_node.get_node().header.get_color(),
                NodeColor::Black
            )
        {
            if extra_black_node == extra_black_node.get_node().parent.get_node().left {
                let mut right_sibling = extra_black_node.get_node().parent.get_node().right;
                if let NodeColor::Red = right_sibling.get_node().header.get_color() {
                    right_sibling
                        .get_node_mut()
                        .header
                        .set_color(NodeColor::Black);
                    extra_black_node
                        .get_node()
                        .parent
                        .get_node_mut()
                        .header
                        .set_color(NodeColor::Red);

                    self.left_rotate(extra_black_node.get_node().parent);
                    right_sibling = extra_black_node.get_node().parent.get_node().right;
                }

                if let (NodeColor::Black, NodeColor::Black) = (
                    right_sibling.get_node().left.get_node().header.get_color(),
                    right_sibling.get_node().right.get_node().header.get_color(),
                ) {
                    right_sibling
                        .get_node_mut()
                        .header
                        .set_color(NodeColor::Red);
                    extra_black_node = extra_black_node.get_node().parent;

                    continue;
                }

                if let NodeColor::Black =
                    right_sibling.get_node().right.get_node().header.get_color()
                {
                    right_sibling
                        .get_node()
                        .left
                        .get_node_mut()
                        .header
                        .set_color(NodeColor::Black);
                    right_sibling
                        .get_node_mut()
                        .header
                        .set_color(NodeColor::Red);

                    self.right_rotate(right_sibling);
                    right_sibling = extra_black_node.get_node().parent.get_node().right;
                }

                right_sibling.get_node_mut().header.set_color(
                    extra_black_node
                        .get_node()
                        .parent
                        .get_node()
                        .header
                        .get_color(),
                );

                extra_black_node
                    .get_node()
                    .parent
                    .get_node_mut()
                    .header
                    .set_color(NodeColor::Black);

                right_sibling
                    .get_node()
                    .right
                    .get_node_mut()
                    .header
                    .set_color(NodeColor::Black);

                self.left_rotate(extra_black_node.get_node().parent);
                extra_black_node = self.root;

                continue;
            }

            let mut left_sibling = extra_black_node.get_node().parent.get_node().left;
            if let NodeColor::Red = left_sibling.get_node().header.get_color() {
                left_sibling
                    .get_node_mut()
                    .header
                    .set_color(NodeColor::Black);
                extra_black_node
                    .get_node()
                    .parent
                    .get_node_mut()
                    .header
                    .set_color(NodeColor::Red);

                self.right_rotate(extra_black_node.get_node().parent);
                left_sibling = extra_black_node.get_node().parent.get_node().left;
            }

            if let (NodeColor::Black, NodeColor::Black) = (
                left_sibling.get_node().left.get_node().header.get_color(),
                left_sibling.get_node().right.get_node().header.get_color(),
            ) {
                left_sibling.get_node_mut().header.set_color(NodeColor::Red);
                extra_black_node = extra_black_node.get_node().parent;

                continue;
            }

            if let NodeColor::Black = left_sibling.get_node().left.get_node().header.get_color() {
                left_sibling
                    .get_node()
                    .right
                    .get_node_mut()
                    .header
                    .set_color(NodeColor::Black);
                left_sibling.get_node_mut().header.set_color(NodeColor::Red);

                self.left_rotate(left_sibling);
                left_sibling = extra_black_node.get_node().parent.get_node().left;
            }

            left_sibling.get_node_mut().header.set_color(
                extra_black_node
                    .get_node()
                    .parent
                    .get_node()
                    .header
                    .get_color(),
            );

            extra_black_node
                .get_node()
                .parent
                .get_node_mut()
                .header
                .set_color(NodeColor::Black);

            left_sibling
                .get_node()
                .left
                .get_node_mut()
                .header
                .set_color(NodeColor::Black);

            self.right_rotate(extra_black_node.get_node().parent);
            extra_black_node = self.root;
        }

        extra_black_node
            .get_node_mut()
            .header
            .set_color(NodeColor::Black);
    }

    fn get_subtree_min(&self, mut subtree_root: NodeLink<P>) -> NodeLink<P> {
        while subtree_root.get_node().left != self.black_nil {
            subtree_root = subtree_root.get_node().left;
        }

        subtree_root
    }

    fn swap_nodes(&mut self, node_to_remove: NodeLink<P>, replacement_node: NodeLink<P>) {
        if node_to_remove.get_node().parent == self.black_nil {
            self.root = replacement_node;
        } else if node_to_remove == node_to_remove.get_node().parent.get_node().left {
            node_to_remove.get_node().parent.get_node_mut().left = replacement_node;
        } else {
            node_to_remove.get_node().parent.get_node_mut().right = replacement_node;
        }

        replacement_node.get_node_mut().parent = node_to_remove.get_node().parent;
    }
}
