//! Parsing support for TiltUI `.component.html` template sources.
//!
//! This crate converts template text directly into `tilt_ui_core::Template`
//! values. Component discovery, CSS processing, and runtime construction are
//! intentionally outside this boundary.

mod mapping;
mod parser;

pub use mapping::{is_builtin_element_tag, map_element};
pub use parser::{
    DocumentHead, ParsedDocument, TemplateParseError, is_valid_component_name, parse_document,
    parse_template,
};
