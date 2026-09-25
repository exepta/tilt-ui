//! Bevy assets and loaders for TiltUI `.component.html` files.

mod asset;
mod error;
mod loader;

pub use asset::UiTemplateAsset;
pub use error::UiTemplateAssetLoaderError;
pub use loader::UiTemplateAssetLoader;
