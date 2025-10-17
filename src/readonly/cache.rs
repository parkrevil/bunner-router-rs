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

    pub fn peek(&self, key: &RouteCacheKey) -> Option<RouteMatch> {
        self.map.get(key).map(|value| value.result.clone())
    }

    pub fn touch(&mut self, key: &RouteCacheKey) {
        if self.map.contains_key(key) {
            self.promote(key);
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RouteParams;

    fn sample_match() -> RouteMatch {
        (42, RouteParams::new())
    }

    #[test]
    fn peek_returns_value_without_changing_order() {
        let mut cache = RouteCache::new(4);
        let key = RouteCacheKey::new(HttpMethod::Get, "/peek".to_string());
        cache.insert(key.clone(), sample_match());

        let front_before = cache.order.front().cloned();
        let result = cache.peek(&key);
        let front_after = cache.order.front().cloned();

        assert_eq!(result, Some(sample_match()));
        assert_eq!(front_before, front_after);
        assert_eq!(front_after, Some(key));
    }

    #[test]
    fn touch_promotes_entry_to_front() {
        let mut cache = RouteCache::new(4);
        let first = RouteCacheKey::new(HttpMethod::Get, "/first".to_string());
        let second = RouteCacheKey::new(HttpMethod::Get, "/second".to_string());
        cache.insert(first.clone(), sample_match());
        cache.insert(second.clone(), sample_match());

        assert_eq!(cache.order.front(), Some(&second));
        cache.touch(&first);
        assert_eq!(cache.order.front(), Some(&first));
    }
}
