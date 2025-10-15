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
