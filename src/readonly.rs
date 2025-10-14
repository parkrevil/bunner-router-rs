use super::RouteMatch;
use super::errors::RouterErrorCode;
use super::path::normalize_and_validate_path;
use super::structures::{RouterError, RouterResult};
use super::{Router, radix_tree::HTTP_METHOD_COUNT};

use crate::enums::HttpMethod;
use crate::pattern::{self, SegmentPattern};
use crate::radix_tree::RadixTree;

use hashbrown::HashMap as FastHashMap;
use std::cell::RefCell;

thread_local! {
    static PARAM_BUF: RefCell<Vec<(String, (usize, usize))>> = RefCell::new(Vec::with_capacity(4));
}

#[derive(Debug, Clone, Default)]
pub struct RouterReadOnly {
    static_maps: [FastHashMap<Box<str>, u16>; HTTP_METHOD_COUNT],
    root: ReadOnlyNode,
}

impl RouterReadOnly {
    /// Read-only router built for lock-free concurrent lookups.
    ///
    /// Safety & Concurrency:
    /// - Contains only immutable owned data structures
    /// - No interior mutability
    /// - Safe to share across threads (`Send + Sync` by construction)
    pub fn from_router(router: &Router) -> Self {
        let guard = router.inner.read();

        let mut maps: [FastHashMap<Box<str>, u16>; HTTP_METHOD_COUNT] = Default::default();
        for (i, out_map) in maps.iter_mut().enumerate().take(HTTP_METHOD_COUNT) {
            *out_map = guard.radix_tree.static_route_full_mapping[i].clone();
        }

        let root = ReadOnlyNode::from_node(&guard.radix_tree.root_node);

        RouterReadOnly {
            static_maps: maps,
            root,
        }
    }

    /// Build a read-only snapshot directly from a radix tree reference.
    pub fn from_radix_tree(tree: &RadixTree) -> Self {
        let mut maps: [FastHashMap<Box<str>, u16>; HTTP_METHOD_COUNT] = Default::default();
        for (i, out_map) in maps.iter_mut().enumerate().take(HTTP_METHOD_COUNT) {
            *out_map = tree.static_route_full_mapping[i].clone();
        }

        let root = ReadOnlyNode::from_node(&tree.root_node);

        RouterReadOnly {
            static_maps: maps,
            root,
        }
    }

    #[inline]
    fn find_static_normalized(&self, method: HttpMethod, normalized: &str) -> Option<u16> {
        let idx = method as usize;
        self.static_maps[idx].get(normalized).cloned()
    }

    #[tracing::instrument(skip(self, path), fields(method=?method, path=%path))]
    pub fn find(&self, method: HttpMethod, path: &str) -> RouterResult<RouteMatch> {
        tracing::event!(tracing::Level::TRACE, operation="find", method=?method, path=%path);

        let normalized = match normalize_and_validate_path(path) {
            Ok(p) => p,
            Err(err_box) => {
                // Unbox so we can mutate context fields, then re-box for the RouterResult
                let mut err = *err_box;
                // Update the error context for route matching operation
                err.stage = "route_matching".to_string();
                err.cause = "routing".to_string();
                if let Some(ref mut extra) = err.extra
                    && let Some(obj) = extra.as_object_mut()
                {
                    obj.insert("method".to_string(), serde_json::json!(method as u8));
                    obj.insert("operation".to_string(), serde_json::json!("route_matching"));
                }
                return Err(Box::new(err));
            }
        };

        if let Some(k) = self.find_static_normalized(method, &normalized) {
            return Ok((k, Vec::new()));
        }

        let found = PARAM_BUF.with(|cell| {
            let mut buf = cell.borrow_mut();
            buf.clear();
            self.root.find_from(method, &normalized, 0, &mut buf)
        });

        if let Some((rk, params)) = found {
            // Clone to return an owned Vec while retaining buffer capacity in TLS
            Ok((rk, params.clone()))
        } else {
            Err(Box::new(RouterError::new(
                RouterErrorCode::PathNotFound,
                "router",
                "route_matching",
                "routing",
                "No route matched for given method and path".to_string(),
                Some(serde_json::json!({
                    "path": normalized,
                    "method": method as u8,
                    "operation": "route_lookup"
                })),
            )))
        }
    }
}

#[derive(Debug, Clone, Default)]
struct ReadOnlyNode {
    fused_edge: Option<Box<str>>,
    routes: [u16; HTTP_METHOD_COUNT],
    wildcard_routes: [u16; HTTP_METHOD_COUNT],
    static_children: FastHashMap<Box<str>, ReadOnlyNode>,
    patterns: Vec<(SegmentPattern, ReadOnlyNode)>,
}

impl ReadOnlyNode {
    fn from_node(n: &super::radix_tree::node::RadixTreeNode) -> Self {
        // Build static_children from any of the available indexed views
        let mut static_children: FastHashMap<Box<str>, ReadOnlyNode> = FastHashMap::new();
        if !n.static_keys.is_empty() && n.static_vals_idx.len() == n.static_keys.len() {
            for (i, key) in n.static_keys.iter().enumerate() {
                let child = n.static_vals_idx[i].as_ref();
                static_children.insert(key.clone(), ReadOnlyNode::from_node(child));
            }
        } else if !n.static_children_idx.is_empty() {
            for (k, v) in n.static_children_idx.iter() {
                static_children.insert(k.clone(), ReadOnlyNode::from_node(v.as_ref()));
            }
        } else {
            for (k, v) in n.static_children.iter() {
                static_children.insert(k.clone(), ReadOnlyNode::from_node(v.as_ref()));
            }
            for (i, key) in n.static_keys.iter().enumerate() {
                static_children.insert(
                    key.clone(),
                    ReadOnlyNode::from_node(n.static_vals[i].as_ref()),
                );
            }
        }

        // Patterns
        let mut patterns: Vec<(SegmentPattern, ReadOnlyNode)> =
            Vec::with_capacity(n.patterns.len());
        for (i, pat) in n.patterns.iter().enumerate() {
            let child = n.pattern_nodes[i].as_ref();
            patterns.push((pat.clone(), ReadOnlyNode::from_node(child)));
        }

        // Fused child
        let mut ro = ReadOnlyNode {
            fused_edge: n.fused_edge.as_ref().map(|s| s.clone().into_boxed_str()),
            routes: n.routes,
            wildcard_routes: n.wildcard_routes,
            static_children,
            patterns,
        };

        if let Some(fc) = n.fused_child.as_ref() {
            let fc_node = ReadOnlyNode::from_node(fc.as_ref());
            // Represent fused child as a single static child with empty key when fused_edge is set
            // so that traversal handles it uniformly.
            ro.static_children.insert(Box::<str>::from(""), fc_node);
        }

        ro
    }

    fn skip_slashes(s: &str, mut i: usize) -> usize {
        let bs = s.as_bytes();
        if i < bs.len() && bs[i] == b'/' {
            i += 1;
        }
        i
    }

    fn handle_end(
        &self,
        method: HttpMethod,
        params: &mut [(String, (usize, usize))],
    ) -> Option<RouteMatch> {
        let idx = method as usize;
        let rk = self.routes[idx];
        if rk != 0 {
            return Some((rk - 1, params.to_owned()));
        }
        let wrk = self.wildcard_routes[idx];
        if wrk != 0 {
            return Some((wrk - 1, params.to_owned()));
        }
        None
    }

    #[tracing::instrument(level = "trace", skip(self, s, params), fields(i=i as u64))]
    fn find_from(
        &self,
        method: HttpMethod,
        s: &str,
        i: usize,
        params: &mut Vec<(String, (usize, usize))>,
    ) -> Option<RouteMatch> {
        let current_i = Self::skip_slashes(s, i);

        if let Some(edge) = &self.fused_edge {
            let rem = &s[current_i..];
            if !rem.starts_with(edge.as_ref()) {
                return None;
            }
            // descend to fused child stored under empty key
            if let Some(child) = self.static_children.get("") {
                return child.find_from(method, s, current_i + edge.len(), params);
            }
            return None;
        }

        if current_i >= s.len() {
            return self.handle_end(method, params);
        }

        let start = current_i;
        let next_slash = s.as_bytes()[start..]
            .iter()
            .position(|&b| b == b'/')
            .map_or(s.len(), |pos| start + pos);
        let seg = &s[start..next_slash];

        if let Some(next_node) = self.static_children.get(seg)
            && let Some(ok) = next_node.find_from(method, s, next_slash, params)
        {
            return Some(ok);
        }

        // Pattern children
        for (pat, child) in self.patterns.iter() {
            if let Some(kvs) = pattern::match_segment(seg, seg, pat) {
                let checkpoint = params.len();
                for (name, (off, len)) in kvs.into_iter() {
                    let abs = start + off;
                    if abs + len <= s.len() {
                        params.push((name, (abs, len)));
                    }
                }
                if let Some(ok) = child.find_from(method, s, next_slash, params) {
                    return Some(ok);
                }
                params.truncate(checkpoint);
            }
        }

        // Wildcard
        let wrk = self.wildcard_routes[method as usize];
        if wrk != 0 {
            let mut cap_start = start;
            if cap_start < s.len() && s.as_bytes()[cap_start] == b'/' {
                cap_start += 1;
            }
            if cap_start <= s.len() {
                let rest_len = s.len().saturating_sub(cap_start);
                if rest_len > 0 {
                    params.push(("*".to_string(), (cap_start, rest_len)));
                }
            }
            return Some((wrk - 1, params.clone()));
        }

        None
    }
}
