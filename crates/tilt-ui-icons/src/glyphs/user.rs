//! SVG glyph fragments for the user family.

pub(crate) const USER: &str =
    r#"<circle cx="12" cy="8" r="4"/><path d="M4 21v-2a8 8 0 0 1 16 0v2z"/>"#;
pub(crate) const USER_FILLED: &str = r#"<circle cx="12" cy="8" r="4" fill="currentColor" stroke="none"/><path d="M4 21v-2a8 8 0 0 1 16 0v2z" fill="currentColor" stroke="none"/>"#;
pub(crate) const USER_CIRCLE: &str = r#"<circle cx="12" cy="12" r="10"/><circle cx="12" cy="9" r="3"/><path d="M5.5 19a7 7 0 0 1 13 0"/>"#;
pub(crate) const USER_SQUARE: &str = r#"<rect x="2" y="2" width="20" height="20" rx="3"/><circle cx="12" cy="9" r="3"/><path d="M6 19a6 6 0 0 1 12 0"/>"#;
pub(crate) const USER_PLUS: &str =
    r#"<circle cx="9" cy="8" r="3"/><path d="M3 20a6 6 0 0 1 12 0m3-11v8m-4-4h8"/>"#;
pub(crate) const USER_GROUP: &str = r#"<circle cx="8" cy="9" r="3"/><circle cx="17" cy="9" r="2.5"/><path d="M2 20a6 6 0 0 1 12 0m0-5a5 5 0 0 1 8 5"/>"#;
