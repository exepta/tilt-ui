//! Native Bevy assets for TiltUI component templates and stylesheets.
//!
//! Component source files are addressed through the `tilt-ui` asset source,
//! for example `tilt-ui://components/header.component.html`.

mod plugin;
mod source;
#[cfg(feature = "svg")]
mod svg;

#[cfg(feature = "css")]
pub mod stylesheet;
#[cfg(feature = "html")]
pub mod template;

pub use plugin::TiltUiAssetsPlugin;
#[cfg(all(feature = "svg", feature = "component", not(target_arch = "wasm32")))]
pub(crate) use source::UiAssetSourceRoot;
pub use source::{DEFAULT_TILT_UI_ASSET_PATH, TILT_UI_ASSET_SOURCE, TiltUiAssetSourcePlugin};
#[cfg(feature = "svg")]
pub use svg::{SvgImageLoader, SvgImageLoaderError};

#[cfg(feature = "css")]
pub use stylesheet::*;
#[cfg(feature = "html")]
pub use template::*;
