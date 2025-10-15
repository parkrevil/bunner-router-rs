use crate::errors::RouterResult;
use crate::radix::RadixTree;
use crate::registry::RegistryMetrics;
use crate::router::RouterOptions;
use crate::types::{HttpMethod, WorkerId};

#[derive(Debug)]
pub struct RouteRegistry {
    tree: RadixTree,
    metrics: RegistryMetrics,
}

impl RouteRegistry {
    pub fn new(options: RouterOptions) -> Self {
        Self {
            tree: RadixTree::new(options),
            metrics: RegistryMetrics::default(),
        }
    }

    pub fn insert(
        &mut self,
        worker_id: WorkerId,
        method: HttpMethod,
        path: &str,
    ) -> RouterResult<u16> {
        let key = self.tree.insert(worker_id, method, path)?;
        self.metrics.record_insert();
        Ok(key)
    }

    pub fn insert_bulk<I>(&mut self, worker_id: WorkerId, entries: I) -> RouterResult<Vec<u16>>
    where
        I: IntoIterator<Item = (HttpMethod, String)>,
    {
        let out = self.tree.insert_bulk(worker_id, entries)?;
        self.metrics.record_bulk(out.len());
        Ok(out)
    }

    pub fn finalize(&mut self) {
        self.tree.finalize();
    }

    pub fn reset_after_seal(&mut self) {
        self.tree = RadixTree::new(RouterOptions::default());
        self.metrics = RegistryMetrics::default();
    }

    pub fn tree(&self) -> &RadixTree {
        &self.tree
    }

    pub fn tree_mut(&mut self) -> &mut RadixTree {
        &mut self.tree
    }

    pub fn metrics(&self) -> &RegistryMetrics {
        &self.metrics
    }
}
