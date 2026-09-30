//! A dependency-free SVG icon catalog for TiltUI.
//!
//! Every glyph uses a 24×24 viewBox and supports 16, 32 or 64 pixels.

mod glyphs;
mod icon;
mod size;
mod source;

pub use icon::Icon;
pub use size::IconSize;
pub use source::parse_source;
