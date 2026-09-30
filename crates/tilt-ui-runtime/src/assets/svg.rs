//! Asynchronous SVG image loading for native and browser asset sources.

use bevy::{
    asset::{AssetLoader, AsyncReadExt, LoadContext, RenderAssetUsages, io::Reader},
    image::{Image, ImageSampler},
    reflect::TypePath,
};
use resvg::{
    tiny_skia::{Pixmap, Transform},
    usvg::Options,
};
use thiserror::Error;
use wgpu_types::{Extent3d, TextureDimension, TextureFormat};

const MAX_SVG_BYTES: usize = 2 * 1024 * 1024;
const MAX_SVG_PIXELS: u64 = 4_194_304;

#[derive(Debug, Error)]
pub enum SvgImageLoaderError {
    #[error("cannot read SVG: {0}")]
    Io(#[from] std::io::Error),
    #[error("SVG exceeds the size limit")]
    TooLarge,
    #[error("invalid SVG: {0}")]
    Invalid(String),
    #[error("cannot allocate SVG image")]
    Allocation,
}

/// Converts `.svg` assets to Bevy images after the asset source supplies bytes.
#[derive(Default, TypePath)]
pub struct SvgImageLoader;

impl AssetLoader for SvgImageLoader {
    type Asset = Image;
    type Settings = ();
    type Error = SvgImageLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader
            .take((MAX_SVG_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .await?;
        rasterize_svg(&bytes)
    }

    fn extensions(&self) -> &[&str] {
        &["svg"]
    }
}

fn rasterize_svg(bytes: &[u8]) -> Result<Image, SvgImageLoaderError> {
    if bytes.len() > MAX_SVG_BYTES {
        return Err(SvgImageLoaderError::TooLarge);
    }
    let tree = resvg::usvg::Tree::from_data(bytes, &Options::default())
        .map_err(|error| SvgImageLoaderError::Invalid(error.to_string()))?;
    let size = tree.size().to_int_size();
    let (width, height) = (size.width(), size.height());
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > MAX_SVG_PIXELS {
        return Err(SvgImageLoaderError::TooLarge);
    }
    let mut pixmap = Pixmap::new(width, height).ok_or(SvgImageLoaderError::Allocation)?;
    resvg::render(&tree, Transform::default(), &mut pixmap.as_mut());
    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixmap.take(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::linear();
    Ok(image)
}

#[cfg(test)]
mod tests {
    use bevy::asset::AssetLoader;

    use super::{SvgImageLoader, SvgImageLoaderError, rasterize_svg};

    #[test]
    fn svg_loader_rasterizes_and_limits_browser_images() {
        assert_eq!(SvgImageLoader.extensions(), ["svg"]);
        let image = rasterize_svg(
            br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="16"><rect width="24" height="16" fill="#db4ce8"/></svg>"##,
        )
        .unwrap();
        assert_eq!(image.texture_descriptor.size.width, 24);
        assert_eq!(image.texture_descriptor.size.height, 16);
        assert!(matches!(
            rasterize_svg(br#"<svg width="10000" height="10000"/>"#),
            Err(SvgImageLoaderError::TooLarge)
        ));
    }
}
