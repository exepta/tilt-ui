use bevy::{asset::Asset, reflect::TypePath};
use tilt_ui_css::StyleSheet;

/// Bevy asset containing a parsed TiltUI component stylesheet.
#[derive(Asset, TypePath, Debug)]
pub struct UiStyleSheetAsset {
    stylesheet: StyleSheet,
}

impl UiStyleSheetAsset {
    /// Wraps a parsed TiltUI stylesheet as a Bevy asset.
    pub fn new(stylesheet: StyleSheet) -> Self {
        Self { stylesheet }
    }

    /// Returns the parsed stylesheet retained by this asset.
    pub fn stylesheet(&self) -> &StyleSheet {
        &self.stylesheet
    }

    /// Consumes the asset and returns its parsed stylesheet.
    pub fn into_stylesheet(self) -> StyleSheet {
        self.stylesheet
    }
}

impl From<StyleSheet> for UiStyleSheetAsset {
    fn from(stylesheet: StyleSheet) -> Self {
        Self::new(stylesheet)
    }
}

#[cfg(test)]
mod tests {
    use super::UiStyleSheetAsset;
    use tilt_ui_css::parse_stylesheet;

    #[test]
    fn retains_the_parsed_stylesheet() {
        let stylesheet = parse_stylesheet("button { width: 200px; }").expect("valid stylesheet");
        let asset = UiStyleSheetAsset::new(stylesheet);

        assert_eq!(asset.stylesheet().rules.len(), 1);
    }
}
