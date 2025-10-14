use super::NodeBox;
use bitflags::bitflags;
use hashbrown::HashMap as FastHashMap;
use smallvec::SmallVec;

use crate::pattern::SegmentPattern;

use super::HTTP_METHOD_COUNT;

pub const MAX_SEGMENT_LENGTH: usize = 255;

#[repr(C, packed)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PatternMeta {
    pub score: u16,
    pub min_len: u8,
    pub last_lit_len: u8,
}

impl PatternMeta {
    pub fn new(score: u16, min_len: u16, last_lit_len: u16) -> Self {
        Self {
            score,
            min_len: min_len as u8,
            last_lit_len: last_lit_len as u8,
        }
    }

    pub fn is_valid_length(min_len: u16, last_lit_len: u16) -> bool {
        is_valid_segment_length(min_len as usize) && is_valid_segment_length(last_lit_len as usize)
    }
}

pub fn is_valid_segment_length(len: usize) -> bool {
    len <= MAX_SEGMENT_LENGTH
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct NodeFlags: u8 {
        const SEALED = 0b00000001;
        const DIRTY = 0b00000010;
    }
}

pub(super) type StaticMap = FastHashMap<Box<str>, NodeBox>;
pub(super) type StaticMapIdx = FastHashMap<Box<str>, super::NodeBox>;

#[derive(Debug, Default)]
pub struct RadixTreeNode {
    // optimize small number of siblings before promoting to map
    pub(crate) static_keys: SmallVec<[Box<str>; 32]>,
    pub(crate) static_vals: SmallVec<[NodeBox; 32]>,
    // index-based mirror for arena-backed nodes (built during sealing)
    pub(crate) static_vals_idx: SmallVec<[super::NodeBox; 16]>,
    // interned ids aligned with static_keys
    pub(super) static_key_ids: SmallVec<[u32; 16]>,
    pub(crate) static_children: StaticMap,
    pub(crate) static_children_idx: StaticMapIdx,
    // id-keyed mirror for static children
    pub(super) static_children_idx_ids: FastHashMap<u32, super::NodeBox>,
    // simple MPHF-like open-addressing table for static_keys (built when many keys)
    pub(super) static_hash_seed: u64,
    pub(super) static_hash_table: SmallVec<[i32; 32]>, // stores index into static_vals_idx/static_keys, -1 means empty
    // SoA: separate pattern specs and node handles
    pub(crate) patterns: SmallVec<[SegmentPattern; 8]>,
    pub(crate) pattern_nodes: SmallVec<[NodeBox; 8]>,
    // ordered view indices for fast iteration
    pub(super) pattern_children_idx: SmallVec<[usize; 32]>,
    // first literal -> indices in pattern_children (to reduce regex calls)
    pub(super) pattern_first_literal: FastHashMap<String, SmallVec<[u16; 32]>>,
    // first literal head byte -> indices (fast prefix filtering)
    pub(super) pattern_first_lit_head: FastHashMap<u8, SmallVec<[u16; 32]>>,
    // param-first patterns (indices) for quick fallback without full scan
    pub(super) pattern_param_first: SmallVec<[u16; 32]>,
    // 압축된 패턴 메타데이터 (기존 3개 SmallVec을 1개로 통합)
    pub(super) pattern_meta: SmallVec<[PatternMeta; 8]>,
    pub(crate) routes: [u16; HTTP_METHOD_COUNT],
    pub(crate) wildcard_routes: [u16; HTTP_METHOD_COUNT],
    pub(super) flags: NodeFlags,
    // bitmask of methods present in this subtree (including this node)
    pub(super) method_mask: u8,
    // prefix compression (set by compress())
    pub(crate) fused_edge: Option<String>,
    pub(crate) fused_child: Option<NodeBox>,
    // index-based mirror for arena-backed nodes (built during sealing)
    pub(super) fused_child_idx: Option<super::NodeBox>,
}

impl RadixTreeNode {
    // Flag accessor methods
    #[inline(always)]
    pub(super) fn is_sealed(&self) -> bool {
        self.flags.contains(NodeFlags::SEALED)
    }

    #[inline(always)]
    pub(super) fn set_sealed(&mut self, sealed: bool) {
        self.flags.set(NodeFlags::SEALED, sealed);
    }

    #[inline(always)]
    pub(super) fn is_dirty(&self) -> bool {
        self.flags.contains(NodeFlags::DIRTY)
    }

    #[inline(always)]
    pub(super) fn set_dirty(&mut self, dirty: bool) {
        self.flags.set(NodeFlags::DIRTY, dirty);
    }

    #[inline(always)]
    pub(super) fn method_mask(&self) -> u8 {
        self.method_mask
    }

    #[inline(always)]
    pub(super) fn set_method_mask(&mut self, mask: u8) {
        self.method_mask = mask;
    }

    /// Same as `descend_static_mut` but uses provided allocator for child creation.
    pub(super) fn descend_static_mut_with_alloc<F>(
        &mut self,
        key: &str,
        alloc: F,
    ) -> &mut RadixTreeNode
    where
        F: FnOnce() -> NodeBox,
    {
        if self.static_children.is_empty() && self.static_keys.len() < 4 {
            if let Some(pos) = self.static_keys.iter().position(|k| k.as_ref() == key) {
                return self.static_vals[pos].as_mut();
            }
            self.static_keys.push(key.to_owned().into_boxed_str());
            self.static_vals.push(alloc());
            let last = self.static_vals.len() - 1;
            return self.static_vals[last].as_mut();
        }
        if self.static_children.is_empty() && !self.static_keys.is_empty() {
            for (k, v) in self.static_keys.drain(..).zip(self.static_vals.drain(..)) {
                self.static_children.insert(k, v);
            }
        }
        self.static_children
            .entry(key.to_owned().into_boxed_str())
            .or_insert_with(alloc)
            .as_mut()
    }
}
