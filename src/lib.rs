pub mod enums;
pub mod errors;
mod interner;
mod path;
mod pattern;
pub mod radix_tree;
pub mod readonly;
pub mod structures;
pub mod types;

use crate::enums::HttpMethod;
use crate::types::WorkerId;

pub use errors::RouterErrorCode;
pub use readonly::RouterReadOnly;
pub use structures::RouterError;
pub use types::RouteMatch;

use parking_lot::RwLock;
use std::sync::Arc;
use structures::RouterResult;

#[derive(Debug)]
struct RouterInner {
    pub radix_tree: radix_tree::RadixTree,
    pub ro: std::sync::OnceLock<Arc<RouterReadOnly>>,
}

#[derive(Debug)]
pub struct Router {
    inner: RwLock<RouterInner>,
}

impl Router {
    pub fn new(options: Option<RouterOptions>) -> Self {
        Self {
            inner: RwLock::new(RouterInner {
                radix_tree: radix_tree::RadixTree::new(options.unwrap_or_default()),
                ro: std::sync::OnceLock::new(),
            }),
        }
    }

    pub fn add(&self, worker_id: WorkerId, method: HttpMethod, path: &str) -> RouterResult<u16> {
        let mut g = self.inner.write();

        if g.ro.get().is_some() {
            let detail = serde_json::json!({ "path": path });

            return Err(Box::new(RouterError::new(
                RouterErrorCode::AlreadySealed,
                "router",
                "add",
                "validation",
                "Router is sealed; cannot insert routes".to_string(),
                Some(detail),
            )));
        }

        g.radix_tree.insert(worker_id, method, path)
    }

    pub fn add_bulk<I>(&self, worker_id: WorkerId, entries: I) -> RouterResult<Vec<u16>>
    where
        I: IntoIterator<Item = (HttpMethod, String)>,
    {
        let mut g = self.inner.write();

        if g.ro.get().is_some() {
            // we can't know the exact count without consuming the iterator; collect temporarily
            let entries_vec: Vec<(HttpMethod, String)> = entries.into_iter().collect();
            let cnt = entries_vec.len();
            let detail = serde_json::json!({ "count": cnt });

            return Err(Box::new(RouterError::new(
                RouterErrorCode::AlreadySealed,
                "router",
                "add_bulk",
                "validation",
                "Router is sealed; cannot insert bulk routes".to_string(),
                Some(detail),
            )));
        }

        g.radix_tree.insert_bulk(worker_id, entries)
    }

    pub fn seal(&self) {
        let mut g = self.inner.write();

        // finalize and build readonly snapshot from the radix tree
        g.radix_tree.finalize();
        let ro = RouterReadOnly::from_radix_tree(&g.radix_tree);
        let arc = Arc::new(ro.clone());

        // replace radix_tree with a fresh one and set readonly snapshot on the inner
        g.radix_tree = radix_tree::RadixTree::new(Default::default());
        let _ = g.ro.set(arc);
    }

    pub fn find(&self, method: HttpMethod, path: &str) -> RouterResult<RouteMatch> {
        let g = self.inner.read();

        match g.ro.get() {
            Some(ro) => ro.find(method, path),
            None => Err(Box::new(RouterError::new(
                RouterErrorCode::NotSealed,
                "router",
                "find",
                "validation",
                "Router is not sealed; cannot perform find".to_string(),
                None,
            ))),
        }
    }

    pub fn get_readonly(&self) -> RouterResult<Arc<RouterReadOnly>> {
        let g = self.inner.read();

        match g.ro.get() {
            Some(ro) => Ok(ro.clone()),
            None => Err(Box::new(RouterError::new(
                RouterErrorCode::NotSealed,
                "router",
                "get_readonly",
                "validation",
                "Router is not sealed; cannot get readonly snapshot".to_string(),
                None,
            ))),
        }
    }
}

// snapshot removed in favor of RouterReadOnly

#[derive(Debug, Clone, Copy)]
pub struct RouterOptions {
    pub enable_root_level_pruning: bool,
    pub enable_static_route_full_mapping: bool,
    pub enable_automatic_optimization: bool,
}

impl Default for RouterOptions {
    fn default() -> Self {
        Self {
            enable_root_level_pruning: false,
            enable_static_route_full_mapping: false,
            enable_automatic_optimization: true,
        }
    }
}
