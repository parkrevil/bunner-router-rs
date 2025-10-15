use thiserror::Error;

#[derive(Debug, Error)]
pub enum PathError {
    #[error("path contains non-ASCII characters: {input}")]
    NonAscii { input: String },
    #[error("path is empty")]
    Empty,
    #[error("path contains control or whitespace byte {byte} in '{input}'")]
    ControlOrWhitespace { input: String, byte: u8 },
    #[error("path contains disallowed character '{character}' (byte {byte}) in '{input}'")]
    DisallowedCharacter {
        input: String,
        character: char,
        byte: u8,
    },
    #[error("path '{input}' normalizes to invalid parent traversal '{normalized}'")]
    InvalidParentTraversal {
        input: String,
        normalized: String,
    },
    #[error("route path syntax invalid after normalization of '{input}' to '{normalized}'")]
    InvalidAfterNormalization {
        input: String,
        normalized: String,
    },
}
