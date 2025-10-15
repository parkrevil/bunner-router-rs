use crate::errors::{RouterError, RouterResult};
use crate::pattern::PatternError;

use super::{SegmentPart, SegmentPattern};

#[tracing::instrument(level = "trace", fields(segment=%seg))]
pub fn parse_segment(seg: &str) -> RouterResult<SegmentPattern> {
    if seg.contains('(') || seg.contains(')') {
        return Err(RouterError::from(PatternError::ParenthesisNotAllowed {
            segment: seg.to_string(),
        }));
    }

    let bytes = seg.as_bytes();

    if bytes.first().copied() == Some(b':') {
        let mut j = 1usize;

        if j >= bytes.len() {
            return Err(RouterError::from(PatternError::ParameterMissingName {
                segment: seg.to_string(),
            }));
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
            return Err(RouterError::from(PatternError::ParameterNameContainsColon {
                segment: seg.to_string(),
                name: name.to_string(),
            }));
        }

        let nb = name.as_bytes();

        if nb.is_empty() {
            return Err(RouterError::from(PatternError::ParameterNameEmpty {
                segment: seg.to_string(),
            }));
        }

        if !(nb[0].is_ascii_alphabetic() || nb[0] == b'_') {
            return Err(RouterError::from(PatternError::ParameterInvalidStart {
                segment: seg.to_string(),
                name: name.to_string(),
                found: nb[0] as char,
            }));
        }

        for &c in &nb[1..] {
            if !(c.is_ascii_alphanumeric() || c == b'_') {
                return Err(RouterError::from(PatternError::ParameterInvalidCharacter {
                    segment: seg.to_string(),
                    name: name.to_string(),
                    invalid: c as char,
                }));
            }
        }

        return Ok(SegmentPattern {
            parts: vec![SegmentPart::Param {
                name: name.to_string(),
            }],
        });
    }

    if seg.contains(':') {
        return Err(RouterError::from(PatternError::MixedParameterLiteralSyntax {
            segment: seg.to_string(),
        }));
    }

    let lit_norm = seg.to_string();

    Ok(SegmentPattern {
        parts: vec![SegmentPart::Literal(lit_norm)],
    })
}
