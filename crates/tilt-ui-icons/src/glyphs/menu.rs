//! SVG glyph fragments for the menu family.

pub(crate) const MENU: &str = r#"<path d="M3 6h18M3 12h18M3 18h18"/>"#;
pub(crate) const MENU_WIDE: &str = r#"<path d="M2 5h20M2 12h20M2 19h20" stroke-width="2.5"/>"#;
pub(crate) const MENU_COMPACT: &str = r#"<path d="M5 8h14M5 12h14M5 16h14"/>"#;
pub(crate) const MENU_DOTS: &str = r#"<circle cx="5" cy="12" r="1.5" fill="currentColor"/><circle cx="12" cy="12" r="1.5" fill="currentColor"/><circle cx="19" cy="12" r="1.5" fill="currentColor"/>"#;
pub(crate) const MENU_DOTS_VERTICAL: &str = r#"<circle cx="12" cy="5" r="1.5" fill="currentColor"/><circle cx="12" cy="12" r="1.5" fill="currentColor"/><circle cx="12" cy="19" r="1.5" fill="currentColor"/>"#;
pub(crate) const MENU_GRID: &str = r#"<rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/>"#;
pub(crate) const MENU_GRID_FILLED: &str = r#"<rect x="3" y="3" width="7" height="7" rx="1" fill="currentColor"/><rect x="14" y="3" width="7" height="7" rx="1" fill="currentColor"/><rect x="3" y="14" width="7" height="7" rx="1" fill="currentColor"/><rect x="14" y="14" width="7" height="7" rx="1" fill="currentColor"/>"#;
pub(crate) const MENU_LIST: &str = r#"<path d="M8 6h13M8 12h13M8 18h13"/><circle cx="4" cy="6" r="1" fill="currentColor"/><circle cx="4" cy="12" r="1" fill="currentColor"/><circle cx="4" cy="18" r="1" fill="currentColor"/>"#;
pub(crate) const MENU_CHEVRON: &str = r#"<path d="M3 6h18M3 12h18M3 18h12m3-2 3 2-3 2"/>"#;
pub(crate) const MENU_CIRCLE: &str =
    r#"<circle cx="12" cy="12" r="10"/><path d="M6 8h12M6 12h12M6 16h12"/>"#;
