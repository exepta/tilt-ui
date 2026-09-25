use bevy::{
    app::{App, Plugin},
    asset::AssetServer,
    log::error,
};

#[cfg(any(feature = "html", feature = "css"))]
use bevy::asset::AssetApp;

#[cfg(feature = "css")]
use super::stylesheet::{UiStyleSheetAsset, UiStyleSheetAssetLoader};
#[cfg(feature = "html")]
use super::template::{UiTemplateAsset, UiTemplateAssetLoader};

/// Registers TiltUI asset types and loaders with Bevy's asset infrastructure.
///
/// This plugin must be added after `AssetPlugin`, which is normally supplied
/// by `DefaultPlugins`. Use `TiltUiAssetSourcePlugin` before `AssetPlugin` to
/// register the dedicated `tilt-ui` source.
#[derive(Debug, Default, Clone, Copy)]
pub struct TiltUiAssetsPlugin;

impl Plugin for TiltUiAssetsPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<AssetServer>() {
            error!(
                "TiltUiAssetsPlugin requires AssetPlugin; add it after AssetPlugin or DefaultPlugins"
            );
            return;
        }

        #[cfg(feature = "html")]
        app.init_asset::<UiTemplateAsset>()
            .register_asset_loader(UiTemplateAssetLoader);

        #[cfg(feature = "css")]
        app.init_asset::<UiStyleSheetAsset>()
            .register_asset_loader(UiStyleSheetAssetLoader);
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        app::App,
        asset::{AssetPlugin, Assets},
    };

    use super::TiltUiAssetsPlugin;
    use crate::assets::source::TiltUiAssetSourcePlugin;

    #[cfg(feature = "css")]
    use crate::assets::stylesheet::UiStyleSheetAsset;
    #[cfg(feature = "html")]
    use crate::assets::template::UiTemplateAsset;

    #[test]
    fn initializes_tilt_ui_asset_stores_after_asset_plugin() {
        let mut app = App::new();
        app.add_plugins(TiltUiAssetSourcePlugin::new("test-src-ui"));
        app.add_plugins(AssetPlugin::default());
        app.add_plugins(TiltUiAssetsPlugin);

        #[cfg(feature = "html")]
        assert!(app.world().contains_resource::<Assets<UiTemplateAsset>>());

        #[cfg(feature = "css")]
        assert!(app.world().contains_resource::<Assets<UiStyleSheetAsset>>());
    }
}
