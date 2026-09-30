use std::path::{Path, PathBuf};

#[cfg(all(feature = "svg", not(target_arch = "wasm32")))]
use bevy::ecs::resource::Resource;
use bevy::{
    app::{App, Plugin},
    asset::{AssetApp, AssetServer, io::AssetSourceBuilder},
    log::error,
};

/// Physical source root used by native SVG rasterization.
#[cfg(all(feature = "svg", not(target_arch = "wasm32")))]
#[derive(Resource, Clone)]
pub(crate) struct UiAssetSourceRoot(pub PathBuf);

/// Stable Bevy asset-source identifier for TiltUI component source files.
pub const TILT_UI_ASSET_SOURCE: &str = "tilt-ui";

/// Default physical directory containing TiltUI component source files.
pub const DEFAULT_TILT_UI_ASSET_PATH: &str = "src-ui";

/// Registers the dedicated Bevy asset source used for TiltUI component files.
///
/// This plugin must be added before `AssetPlugin`, including before
/// `DefaultPlugins`, because Bevy constructs asset sources while initializing
/// the asset server.
#[derive(Debug, Clone)]
pub struct TiltUiAssetSourcePlugin {
    root: PathBuf,
}

impl TiltUiAssetSourcePlugin {
    /// Creates a source plugin rooted at the supplied component source directory.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { root: path.into() }
    }

    /// Returns the physical directory registered for the `tilt-ui` asset source.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

impl Default for TiltUiAssetSourcePlugin {
    fn default() -> Self {
        Self::new(DEFAULT_TILT_UI_ASSET_PATH)
    }
}

impl Plugin for TiltUiAssetSourcePlugin {
    fn build(&self, app: &mut App) {
        if app.world().contains_resource::<AssetServer>() {
            error!(
                "TiltUiAssetSourcePlugin must be added before AssetPlugin or DefaultPlugins; the tilt-ui asset source was not registered"
            );
            return;
        }

        let root = self.root.to_string_lossy();
        #[cfg(all(feature = "svg", not(target_arch = "wasm32")))]
        app.insert_resource(UiAssetSourceRoot(self.root.clone()));
        app.register_asset_source(
            TILT_UI_ASSET_SOURCE,
            AssetSourceBuilder::platform_default(root.as_ref(), None),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_TILT_UI_ASSET_PATH, TILT_UI_ASSET_SOURCE, TiltUiAssetSourcePlugin};
    use std::path::Path;

    #[test]
    fn default_source_configuration_uses_tilt_ui_root() {
        let plugin = TiltUiAssetSourcePlugin::default();

        assert_eq!(TILT_UI_ASSET_SOURCE, "tilt-ui");
        assert_eq!(plugin.root(), Path::new(DEFAULT_TILT_UI_ASSET_PATH));
    }

    #[test]
    fn custom_source_root_is_preserved() {
        let plugin = TiltUiAssetSourcePlugin::new("interface");

        assert_eq!(plugin.root(), Path::new("interface"));
    }
}
