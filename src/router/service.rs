use crate::errors::{RouterError, RouterResult};
use crate::readonly::RouterReadOnly;
use crate::registry::RouteRegistry;
use crate::router::RouterOptions;
use crate::types::{HttpMethod, RouteMatch, WorkerId};
use parking_lot::RwLock;
use std::sync::Arc;
use std::sync::OnceLock;

#[derive(Debug)]
struct RouterState {
    registry: RouteRegistry,
    readonly: OnceLock<Arc<RouterReadOnly>>,
}

impl RouterState {
    fn new(options: RouterOptions) -> Self {
        Self {
            registry: RouteRegistry::new(options),
            readonly: OnceLock::new(),
        }
    }
}

#[derive(Debug)]
pub struct Router {
    inner: RwLock<RouterState>,
}

impl Router {
    pub fn new(options: Option<RouterOptions>) -> Self {
        let state = RouterState::new(options.unwrap_or_default());
        Self {
            inner: RwLock::new(state),
        }
    }

    pub fn add(&self, worker_id: WorkerId, method: HttpMethod, path: &str) -> RouterResult<u16> {
        let mut guard = self.inner.write();

        if guard.readonly.get().is_some() {
            return Err(RouterError::AddWhileSealed {
                path: path.to_string(),
            });
        }

        guard.registry.insert(worker_id, method, path)
    }

    pub fn add_bulk<I>(&self, worker_id: WorkerId, entries: I) -> RouterResult<Vec<u16>>
    where
        I: IntoIterator<Item = (HttpMethod, String)>,
    {
        let mut guard = self.inner.write();

        if guard.readonly.get().is_some() {
            let entries_vec: Vec<(HttpMethod, String)> = entries.into_iter().collect();
            return Err(RouterError::BulkAddWhileSealed {
                count: entries_vec.len(),
            });
        }

        guard.registry.insert_bulk(worker_id, entries)
    }

    pub fn seal(&self) {
        let mut guard = self.inner.write();

        guard.registry.finalize();
        let snapshot = RouterReadOnly::from_radix_tree(guard.registry.tree());
        let arc = Arc::new(snapshot);
        guard.registry.reset_after_seal();
        let _ = guard.readonly.set(arc);
    }

    pub fn find(&self, method: HttpMethod, path: &str) -> RouterResult<RouteMatch> {
        let guard = self.inner.read();

        match guard.readonly.get() {
            Some(ro) => ro.find(method, path),
            None => Err(RouterError::FindWhileMutable),
        }
    }

    pub fn get_readonly(&self) -> RouterResult<Arc<RouterReadOnly>> {
        let guard = self.inner.read();

        match guard.readonly.get() {
            Some(ro) => Ok(ro.clone()),
            None => Err(RouterError::ReadOnlyUnavailable),
        }
    }

    pub(crate) fn with_registry<R>(&self, f: impl FnOnce(&RouteRegistry) -> R) -> R {
        let guard = self.inner.read();
        f(&guard.registry)
    }
}
