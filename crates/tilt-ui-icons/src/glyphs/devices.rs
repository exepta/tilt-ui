//! SVG glyph fragments for devices, media, and commerce.

pub(crate) const CAMERA: &str =
    r#"<path d="M3 7h4l2-3h6l2 3h4v13H3z"/><circle cx="12" cy="13" r="4"/>"#;
pub(crate) const VIDEO: &str =
    r#"<rect x="3" y="5" width="13" height="14" rx="2"/><path d="m16 10 5-3v10l-5-3"/>"#;
pub(crate) const MUSIC: &str = r#"<path d="M9 18V5l11-2v13M9 8l11-2"/><circle cx="6" cy="19" r="3"/><circle cx="17" cy="17" r="3"/>"#;
pub(crate) const MIC: &str = r#"<rect x="9" y="3" width="6" height="12" rx="3"/><path d="M5 11a7 7 0 0 0 14 0M12 18v3m-4 0h8"/>"#;
pub(crate) const VOLUME: &str =
    r#"<path d="M4 9h4l5-4v14l-5-4H4zM17 9a4 4 0 0 1 0 6m2-9a8 8 0 0 1 0 12"/>"#;
pub(crate) const WIFI: &str = r#"<path d="M2 9a16 16 0 0 1 20 0M5 12a11 11 0 0 1 14 0M8 15a6 6 0 0 1 8 0"/><circle cx="12" cy="19" r="1"/>"#;
pub(crate) const BATTERY: &str =
    r#"<rect x="2" y="7" width="18" height="10" rx="2"/><path d="M22 10v4M5 10h8"/>"#;
pub(crate) const MONITOR: &str =
    r#"<rect x="2" y="3" width="20" height="15" rx="2"/><path d="M12 18v3m-5 0h10"/>"#;
pub(crate) const SMARTPHONE: &str =
    r#"<rect x="6" y="2" width="12" height="20" rx="2"/><path d="M10 18h4"/>"#;
pub(crate) const PRINTER: &str = r#"<path d="M6 8V3h12v5M6 17H3V8h18v9h-3M6 14h12v7H6zM17 11h1"/>"#;
pub(crate) const GIFT: &str = r#"<rect x="3" y="10" width="18" height="11" rx="1"/><path d="M2 7h20v3H2zM12 7v14m0-14c-5 0-7-1-7-3a3 3 0 0 1 3-3c2 0 4 3 4 6zm0 0c5 0 7-1 7-3a3 3 0 0 0-3-3c-2 0-4 3-4 6z"/>"#;
pub(crate) const SHOPPING_CART: &str = r#"<path d="M2 3h2l3 13h12l3-9H5"/><circle cx="7" cy="20" r="1"/><circle cx="18" cy="20" r="1"/>"#;
