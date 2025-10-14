use super::node::RadixTreeNode;
use super::traversal::traverse;

pub(super) fn shrink_node(n: &mut RadixTreeNode) {
    n.static_keys.shrink_to_fit();
    n.static_vals.shrink_to_fit();
    n.static_vals_idx.shrink_to_fit();
    n.pattern_children_idx.shrink_to_fit();
    n.patterns.shrink_to_fit();
    n.pattern_nodes.shrink_to_fit();
    n.pattern_first_literal.shrink_to_fit();
    n.pattern_meta.shrink_to_fit();
    for (_, v) in n.static_children.iter_mut() {
        shrink_node(v.as_mut());
    }
    for nb in n.pattern_nodes.iter_mut() {
        shrink_node(nb.as_mut());
    }
    if let Some(fc) = n.fused_child.as_mut() {
        shrink_node(fc.as_mut());
    }
}

pub(super) fn warm_node(root: &RadixTreeNode) {
    traverse(root, |n| {
        for v in n.static_vals.iter() {
            let _ = v.as_ref().routes[0];
        }
        for (_, v) in n.static_children.iter() {
            let _ = v.as_ref().routes[0];
        }
        for nb in n.pattern_nodes.iter() {
            let _ = nb.as_ref().routes[0];
        }
        if let Some(fc) = n.fused_child.as_ref() {
            let _ = fc.as_ref().routes[0];
        }
    });
}
