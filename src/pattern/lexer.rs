use crate::errors::{RouterError, RouterErrorCode, RouterResult};
use serde_json::json;

use super::{SegmentPart, SegmentPattern};

#[tracing::instrument(level = "trace", fields(segment=%seg))]
pub fn parse_segment(seg: &str) -> RouterResult<SegmentPattern> {
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
