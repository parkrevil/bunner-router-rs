use crate::path::{PathError, PathResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NormalizationOptions {
    pub decode_percent: bool,
    pub normalize_path: bool,
    pub allow_duplicate_slash: bool,
    pub strict_trailing_slash: bool,
    pub case_sensitive: bool,
}

impl Default for NormalizationOptions {
    fn default() -> Self {
        Self {
            decode_percent: false,
            normalize_path: true,
            allow_duplicate_slash: false,
            strict_trailing_slash: false,
            case_sensitive: true,
        }
    }
}

#[inline]
#[tracing::instrument(level = "trace", skip(path, options), fields(path_len=path.len() as u64))]
pub fn normalize_path(path: &str, options: &NormalizationOptions) -> PathResult<String> {
    if path.is_empty() {
        return Err(PathError::Empty);
    }

    let mut output = Vec::with_capacity(path.len());
    let mut prev_was_slash = false;
    let mut segment_start = 0usize;
    let mut saw_parent_traversal = false;

    let bytes = path.as_bytes();
    let mut idx = 0usize;
    while idx < bytes.len() {
        let byte = bytes[idx];
        if options.decode_percent && byte == b'%' {
            if idx + 2 >= bytes.len() {
                return Err(PathError::InvalidPercentEncoding {
                    input: path.to_string(),
                    index: idx,
                });
            }
            let hi = bytes[idx + 1];
            let lo = bytes[idx + 2];
            let value =
                decode_hex_pair(hi, lo).ok_or_else(|| PathError::InvalidPercentEncoding {
                    input: path.to_string(),
                    index: idx,
                })?;
            process_byte(
                value,
                options,
                path,
                &mut output,
                &mut prev_was_slash,
                &mut segment_start,
                &mut saw_parent_traversal,
            )?;
            idx += 3;
            continue;
        }

        process_byte(
            byte,
            options,
            path,
            &mut output,
            &mut prev_was_slash,
            &mut segment_start,
            &mut saw_parent_traversal,
        )?;
        idx += 1;
    }

    finalize_segment(&output, segment_start, &mut saw_parent_traversal);

    if options.normalize_path && !options.strict_trailing_slash {
        while output.len() > 1 && output.last() == Some(&b'/') {
            output.pop();
        }
    }

    if output.is_empty() {
        return Err(PathError::Empty);
    }

    let normalized =
        String::from_utf8(output).map_err(|_| PathError::InvalidUtf8AfterDecoding {
            input: path.to_string(),
        })?;

    if saw_parent_traversal {
        return Err(PathError::InvalidParentTraversal {
            input: path.to_string(),
            normalized,
        });
    }

    Ok(normalized)
}

#[inline]
pub fn normalize_and_validate_path(path: &str) -> PathResult<String> {
    normalize_path(path, &NormalizationOptions::default())
}

fn process_byte(
    byte: u8,
    options: &NormalizationOptions,
    original: &str,
    output: &mut Vec<u8>,
    prev_was_slash: &mut bool,
    segment_start: &mut usize,
    saw_parent_traversal: &mut bool,
) -> PathResult<()> {
    if byte == b'/' {
        if options.normalize_path && !options.allow_duplicate_slash && *prev_was_slash {
            return Ok(());
        }

        finalize_segment(output, *segment_start, saw_parent_traversal);
        output.push(b'/');
        *prev_was_slash = true;
        *segment_start = output.len();
        return Ok(());
    }

    if byte <= 0x20 {
        return Err(PathError::ControlOrWhitespace {
            input: original.to_string(),
            byte,
        });
    }

    let mut value = byte;
    if !options.case_sensitive && value.is_ascii_uppercase() {
        value = value.to_ascii_lowercase();
    }

    output.push(value);
    *prev_was_slash = false;
    Ok(())
}

fn finalize_segment(output: &[u8], segment_start: usize, saw_parent_traversal: &mut bool) {
    if segment_start >= output.len() {
        return;
    }

    let segment_len = output.len() - segment_start;
    if segment_len == 2 && output[segment_start..segment_start + 2] == *b".." {
        *saw_parent_traversal = true;
    }
}

fn decode_hex_pair(hi: u8, lo: u8) -> Option<u8> {
    fn val(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }

    Some(val(hi)? << 4 | val(lo)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_duplicates_and_trims_trailing_slashes() {
        let normalized = normalize_path("//foo//bar///", &NormalizationOptions::default()).unwrap();
        assert_eq!(normalized, "/foo/bar");
    }

    #[test]
    fn preserves_duplicates_when_allowed() {
        let options = NormalizationOptions {
            allow_duplicate_slash: true,
            ..Default::default()
        };
        let normalized = normalize_path("//foo//bar///", &options).unwrap();
        assert_eq!(normalized, "//foo//bar");
    }

    #[test]
    fn percent_decoding_is_optional() {
        let normalized = normalize_path("/caf%C3%A9", &NormalizationOptions::default()).unwrap();
        assert_eq!(normalized, "/caf%C3%A9");
    }

    #[test]
    fn percent_decoding_supports_utf8_sequences() {
        let options = NormalizationOptions {
            decode_percent: true,
            ..Default::default()
        };
        let normalized = normalize_path("/caf%C3%A9", &options).unwrap();
        assert_eq!(normalized, "/café");
    }

    #[test]
    fn accepts_unicode_input_without_ascii_enforcement() {
        let normalized = normalize_path("/こんにちは", &NormalizationOptions::default()).unwrap();
        assert_eq!(normalized, "/こんにちは");
    }

    #[test]
    fn rejects_control_bytes_after_decoding() {
        let options = NormalizationOptions {
            decode_percent: true,
            ..Default::default()
        };
        let err = normalize_path("/foo%00bar", &options).unwrap_err();
        match err {
            PathError::ControlOrWhitespace { byte, .. } => assert_eq!(byte, 0),
            other => panic!("expected ControlOrWhitespace, got {other:?}"),
        }
    }

    #[test]
    fn rejects_parent_traversal_segments() {
        let err = normalize_path("/foo/../bar", &NormalizationOptions::default()).unwrap_err();
        match err {
            PathError::InvalidParentTraversal { normalized, .. } => {
                assert_eq!(normalized, "/foo/../bar");
            }
            other => panic!("expected InvalidParentTraversal, got {other:?}"),
        }
    }

    #[test]
    fn lowercases_ascii_when_case_insensitive() {
        let options = NormalizationOptions {
            case_sensitive: false,
            ..Default::default()
        };
        let normalized = normalize_path("/Foo/BAR", &options).unwrap();
        assert_eq!(normalized, "/foo/bar");
    }
}
