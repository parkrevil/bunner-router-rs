mod error;
mod lexer;
mod matcher;
mod scoring;
mod segment;

pub use error::{PatternError, PatternResult};
pub use lexer::parse_segment;
pub use matcher::{CaptureList, CapturedParam, ParamOffset, match_segment};
pub use scoring::{pattern_compatible_policy, pattern_is_pure_static, pattern_score};
pub use segment::{SegmentPart, SegmentPattern};
