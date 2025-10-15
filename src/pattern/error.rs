use thiserror::Error;

#[derive(Debug, Error)]
pub enum PatternError {
    #[error("segment '{segment}' contains parentheses, which are not allowed")]
    ParenthesisNotAllowed { segment: String },
    #[error("parameter segment '{segment}' is missing a name")]
    ParameterMissingName { segment: String },
    #[error("parameter name '{name}' in segment '{segment}' contains ':'")]
    ParameterNameContainsColon { segment: String, name: String },
    #[error("parameter name in segment '{segment}' is empty")]
    ParameterNameEmpty { segment: String },
    #[error(
        "parameter name '{name}' in segment '{segment}' must start with an alphabetic character or underscore (found '{found}')"
    )]
    ParameterInvalidStart {
        segment: String,
        name: String,
        found: char,
    },
    #[error(
        "parameter name '{name}' in segment '{segment}' contains invalid character '{invalid}'"
    )]
    ParameterInvalidCharacter {
        segment: String,
        name: String,
        invalid: char,
    },
    #[error("segment '{segment}' mixes parameter and literal syntax")]
    MixedParameterLiteralSyntax { segment: String },
    #[error("unexpected closing parenthesis at index {index} in pattern '{pattern}'")]
    UnexpectedClosingParenthesis { pattern: String, index: usize },
    #[error("unterminated group starting at index {start} in pattern '{pattern}'")]
    UnterminatedGroup { pattern: String, start: usize },
    #[error("group starting at index {start} in pattern '{pattern}' cannot be empty")]
    EmptyGroup { pattern: String, start: usize },
    #[error(
        "unterminated inline constraint for parameter '{name}' starting at index {start} in pattern '{pattern}'"
    )]
    UnterminatedParameterConstraint {
        pattern: String,
        name: String,
        start: usize,
    },
    #[error(
        "inline constraint for parameter '{name}' is not allowed by parser options (pattern '{pattern}')"
    )]
    RegexConstraintNotAllowed { pattern: String, name: String },
    #[error("inline constraint for parameter '{name}' in pattern '{pattern}' is invalid: {error}")]
    RegexConstraintInvalid {
        pattern: String,
        name: String,
        error: String,
    },
    #[error(
        "quantifier '{modifier}' at index {index} in pattern '{pattern}' does not apply to any token"
    )]
    DanglingQuantifier {
        pattern: String,
        index: usize,
        modifier: char,
    },
    #[error(
        "quantifier '{modifier}' applied to wildcard at index {index} in pattern '{pattern}' is not supported"
    )]
    WildcardQuantifierUnsupported {
        pattern: String,
        index: usize,
        modifier: char,
    },
    #[error(
        "nested optional element detected in pattern '{pattern}' but parser does not allow nested optional groups"
    )]
    NestedOptionalNotAllowed { pattern: String },
    #[error(
        "repeating quantifier '{modifier}' inside optional context is not allowed in pattern '{pattern}'"
    )]
    RepeatInOptionalNotAllowed { pattern: String, modifier: char },
    #[error(
        "escape character at index {index} in pattern '{pattern}' has no following character to escape"
    )]
    LoneEscapeCharacter { pattern: String, index: usize },
}

pub type PatternResult<T> = Result<T, PatternError>;
