mod attribute;
mod control;
mod document;
mod error;
// The module name mirrors the parser implementation and the prescribed file layout.
#[allow(clippy::module_inception)]
mod parser;

pub use document::{DocumentHead, ParsedDocument, parse_document};
pub use error::TemplateParseError;
pub use parser::{is_valid_component_name, parse_template};
