//! SVG glyphs adapted from Lucide Icons (66d8f9fc394b8530377e5f6112f0b8908ba01280).
//! License: ../../LUCIDE-LICENSE

pub(crate) const SKIP_BACK: &str = r#"<path d="M17.971 4.285A2 2 0 0 1 21 6v12a2 2 0 0 1-3.029 1.715l-9.997-5.998a2 2 0 0 1-.003-3.432z" /><path d="M3 20V4" />"#;
pub(crate) const SKIP_FORWARD: &str = r#"<path d="M21 4v16" /><path d="M6.029 4.285A2 2 0 0 0 3 6v12a2 2 0 0 0 3.029 1.715l9.997-5.998a2 2 0 0 0 .003-3.432z" />"#;
pub(crate) const REWIND: &str = r#"<path d="M12 6a2 2 0 0 0-3.414-1.414l-6 6a2 2 0 0 0 0 2.828l6 6A2 2 0 0 0 12 18z" /><path d="M22 6a2 2 0 0 0-3.414-1.414l-6 6a2 2 0 0 0 0 2.828l6 6A2 2 0 0 0 22 18z" />"#;
pub(crate) const FAST_FORWARD: &str = r#"<path d="M12 6a2 2 0 0 1 3.414-1.414l6 6a2 2 0 0 1 0 2.828l-6 6A2 2 0 0 1 12 18z" /><path d="M2 6a2 2 0 0 1 3.414-1.414l6 6a2 2 0 0 1 0 2.828l-6 6A2 2 0 0 1 2 18z" />"#;
pub(crate) const SQUARE: &str = r#"<rect width="18" height="18" x="3" y="3" rx="2" />"#;
pub(crate) const CIRCLE_DOT: &str =
    r#"<circle cx="12" cy="12" r="1" /><circle cx="12" cy="12" r="10" />"#;
pub(crate) const VOLUME_X: &str = r#"<path d="M11 4.702a.7.7 0 0 0-1.203-.498L6.413 7.587A1.4 1.4 0 0 1 5.416 8H3a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2.416a1.4 1.4 0 0 1 .997.413l3.383 3.384A.7.7 0 0 0 11 19.298z" /><path d="m16.5 14.5 5-5" /><path d="m16.5 9.5 5 5" />"#;
pub(crate) const FILM: &str = r#"<rect width="18" height="18" x="3" y="3" rx="2" /><path d="M7 3v18" /><path d="M3 7.5h4" /><path d="M3 12h18" /><path d="M3 16.5h4" /><path d="M17 3v18" /><path d="M17 7.5h4" /><path d="M17 16.5h4" />"#;
pub(crate) const SCAN: &str = r#"<path d="M3 7V5a2 2 0 0 1 2-2h2" /><path d="M17 3h2a2 2 0 0 1 2 2v2" /><path d="M21 17v2a2 2 0 0 1-2 2h-2" /><path d="M7 21H5a2 2 0 0 1-2-2v-2" />"#;
pub(crate) const PICTURE_IN_PICTURE: &str = r#"<path d="M2 10h6V4" /><path d="m2 4 6 6" /><path d="M21 10V7a2 2 0 0 0-2-2h-7" /><path d="M3 14v2a2 2 0 0 0 2 2h3" /><rect x="12" y="14" width="10" height="7" rx="1" />"#;
