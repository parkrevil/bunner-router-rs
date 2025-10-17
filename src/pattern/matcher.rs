use crate::radix::node::is_valid_segment_length;
use memchr::{memchr, memmem};
use regex::Regex;
use smallvec::SmallVec;

use super::{SegmentPart, SegmentPattern};

pub type ParamOffset = (usize, usize);
pub type CapturedParam = (String, ParamOffset);
pub type CaptureList = SmallVec<[CapturedParam; 4]>;

#[tracing::instrument(level = "trace", skip(pat, default_pattern), fields(seg=%seg, parts=pat.parts.len() as u64))]
pub fn match_segment(
    seg: &str,
    pat: &SegmentPattern,
    default_pattern: &Regex,
) -> Option<CaptureList> {
    let mut i = 0usize;
    let bytes = seg.as_bytes();
    let mut out: CaptureList = SmallVec::new();
    let mut idx = 0usize;

    while idx < pat.parts.len() {
        match &pat.parts[idx] {
            SegmentPart::Literal(lit) => {
                if i + lit.len() > seg.len() {
                    return None;
                }

                if &seg[i..i + lit.len()] != lit.as_str() {
                    return None;
                }

                i += lit.len();
            }
            SegmentPart::Param { name, constraint } => {
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

                        if let Some(pos) = memchr(target, &seg.as_bytes()[i..]) {
                            end = i + pos;
                        } else {
                            return None;
                        }
                    } else if let Some(rel) = memmem::find(&seg.as_bytes()[i..], nl_str.as_bytes())
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

                let capture = &seg[i..end];
                if !default_pattern.is_match(capture) {
                    return None;
                }

                if let Some(constraint) = constraint {
                    if let Some(regex) = constraint.compiled() {
                        if !regex.is_match(capture) {
                            return None;
                        }
                    } else {
                        debug_assert!(
                            false,
                            "parameter constraint missing compiled regex; falling back to runtime compile",
                        );
                        let pattern = format!("^(?:{})$", constraint.raw());
                        match Regex::new(&pattern) {
                            Ok(regex) => {
                                if !regex.is_match(capture) {
                                    return None;
                                }
                            }
                            Err(_) => {
                                return None;
                            }
                        }
                    }
                }

                out.push((name.clone(), (i, end - i)));
                i = end;
            }
        }

        idx += 1;
    }

    if i == seg.len() { Some(out) } else { None }
}
