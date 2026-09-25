use crate::{ParsedSelectorList, Specificity};

use super::StyleDeclaration;

/// Represents one selector list and its typed declarations.
///
/// Selector-specific specificity values and rule source order are preserved for
/// a later cascade implementation.
#[derive(Debug, Clone)]
pub struct StyleRule {
    pub selectors: ParsedSelectorList,
    pub specificity: Vec<Specificity>,
    pub declarations: Vec<StyleDeclaration>,
    pub source_order: usize,
}
