use super::alloc::NodeBox;
use super::node::RadixTreeNode;
use crate::tools::Interner;
use smallvec::SmallVec;

/// Clears all index mirrors across the subtree rooted at `root`.
#[inline]
pub(super) fn invalidate_all_indices(root: &mut RadixTreeNode) {
    super::traversal::traverse_mut(root, |node| {
        node.static_vals_idx.clear();
        node.static_children_idx.clear();
        node.pattern_children_idx.clear();
        node.fused_child_idx = None;
    });
}

/// Rebuilds index mirrors used by fast lookup paths.
#[inline]
pub(super) fn build_indices(root: &mut RadixTreeNode, interner: &Interner) {
    super::traversal::traverse_mut(root, |node| {
        if node.is_dirty() {
            node.set_method_mask(0);
            node.static_vals_idx.clear();
            node.static_children_idx.clear();
            node.pattern_children_idx.clear();
            node.fused_child_idx = None;
            if !node.static_keys.is_empty() {
                let mut tmp_idxs: SmallVec<[NodeBox; 16]> = SmallVec::new();
                tmp_idxs.reserve(node.static_vals.len());
                for child in node.static_vals.iter() {
                    tmp_idxs.push(NodeBox(child.0));
                }
                node.static_vals_idx.extend(tmp_idxs);
            }
            if !node.static_children.is_empty() {
                let keys: SmallVec<[Box<str>; 16]> = node.static_children.keys().cloned().collect();
                for k in keys {
                    if let Some(v) = node.static_children.get(&k) {
                        node.static_children_idx.insert(k, super::NodeBox(v.0));
                    }
                }
            }
            if !node.patterns.is_empty() {
                node.pattern_children_idx.clear();
                node.pattern_children_idx.reserve(node.patterns.len());
                for i in 0..node.patterns.len() {
                    node.pattern_children_idx.push(i);
                }
            }
            if let Some(fc) = node.fused_child.as_ref() {
                node.fused_child_idx = Some(super::NodeBox(fc.0));
            }
            node.set_dirty(false);
        }
    });

    super::traversal::traverse_mut(root, |node| {
        if !node.is_dirty() {
            super::builder::rebuild_intern_ids(node, interner);
        }
    });
}
