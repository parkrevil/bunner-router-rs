use super::snapshot::ReadOnlyNode;
use crate::pattern::SegmentPattern;
use crate::radix::{HTTP_METHOD_COUNT, RadixTree, RadixTreeNode};
use hashbrown::HashMap as FastHashMap;

pub(crate) fn copy_static_maps(
    tree: &RadixTree,
) -> [FastHashMap<Box<str>, u16>; HTTP_METHOD_COUNT] {
    let mut maps: [FastHashMap<Box<str>, u16>; HTTP_METHOD_COUNT] = Default::default();
    for (dst, src) in maps.iter_mut().zip(tree.static_route_full_mapping.iter()) {
        *dst = src.clone();
    }
    maps
}

pub(crate) fn extract_root(node: &RadixTreeNode) -> ReadOnlyNode {
    build_node(node)
}

fn build_node(source: &RadixTreeNode) -> ReadOnlyNode {
    let mut static_children: FastHashMap<Box<str>, ReadOnlyNode> = FastHashMap::new();

    if !source.static_keys.is_empty() && source.static_vals_idx.len() == source.static_keys.len() {
        for (i, key) in source.static_keys.iter().enumerate() {
            let child = source.static_vals_idx[i].as_ref();
            static_children.insert(key.clone(), build_node(child));
        }
    } else if !source.static_children_idx.is_empty() {
        for (key, child) in source.static_children_idx.iter() {
            static_children.insert(key.clone(), build_node(child.as_ref()));
        }
    } else {
        for (key, child) in source.static_children.iter() {
            static_children.insert(key.clone(), build_node(child.as_ref()));
        }
        for (i, key) in source.static_keys.iter().enumerate() {
            static_children.insert(key.clone(), build_node(source.static_vals[i].as_ref()));
        }
    }

    let mut patterns: Vec<(SegmentPattern, ReadOnlyNode)> =
        Vec::with_capacity(source.patterns.len());
    for (i, pattern) in source.patterns.iter().enumerate() {
        let child = source.pattern_nodes[i].as_ref();
        patterns.push((pattern.clone(), build_node(child)));
    }

    let fused_child = source
        .fused_child
        .as_ref()
        .map(|child| Box::new(build_node(child.as_ref())));

    ReadOnlyNode {
        fused_edge: source
            .fused_edge
            .as_ref()
            .map(|edge| edge.clone().into_boxed_str()),
        fused_child,
        routes: source.routes,
        wildcard_routes: source.wildcard_routes,
        static_children,
        patterns,
    }
}
