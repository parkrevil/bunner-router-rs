use crate::enums::HttpMethod;
use crate::path::PathError;
use crate::pattern::PatternError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RadixError {
    #[error("radix tree is sealed; cannot perform {operation}")]
    TreeSealed {
        operation: &'static str,
        path: Option<String>,
    },
    #[error("wildcard segment must be terminal: index {segment_index} of {total_segments}")]
    WildcardMustBeTerminal {
        segment_index: usize,
        total_segments: usize,
    },
    #[error(
        "maximum number of routes exceeded: requested {requested:?}, current_next_key {current_next_key}, limit {limit}"
    )]
    MaxRoutesExceeded {
        requested: Option<usize>,
        current_next_key: u16,
        limit: u16,
    },
    #[error("parameter name conflict between patterns: {pattern}")]
    ParamNameConflict { pattern: String },
    #[error(
        "pattern length exceeds allowed limits for segment '{segment}' in path '{path}' (min_length {min_length}, last_literal_length {last_literal_length})"
    )]
    PatternLengthExceeded {
        segment: String,
        path: String,
        min_length: u16,
        last_literal_length: u16,
    },
    #[error("duplicate parameter name '{param}' in path '{path}'")]
    DuplicateParamName { param: String, path: String },
    #[error("duplicate route for method {method:?} (existing key {existing_key})")]
    DuplicateRoute {
        method: HttpMethod,
        existing_key: u16,
    },
    #[error("duplicate wildcard route for method {method:?} (existing key {existing_key})")]
    DuplicateWildcardRoute {
        method: HttpMethod,
        existing_key: u16,
    },
    #[error(transparent)]
    Path(#[from] PathError),
    #[error(transparent)]
    Pattern(#[from] PatternError),
}

pub type RadixResult<T> = Result<T, RadixError>;
