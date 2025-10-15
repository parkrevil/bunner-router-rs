use super::{SegmentPart, SegmentPattern};

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
