use crate::enums::HttpMethod;
use crate::matcher::{find_route, with_param_buffer};
use crate::pattern::SegmentPattern;
use crate::radix::{HTTP_METHOD_COUNT, RadixTree};
use crate::router::{Preprocessor, Router};
use crate::types::RouteMatch;
use hashbrown::HashMap as FastHashMap;
use parking_lot::{RwLock, RwLockUpgradableReadGuard};
use regex::Regex;
use std::sync::Arc;

use super::cache::{CacheStats, DEFAULT_CACHE_CAPACITY, RouteCache, RouteCacheKey};
use super::converter::{copy_static_maps, extract_root};
use super::{ReadOnlyError, ReadOnlyResult};

#[derive(Debug)]
pub struct RouterReadOnly {
    pub(crate) static_maps: [FastHashMap<Box<str>, u16>; HTTP_METHOD_COUNT],
    pub(crate) root: ReadOnlyNode,
    preprocessor: Preprocessor,
    cache: Option<Arc<RwLock<RouteCache>>>,
    cache_stats: Option<Arc<CacheStats>>,
    debug: bool,
    param_pattern_default: Arc<Regex>,
}

impl RouterReadOnly {
    pub fn from_router(router: &Router) -> Self {
        router.with_registry(|registry| {
            let tree = registry.tree();
            let static_maps = copy_static_maps(tree);
            let root = extract_root(&tree.root_node);
            let preprocessor = tree.preprocessor.clone();
            let options = tree.options.clone();
            let cache = if options.cache_routes {
                Some(Arc::new(RwLock::new(RouteCache::new(
                    DEFAULT_CACHE_CAPACITY,
                ))))
            } else {
                None
            };
            let cache_stats = cache.as_ref().map(|_| Arc::new(CacheStats::default()));
            let debug = options.debug;
            let param_pattern_default = Arc::new(options.param_pattern_default_regex());

            RouterReadOnly {
                static_maps,
                root,
                preprocessor,
                cache,
                cache_stats,
                debug,
                param_pattern_default,
            }
        })
    }

    pub fn from_radix_tree(tree: &RadixTree) -> Self {
        let static_maps = copy_static_maps(tree);
        let root = extract_root(&tree.root_node);
        let preprocessor = tree.preprocessor.clone();
        let options = tree.options.clone();
        let cache = if options.cache_routes {
            Some(Arc::new(RwLock::new(RouteCache::new(
                DEFAULT_CACHE_CAPACITY,
            ))))
        } else {
            None
        };
        let cache_stats = cache.as_ref().map(|_| Arc::new(CacheStats::default()));
        let debug = options.debug;
        let param_pattern_default = Arc::new(options.param_pattern_default_regex());

        RouterReadOnly {
            static_maps,
            root,
            preprocessor,
            cache,
            cache_stats,
            debug,
            param_pattern_default,
        }
    }

    fn find_static_normalized(&self, method: HttpMethod, normalized: &str) -> Option<u16> {
        let idx = method as usize;
        self.static_maps[idx].get(normalized).cloned()
    }

    #[tracing::instrument(skip(self, path), fields(method=?method, path=%path))]
    pub fn find(&self, method: HttpMethod, path: &str) -> ReadOnlyResult<RouteMatch> {
        tracing::event!(tracing::Level::TRACE, operation="find", method=?method, path=%path);

        let outcome = self.preprocessor.apply(path).map_err(ReadOnlyError::from)?;
        let normalized = outcome.normalized();
        let cache_key = outcome.cache_key();
        let cache_lookup_key = self
            .cache
            .as_ref()
            .map(|_| RouteCacheKey::new(method, cache_key.to_string()));

        if let (Some(cache), Some(key)) = (self.cache.as_ref(), cache_lookup_key.as_ref()) {
            let guard = cache.upgradable_read();
            if let Some(hit) = guard.peek(key) {
                if let Some(stats) = &self.cache_stats {
                    stats.record_hit();
                }
                if self.debug {
                    tracing::event!(
                        tracing::Level::DEBUG,
                        cache = "hit",
                        method = ?method,
                        cache_key = %cache_key,
                        "router cache hit"
                    );
                }
                let mut guard = RwLockUpgradableReadGuard::upgrade(guard);
                guard.touch(key);
                return Ok(hit);
            } else {
                if let Some(stats) = &self.cache_stats {
                    stats.record_miss();
                }
                if self.debug {
                    tracing::event!(
                        tracing::Level::DEBUG,
                        cache = "miss",
                        method = ?method,
                        cache_key = %cache_key,
                        "router cache miss"
                    );
                }
            }
        }

        if let Some(route_key) = self.find_static_normalized(method, cache_key) {
            let result = (route_key, Vec::new());
            if let (Some(cache), Some(key)) = (self.cache.as_ref(), cache_lookup_key.as_ref()) {
                cache.write().insert(key.clone(), result.clone());
            }
            return Ok(result);
        }

        let found = with_param_buffer(|buf| {
            find_route(
                &self.root,
                method,
                normalized,
                buf,
                &self.param_pattern_default,
            )
        });

        if let Some((route_key, params)) = found {
            if let (Some(cache), Some(key)) = (self.cache.as_ref(), cache_lookup_key.as_ref()) {
                cache
                    .write()
                    .insert(key.clone(), (route_key, params.clone()));
            }
            Ok((route_key, params))
        } else {
            Err(ReadOnlyError::RouteNotFound {
                method,
                path: normalized.to_string(),
            })
        }
    }

    pub fn cache_metrics(&self) -> Option<(u64, u64)> {
        self.cache_stats.as_ref().map(|stats| stats.snapshot())
    }
}

impl Clone for RouterReadOnly {
    fn clone(&self) -> Self {
        Self {
            static_maps: self.static_maps.clone(),
            root: self.root.clone(),
            preprocessor: self.preprocessor.clone(),
            cache: self.cache.clone(),
            cache_stats: self.cache_stats.clone(),
            debug: self.debug,
            param_pattern_default: self.param_pattern_default.clone(),
        }
    }
}

impl Default for RouterReadOnly {
    fn default() -> Self {
        Self {
            static_maps: std::array::from_fn(|_| FastHashMap::default()),
            root: ReadOnlyNode::default(),
            preprocessor: Preprocessor::default(),
            cache: None,
            cache_stats: None,
            debug: false,
            param_pattern_default: Arc::new(
                Regex::new("^(?:[^/]+)$").expect("default param pattern should compile"),
            ),
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
