use thiserror::Error;

/// Describes a failure while parsing a TiltUI stylesheet.
#[derive(Debug, Error)]
pub enum StyleParseError {
    /// CSS tokenization or rule syntax could not be parsed.
    #[error("CSS syntax error: {0}")]
    CssSyntax(String),
    /// A selector is not valid for the TiltUI selector subset.
    #[error("invalid selector: {0}")]
    InvalidSelector(String),
    /// A declaration has invalid property syntax.
    #[error("invalid declaration: {0}")]
    InvalidDeclaration(String),
    /// A property is not supported by the current TiltUI CSS subset.
    #[error("unsupported CSS property: {0}")]
    UnsupportedProperty(String),
    /// A supported property has an invalid value.
    #[error("invalid value for `{property}`: {value}")]
    InvalidPropertyValue { property: String, value: String },
    /// A length uses an unsupported unit or invalid value.
    #[error("invalid CSS length: {0}")]
    InvalidLength(String),
    /// A color uses an unsupported syntax or invalid channel value.
    #[error("invalid CSS color: {0}")]
    InvalidColor(String),
}
