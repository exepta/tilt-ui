//! Typed CSS parsing and selector representation for TiltUI component styles.
//!
//! The crate parses stylesheet source into Bevy-independent data. Runtime
//! matching, asset loading, and style application remain separate concerns.

mod computed;
mod parser;
mod selector;
mod stylesheet;
mod values;

pub use computed::*;
pub use parser::{StyleParseError, parse_color_value, parse_declaration_value, parse_stylesheet};
pub use selector::*;
pub use stylesheet::*;
pub use values::*;
