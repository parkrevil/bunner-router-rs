use crate::errors::{RouterError, RouterResult};
use crate::matcher::{find_route, with_param_buffer};
use crate::path::normalize_and_validate_path;
use crate::pattern::SegmentPattern;
use crate::radix::{HTTP_METHOD_COUNT, RadixTree};
use crate::readonly::ReadOnlyError;
use crate::router::Router;
use crate::types::{HttpMethod, RouteMatch};
use hashbrown::HashMap as FastHashMap;

use super::converter::{copy_static_maps, extract_root};

#[derive(Debug, Clone, Default)]
pub struct RouterReadOnly {
    pub(crate) static_maps: [FastHashMap<Box<str>, u16>; HTTP_METHOD_COUNT],
    pub(crate) root: ReadOnlyNode,
}

impl RouterReadOnly {
    pub fn from_router(router: &Router) -> Self {
        router.with_registry(|registry| {
            let tree = registry.tree();
            let static_maps = copy_static_maps(tree);
            let root = extract_root(&tree.root_node);

            RouterReadOnly { static_maps, root }
        })
    }

    pub fn from_radix_tree(tree: &RadixTree) -> Self {
        let static_maps = copy_static_maps(tree);
        let root = extract_root(&tree.root_node);

        RouterReadOnly { static_maps, root }
    }

    fn find_static_normalized(&self, method: HttpMethod, normalized: &str) -> Option<u16> {
        let idx = method as usize;
        self.static_maps[idx].get(normalized).cloned()
    }

    #[tracing::instrument(skip(self, path), fields(method=?method, path=%path))]
    pub fn find(&self, method: HttpMethod, path: &str) -> RouterResult<RouteMatch> {
        tracing::event!(tracing::Level::TRACE, operation="find", method=?method, path=%path);

        let normalized = normalize_and_validate_path(path)?;

        if let Some(route_key) = self.find_static_normalized(method, &normalized) {
            return Ok((route_key, Vec::new()));
        }

        let found = with_param_buffer(|buf| find_route(&self.root, method, &normalized, buf));

        if let Some((route_key, params)) = found {
            Ok((route_key, params))
        } else {
            Err(RouterError::from(ReadOnlyError::RouteNotFound {
                method,
                path: normalized,
            }))
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ReadOnlyNode {
    pub(crate) fused_edge: Option<Box<str>>,
    pub(crate) fused_child: Option<Box<ReadOnlyNode>>,
    pub(crate) routes: [u16; HTTP_METHOD_COUNT],
    pub(crate) wildcard_routes: [u16; HTTP_METHOD_COUNT],
    pub(crate) static_children: FastHashMap<Box<str>, ReadOnlyNode>,
    pub(crate) patterns: Vec<(SegmentPattern, ReadOnlyNode)>,
}
