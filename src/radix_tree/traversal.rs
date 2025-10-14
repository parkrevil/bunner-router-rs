use super::node::RadixTreeNode;

const TRAVERSAL_STACK_CAPACITY: usize = 1024;

/// Depth-first traversal over an immutable `RadixTreeNode`.
/// Uses an explicit stack to avoid recursion on deep trees.
#[inline]
pub(super) fn traverse<F>(root: &RadixTreeNode, mut action: F)
where
    F: FnMut(&RadixTreeNode),
{
    let mut stack: Vec<&RadixTreeNode> = Vec::with_capacity(TRAVERSAL_STACK_CAPACITY);
    stack.push(root);

    while let Some(node) = stack.pop() {
        action(node);

        for child in node.static_vals.iter() {
            stack.push(child.as_ref());
        }
        for (_, v) in node.static_children.iter() {
            stack.push(v.as_ref());
        }
        for nb in node.pattern_nodes.iter() {
            stack.push(nb.as_ref());
        }
        if let Some(fc) = node.fused_child.as_ref() {
            stack.push(fc.as_ref());
        }
    }
}

/// Depth-first traversal over a mutable `RadixTreeNode`.
/// Stores raw pointers locally to satisfy borrow checker safely within scope.
#[inline]
pub(super) fn traverse_mut<F>(root: &mut RadixTreeNode, mut action: F)
where
    F: FnMut(&mut RadixTreeNode),
{
    let mut stack: Vec<*mut RadixTreeNode> = Vec::with_capacity(TRAVERSAL_STACK_CAPACITY);
    stack.push(root as *mut _);

    while let Some(ptr) = stack.pop() {
        let node = unsafe { &mut *ptr };
        action(node);

        for child in node.static_vals.iter_mut() {
            stack.push(child.as_mut() as *mut _);
        }
        for (_, v) in node.static_children.iter_mut() {
            stack.push(v.as_mut() as *mut _);
        }
        for nb in node.pattern_nodes.iter_mut() {
            stack.push(nb.as_mut() as *mut _);
        }
        if let Some(fc) = node.fused_child.as_mut() {
            stack.push(fc.as_mut() as *mut _);
        }
    }
}
