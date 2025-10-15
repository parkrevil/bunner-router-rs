use crate::radix::node::is_valid_segment_length;
use memchr::{memchr, memmem};
use smallvec::SmallVec;

use super::{SegmentPart, SegmentPattern};

pub type ParamOffset = (usize, usize);
pub type CapturedParam = (String, ParamOffset);
pub type CaptureList = SmallVec<[CapturedParam; 4]>;

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

                        if let Some(pos) = memchr(target, &seg_l.as_bytes()[i_l..]) {
                            end = i + pos;
                        } else {
                            return None;
                        }
                    } else if let Some(rel) =
                        memmem::find(&seg_l.as_bytes()[i_l..], nl_str.as_bytes())
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
