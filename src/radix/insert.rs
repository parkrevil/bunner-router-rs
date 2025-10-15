use super::{
    MAX_ROUTES, RadixTree, RadixTreeNode, create_node_box_from_arena_pointer, node::PatternMeta,
};
use crate::errors::{RouterError, RouterResult};
use crate::path::{PathError, normalize_and_validate_path};
use crate::pattern::{
    SegmentPart, SegmentPattern, parse_segment, pattern_compatible_policy, pattern_is_pure_static,
    pattern_score,
};
use crate::radix::RadixError;
use crate::tools::Interner;
use crate::types::{HttpMethod, WorkerId};
use hashbrown::HashSet;
use std::sync::atomic::AtomicU16;

impl RadixTree {
    pub fn insert(
        &mut self,
        worker_id: WorkerId,
        method: HttpMethod,
        path: &str,
    ) -> RouterResult<u16> {
        tracing::event!(tracing::Level::TRACE, operation="insert", method=?method, path=%path);
        if self.root_node.is_sealed() {
            return Err(RouterError::from(RadixError::TreeSealed {
                operation: "insert",
                path: Some(path.to_string()),
            }));
        }
        self.root_node.set_dirty(true);

        if path == "/" {
            let key = assign_route_key(
                &mut self.route_worker_side_table,
                &mut self.root_node,
                method,
                &self.next_route_key,
                worker_id,
            )?;
            return Ok(key);
        }

        let parsed_segments = self.prepare_path_segments(path)?;
        self.insert_parsed(worker_id, method, parsed_segments)
    }

    pub(super) fn insert_parsed(
        &mut self,
        worker_id: WorkerId,
        method: HttpMethod,
        parsed_segments: Vec<SegmentPattern>,
    ) -> RouterResult<u16> {
        tracing::event!(tracing::Level::TRACE, operation="insert_parsed", method=?method, segments=parsed_segments.len() as u64);
        if self.root_node.is_sealed() {
            return Err(RouterError::from(RadixError::TreeSealed {
                operation: "insert_parsed",
                path: None,
            }));
        }
        self.root_node.set_dirty(true);

        let mut current = &mut self.root_node;
        let arena_ptr: *const bumpalo::Bump = &self.arena;

        for (i, pat) in parsed_segments.iter().enumerate() {
            // Fast check without allocation: single literal '*' means wildcard
            let is_wildcard =
                matches!(pat.parts.as_slice(), [SegmentPart::Literal(s)] if s.as_str() == "*");
            if is_wildcard {
                let key = handle_wildcard_insert(
                    &mut self.route_worker_side_table,
                    current,
                    method,
                    i,
                    parsed_segments.len(),
                    &self.next_route_key,
                    worker_id,
                )?;
                return Ok(key);
            }

            // Detect pure static without building a joined string
            if pat.parts.len() == 1 {
                if let SegmentPart::Literal(lit) = &pat.parts[0] {
                    current = current.descend_static_mut_with_alloc(lit.as_str(), || {
                        create_node_box_from_arena_pointer(arena_ptr)
                    });
                    sort_static_children(current, &self.interner);
                } else {
                    current = find_or_create_pattern_child(current, pat, arena_ptr)?;
                }
            } else if pattern_is_pure_static(pat, "") {
                // Unlikely path; keep safety for helper parity
                let joined = pat
                    .parts
                    .iter()
                    .map(|p| match p {
                        SegmentPart::Literal(s) => s.as_str(),
                        _ => "",
                    })
                    .collect::<String>();
                current = current.descend_static_mut_with_alloc(joined.as_str(), || {
                    create_node_box_from_arena_pointer(arena_ptr)
                });
                sort_static_children(current, &self.interner);
            } else {
                current = find_or_create_pattern_child(current, pat, arena_ptr)?;
            }

            // method mask is delayed to finalize()
            current.set_dirty(true);
        }

        let key = assign_route_key(
            &mut self.route_worker_side_table,
            current,
            method,
            &self.next_route_key,
            worker_id,
        )?;
        Ok(key)
    }

    pub(super) fn insert_parsed_preassigned(
        &mut self,
        worker_id: WorkerId,
        method: HttpMethod,
        parsed_segments: Vec<SegmentPattern>,
        assigned_key: u16,
    ) -> RouterResult<u16> {
        tracing::event!(tracing::Level::TRACE, operation="insert_parsed_preassigned", method=?method, segments=parsed_segments.len() as u64, assigned_key=assigned_key as u64);
        if self.root_node.is_sealed() {
            return Err(RouterError::from(RadixError::TreeSealed {
                operation: "insert_parsed_preassigned",
                path: None,
            }));
        }
        self.root_node.set_dirty(true);

        let mut current = &mut self.root_node;
        let arena_ptr: *const bumpalo::Bump = &self.arena;

        for (i, pat) in parsed_segments.iter().enumerate() {
            let is_wildcard =
                matches!(pat.parts.as_slice(), [SegmentPart::Literal(s)] if s.as_str() == "*");
            if is_wildcard {
                return handle_wildcard_insert_preassigned(
                    &mut self.route_worker_side_table,
                    current,
                    method,
                    i,
                    parsed_segments.len(),
                    assigned_key,
                    worker_id,
                );
            }

            if pat.parts.len() == 1 {
                if let SegmentPart::Literal(lit) = &pat.parts[0] {
                    current = current.descend_static_mut_with_alloc(&lit.clone(), || {
                        create_node_box_from_arena_pointer(arena_ptr)
                    });
                    sort_static_children(current, &self.interner);
                } else {
                    current = find_or_create_pattern_child(current, pat, arena_ptr)?;
                }
            } else if pattern_is_pure_static(pat, "") {
                let joined = pat
                    .parts
                    .iter()
                    .map(|p| match p {
                        SegmentPart::Literal(s) => s.as_str(),
                        _ => "",
                    })
                    .collect::<String>();
                current = current.descend_static_mut_with_alloc(&joined, || {
                    create_node_box_from_arena_pointer(arena_ptr)
                });
                sort_static_children(current, &self.interner);
            } else {
                current = find_or_create_pattern_child(current, pat, arena_ptr)?;
            }
            // Do not set method_mask here; delayed to finalize for bulk path
            current.set_dirty(true);
        }

        assign_route_key_preassigned(
            &mut self.route_worker_side_table,
            current,
            method,
            assigned_key,
            worker_id,
        )
    }

    pub(super) fn prepare_path_segments(&self, path: &str) -> RouterResult<Vec<SegmentPattern>> {
        prepare_path_segments_standalone(path)
    }
}

fn sort_static_children(node: &mut RadixTreeNode, interner: &Interner) {
    let len = node.static_keys.len();
    if len == node.static_vals.len() && len > 1 {
        // Ensure key id cache is aligned; avoid repeated interner allocations
        if node.static_key_ids.len() != len {
            node.static_key_ids.clear();
            node.static_key_ids.reserve(len);
            for k in node.static_keys.iter() {
                node.static_key_ids.push(interner.intern(k));
            }
        }

        // Indices sorted by cached key ids
        let mut indices: Vec<usize> = (0..len).collect();
        indices.sort_unstable_by_key(|&i| node.static_key_ids[i]);

        // Move out keys/vals/ids without reallocating, then rebuild in order
        let mut old_keys = std::mem::take(&mut node.static_keys);
        let old_vals = std::mem::take(&mut node.static_vals);
        let old_ids = std::mem::take(&mut node.static_key_ids);

        node.static_keys.reserve(len);
        node.static_vals.reserve(len);
        node.static_key_ids.reserve(len);

        for &i in indices.iter() {
            node.static_keys.push(std::mem::take(&mut old_keys[i]));
            // NodeBox clone is a cheap pointer copy; avoids moving out of index
            node.static_vals.push(old_vals[i].clone());
            node.static_key_ids.push(old_ids[i]);
        }
    }
}

fn find_or_create_pattern_child<'a>(
    node: &'a mut RadixTreeNode,
    pat: &SegmentPattern,
    arena_ptr: *const bumpalo::Bump,
) -> RouterResult<&'a mut RadixTreeNode> {
    for exist in node.patterns.iter() {
        if !pattern_compatible_policy(exist, pat) {
            return Err(RouterError::from(RadixError::ParamNameConflict {
                pattern: format!("{:?}", pat),
            }));
        }
    }

    if let Some(existing_idx) = node.patterns.iter().position(|exist| exist == pat) {
        return Ok(node.pattern_nodes.get_mut(existing_idx).unwrap().as_mut());
    }

    let score = pattern_score(pat);
    let insert_pos = node
        .pattern_meta
        .iter()
        .position(|&meta| meta.score < score)
        .unwrap_or(node.patterns.len());

    node.patterns.insert(insert_pos, pat.clone());
    node.pattern_nodes
        .insert(insert_pos, create_node_box_from_arena_pointer(arena_ptr));

    Ok(node.pattern_nodes.get_mut(insert_pos).unwrap().as_mut())
}

fn handle_wildcard_insert(
    side_table: &mut Vec<Option<u32>>,
    node: &mut RadixTreeNode,
    method: HttpMethod,
    index: usize,
    total_segments: usize,
    next_route_key: &AtomicU16,
    worker_id: WorkerId,
) -> RouterResult<u16> {
    if index != total_segments - 1 {
        return Err(RouterError::from(RadixError::WildcardMustBeTerminal {
            segment_index: index,
            total_segments,
        }));
    }
    let method_idx = method as usize;
    if node.wildcard_routes[method_idx] != 0 {
        let existing_key = node.wildcard_routes[method_idx] - 1;
        if side_table.get(existing_key as usize).and_then(|v| *v) == Some(worker_id) {
            return Err(RouterError::from(RadixError::DuplicateWildcardRoute {
                worker_id,
                existing_key,
            }));
        }
        return Ok(existing_key);
    }
    let key = next_route_key.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    node.wildcard_routes[method_idx] = key + 1;
    record_route_worker_raw(side_table, key, worker_id);

    node.set_dirty(true);

    Ok(key)
}

fn handle_wildcard_insert_preassigned(
    side_table: &mut Vec<Option<u32>>,
    node: &mut RadixTreeNode,
    method: HttpMethod,
    index: usize,
    total_segments: usize,
    assigned_key: u16,
    worker_id: WorkerId,
) -> RouterResult<u16> {
    if index != total_segments - 1 {
        return Err(RouterError::from(RadixError::WildcardMustBeTerminal {
            segment_index: index,
            total_segments,
        }));
    }
    let method_idx = method as usize;
    if node.wildcard_routes[method_idx] != 0 {
        let existing_key = node.wildcard_routes[method_idx] - 1;
        if side_table.get(existing_key as usize).and_then(|v| *v) == Some(worker_id) {
            return Err(RouterError::from(RadixError::DuplicateWildcardRoute {
                worker_id,
                existing_key,
            }));
        }
        return Ok(existing_key);
    }
    node.wildcard_routes[method_idx] = assigned_key + 1;
    record_route_worker_raw(side_table, assigned_key, worker_id);
    node.set_dirty(true);
    Ok(assigned_key)
}

fn assign_route_key(
    side_table: &mut Vec<Option<u32>>,
    node: &mut RadixTreeNode,
    method: HttpMethod,
    next_route_key: &AtomicU16,
    worker_id: WorkerId,
) -> RouterResult<u16> {
    let method_idx = method as usize;
    if node.routes[method_idx] != 0 {
        let existing_key = node.routes[method_idx] - 1;
        if side_table.get(existing_key as usize).and_then(|v| *v) == Some(worker_id) {
            return Err(RouterError::from(RadixError::DuplicateRoute {
                worker_id,
                existing_key,
            }));
        }
        return Ok(existing_key);
    }
    let current_key = next_route_key.load(std::sync::atomic::Ordering::Relaxed);
    if current_key == MAX_ROUTES {
        return Err(RouterError::from(RadixError::MaxRoutesExceeded {
            requested: None,
            current_next_key: current_key,
            limit: MAX_ROUTES,
        }));
    }
    let key = next_route_key.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    node.routes[method_idx] = key + 1;
    record_route_worker_raw(side_table, key, worker_id);

    let current_mask = node.method_mask();
    node.set_method_mask(current_mask | (1 << method_idx));
    node.set_dirty(true);

    Ok(key)
}

fn assign_route_key_preassigned(
    side_table: &mut Vec<Option<u32>>,
    node: &mut RadixTreeNode,
    method: HttpMethod,
    assigned_key: u16,
    worker_id: WorkerId,
) -> RouterResult<u16> {
    let method_idx = method as usize;
    if node.routes[method_idx] != 0 {
        let existing_key = node.routes[method_idx] - 1;
        if side_table.get(existing_key as usize).and_then(|v| *v) == Some(worker_id) {
            return Err(RouterError::from(RadixError::DuplicateRoute {
                worker_id,
                existing_key,
            }));
        }
        return Ok(existing_key);
    }
    node.routes[method_idx] = assigned_key + 1;
    record_route_worker_raw(side_table, assigned_key, worker_id);
    node.set_dirty(true);
    Ok(assigned_key)
}

#[inline(always)]
fn record_route_worker_raw(
    side_table: &mut Vec<Option<u32>>,
    route_key: u16,
    worker_id: u32,
) -> bool {
    let idx = route_key as usize;
    let needed = idx + 1;
    if side_table.len() < needed {
        side_table.resize(needed, None);
    }
    if side_table[idx].is_none() {
        side_table[idx] = Some(worker_id);
        true
    } else {
        false
    }
}

// Helper for SegmentPart
impl SegmentPart {
    fn is_literal(&self) -> bool {
        matches!(self, SegmentPart::Literal(_))
    }
}

// Thread-safe standalone parser for bulk preprocess
pub(super) fn prepare_path_segments_standalone(path: &str) -> RouterResult<Vec<SegmentPattern>> {
    // Use the unified path validation function
    let norm = normalize_and_validate_path(path)?;
    if norm == "/" {
        return Ok(Vec::new());
    }

    let segments: Vec<&str> = norm.split('/').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return Err(RouterError::from(PathError::InvalidAfterNormalization {
            input: path.to_string(),
            normalized: norm,
        }));
    }

    let mut parsed_segments = Vec::with_capacity(segments.len());
    let mut seen_params = HashSet::new();

    for seg in segments {
        let pat = parse_segment(seg)?;

        let mut min_len = 0u16;
        let mut last_lit_len = 0u16;
        for part in pat.parts.iter() {
            if let SegmentPart::Literal(l) = part {
                min_len += l.len() as u16;
            }
        }
        if let Some(SegmentPart::Literal(l)) = pat.parts.iter().rev().find(|p| p.is_literal()) {
            last_lit_len = l.len() as u16;
        }

        if !PatternMeta::is_valid_length(min_len, last_lit_len) {
            return Err(RouterError::from(RadixError::PatternLengthExceeded {
                segment: seg.to_string(),
                path: path.to_string(),
                min_length: min_len,
                last_literal_length: last_lit_len,
            }));
        }

        for part in &pat.parts {
            if let SegmentPart::Param { name, .. } = part {
                let name_owned = name.clone();
                if !seen_params.insert(name_owned.clone()) {
                    return Err(RouterError::from(RadixError::DuplicateParamName {
                        param: name_owned,
                        path: path.to_string(),
                    }));
                }
            }
        }
        parsed_segments.push(pat);
    }
    Ok(parsed_segments)
}
