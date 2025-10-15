#[derive(Debug, Default, Clone)]
pub struct RegistryMetrics {
    pub total_routes_registered: usize,
}

impl RegistryMetrics {
    pub fn record_insert(&mut self) {
        self.total_routes_registered += 1;
    }

    pub fn record_bulk(&mut self, count: usize) {
        self.total_routes_registered += count;
    }
}
