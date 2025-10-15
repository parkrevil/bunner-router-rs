mod error;
mod lexer;
mod matcher;
mod scoring;
mod segment;

pub use error::PatternError;
pub use lexer::parse_segment;
pub use matcher::{match_segment, CaptureList, CapturedParam, ParamOffset};
pub use scoring::{pattern_compatible_policy, pattern_is_pure_static, pattern_score};
pub use segment::{SegmentPart, SegmentPattern};
