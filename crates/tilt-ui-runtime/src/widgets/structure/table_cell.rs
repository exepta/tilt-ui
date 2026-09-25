//! Semantic position and type of a table cell.

use bevy::ecs::component::Component;
use tilt_ui_core::TemplateAttribute;

/// Authored location of a `td`, `th`, or `table-cell`.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TableCellInfo {
    pub row: Option<usize>,
    pub column: Option<usize>,
    pub header: bool,
    pub section: TableSection,
}

/// HTML table section from which a cell originated.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TableSection {
    Head,
    #[default]
    Body,
    Foot,
}

impl TableCellInfo {
    pub(crate) fn from_attributes(attributes: &[TemplateAttribute]) -> Self {
        let value = |name| crate::component::static_attribute_value(attributes, name);
        Self {
            row: value("data-table-row").and_then(|value| value.parse().ok()),
            column: value("data-table-column").and_then(|value| value.parse().ok()),
            header: value("data-table-kind") == Some("head"),
            section: match value("data-table-section") {
                Some("head") => TableSection::Head,
                Some("foot") => TableSection::Foot,
                _ => TableSection::Body,
            },
        }
    }
}
