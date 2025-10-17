use crate::pattern::{PatternError, PatternResult};

use super::{SegmentPart, SegmentPattern, segment::ParamConstraint};

#[tracing::instrument(level = "trace", fields(segment=%seg))]
pub fn parse_segment(seg: &str) -> PatternResult<SegmentPattern> {
    let bytes = seg.as_bytes();

    if bytes.first().copied() == Some(b':') {
        let mut j = 1usize;

        if j >= bytes.len() {
            return Err(PatternError::ParameterMissingName {
                segment: seg.to_string(),
            });
        }

        while j < bytes.len() {
            let b = bytes[j];

            if b == b'(' {
                break;
            }

            if !(b.is_ascii_alphanumeric() || b == b'_') {
                return Err(PatternError::ParameterInvalidCharacter {
                    segment: seg.to_string(),
                    name: seg[1..j + 1].to_string(),
                    invalid: b as char,
                });
            }

            j += 1;
        }

        let name = &seg[1..j];

        if name.is_empty() {
            return Err(PatternError::ParameterMissingName {
                segment: seg.to_string(),
            });
        }

        let nb = name.as_bytes();

        if !(nb[0].is_ascii_alphabetic() || nb[0] == b'_') {
            return Err(PatternError::ParameterInvalidStart {
                segment: seg.to_string(),
                name: name.to_string(),
                found: nb[0] as char,
            });
        }

        for &c in &nb[1..] {
            if !(c.is_ascii_alphanumeric() || c == b'_') {
                return Err(PatternError::ParameterInvalidCharacter {
                    segment: seg.to_string(),
                    name: name.to_string(),
                    invalid: c as char,
                });
            }
        }

        let mut constraint = None;
        if j < bytes.len() {
            if bytes[j] != b'(' {
                return Err(PatternError::MixedParameterLiteralSyntax {
                    segment: seg.to_string(),
                });
            }
            let mut depth = 1usize;
            let mut escaped = false;
            let mut buf = String::new();
            let mut closing_abs = None;

            for (offset, ch) in seg[j + 1..].char_indices() {
                let abs = j + 1 + offset;
                if escaped {
                    buf.push(ch);
                    escaped = false;
                    continue;
                }
                match ch {
                    '\\' => {
                        escaped = true;
                        buf.push(ch);
                    }
                    '(' => {
                        depth += 1;
                        buf.push(ch);
                    }
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            closing_abs = Some(abs);
                            break;
                        }
                        buf.push(ch);
                    }
                    _ => buf.push(ch),
                }
            }

            if depth != 0 {
                return Err(PatternError::UnterminatedParameterConstraint {
                    pattern: seg.to_string(),
                    name: name.to_string(),
                    start: j,
                });
            }

            let end_idx = closing_abs.expect("constraint parsing depth reached zero");
            if end_idx + 1 != seg.len() {
                return Err(PatternError::MixedParameterLiteralSyntax {
                    segment: seg.to_string(),
                });
            }

            constraint = Some(ParamConstraint::new(buf));
        }

        return Ok(SegmentPattern {
            parts: vec![SegmentPart::Param {
                name: name.to_string(),
                constraint,
            }],
        });
    }

    if seg.contains(':') {
        return Err(PatternError::MixedParameterLiteralSyntax {
            segment: seg.to_string(),
        });
    }

    let lit_norm = seg.to_string();

    Ok(SegmentPattern {
        parts: vec![SegmentPart::Literal(lit_norm)],
    })
}
