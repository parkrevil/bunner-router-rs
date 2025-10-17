use bumpalo::Bump;
use core::ptr::NonNull;
use std::rc::Rc;

use super::RadixTreeNode;

#[derive(Clone, Debug)]
pub(crate) struct ArenaHandle(Rc<Bump>);

impl ArenaHandle {
    #[inline]
    pub fn new(bump: Rc<Bump>) -> Self {
        Self(bump)
    }

    #[inline]
    pub fn alloc_node(&self) -> NodeBox {
        let node_ref: &mut RadixTreeNode = self.0.alloc(RadixTreeNode::default());
        NodeBox(NonNull::from(node_ref))
    }
}

#[repr(transparent)]
#[derive(Clone)]
pub(crate) struct NodeBox(pub(crate) NonNull<RadixTreeNode>);

impl NodeBox {
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
