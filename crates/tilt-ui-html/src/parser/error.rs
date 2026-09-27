use thiserror::Error;

/// Describes a failure while parsing a TiltUI component template.
#[derive(Debug, Error)]
pub enum TemplateParseError {
    /// A template import does not use the supported `@use "Target" as alias;` form.
    #[error("invalid @use directive: {0}")]
    InvalidUse(String),
    /// A malformed XML-like construct was reported by `quick-xml`.
    #[error("XML parsing error: {0}")]
    Xml(#[from] quick_xml::Error),

    /// A closing tag did not match the currently open element.
    #[error("unexpected closing tag `{found}`; expected `{expected}`")]
    UnexpectedClosingTag { found: String, expected: String },

    /// A closing tag appeared without an open element.
    #[error("unexpected closing tag `{0}`")]
    ClosingTagWithoutOpenElement(String),

    /// The document ended before an element was closed.
    #[error("unclosed element `{0}`")]
    UnclosedElement(String),

    /// An attribute name or structure is not valid for a TiltUI template.
    #[error("invalid attribute `{0}")]
    InvalidAttribute(String),

    /// A property or event binding does not contain a valid binding name.
    #[error("invalid binding `{0}")]
    InvalidBinding(String),

    /// A non-built-in tag does not meet the component naming convention.
    #[error("invalid component name `{0}`")]
    InvalidComponentName(String),

    /// A template cannot contain more nodes than fit in a `NodeId`.
    #[error("template contains too many nodes")]
    TooManyNodes,
}
