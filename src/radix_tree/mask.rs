use super::HTTP_METHOD_COUNT;
use super::node::RadixTreeNode;

/// Computes and sets the method bitmask for a node and its subtree.
#[inline]
pub(super) fn compute_mask(n: &mut RadixTreeNode) -> u8 {
    let mut _m: u8 = 0;

    for i in 0..HTTP_METHOD_COUNT {
        if n.routes[i] != 0 || n.wildcard_routes[i] != 0 {
            _m |= 1 << i;
        }
    }

    let mut m = _m;

    for child in n.static_vals.iter_mut() {
        m |= compute_mask(child.as_mut());
    }

    for (_, v) in n.static_children.iter_mut() {
        m |= compute_mask(v.as_mut());
    }

    for nb in n.pattern_nodes.iter_mut() {
        m |= compute_mask(nb.as_mut());
    }

    if let Some(fc) = n.fused_child.as_mut() {
        m |= compute_mask(fc.as_mut());
    }

    n.set_method_mask(m);

    m
}
