//! SVG geometry adapted from Tabler Icons 74929e50416e2b7c0abb8368cdc74bdcb2560ab6.
//! Source licenses and attribution are kept at the crate root.

pub(crate) const SORT_ASCENDING: &str = r#"<path d="M4 6l7 0" />
  <path d="M4 12l7 0" />
  <path d="M4 18l9 0" />
  <path d="M15 9l3 -3l3 3" />
  <path d="M18 6l0 12" />
"#;
pub(crate) const SORT_DESCENDING: &str = r#"<path d="M4 6l9 0" />
  <path d="M4 12l7 0" />
  <path d="M4 18l7 0" />
  <path d="M15 15l3 3l3 -3" />
  <path d="M18 6l0 12" />
"#;
pub(crate) const ARROWS_SORT: &str = r#"<path d="M3 9l4 -4l4 4m-4 -4v14" />
  <path d="M21 15l-4 4l-4 -4m4 4v-14" />
"#;
pub(crate) const SETTINGS_AUTOMATION: &str = r#"<path d="M10.325 4.317c.426 -1.756 2.924 -1.756 3.35 0a1.724 1.724 0 0 0 2.573 1.066c1.543 -.94 3.31 .826 2.37 2.37a1.724 1.724 0 0 0 1.065 2.572c1.756 .426 1.756 2.924 0 3.35a1.724 1.724 0 0 0 -1.066 2.573c.94 1.543 -.826 3.31 -2.37 2.37a1.724 1.724 0 0 0 -2.572 1.065c-.426 1.756 -2.924 1.756 -3.35 0a1.724 1.724 0 0 0 -2.573 -1.066c-1.543 .94 -3.31 -.826 -2.37 -2.37a1.724 1.724 0 0 0 -1.065 -2.572c-1.756 -.426 -1.756 -2.924 0 -3.35a1.724 1.724 0 0 0 1.066 -2.573c-.94 -1.543 .826 -3.31 2.37 -2.37c1 .608 2.296 .07 2.572 -1.065" />
  <path d="M10 9v6l5 -3l-5 -3" />
"#;
pub(crate) const ADJUSTMENTS: &str = r#"<path d="M4 10a2 2 0 1 0 4 0a2 2 0 0 0 -4 0" />
  <path d="M6 4v4" />
  <path d="M6 12v8" />
  <path d="M10 16a2 2 0 1 0 4 0a2 2 0 0 0 -4 0" />
  <path d="M12 4v10" />
  <path d="M12 18v2" />
  <path d="M16 7a2 2 0 1 0 4 0a2 2 0 0 0 -4 0" />
  <path d="M18 4v1" />
  <path d="M18 9v11" />
"#;
pub(crate) const ADJUSTMENTS_HORIZONTAL: &str = r#"<path d="M12 6a2 2 0 1 0 4 0a2 2 0 1 0 -4 0" />
  <path d="M4 6l8 0" />
  <path d="M16 6l4 0" />
  <path d="M6 12a2 2 0 1 0 4 0a2 2 0 1 0 -4 0" />
  <path d="M4 12l2 0" />
  <path d="M10 12l10 0" />
  <path d="M15 18a2 2 0 1 0 4 0a2 2 0 1 0 -4 0" />
  <path d="M4 18l11 0" />
  <path d="M19 18l1 0" />
"#;
pub(crate) const TOOL: &str = r#"<path d="M7 10h3v-3l-3.5 -3.5a6 6 0 0 1 8 8l6 6a2 2 0 0 1 -3 3l-6 -6a6 6 0 0 1 -8 -8l3.5 3.5" />
"#;
pub(crate) const TOOLS_OFF: &str = r#"<path d="M16 12l4 -4a2.828 2.828 0 1 0 -4 -4l-4 4m-2 2l-7 7v4h4l7 -7" />
  <path d="M14.5 5.5l4 4" />
  <path d="M12 8l-5 -5m-2 2l-2 2l5 5" />
  <path d="M7 8l-1.5 1.5" />
  <path d="M16 12l5 5m-2 2l-2 2l-5 -5" />
  <path d="M16 17l-1.5 1.5" />
  <path d="M3 3l18 18" />
"#;
pub(crate) const WAND: &str = r#"<path d="M6 21l15 -15l-3 -3l-15 15l3 3" />
  <path d="M15 6l3 3" />
  <path d="M9 3a2 2 0 0 0 2 2a2 2 0 0 0 -2 2a2 2 0 0 0 -2 -2a2 2 0 0 0 2 -2" />
  <path d="M19 13a2 2 0 0 0 2 2a2 2 0 0 0 -2 2a2 2 0 0 0 -2 -2a2 2 0 0 0 2 -2" />
"#;
pub(crate) const WAND_OFF: &str = r#"<path d="M10.5 10.5l-7.5 7.5l3 3l7.5 -7.5m2 -2l5.5 -5.5l-3 -3l-5.5 5.5" />
  <path d="M15 6l3 3" />
  <path d="M8.433 4.395c.35 -.36 .567 -.852 .567 -1.395a2 2 0 0 0 2 2c-.554 0 -1.055 .225 -1.417 .589" />
  <path d="M18.418 14.41c.36 -.36 .582 -.86 .582 -1.41a2 2 0 0 0 2 2c-.555 0 -1.056 .226 -1.419 .59" />
  <path d="M3 3l18 18" />
"#;
