use bumpalo::Bump;
use core::ptr::NonNull;

use super::RadixTreeNode;

#[repr(transparent)]
#[derive(Clone)]
pub(crate) struct NodeBox(pub(crate) NonNull<RadixTreeNode>);

impl NodeBox {
    #[inline(always)]
    pub fn from_arena(arena: &Bump) -> Self {
        let node_ref: &mut RadixTreeNode = arena.alloc(RadixTreeNode::default());
        Self(NonNull::from(node_ref))
    }
    #[inline(always)]
    pub fn as_ref(&self) -> &RadixTreeNode {
        unsafe { self.0.as_ref() }
    }
    #[inline(always)]
    pub fn as_mut(&mut self) -> &mut RadixTreeNode {
        unsafe { self.0.as_mut() }
    }
}

impl core::ops::Deref for NodeBox {
    type Target = RadixTreeNode;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl core::ops::DerefMut for NodeBox {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut()
    }
}

impl core::fmt::Debug for NodeBox {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("NodeBox")
            .field(&(self.0.as_ptr() as usize))
            .finish()
    }
}

#[inline(always)]
pub(crate) fn create_node_box_from_arena_pointer(arena_ptr: *const Bump) -> NodeBox {
    let arena_ref = unsafe { &*arena_ptr };
    NodeBox::from_arena(arena_ref)
}
