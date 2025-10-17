use super::{ArenaHandle, MAX_ROUTES, RadixTree, RadixTreeNode, node::PatternMeta};
use crate::enums::HttpMethod;
use crate::path::PathError;
use crate::pattern::{
    SegmentPart, SegmentPattern, parse_segment, pattern_compatible_policy, pattern_is_pure_static,
    pattern_score,
};
use crate::radix::{RadixError, RadixResult};
use crate::router::{PreprocessOutcome, Preprocessor, RouterOptions};
use crate::tools::Interner;
use hashbrown::HashSet;
use std::sync::atomic::AtomicU16;

impl RadixTree {
    pub fn insert(&mut self, method: HttpMethod, path: &str) -> RadixResult<u16> {
        tracing::event!(tracing::Level::TRACE, operation="insert", method=?method, path=%path);
        if self.root_node.is_sealed() {
            return Err(RadixError::TreeSealed {
                operation: "insert",
                path: Some(path.to_string()),
            });
        }
        self.root_node.set_dirty(true);

        let (_outcome, parsed_segments, _) = preprocess_and_parse(path, &self.preprocessor)?;
        self.insert_parsed(method, parsed_segments)
    }

    pub(super) fn insert_parsed(
        &mut self,
        method: HttpMethod,
        parsed_segments: Vec<SegmentPattern>,
    ) -> RadixResult<u16> {
        tracing::event!(tracing::Level::TRACE, operation="insert_parsed", method=?method, segments=parsed_segments.len() as u64);
        if self.root_node.is_sealed() {
            return Err(RadixError::TreeSealed {
                operation: "insert_parsed",
                path: None,
            });
        }
        self.root_node.set_dirty(true);

        let mut parsed_segments = parsed_segments;
        self.hydrate_constraints(&mut parsed_segments)?;

        let mut current = &mut self.root_node;
        let arena = self.arena_handle.clone();

        for (i, pat) in parsed_segments.iter().enumerate() {
            // Fast check without allocation: single literal '*' means wildcard
            let is_wildcard =
                matches!(pat.parts.as_slice(), [SegmentPart::Literal(s)] if s.as_str() == "*");
            if is_wildcard {
                let key = handle_wildcard_insert(
                    current,
                    method,
                    i,
                    parsed_segments.len(),
                    &self.next_route_key,
                )?;
                return Ok(key);
            }

            // Detect pure static without building a joined string
            if pat.parts.len() == 1 {
                if let SegmentPart::Literal(lit) = &pat.parts[0] {
                    current =
                        current.descend_static_mut_with_alloc(lit.as_str(), || arena.alloc_node());
                    sort_static_children(current, &self.interner);
                } else {
                    current = find_or_create_pattern_child(current, pat, &arena)?;
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
                current =
                    current.descend_static_mut_with_alloc(joined.as_str(), || arena.alloc_node());
                sort_static_children(current, &self.interner);
            } else {
                current = find_or_create_pattern_child(current, pat, &arena)?;
            }

            // method mask is delayed to finalize()
            current.set_dirty(true);
        }

        let key = assign_route_key(current, method, &self.next_route_key)?;
        Ok(key)
    }

    pub(super) fn insert_parsed_preassigned(
        &mut self,
        method: HttpMethod,
        parsed_segments: Vec<SegmentPattern>,
        assigned_key: u16,
    ) -> RadixResult<u16> {
        tracing::event!(tracing::Level::TRACE, operation="insert_parsed_preassigned", method=?method, segments=parsed_segments.len() as u64, assigned_key=assigned_key as u64);
        if self.root_node.is_sealed() {
            return Err(RadixError::TreeSealed {
                operation: "insert_parsed_preassigned",
                path: None,
            });
        }
        self.root_node.set_dirty(true);

        let mut parsed_segments = parsed_segments;
        self.hydrate_constraints(&mut parsed_segments)?;

        let mut current = &mut self.root_node;
        let arena = self.arena_handle.clone();

        for (i, pat) in parsed_segments.iter().enumerate() {
            let is_wildcard =
                matches!(pat.parts.as_slice(), [SegmentPart::Literal(s)] if s.as_str() == "*");
            if is_wildcard {
                return handle_wildcard_insert_preassigned(
                    current,
                    method,
                    i,
                    parsed_segments.len(),
                    assigned_key,
                );
            }

            if pat.parts.len() == 1 {
                if let SegmentPart::Literal(lit) = &pat.parts[0] {
                    current =
                        current.descend_static_mut_with_alloc(&lit.clone(), || arena.alloc_node());
                    sort_static_children(current, &self.interner);
                } else {
                    current = find_or_create_pattern_child(current, pat, &arena)?;
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
                current = current.descend_static_mut_with_alloc(&joined, || arena.alloc_node());
                sort_static_children(current, &self.interner);
            } else {
                current = find_or_create_pattern_child(current, pat, &arena)?;
            }
            // Do not set method_mask here; delayed to finalize for bulk path
            current.set_dirty(true);
        }

        let key = assign_route_key_preassigned(current, method, assigned_key)?;
        Ok(key)
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
    arena: &ArenaHandle,
) -> RadixResult<&'a mut RadixTreeNode> {
    for exist in node.patterns.iter() {
        if !pattern_compatible_policy(exist, pat) {
            return Err(RadixError::ParamNameConflict {
                pattern: format!("{:?}", pat),
            });
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
    node.pattern_nodes.insert(insert_pos, arena.alloc_node());

    Ok(node.pattern_nodes.get_mut(insert_pos).unwrap().as_mut())
}

fn handle_wildcard_insert(
    node: &mut RadixTreeNode,
    method: HttpMethod,
    index: usize,
    total_segments: usize,
    next_route_key: &AtomicU16,
) -> RadixResult<u16> {
    if index != total_segments - 1 {
        return Err(RadixError::WildcardMustBeTerminal {
            segment_index: index,
            total_segments,
        });
    }
    let method_idx = method as usize;
    if node.wildcard_routes[method_idx] != 0 {
        let existing_key = node.wildcard_routes[method_idx] - 1;
        return Err(RadixError::DuplicateWildcardRoute {
            method,
            existing_key,
        });
    }
    let key = next_route_key.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    node.wildcard_routes[method_idx] = key + 1;

    node.set_dirty(true);

    Ok(key)
}

fn handle_wildcard_insert_preassigned(
    node: &mut RadixTreeNode,
    method: HttpMethod,
    index: usize,
    total_segments: usize,
    assigned_key: u16,
) -> RadixResult<u16> {
    if index != total_segments - 1 {
        return Err(RadixError::WildcardMustBeTerminal {
            segment_index: index,
            total_segments,
        });
    }
    let method_idx = method as usize;
    if node.wildcard_routes[method_idx] != 0 {
        let existing_key = node.wildcard_routes[method_idx] - 1;
        return Err(RadixError::DuplicateWildcardRoute {
            method,
            existing_key,
        });
    }
    node.wildcard_routes[method_idx] = assigned_key + 1;
    node.set_dirty(true);
    Ok(assigned_key)
}

fn assign_route_key(
    node: &mut RadixTreeNode,
    method: HttpMethod,
    next_route_key: &AtomicU16,
) -> RadixResult<u16> {
    let method_idx = method as usize;
    if node.routes[method_idx] != 0 {
        let existing_key = node.routes[method_idx] - 1;
        return Err(RadixError::DuplicateRoute {
            method,
            existing_key,
        });
    }
    let current_key = next_route_key.load(std::sync::atomic::Ordering::Relaxed);
    if current_key == MAX_ROUTES {
        return Err(RadixError::MaxRoutesExceeded {
            requested: None,
            current_next_key: current_key,
            limit: MAX_ROUTES,
        });
    }
    let key = next_route_key.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    node.routes[method_idx] = key + 1;

    let current_mask = node.method_mask();
    node.set_method_mask(current_mask | (1 << method_idx));
    node.set_dirty(true);

    Ok(key)
}

fn assign_route_key_preassigned(
    node: &mut RadixTreeNode,
    method: HttpMethod,
    assigned_key: u16,
) -> RadixResult<u16> {
    let method_idx = method as usize;
    if node.routes[method_idx] != 0 {
        let existing_key = node.routes[method_idx] - 1;
        return Err(RadixError::DuplicateRoute {
            method,
            existing_key,
        });
    }
    node.routes[method_idx] = assigned_key + 1;
    node.set_dirty(true);
    Ok(assigned_key)
}

// Helper for SegmentPart
impl SegmentPart {
    fn is_literal(&self) -> bool {
        matches!(self, SegmentPart::Literal(_))
    }
}

pub(super) fn preprocess_and_parse(
    path: &str,
    preprocessor: &Preprocessor,
) -> RadixResult<(PreprocessOutcome, Vec<SegmentPattern>, Vec<String>)> {
    let outcome = preprocessor.apply(path)?;
    let segments = parse_segments(&outcome, preprocessor.config())?;
    let literals = collect_literals(&segments);
    Ok((outcome, segments, literals))
}

fn parse_segments(
    outcome: &PreprocessOutcome,
    config: &RouterOptions,
) -> RadixResult<Vec<SegmentPattern>> {
    let normalized = outcome.normalized();
    if normalized == "/" {
        return Ok(Vec::new());
    }

    let parts: Vec<&str> = normalized.split('/').collect();
    let total_parts = parts.len();
    let leading_slash = normalized.starts_with('/');

    let mut segments: Vec<&str> = Vec::new();
    for (idx, part) in parts.into_iter().enumerate() {
        let is_first = idx == 0;
        let is_last = idx == total_parts - 1;

        if is_first && leading_slash && part.is_empty() {
            continue;
        }

        if part.is_empty() {
            let keep = if is_last {
                config.strict_trailing_slash
            } else {
                config.allow_duplicate_slash
            };

            if !keep {
                continue;
            }
        }

        segments.push(part);
    }

    if segments.is_empty() {
        return Err(PathError::InvalidAfterNormalization {
            input: outcome.original().to_string(),
            normalized: normalized.to_string(),
        }
        .into());
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
            return Err(RadixError::PatternLengthExceeded {
                segment: seg.to_string(),
                path: outcome.original().to_string(),
                min_length: min_len,
                last_literal_length: last_lit_len,
            });
        }

        for part in &pat.parts {
            if let SegmentPart::Param { name, .. } = part {
                let name_owned = name.clone();
                if !seen_params.insert(name_owned.clone()) {
                    return Err(RadixError::DuplicateParamName {
                        param: name_owned,
                        path: outcome.original().to_string(),
                    });
                }
            }
        }
        parsed_segments.push(pat);
    }
    Ok(parsed_segments)
}

fn collect_literals(segments: &[SegmentPattern]) -> Vec<String> {
    let mut literals = Vec::new();
    for pat in segments.iter() {
        for part in pat.parts.iter() {
            if let SegmentPart::Literal(l) = part {
                literals.push(l.clone());
            }
        }
    }
    literals
}

pub(super) fn first_non_slash_byte(path: &str) -> u8 {
    path.as_bytes()
        .iter()
        .copied()
        .find(|b| *b != b'/')
        .unwrap_or(0)
}

pub(super) fn infer_static_guess(path: &str) -> bool {
    !path.contains(':') && !path.contains("/*") && !path.ends_with('*')
}
