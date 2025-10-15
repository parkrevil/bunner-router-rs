use crate::types::{ErrorCode, StaticString};

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RouterErrorCode {
    AlreadySealed = 10000,
    NotSealed,

    EmptyPath,
    InvalidPath,
    DuplicatedPath,

    InvalidParamName,
    DuplicateParamName,
    ParamNameConflicted,

    PatternTooLong,

    InvalidWildcard,
    WildcardAlreadyExists,

    MaxRoutesExceeded,
    PathNotFound,
}

impl RouterErrorCode {
    pub fn code(self) -> ErrorCode {
        self as ErrorCode
    }

    pub fn name(self) -> StaticString {
        match self {
            RouterErrorCode::AlreadySealed => "AlreadySealed",
            RouterErrorCode::NotSealed => "NotSealed",

            RouterErrorCode::EmptyPath => "EmptyPath",
            RouterErrorCode::InvalidPath => "InvalidPath",
            RouterErrorCode::DuplicatedPath => "DuplicatedPath",

            RouterErrorCode::InvalidParamName => "InvalidParamName",
            RouterErrorCode::DuplicateParamName => "DuplicateParamName",
            RouterErrorCode::ParamNameConflicted => "ParamNameConflicted",

            RouterErrorCode::PatternTooLong => "PatternTooLong",

            RouterErrorCode::InvalidWildcard => "InvalidWildcard",
            RouterErrorCode::WildcardAlreadyExists => "WildcardAlreadyExists",

            RouterErrorCode::MaxRoutesExceeded => "MaxRoutesExceeded",
            RouterErrorCode::PathNotFound => "PathNotFound",
        }
    }
}
