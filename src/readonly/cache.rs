use crate::enums::HttpMethod;
use crate::types::RouteMatch;
use hashbrown::HashMap as FastHashMap;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};

pub const DEFAULT_CACHE_CAPACITY: usize = 256;

#[derive(Debug)]
pub struct RouteCache {
    capacity: usize,
    map: FastHashMap<RouteCacheKey, RouteCacheValue>,
    order: VecDeque<RouteCacheKey>,
}

impl RouteCache {
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.max(1);
        Self {
            capacity: cap,
            map: FastHashMap::with_capacity(cap),
            order: VecDeque::with_capacity(cap),
        }
    }

    pub fn get(&mut self, key: &RouteCacheKey) -> Option<RouteMatch> {
        let result = self.map.get(key).map(|value| value.result.clone());
        if result.is_some() {
            self.promote(key);
        }
        result
    }

    pub fn insert(&mut self, key: RouteCacheKey, result: RouteMatch) {
        if self.map.contains_key(&key) {
            if let Some(entry) = self.map.get_mut(&key) {
                entry.result = result;
            }
            self.promote(&key);
            return;
        }

        if self.order.len() == self.capacity
            && let Some(oldest) = self.order.pop_back()
        {
            self.map.remove(&oldest);
        }

        self.order.push_front(key.clone());
        self.map.insert(key, RouteCacheValue { result });
    }

    fn promote(&mut self, key: &RouteCacheKey) {
        if let Some(pos) = self.order.iter().position(|existing| existing == key) {
            self.order.remove(pos);
        }
        self.order.push_front(key.clone());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RouteCacheKey {
    method: HttpMethod,
    path: String,
}

impl RouteCacheKey {
    pub fn new(method: HttpMethod, path: String) -> Self {
        Self { method, path }
    }
}

#[derive(Debug, Clone)]
struct RouteCacheValue {
    result: RouteMatch,
}

#[derive(Debug, Default)]
pub struct CacheStats {
    hits: AtomicU64,
    misses: AtomicU64,
}

impl CacheStats {
    pub fn record_hit(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> (u64, u64) {
        (
            self.hits.load(Ordering::Relaxed),
            self.misses.load(Ordering::Relaxed),
        )
    }
}
