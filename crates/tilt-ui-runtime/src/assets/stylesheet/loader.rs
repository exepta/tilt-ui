use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    reflect::TypePath,
};
use tilt_ui_css::parse_stylesheet;

use super::{UiStyleSheetAsset, UiStyleSheetAssetLoaderError};

/// Loads `.component.css` files into parsed TiltUI stylesheet assets.
#[derive(Default, TypePath)]
pub struct UiStyleSheetAssetLoader;

impl AssetLoader for UiStyleSheetAssetLoader {
    type Asset = UiStyleSheetAsset;
    type Settings = ();
    type Error = UiStyleSheetAssetLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        parse_stylesheet_bytes(bytes)
    }

    fn extensions(&self) -> &[&str] {
        &["component.css"]
    }
}

fn parse_stylesheet_bytes(
    bytes: Vec<u8>,
) -> Result<UiStyleSheetAsset, UiStyleSheetAssetLoaderError> {
    let source = String::from_utf8(bytes)?;
    Ok(parse_stylesheet(&source)?.into())
}

#[cfg(test)]
mod tests {
    use bevy::asset::AssetLoader;

    use super::{UiStyleSheetAssetLoader, parse_stylesheet_bytes};
    use crate::assets::stylesheet::UiStyleSheetAssetLoaderError;

    #[test]
    fn claims_only_component_css_files() {
        let loader = UiStyleSheetAssetLoader;

        assert_eq!(loader.extensions(), ["component.css"]);
    }

    #[test]
    fn invalid_stylesheet_preserves_parse_error() {
        let error = parse_stylesheet_bytes(b"button { width: wide; }".to_vec())
            .expect_err("invalid stylesheet");

        assert!(matches!(error, UiStyleSheetAssetLoaderError::StyleParse(_)));
    }
}
