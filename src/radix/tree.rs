use bumpalo::Bump;
use hashbrown::HashMap as FastHashMap;
use hashbrown::HashSet as FastHashSet;
use regex::Regex;

use super::{ArenaHandle, RadixError, RadixResult};
use crate::enums::HttpMethod;
use crate::pattern::{PatternError, SegmentPart, SegmentPattern};
use crate::radix::insert::{first_non_slash_byte, infer_static_guess, preprocess_and_parse};
use crate::router::{Preprocessor, RouterOptions};
use crate::tools::Interner;
use std::rc::Rc;
use std::sync::Arc;

pub const HTTP_METHOD_COUNT: usize = 7;

// Fixed maximum routes across all builds for predictable memory layout
pub const MAX_ROUTES: u16 = 65_535;

pub(crate) const STATIC_MAP_THRESHOLD: usize = 50;

type ParsedEntry = (
    usize,
    HttpMethod,
    Vec<SegmentPattern>,
    u8,
    usize,
    bool,
    Vec<String>,
);

#[derive(Debug)]
pub struct RadixTree {
    pub(crate) root_node: super::node::RadixTreeNode,
    pub(crate) options: RouterOptions,
    pub(crate) preprocessor: Preprocessor,
    pub(crate) arena_handle: ArenaHandle,
    pub(crate) interner: Interner,
    pub(crate) method_first_byte_bitmaps: [[u64; 4]; HTTP_METHOD_COUNT],
    pub(crate) root_parameter_first_present: [bool; HTTP_METHOD_COUNT],
    pub(crate) root_wildcard_present: [bool; HTTP_METHOD_COUNT],
    pub(crate) static_route_full_mapping: [FastHashMap<Box<str>, u16>; HTTP_METHOD_COUNT],
    pub(crate) method_length_buckets: [u64; HTTP_METHOD_COUNT],
    pub(crate) constraint_regex_cache: FastHashMap<Box<str>, Arc<Regex>>,
    pub enable_root_level_pruning: bool,
    pub enable_static_route_full_mapping: bool,
    pub(crate) next_route_key: std::sync::atomic::AtomicU16,
}

impl RadixTree {
    pub fn new(configuration: RouterOptions) -> Self {
        let enable_root_level_pruning = configuration.tuning.enable_root_level_pruning;
        let enable_static_route_full_mapping =
            configuration.tuning.enable_static_route_full_mapping;
        let preprocessor = Preprocessor::new(configuration.clone());
        let arena = Rc::new(Bump::with_capacity(128 * 1024));
        let arena_handle = ArenaHandle::new(arena);

        Self {
            root_node: super::node::RadixTreeNode::default(),
            options: configuration,
            preprocessor,
            arena_handle,
            interner: Interner::new(),
            method_first_byte_bitmaps: [[0; 4]; HTTP_METHOD_COUNT],
            root_parameter_first_present: [false; HTTP_METHOD_COUNT],
            root_wildcard_present: [false; HTTP_METHOD_COUNT],
            static_route_full_mapping: Default::default(),
            method_length_buckets: [0; HTTP_METHOD_COUNT],
            constraint_regex_cache: FastHashMap::new(),
            enable_root_level_pruning,
            enable_static_route_full_mapping,
            next_route_key: std::sync::atomic::AtomicU16::new(0),
        }
    }

    pub(crate) fn hydrate_constraints(
        &mut self,
        segments: &mut [SegmentPattern],
    ) -> RadixResult<()> {
        for pattern in segments.iter_mut() {
            for part in pattern.parts.iter_mut() {
                if let SegmentPart::Param {
                    name,
                    constraint: Some(constraint),
                } = part
                    && constraint.compiled().is_none()
                {
                    let compiled = self.compile_constraint(name.as_str(), constraint.raw())?;
                    constraint.set_compiled(compiled);
                }
            }
        }
        Ok(())
    }

    fn compile_constraint(&mut self, name: &str, raw: &str) -> RadixResult<Arc<Regex>> {
        if let Some(existing) = self.constraint_regex_cache.get(raw) {
            return Ok(existing.clone());
        }

        let pattern = format!("^(?:{})$", raw);
        match Regex::new(&pattern) {
            Ok(regex) => {
                let arc = Arc::new(regex);
                self.constraint_regex_cache
                    .insert(raw.to_string().into_boxed_str(), arc.clone());
                Ok(arc)
            }
            Err(err) => Err(PatternError::RegexConstraintInvalid {
                pattern: format!(":{}({})", name, raw),
                name: name.to_string(),
                error: err.to_string(),
            }
            .into()),
        }
    }

    pub fn finalize(&mut self) {
        super::builder::finalize(self);
    }

    pub fn insert_bulk<I>(&mut self, entries: I) -> RadixResult<Vec<u16>>
    where
        I: IntoIterator<Item = (HttpMethod, String)>,
    {
        if self.root_node.is_sealed() {
            return Err(RadixError::TreeSealed {
                operation: "insert_bulk",
                path: None,
            });
        }

        // Phase A: preprocess (normalize/parse) with light metadata
        let enumerated: Vec<(usize, HttpMethod, String)> = entries
            .into_iter()
            .enumerate()
            .map(|(idx, (method, path))| (idx, method, path))
            .collect();

        let total = enumerated.len();
        let mut pre: Vec<ParsedEntry> = Vec::with_capacity(total);
        let preprocessor = self.preprocessor.clone();

        if total > 1 {
            use std::sync::mpsc;
            use std::thread;

            let (tx, rx) = mpsc::channel();
            let workers = thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
                .min(total);
            let chunk_size = total.div_ceil(workers);
            let chunk_refs: Vec<&[(usize, HttpMethod, String)]> =
                enumerated.chunks(chunk_size).collect();
            let mut handles = Vec::with_capacity(chunk_refs.len());

            for chunk in chunk_refs.into_iter() {
                let txc = tx.clone();
                let local: Vec<(usize, HttpMethod, String)> = chunk.to_vec();
                let worker_preprocessor = preprocessor.clone();

                handles.push(thread::spawn(move || {
                    for (idx, method, path) in local.into_iter() {
                        match preprocess_and_parse(&path, &worker_preprocessor) {
                            Ok((outcome, segs, lits)) => {
                                let normalized = outcome.normalized();
                                let head = first_non_slash_byte(normalized);
                                let plen = normalized.len();
                                let is_static = infer_static_guess(normalized);
                                let _ =
                                    txc.send(Ok((idx, method, segs, head, plen, is_static, lits)));
                            }
                            Err(e) => {
                                let _ = txc.send(Err((idx, e)));
                            }
                        }
                    }
                }));
            }
            drop(tx);

            let mut first_err: Option<RadixError> = None;
            for msg in rx.iter() {
                match msg {
                    Ok((idx, method, segs, head, plen, is_static, lits)) => {
                        pre.push((idx, method, segs, head, plen, is_static, lits))
                    }
                    Err((_idx, e)) => {
                        if first_err.is_none() {
                            first_err = Some(e);
                        }
                    }
                }
            }
            // ensure all workers finished
            for h in handles {
                let _ = h.join();
            }
            if let Some(e) = first_err {
                return Err(e);
            }
        } else {
            // fast path: single item
            for (idx, method, path) in enumerated.into_iter() {
                let (outcome, segs, lits) = preprocess_and_parse(&path, &preprocessor)?;
                let normalized = outcome.normalized();
                let head = first_non_slash_byte(normalized);
                let plen = normalized.len();
                let is_static = infer_static_guess(normalized);
                pre.push((idx, method, segs, head, plen, is_static, lits));
            }
        }

        // Phase B prep: thread-local literal sets merged, then intern unique literals once
        let mut uniq: FastHashSet<String> = FastHashSet::new();
        for (_idx, _method, _segs, _h, _l, _s, lits) in pre.iter() {
            for s in lits.iter() {
                uniq.insert(s.clone());
            }
        }
        // Warm interner with literals in deterministic order to reduce hash collisions
        let mut uniq_vec: Vec<&str> = uniq.iter().map(|s| s.as_str()).collect();
        uniq_vec.sort_unstable();
        for s in uniq_vec.into_iter() {
            let _ = self.interner.intern(s);
        }

        // Phase B: preassign keys then commit; bucket sort for locality then preserve idx mapping
        pre.sort_by(|a, b| {
            // head byte asc, length asc, static-first
            let (ah, al, asg) = (a.3, a.4, a.5);
            let (bh, bl, bsg) = (b.3, b.4, b.5);
            ah.cmp(&bh)
                .then_with(|| al.cmp(&bl))
                .then_with(|| bsg.cmp(&asg))
        });
        let n = pre.len();
        let base = {
            use std::sync::atomic::Ordering;
            let cur = self.next_route_key.load(Ordering::Relaxed);
            if cur as usize + n >= MAX_ROUTES as usize {
                return Err(RadixError::MaxRoutesExceeded {
                    requested: Some(n),
                    current_next_key: cur,
                    limit: MAX_ROUTES,
                });
            }
            self.next_route_key.fetch_add(n as u16, Ordering::Relaxed)
        };
        let mut out = vec![0u16; n];
        let mut max_assigned_key: Option<u16> = None;
        for (idx, method, segs, _h, _l, _s, _lits) in pre.into_iter() {
            let assigned = base + (idx as u16) + 1; // stored keys are +1 encoded
            // pass decoded value to helper (helper will re-encode)
            match self.insert_parsed_preassigned(method, segs, assigned - 1) {
                Ok(route_key) => {
                    out[idx] = route_key;
                    max_assigned_key = match max_assigned_key {
                        Some(existing) => Some(existing.max(route_key)),
                        None => Some(route_key),
                    };
                }
                Err(err) => {
                    use std::sync::atomic::Ordering;
                    let new_next = max_assigned_key
                        .map(|key| key.saturating_add(1))
                        .unwrap_or(base);
                    self.next_route_key.store(new_next, Ordering::Relaxed);
                    return Err(err);
                }
            }
        }
        Ok(out)
    }
}
