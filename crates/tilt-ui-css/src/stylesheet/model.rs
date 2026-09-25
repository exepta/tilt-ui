use super::{KeyframesRule, MediaRule, StyleRule};

/// Represents a parsed CSS stylesheet used by TiltUI components.
///
/// Rules retain their source order for a later cascade stage.
#[derive(Debug, Clone, Default)]
pub struct StyleSheet {
    pub rules: Vec<StyleRule>,
    /// Typed responsive rule blocks.
    pub media_rules: Vec<MediaRule>,
    /// Stylesheet-local keyframe definitions.
    pub keyframes: Vec<KeyframesRule>,
}

impl StyleSheet {
    /// Returns stylesheet rules in source order.
    pub fn rules(&self) -> &[StyleRule] {
        &self.rules
    }

    /// Returns responsive rule blocks in authored order.
    pub fn media_rules(&self) -> &[MediaRule] {
        &self.media_rules
    }

    /// Returns stylesheet-local keyframe definitions.
    pub fn keyframes(&self) -> &[KeyframesRule] {
        &self.keyframes
    }

    /// Returns whether this stylesheet contains responsive rules.
    pub fn has_media_rules(&self) -> bool {
        !self.media_rules.is_empty()
    }
}
