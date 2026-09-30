//! SVG glyph fragments for navigation and page structure.

pub(crate) const DASHBOARD: &str = r#"<rect x="3" y="3" width="8" height="8" rx="1"/><rect x="13" y="3" width="8" height="5" rx="1"/><rect x="3" y="13" width="8" height="8" rx="1"/><rect x="13" y="10" width="8" height="11" rx="1"/>"#;
pub(crate) const LIST: &str = r#"<path d="M8 5h13M8 12h13M8 19h13M3 5h.01M3 12h.01M3 19h.01"/>"#;
pub(crate) const GRID: &str = r#"<rect x="3" y="3" width="8" height="8" rx="1"/><rect x="13" y="3" width="8" height="8" rx="1"/><rect x="3" y="13" width="8" height="8" rx="1"/><rect x="13" y="13" width="8" height="8" rx="1"/>"#;
pub(crate) const LAYERS: &str = r#"<path d="m12 3 9 5-9 5-9-5 9-5zm-9 9 9 5 9-5M3 16l9 5 9-5"/>"#;
pub(crate) const TARGET: &str = r#"<circle cx="12" cy="12" r="9"/><circle cx="12" cy="12" r="5"/><circle cx="12" cy="12" r="1"/>"#;
pub(crate) const GLOBE: &str = r#"<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c-3 2.5-4 5.5-4 9s1 6.5 4 9m0-18c3 2.5 4 5.5 4 9s-1 6.5-4 9"/>"#;
pub(crate) const COMPASS: &str =
    r#"<circle cx="12" cy="12" r="9"/><path d="m15.5 8.5-2 5-5 2 2-5z"/>"#;
pub(crate) const MAP: &str = r#"<path d="m3 5 6-2 6 2 6-2v16l-6 2-6-2-6 2zM9 3v16m6-14v16"/>"#;
pub(crate) const ROUTE: &str = r#"<circle cx="6" cy="5" r="2"/><circle cx="18" cy="19" r="2"/><path d="M6 7v6a4 4 0 0 0 4 4h4a4 4 0 0 0 0-8h-1"/>"#;
pub(crate) const ARROW_UP: &str = r#"<path d="m5 11 7-7 7 7M12 4v17"/>"#;
pub(crate) const ARROW_DOWN: &str = r#"<path d="m5 13 7 7 7-7m-7 7V3"/>"#;
pub(crate) const CHEVRON_UP: &str = r#"<path d="m4 15 8-8 8 8"/>"#;
