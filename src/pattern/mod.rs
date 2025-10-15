mod ast;
mod compiled;
mod error;
mod lexer;
mod matcher;
mod parser;
mod scoring;
mod segment;

pub use ast::{
    GroupNode, ParameterConstraint, ParameterNode, PatternAst, PatternNode, Quantifier,
    WildcardNode,
};
pub use compiled::{
    CompiledPattern, GroupElement, ParameterElement, QuantifierSpan, RouteElement, SegmentAtom,
    SegmentElement, WildcardElement, compile_pattern_ast,
};
pub use error::{PatternError, PatternResult};
pub use lexer::parse_segment;
pub use matcher::{CaptureList, CapturedParam, ParamOffset, match_segment};
pub use parser::parse_pattern;
pub use scoring::{pattern_compatible_policy, pattern_is_pure_static, pattern_score};
pub use segment::{SegmentPart, SegmentPattern};
