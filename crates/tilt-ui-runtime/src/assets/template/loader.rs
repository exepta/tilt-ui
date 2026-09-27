use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    reflect::TypePath,
};
use tilt_ui_html::{parse_document, parse_template};

use super::{UiTemplateAsset, UiTemplateAssetLoaderError};

/// Loads `.component.html` files into parsed TiltUI template assets.
#[derive(Default, TypePath)]
pub struct UiTemplateAssetLoader;

impl AssetLoader for UiTemplateAssetLoader {
    type Asset = UiTemplateAsset;
    type Settings = ();
    type Error = UiTemplateAssetLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        parse_template_bytes(bytes, load_context.path().path().ends_with("index.html"))
    }

    fn extensions(&self) -> &[&str] {
        &["component.html", "html"]
    }
}

fn parse_template_bytes(
    bytes: Vec<u8>,
    document: bool,
) -> Result<UiTemplateAsset, UiTemplateAssetLoaderError> {
    let source = String::from_utf8(bytes)?;
    if document {
        Ok(UiTemplateAsset::from_document(parse_document(&source)?))
    } else {
        Ok(parse_template(&source)?.into())
    }
}

#[cfg(test)]
mod tests {
    use bevy::asset::AssetLoader;

    use super::{UiTemplateAssetLoader, parse_template_bytes};
    use crate::assets::template::UiTemplateAssetLoaderError;

    #[test]
    fn claims_only_component_html_files() {
        let loader = UiTemplateAssetLoader;

        assert_eq!(loader.extensions(), ["component.html", "html"]);
    }

    #[test]
    fn malformed_template_preserves_parse_error() {
        let error = parse_template_bytes(b"<button [bad=\"x\"></button>".to_vec(), false)
            .expect_err("invalid template");

        assert!(matches!(
            error,
            UiTemplateAssetLoaderError::TemplateParse(_)
        ));
    }
}
