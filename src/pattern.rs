use crate::errors::RouterErrorCode;
use crate::radix_tree::node::is_valid_segment_length;
use crate::structures::{RouterError, RouterResult};
use serde_json::json;
use smallvec::SmallVec;

// Reduce type complexity with aliases for readability and clippy friendliness
type ParamOffset = (usize, usize);
type CapturedParam = (String, ParamOffset);
type CaptureList = SmallVec<[CapturedParam; 4]>;

#[derive(Debug, Clone)]
pub enum SegmentPart {
    Literal(String),
    Param { name: String },
}

#[derive(Debug, Clone)]
pub struct SegmentPattern {
    pub parts: Vec<SegmentPart>,
}

impl PartialEq for SegmentPattern {
    fn eq(&self, other: &Self) -> bool {
        if self.parts.len() != other.parts.len() {
            return false;
        }
        for (a, b) in self.parts.iter().zip(other.parts.iter()) {
            match (a, b) {
                (SegmentPart::Literal(la), SegmentPart::Literal(lb)) => {
                    if la != lb {
                        return false;
                    }
                }
                (SegmentPart::Param { name: na, .. }, SegmentPart::Param { name: nb, .. }) => {
                    if na != nb {
                        return false;
                    }
                }
                _ => {
                    return false;
                }
            }
        }
        true
    }
}

pub fn pattern_score(p: &SegmentPattern) -> u16 {
    let mut s = 0u16;
    let mut last_lit_len = 0u16;

    for part in p.parts.iter().rev() {
        if let SegmentPart::Literal(l) = part {
            last_lit_len = l.len() as u16;
            break;
        }
    }

    let mut param_count = 0u16;

    // Heuristic: prefer shorter trailing literals to tighten early mismatches
    for (idx, part) in p.parts.iter().enumerate() {
        match part {
            SegmentPart::Literal(l) => {
                s += (if idx == 0 { 600 } else { 120 }) + l.len() as u16;
            }
            SegmentPart::Param { .. } => {
                param_count += 1;
                s += 8;
            }
        }
    }

    // Boost shorter last literal a bit more for earlier pruning
    s += (32u16.saturating_sub(last_lit_len.min(32))) * 2;

    if param_count > 0 {
        s = s.saturating_sub((param_count - 1) * 6);
    }

    s
}

pub fn pattern_compatible_policy(a: &SegmentPattern, b: &SegmentPattern) -> bool {
    if a.parts.len() != b.parts.len() {
        return true;
    }
    for (pa, pb) in a.parts.iter().zip(b.parts.iter()) {
        match (pa, pb) {
            (SegmentPart::Literal(_), SegmentPart::Literal(_)) => { /* allowed */ }
            (SegmentPart::Param { name: na, .. }, SegmentPart::Param { name: nb, .. }) => {
                if na != nb {
                    return false;
                }
            }
            _ => { /* literal vs param allowed */ }
        }
    }
    true
}

pub fn pattern_is_pure_static(p: &SegmentPattern, key_seg: &str) -> bool {
    if p.parts.len() != 1 {
        return false;
    }
    match &p.parts[0] {
        SegmentPart::Literal(l) => l == key_seg,
        _ => false,
    }
}

#[tracing::instrument(level = "trace", skip(seg_l, pat), fields(seg=%seg, parts=pat.parts.len() as u64))]
pub fn match_segment(seg: &str, seg_l: &str, pat: &SegmentPattern) -> Option<CaptureList> {
    let mut i = 0usize;
    let mut i_l = 0usize;
    let bytes = seg.as_bytes();
    let mut out: CaptureList = SmallVec::new();
    let mut idx = 0usize;

    while idx < pat.parts.len() {
        match &pat.parts[idx] {
            SegmentPart::Literal(lit) => {
                if i_l + lit.len() > seg_l.len() {
                    return None;
                }

                if &seg_l[i_l..i_l + lit.len()] != lit.as_str() {
                    return None;
                }

                i += lit.len();
                i_l += lit.len();
            }
            SegmentPart::Param { name } => {
                let mut next_lit: Option<&str> = None;

                if idx + 1 < pat.parts.len()
                    && let SegmentPart::Literal(l) = &pat.parts[idx + 1]
                {
                    next_lit = Some(l.as_str());
                }

                let mut end = bytes.len();

                if let Some(nl_str) = next_lit {
                    if nl_str.len() == 1 {
                        let target = nl_str.as_bytes()[0];

                        if let Some(pos) = memchr::memchr(target, &seg_l.as_bytes()[i_l..]) {
                            end = i + pos;
                        } else {
                            return None;
                        }
                    } else if let Some(rel) =
                        memchr::memmem::find(&seg_l.as_bytes()[i_l..], nl_str.as_bytes())
                    {
                        end = i + rel;
                    } else {
                        return None;
                    }
                }
                if end < i {
                    return None;
                }

                if i == end {
                    return None;
                }

                if !is_valid_segment_length(end - i) {
                    return None;
                }

                out.push((name.clone(), (i, end - i)));

                i = end;
                i_l = end;
            }
        }

        idx += 1;
    }

    if i == seg.len() { Some(out) } else { None }
}

#[tracing::instrument(level = "trace", fields(segment=%seg))]
pub(crate) fn parse_segment(seg: &str) -> RouterResult<SegmentPattern> {
    if seg.contains('(') || seg.contains(')') {
        return Err(Box::new(RouterError::new(
            RouterErrorCode::InvalidPath,
            "router",
            "pattern_parsing",
            "validation",
            "Segment contains parenthesis which is invalid".to_string(),
            Some(json!({"segment": seg, "issue": "parenthesis_not_allowed"})),
        )));
    }

    let bytes = seg.as_bytes();

    if bytes.first().copied() == Some(b':') {
        let mut j = 1usize;

        if j >= bytes.len() {
            return Err(Box::new(RouterError::new(
                RouterErrorCode::InvalidParamName,
                "router",
                "pattern_parsing",
                "validation",
                "Parameter segment missing name".to_string(),
                Some(json!({"segment": seg, "issue": "param_missing_name"})),
            )));
        }

        while j < bytes.len() {
            let b = bytes[j];

            if !(b.is_ascii_alphanumeric() || b == b'_') {
                break;
            }

            j += 1;
        }

        let name = &seg[1..];

        if name.contains(':') {
            return Err(Box::new(RouterError::new(
                RouterErrorCode::InvalidParamName,
                "router",
                "pattern_parsing",
                "validation",
                "Parameter name contains ':' which is invalid".to_string(),
                Some(json!({"segment": seg, "param": name, "issue": "param_name_contains_colon"})),
            )));
        }

        let nb = name.as_bytes();

        if nb.is_empty() {
            return Err(Box::new(RouterError::new(
                RouterErrorCode::InvalidParamName,
                "router",
                "pattern_parsing",
                "validation",
                "Parameter name is empty".to_string(),
                Some(json!({"segment": seg, "issue": "param_name_empty"})),
            )));
        }

        if !(nb[0].is_ascii_alphabetic() || nb[0] == b'_') {
            return Err(Box::new(RouterError::new(
                RouterErrorCode::InvalidParamName,
                "router",
                "pattern_parsing",
                "validation",
                "Parameter name must start with an alphabetic character or underscore".to_string(),
                Some(
                    json!({"segment": seg, "param": name, "first_char": nb[0] as char, "issue": "param_invalid_start"}),
                ),
            )));
        }

        for &c in &nb[1..] {
            if !(c.is_ascii_alphanumeric() || c == b'_') {
                return Err(Box::new(RouterError::new(
                    RouterErrorCode::InvalidParamName,
                    "router",
                    "pattern_parsing",
                    "validation",
                    "Parameter name contains invalid character".to_string(),
                    Some(
                        json!({"segment": seg, "param": name, "invalid_char": c as char, "issue": "param_invalid_char"}),
                    ),
                )));
            }
        }

        return Ok(SegmentPattern {
            parts: vec![SegmentPart::Param {
                name: name.to_string(),
            }],
        });
    }

    if seg.contains(':') {
        return Err(Box::new(RouterError::new(
            RouterErrorCode::InvalidParamName,
            "router",
            "pattern_parsing",
            "validation",
            "Segment contains mixed parameter and literal syntax".to_string(),
            Some(json!({"segment": seg, "issue": "mixed_param_literal"})),
        )));
    }

    let lit_norm = seg.to_string();

    Ok(SegmentPattern {
        parts: vec![SegmentPart::Literal(lit_norm)],
    })
}
