//! SVG glyph fragments for the close family.

pub(crate) const CLOSE: &str = r#"<path d="M5 5 19 19M19 5 5 19"/>"#;
pub(crate) const CLOSE_CIRCLE: &str =
    r#"<circle cx="12" cy="12" r="10"/><path d="m8 8 8 8m0-8-8 8"/>"#;
pub(crate) const CLOSE_SQUARE: &str =
    r#"<rect x="2" y="2" width="20" height="20" rx="3"/><path d="m8 8 8 8m0-8-8 8"/>"#;
pub(crate) const CLOSE_BOLD: &str = r#"<path d="M5 5 19 19M19 5 5 19" stroke-width="3.5"/>"#;
