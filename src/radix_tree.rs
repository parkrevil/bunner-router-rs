use bumpalo::Bump;
use hashbrown::HashMap as FastHashMap;
use hashbrown::HashSet as FastHashSet;
type IndexedEntry = (usize, HttpMethod, String, u8, usize, bool);
type ParsedEntry = (
    usize,
    HttpMethod,
    Vec<SegmentPattern>,
    u8,
    usize,
    bool,
    Vec<String>,
);
use crate::pattern::SegmentPart;
use crate::pattern::SegmentPattern;

use super::RouterOptions;
use crate::enums::HttpMethod;
use crate::interner::Interner;
use crate::types::WorkerId;

pub(super) const HTTP_METHOD_COUNT: usize = 7;

// Fixed maximum routes across all builds for predictable memory layout
pub(super) const MAX_ROUTES: u16 = 65_535;

const STATIC_MAP_THRESHOLD: usize = 50;

mod alloc;
mod builder;
mod compression;
mod indices;
mod insert;
mod mask;
mod memory;
pub mod node;
mod static_map;
pub mod traversal;

pub(crate) use alloc::{NodeBox, create_node_box_from_arena_pointer};
pub use node::RadixTreeNode;

#[derive(Debug, Default)]
pub struct RadixTree {
    pub(super) root_node: RadixTreeNode,
    pub(super) options: RouterOptions,
    pub(super) arena: Bump,
    pub(super) interner: Interner,
    pub(super) method_first_byte_bitmaps: [[u64; 4]; HTTP_METHOD_COUNT],
    pub(super) root_parameter_first_present: [bool; HTTP_METHOD_COUNT],
    pub(super) root_wildcard_present: [bool; HTTP_METHOD_COUNT],
    pub(super) static_route_full_mapping: [FastHashMap<Box<str>, u16>; HTTP_METHOD_COUNT],
    pub(super) method_length_buckets: [u64; HTTP_METHOD_COUNT],
    pub enable_root_level_pruning: bool,
    pub enable_static_route_full_mapping: bool,
    pub(super) next_route_key: std::sync::atomic::AtomicU16,
    // Side-table mapping route key index -> initial registrant worker id.
    // Kept here so we can drop it at finalize() to reclaim heap memory.
    pub(super) route_worker_side_table: Vec<Option<u32>>,
}

impl RadixTree {
    pub fn new(configuration: RouterOptions) -> Self {
        Self {
            root_node: RadixTreeNode::default(),
            options: configuration,
            arena: Bump::with_capacity(128 * 1024),
            interner: Interner::new(),
            method_first_byte_bitmaps: [[0; 4]; HTTP_METHOD_COUNT],
            root_parameter_first_present: [false; HTTP_METHOD_COUNT],
            root_wildcard_present: [false; HTTP_METHOD_COUNT],
            static_route_full_mapping: Default::default(),
            method_length_buckets: [0; HTTP_METHOD_COUNT],
            enable_root_level_pruning: configuration.enable_root_level_pruning,
            enable_static_route_full_mapping: configuration.enable_static_route_full_mapping,
            next_route_key: std::sync::atomic::AtomicU16::new(0),
            route_worker_side_table: Vec::new(),
        }
    }

    pub fn finalize(&mut self) {
        builder::finalize(self);
    }

    pub fn insert_bulk<I>(
        &mut self,
        worker_id: WorkerId,
        entries: I,
    ) -> super::structures::RouterResult<Vec<u16>>
    where
        I: IntoIterator<Item = (HttpMethod, String)>,
    {
        if self.root_node.is_sealed() {
            return Err(Box::new(super::structures::RouterError::new(
                super::errors::RouterErrorCode::AlreadySealed,
                "router",
                "route_registration",
                "validation",
                "Router is sealed; cannot insert bulk routes".to_string(),
                Some(serde_json::json!({"operation": "insert_bulk", "sealed": true})),
            )));
        }

        // Phase A: parallel preprocess (normalize/parse) with light metadata
        let indexed: Vec<IndexedEntry> = entries
            .into_iter()
            .enumerate()
            .map(|(i, (m, p))| {
                let bs = p.as_bytes();
                let mut j = 0usize;
                while j < bs.len() && bs[j] == b'/' {
                    j += 1;
                }
                let head = if j < bs.len() { bs[j] } else { 0 };
                let is_static_guess = !p.contains(':') && !p.contains("/*") && !p.ends_with('*');
                (i, m, p.to_string(), head, bs.len(), is_static_guess)
            })
            .collect();

        let total = indexed.len();
        let mut pre: Vec<ParsedEntry> = Vec::with_capacity(total);

        if total > 1 {
            use std::sync::mpsc;
            use std::thread;

            let (tx, rx) = mpsc::channel();
            let workers = thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
                .min(total);
            let chunk_size = total.div_ceil(workers);
            let chunk_refs: Vec<&[IndexedEntry]> = indexed.chunks(chunk_size).collect();
            let mut handles = Vec::with_capacity(chunk_refs.len());

            for chunk in chunk_refs.into_iter() {
                let txc = tx.clone();
                let local: Vec<IndexedEntry> = chunk.to_vec();

                handles.push(thread::spawn(move || {
                    for (idx, method, path, head, plen, is_static) in local.into_iter() {
                        let parsed =
                            super::radix_tree::insert::prepare_path_segments_standalone(&path);
                        match parsed {
                            Ok(segs) => {
                                let mut lits: Vec<String> = Vec::new();
                                for pat in segs.iter() {
                                    for part in pat.parts.iter() {
                                        if let SegmentPart::Literal(l) = part {
                                            lits.push(l.clone());
                                        }
                                    }
                                }
                                // ignore send error if receiver dropped
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

            let mut first_err: Option<Box<super::structures::RouterError>> = None;
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
            for (idx, method, path, head, plen, is_static) in indexed.into_iter() {
                let segs = super::radix_tree::insert::prepare_path_segments_standalone(&path)?;
                let mut lits: Vec<String> = Vec::new();
                for pat in segs.iter() {
                    for part in pat.parts.iter() {
                        if let SegmentPart::Literal(l) = part {
                            lits.push(l.clone());
                        }
                    }
                }
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
                return Err(Box::new(super::structures::RouterError::new(
                    super::errors::RouterErrorCode::MaxRoutesExceeded,
                    "router",
                    "route_registration",
                    "validation",
                    "Maximum number of routes exceeded when reserving bulk keys".to_string(),
                    Some(serde_json::json!({
                        "requested": n,
                        "currentNextKey": cur,
                        "maxRoutes": MAX_ROUTES,
                        "operation": "bulk_key_reservation"
                    })),
                )));
            }
            self.next_route_key.fetch_add(n as u16, Ordering::Relaxed)
        };
        let mut out = vec![0u16; n];
        for (idx, method, segs, _h, _l, _s, _lits) in pre.into_iter() {
            let assigned = base + (idx as u16) + 1; // stored keys are +1 encoded
            // pass decoded value to helper (helper will re-encode)
            let route_key =
                self.insert_parsed_preassigned(worker_id, method, segs, assigned - 1)?;
            out[idx] = route_key;
        }
        Ok(out)
    }
}
