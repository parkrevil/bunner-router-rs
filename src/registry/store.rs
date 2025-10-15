use crate::enums::HttpMethod;
use crate::radix::{RadixResult, RadixTree};
use crate::registry::RegistryMetrics;
use crate::router::RouterOptions;

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

    pub fn insert(&mut self, method: HttpMethod, path: &str) -> RadixResult<u16> {
        let key = self.tree.insert(method, path)?;
        self.metrics.record_insert();
        Ok(key)
    }

    pub fn insert_bulk(&mut self, entries: Vec<(HttpMethod, String)>) -> RadixResult<Vec<u16>> {
        let out = self.tree.insert_bulk(entries)?;
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
