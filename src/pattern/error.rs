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
}
