use crate::path::{PathError, PathResult};
use crate::router::RouterOptions;

#[derive(Debug, Clone)]
pub struct PreprocessOutcome {
    original: String,
    normalized: String,
    cache_key: String,
}

impl PreprocessOutcome {
    pub fn original(&self) -> &str {
        &self.original
    }

    pub fn normalized(&self) -> &str {
        &self.normalized
    }

    pub fn cache_key(&self) -> &str {
        &self.cache_key
    }
}

#[derive(Debug, Clone, Default)]
pub struct Preprocessor {
    config: RouterOptions,
}

impl Preprocessor {
    pub fn new(config: RouterOptions) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &RouterOptions {
        &self.config
    }

    pub fn update_config(&mut self, config: RouterOptions) {
        self.config = config;
    }

    pub fn apply(&self, path: &str) -> PathResult<PreprocessOutcome> {
        apply(path, &self.config)
    }
}

pub fn apply(path: &str, config: &RouterOptions) -> PathResult<PreprocessOutcome> {
    let mut working = if config.decode_uri {
        decode_percent(path)?
    } else {
        path.to_string()
    };

    if !config.case_sensitive {
        working = working.to_ascii_lowercase();
    }

    validate_characters(&working, path)?;

    let mut normalized = if config.normalize_path {
        normalize(&working, config)?
    } else {
        ensure_non_empty(&working)?
    };

    if !config.case_sensitive {
        // ensure any transformations inside normalize respect case-insensitive mode
        normalized = normalized.to_ascii_lowercase();
    }

    if normalized.is_empty() {
        return Err(PathError::Empty);
    }

    if contains_parent_traversal(&normalized) {
        return Err(PathError::InvalidParentTraversal {
            input: path.to_string(),
            normalized,
        });
    }

    let cache_key = normalized.clone();

    Ok(PreprocessOutcome {
        original: path.to_string(),
        normalized,
        cache_key,
    })
}

fn decode_percent(input: &str) -> PathResult<String> {
    let mut output = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                if i + 2 >= bytes.len() {
                    return Err(PathError::InvalidPercentEncoding {
                        input: input.to_string(),
                        index: i,
                    });
                }
                let hi = bytes[i + 1];
                let lo = bytes[i + 2];
                let value =
                    decode_hex_pair(hi, lo).ok_or_else(|| PathError::InvalidPercentEncoding {
                        input: input.to_string(),
                        index: i,
                    })?;
                output.push(value as char);
                i += 3;
            }
            ch => {
                output.push(ch as char);
                i += 1;
            }
        }
    }
    Ok(output)
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

fn validate_characters(candidate: &str, original: &str) -> PathResult<()> {
    if candidate.is_empty() {
        return Err(PathError::Empty);
    }
    if !candidate.is_ascii() {
        return Err(PathError::NonAscii {
            input: original.to_string(),
        });
    }
    for &b in candidate.as_bytes() {
        if b <= 0x20 {
            return Err(PathError::ControlOrWhitespace {
                input: original.to_string(),
                byte: b,
            });
        }
        match b {
            b'a'..=b'z'
            | b'A'..=b'Z'
            | b'0'..=b'9'
            | b'-'
            | b'.'
            | b'_'
            | b'~'
            | b'!'
            | b'$'
            | b'&'
            | b'\''
            | b'('
            | b')'
            | b'*'
            | b'+'
            | b','
            | b';'
            | b'='
            | b':'
            | b'@'
            | b'/'
            | b'%' => {}
            _ => {
                return Err(PathError::DisallowedCharacter {
                    input: original.to_string(),
                    character: b as char,
                    byte: b,
                });
            }
        }
    }
    Ok(())
}

fn normalize(input: &str, config: &RouterOptions) -> PathResult<String> {
    let mut output = if config.allow_duplicate_slash {
        input.to_string()
    } else {
        collapse_duplicate_slashes(input)
    };

    if !config.strict_trailing_slash {
        trim_trailing_slashes(&mut output);
    } else if output.is_empty() {
        return Err(PathError::Empty);
    }

    if output.is_empty() {
        return Err(PathError::Empty);
    }

    Ok(output)
}

fn ensure_non_empty(value: &str) -> PathResult<String> {
    if value.is_empty() {
        Err(PathError::Empty)
    } else {
        Ok(value.to_string())
    }
}

fn trim_trailing_slashes(value: &mut String) {
    while value.len() > 1 && value.ends_with('/') {
        value.pop();
    }
}

fn collapse_duplicate_slashes(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut prev_was_slash = false;
    for ch in input.chars() {
        if ch == '/' {
            if !prev_was_slash {
                output.push(ch);
                prev_was_slash = true;
            }
        } else {
            output.push(ch);
            prev_was_slash = false;
        }
    }
    if output.is_empty() {
        "/".to_string()
    } else {
        output
    }
}

fn contains_parent_traversal(path: &str) -> bool {
    path == "/.." || path.starts_with("/../") || path.contains("/../")
}
