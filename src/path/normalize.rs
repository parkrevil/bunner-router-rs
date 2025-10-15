use crate::errors::{RouterError, RouterResult};
use crate::path::PathError;

#[inline]
#[tracing::instrument(level = "trace", skip(path), fields(path_len=path.len() as u64))]
pub fn normalize_and_validate_path(path: &str) -> RouterResult<String> {
    if !path.is_ascii() {
        return Err(RouterError::from(PathError::NonAscii {
            input: path.to_string(),
        }));
    }
    let bytes = path.as_bytes();
    if bytes.is_empty() {
        return Err(RouterError::from(PathError::Empty));
    }
    let mut end = bytes.len();
    while end > 1 && bytes[end - 1] == b'/' {
        end -= 1;
    }

    // Validate allowed characters while scanning once
    for &b in &bytes[..end] {
        if b <= 0x20 {
            return Err(RouterError::from(PathError::ControlOrWhitespace {
                input: path.to_string(),
                byte: b,
            }));
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
                    return Err(RouterError::from(PathError::DisallowedCharacter {
                        input: path.to_string(),
                        character: b as char,
                        byte: b,
                    }));
            }
        }
    }

    let mut normalized = if end == bytes.len() {
        path.to_string()
    } else {
        path[..end].to_string()
    };

    // Collapse duplicate slashes and forbid '/../'
    while normalized.contains("//") {
        normalized = normalized.replace("//", "/");
    }
    if normalized == "/.." || normalized.starts_with("/../") || normalized.contains("/../") {
        return Err(RouterError::from(PathError::InvalidParentTraversal {
            input: path.to_string(),
            normalized,
        }));
    }

    Ok(normalized)
}
