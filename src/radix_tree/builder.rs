use super::compression::compress_root_node;
use super::mask::compute_mask;
use super::memory::{shrink_node, warm_node};
use super::static_map::collect_static;
use super::{HTTP_METHOD_COUNT, RadixTree, STATIC_MAP_THRESHOLD, node::RadixTreeNode};
use super::{traversal::traverse, traversal::traverse_mut};
use crate::interner::Interner;
use crate::pattern::SegmentPart;
use crate::pattern::pattern_score;

pub(super) fn finalize(tree: &mut RadixTree) {
    if tree.root_node.is_sealed() {
        return;
    }

    // First, rebuild all pattern metadata and indices across the tree
    // so that the auto-optimization logic can make correct decisions.
    traverse_mut(&mut tree.root_node, |node| {
        rebuild_pattern_meta(node);
        rebuild_pattern_index(node);
    });

    // Clear worker id metadata if present. After sealing there will be no re-registrations,
    // Drop the side-table tracking initial registrant worker ids to reclaim heap memory.
    // This actually frees memory; per-node inlined fields would not.
    tree.route_worker_side_table.clear();
    tree.route_worker_side_table.shrink_to_fit();

    // --- Automatic Optimization Logic ---
    if tree.options.enable_automatic_optimization {
        // 1. Auto-enable root pruning
        let has_root_param_or_wildcard = {
            let n = &tree.root_node;
            let mut has_dynamic = false;
            for m in 0..HTTP_METHOD_COUNT {
                if n.wildcard_routes[m] != 0 {
                    has_dynamic = true;
                    break;
                }
            }
            if !has_dynamic {
                has_dynamic = !n.pattern_param_first.is_empty();
            }
            has_dynamic
        };

        if !has_root_param_or_wildcard {
            tree.enable_root_level_pruning = true;
        }

        // 2. Auto-enable static full map based on heuristics
        let mut static_route_count = 0;
        count_static(&tree.root_node, &mut static_route_count);

        if static_route_count >= STATIC_MAP_THRESHOLD {
            tree.enable_static_route_full_mapping = true;
        }
    }
    // --- End of Automatic Optimization Logic ---

    tree.root_node.set_sealed(true);
    compress_tree(tree);

    // 1. Finalize node structure: Move all static children from HashMap to Vecs for sorting.
    traverse_mut(&mut tree.root_node, |node| {
        if !node.static_children.is_empty() {
            for (k, v) in node.static_children.drain() {
                node.static_keys.push(k);
                node.static_vals.push(v);
            }
        }
    });

    // 2. Sort the static children Vecs recursively for the entire tree.
    sort_node_recursively(&mut tree.root_node);

    // 3. Now build all indices based on the sorted and finalized structure.
    build_indices(tree);

    // build root-level bitmaps and flags for pruning
    tree.method_first_byte_bitmaps = [[0; 4]; HTTP_METHOD_COUNT];
    tree.root_parameter_first_present = [false; HTTP_METHOD_COUNT];
    tree.root_wildcard_present = [false; HTTP_METHOD_COUNT];
    tree.method_length_buckets = [0; HTTP_METHOD_COUNT];

    build_pruning_maps(tree);

    shrink_node(&mut tree.root_node);

    // Optional cache warm-up to reduce first-hit latency
    warm_node(&tree.root_node);

    // Build static full maps for O(1) lookup when path is entirely static
    build_static_map(tree);

    super::traversal::traverse_mut(&mut tree.root_node, |n| {
        n.pattern_children_idx.clear();
        n.pattern_children_idx.shrink_to_fit();

        if !tree.enable_static_route_full_mapping {
            n.static_children_idx_ids.shrink_to_fit();
            n.static_children.shrink_to_fit();
        }
    });

    tree.interner.runtime_cleanup();

    // For stability, only drop bitmaps if pruning is disabled
    if !tree.enable_root_level_pruning {
        tree.method_first_byte_bitmaps = [[0; 4]; super::HTTP_METHOD_COUNT];
        tree.method_length_buckets = [0; super::HTTP_METHOD_COUNT];
    }

    for m in 0..super::HTTP_METHOD_COUNT {
        tree.static_route_full_mapping[m].shrink_to_fit();
    }
}

fn compress_tree(tree: &mut RadixTree) {
    super::indices::invalidate_all_indices(&mut tree.root_node);
    compress_root_node(&mut tree.root_node);
}

fn build_indices(tree: &mut RadixTree) {
    super::indices::build_indices(&mut tree.root_node, &tree.interner);
    compute_mask(&mut tree.root_node);
}

fn count_static(root: &RadixTreeNode, count: &mut usize) {
    traverse(root, |n| {
        for i in 0..HTTP_METHOD_COUNT {
            if n.routes[i] != 0 {
                *count += 1;
            }
        }
    });
}

fn sort_node_recursively(n: &mut RadixTreeNode) {
    // 1. Sort current node's static children
    if !n.static_keys.is_empty() {
        let mut pairs: Vec<_> = n
            .static_keys
            .drain(..)
            .zip(n.static_vals.drain(..))
            .collect();

        pairs.sort_unstable_by(|(a, _), (b, _)| a.cmp(b));

        for (k, v) in pairs {
            n.static_keys.push(k);
            n.static_vals.push(v);
        }
    }

    // 2. Recurse into all child types
    for child in n.static_vals.iter_mut() {
        sort_node_recursively(child.as_mut());
    }
    for nb in n.pattern_nodes.iter_mut() {
        sort_node_recursively(nb.as_mut());
    }
    if let Some(fc) = n.fused_child.as_mut() {
        sort_node_recursively(fc.as_mut());
    }
}

fn build_pruning_maps(tree: &mut RadixTree) {
    let n = &tree.root_node;
    for m in 0..HTTP_METHOD_COUNT {
        if n.wildcard_routes[m] != 0 {
            tree.root_wildcard_present[m] = true;
        }
    }
    if let Some(edge) = n.fused_edge.as_ref() {
        if let Some(&b0) = edge.as_str().as_bytes().first() {
            let b = b0;
            let blk = (b as usize) >> 6;
            let bit = 1u64 << ((b as usize) & 63);
            let mask = n.method_mask();
            for mi in 0..HTTP_METHOD_COUNT {
                if (mask & (1 << mi)) != 0 {
                    tree.method_first_byte_bitmaps[mi][blk] |= bit;
                }
            }
        }
        let l = edge.len().min(63) as u32;
        let mask = n.method_mask();
        for mi in 0..HTTP_METHOD_COUNT {
            if (mask & (1 << mi)) != 0 {
                tree.method_length_buckets[mi] |= 1u64 << l;
            }
        }
    }
    for k in n.static_keys.iter() {
        if let Some(&b) = k.as_bytes().first() {
            let blk = (b as usize) >> 6;
            let bit = 1u64 << ((b as usize) & 63);
            let mask = n.method_mask();
            for m in 0..HTTP_METHOD_COUNT {
                if (mask & (1 << m)) != 0 {
                    tree.method_first_byte_bitmaps[m][blk] |= bit;
                }
            }
        }
        let l = k.len().min(63) as u32;
        let mask = n.method_mask();
        for m in 0..HTTP_METHOD_COUNT {
            if (mask & (1 << m)) != 0 {
                tree.method_length_buckets[m] |= 1u64 << l;
            }
        }
    }
    for (k, _) in n.static_children.iter() {
        if let Some(&b) = k.as_bytes().first() {
            let blk = (b as usize) >> 6;
            let bit = 1u64 << ((b as usize) & 63);
            let mask = n.method_mask();
            for m in 0..HTTP_METHOD_COUNT {
                if (mask & (1 << m)) != 0 {
                    tree.method_first_byte_bitmaps[m][blk] |= bit;
                }
            }
        }
        let l = k.len().min(63) as u32;
        let mask = n.method_mask();
        for m in 0..HTTP_METHOD_COUNT {
            if (mask & (1 << m)) != 0 {
                tree.method_length_buckets[m] |= 1u64 << l;
            }
        }
    }
    for (&hb, _) in n.pattern_first_lit_head.iter() {
        let blk = (hb as usize) >> 6;
        let bit = 1u64 << ((hb as usize) & 63);
        let mask = n.method_mask();
        for m in 0..HTTP_METHOD_COUNT {
            if (mask & (1 << m)) != 0 {
                tree.method_first_byte_bitmaps[m][blk] |= bit;
            }
        }
    }
    for pat in n.patterns.iter() {
        if let Some(SegmentPart::Literal(l0)) = pat.parts.first() {
            let l = l0.len().min(63) as u32;
            let mask = n.method_mask();
            for m in 0..HTTP_METHOD_COUNT {
                if (mask & (1 << m)) != 0 {
                    tree.method_length_buckets[m] |= 1u64 << l;
                }
            }
        }
    }
    if !n.pattern_param_first.is_empty() {
        let mask = n.method_mask();
        for m in 0..HTTP_METHOD_COUNT {
            if (mask & (1 << m)) != 0 {
                tree.root_parameter_first_present[m] = true;
            }
        }
    }
}

fn build_static_map(tree: &mut RadixTree) {
    for m in 0..HTTP_METHOD_COUNT {
        tree.static_route_full_mapping[m].clear();
    }
    if tree.enable_static_route_full_mapping {
        let mut path_buf = String::from("");
        collect_static(
            &tree.root_node,
            &mut path_buf,
            &mut tree.static_route_full_mapping,
        );
    }
}

// rebuild_intern_ids / rebuild_pattern_* remain in this file
pub(super) fn rebuild_intern_ids(node: &mut RadixTreeNode, interner: &Interner) {
    node.static_key_ids.clear();
    node.static_hash_table.clear();
    node.static_hash_seed = 0;

    if !node.static_keys.is_empty() {
        node.static_key_ids.reserve(node.static_keys.len());

        for k in node.static_keys.iter() {
            node.static_key_ids.push(interner.intern(k.as_ref()));
        }

        if node.static_vals_idx.len() == node.static_keys.len() && node.static_keys.len() >= 16 {
            let mut size: usize = (node.static_keys.len() * 2).next_power_of_two();
            let max_size: usize = node.static_keys.len() * 8;
            let mut seed: u64 = 1469598103934665603;
            while size <= max_size {
                let mut table: Vec<i32> = vec![-1; size];
                let mut ok = true;
                for (i, k) in node.static_keys.iter().enumerate() {
                    let mut h: u64 = seed;
                    for &b in k.as_bytes() {
                        h ^= b as u64;
                        h = h.wrapping_mul(1099511628211);
                    }
                    let mut idx = (h as usize) & (size - 1);
                    let mut steps = 0usize;
                    while table[idx] != -1 {
                        idx = (idx + 1) & (size - 1);
                        steps += 1;
                        if steps > size {
                            ok = false;
                            break;
                        }
                    }
                    if !ok {
                        break;
                    }
                    table[idx] = i as i32;
                }
                if ok {
                    node.static_hash_seed = seed;
                    node.static_hash_table.clear();
                    node.static_hash_table.extend_from_slice(&table);
                    break;
                }
                seed = seed
                    .wrapping_mul(1315423911)
                    .wrapping_add(0x9e3779b97f4a7c15);
                size *= 2;
            }
        }
    }
    node.static_children_idx_ids.clear();
    if !node.static_children.is_empty() {
        for (k, v) in node.static_children.iter() {
            let id = interner.intern(k);
            node.static_children_idx_ids.insert(id, super::NodeBox(v.0));
        }
    }
}

#[inline]
pub(super) fn rebuild_pattern_index(node: &mut RadixTreeNode) {
    node.pattern_first_literal.clear();
    node.pattern_first_lit_head.clear();
    node.pattern_param_first.clear();

    for (idx, pat) in node.patterns.iter().enumerate() {
        if let Some(SegmentPart::Literal(l0)) = pat.parts.first() {
            let entry = node
                .pattern_first_literal
                .entry(l0.clone())
                .or_insert_with(smallvec::SmallVec::new);
            entry.push(idx as u16);
            if let Some(&b) = l0.as_bytes().first() {
                let entry2 = node
                    .pattern_first_lit_head
                    .entry(b)
                    .or_insert_with(smallvec::SmallVec::new);
                entry2.push(idx as u16);
            }
        } else if let Some(SegmentPart::Param { .. }) = pat.parts.first() {
            node.pattern_param_first.push(idx as u16);
        }
    }
}

#[inline]
pub(super) fn rebuild_pattern_meta(node: &mut RadixTreeNode) {
    node.pattern_meta.clear();
    node.pattern_meta.reserve(node.patterns.len());

    for pat in node.patterns.iter() {
        let score = pattern_score(pat);

        let mut min_len = 0u16;
        for part in pat.parts.iter() {
            match part {
                SegmentPart::Literal(l) => {
                    min_len += l.len() as u16;
                }
                SegmentPart::Param { .. } => {}
            }
        }

        let mut last_len = 0u16;
        for part in pat.parts.iter().rev() {
            if let SegmentPart::Literal(l) = part {
                last_len = l.len() as u16;
                break;
            }
        }

        let meta = super::node::PatternMeta::new(score, min_len, last_len);
        node.pattern_meta.push(meta);
    }

    debug_assert_eq!(node.patterns.len(), node.pattern_meta.len());
}

// traversal moved to traversal.rs
