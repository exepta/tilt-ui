//! SVG glyph fragments for the general family.

pub(crate) const SEARCH: &str = r#"<circle cx="10.5" cy="10.5" r="7"/><path d="m16 16 5 5"/>"#;
pub(crate) const BELL: &str = r#"<path d="M5 17h14l-2-3V9a5 5 0 0 0-10 0v5zM10 21h4"/>"#;
pub(crate) const HEART: &str =
    r#"<path d="M12 21 3.5 12.5a5 5 0 0 1 7-7L12 7l1.5-1.5a5 5 0 0 1 7 7z"/>"#;
pub(crate) const STAR: &str =
    r#"<path d="m12 2 3.1 6.4 7 1-5 4.9 1.2 7-6.3-3.3-6.3 3.3 1.2-7-5-4.9 7-1z"/>"#;
pub(crate) const CHECK: &str = r#"<path d="m4 12 5 5L20 6"/>"#;
pub(crate) const PLUS: &str = r#"<path d="M12 3v18M3 12h18"/>"#;
pub(crate) const MINUS: &str = r#"<path d="M3 12h18"/>"#;
pub(crate) const ARROW_LEFT: &str = r#"<path d="m11 5-7 7 7 7M4 12h17"/>"#;
pub(crate) const ARROW_RIGHT: &str = r#"<path d="m13 5 7 7-7 7m7-7H3"/>"#;
pub(crate) const CHEVRON_LEFT: &str = r#"<path d="m15 4-8 8 8 8"/>"#;
pub(crate) const CHEVRON_RIGHT: &str = r#"<path d="m9 4 8 8-8 8"/>"#;
pub(crate) const CHEVRON_DOWN: &str = r#"<path d="m4 9 8 8 8-8"/>"#;
pub(crate) const DOWNLOAD: &str = r#"<path d="M12 3v13m-5-5 5 5 5-5M3 19v2h18v-2"/>"#;
pub(crate) const UPLOAD: &str = r#"<path d="M12 16V3m-5 5 5-5 5 5M3 19v2h18v-2"/>"#;
pub(crate) const TRASH: &str = r#"<path d="M4 6h16M8 6V4h8v2m-10 0 1 15h10l1-15M10 10v7m4-7v7"/>"#;
pub(crate) const EDIT: &str = r#"<path d="m4 17 12-12 3 3L7 20H4zm10-10 3 3M3 21h18"/>"#;
pub(crate) const MAIL: &str =
    r#"<rect x="2" y="5" width="20" height="14" rx="2"/><path d="m3 7 9 7 9-7"/>"#;
pub(crate) const CALENDAR: &str = r#"<rect x="3" y="5" width="18" height="16" rx="2"/><path d="M7 3v4m10-4v4M3 10h18m-13 4h3m3 0h2"/>"#;
pub(crate) const CLOCK: &str = r#"<circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 3"/>"#;
pub(crate) const FOLDER: &str =
    r#"<path d="M2 6a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2z"/>"#;
pub(crate) const FILE: &str = r#"<path d="M5 2h9l5 5v15H5zM14 2v5h5M8 12h8M8 16h8"/>"#;
pub(crate) const LOCK: &str = r#"<rect x="4" y="10" width="16" height="12" rx="2"/><path d="M8 10V7a4 4 0 1 1 8 0v3m-4 5v2"/>"#;
pub(crate) const EYE: &str =
    r#"<path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12"/><circle cx="12" cy="12" r="3"/>"#;
pub(crate) const SUN: &str = r#"<circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1.5 1.5M17.5 17.5 19 19M19 5l-1.5 1.5M6.5 17.5 5 19"/>"#;
pub(crate) const MOON: &str = r#"<path d="M20 16A8 8 0 0 1 8 4 9 9 0 1 0 20 16z"/>"#;

// Workflow and navigation.
pub(crate) const COPY: &str = r#"<rect x="8" y="8" width="12" height="13" rx="2"/><path d="M16 8V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v11a2 2 0 0 0 2 2h3"/>"#;
pub(crate) const LINK: &str = r#"<path d="M10 13a5 5 0 0 0 7.1 0l2.4-2.4a5 5 0 0 0-7.1-7.1L11 5M14 11a5 5 0 0 0-7.1 0l-2.4 2.4a5 5 0 0 0 7.1 7.1L13 19"/>"#;
pub(crate) const EXTERNAL_LINK: &str = r#"<path d="M13 4h7v7m0-7-10 10"/><path d="M19 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1h5"/>"#;
pub(crate) const SHARE: &str = r#"<circle cx="18" cy="5" r="2"/><circle cx="5" cy="12" r="2"/><circle cx="18" cy="19" r="2"/><path d="m7 11 9-5M7 13l9 5"/>"#;
pub(crate) const FILTER: &str = r#"<path d="M3 4h18l-7 8v7l-4 2v-9z"/>"#;
pub(crate) const REFRESH: &str =
    r#"<path d="M20 11a8 8 0 0 0-14-5L3 9m0-5v5h5M4 13a8 8 0 0 0 14 5l3-3m0 5v-5h-5"/>"#;
pub(crate) const BOOKMARK: &str = r#"<path d="M5 3h14v18l-7-5-7 5z"/>"#;
pub(crate) const MAP_PIN: &str = r#"<path d="M20 10c0 6-8 12-8 12S4 16 4 10a8 8 0 1 1 16 0z"/><circle cx="12" cy="10" r="2.5"/>"#;

// Feedback and media.
pub(crate) const INFO: &str = r#"<circle cx="12" cy="12" r="10"/><path d="M12 11v6m0-10h.01"/>"#;
pub(crate) const WARNING: &str = r#"<path d="M10.3 3.7a2 2 0 0 1 3.4 0l9 15.5A2 2 0 0 1 21 22H3a2 2 0 0 1-1.7-2.8z"/><path d="M12 9v5m0 4h.01"/>"#;
pub(crate) const HELP_CIRCLE: &str = r#"<circle cx="12" cy="12" r="10"/><path d="M9.3 9a2.8 2.8 0 1 1 4.6 2.2c-1.1.9-1.9 1.4-1.9 3.1m0 3.5h.01"/>"#;
pub(crate) const CHECK_CIRCLE: &str =
    r#"<circle cx="12" cy="12" r="10"/><path d="m7 12 3.3 3.3L17 8.5"/>"#;
pub(crate) const PLAY: &str =
    r#"<path d="M7 4.5a1 1 0 0 1 1.5-.9l12 7.5a1 1 0 0 1 0 1.8l-12 7.5A1 1 0 0 1 7 19.5z"/>"#;
pub(crate) const PAUSE: &str = r#"<rect x="5" y="4" width="5" height="16" rx="1"/><rect x="14" y="4" width="5" height="16" rx="1"/>"#;
pub(crate) const IMAGE: &str = r#"<rect x="2" y="3" width="20" height="18" rx="2"/><circle cx="8" cy="9" r="2"/><path d="m3 18 5-5 4 3 4-5 5 6"/>"#;
pub(crate) const PHONE: &str = r#"<path d="M7 3 4 5a2 2 0 0 0-.7 2.2A19 19 0 0 0 16.8 20.7 2 2 0 0 0 19 20l2-3-5-3-2.2 2.2a15 15 0 0 1-6-6L10 8z"/>"#;
