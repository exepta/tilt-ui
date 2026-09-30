use bevy::{
    app::{App, Plugin},
    asset::AssetServer,
    log::error,
};

#[cfg(any(feature = "html", feature = "css", feature = "svg"))]
use bevy::asset::AssetApp;

#[cfg(feature = "css")]
use super::stylesheet::{UiStyleSheetAsset, UiStyleSheetAssetLoader};
#[cfg(feature = "svg")]
use super::svg::SvgImageLoader;
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

        #[cfg(feature = "svg")]
        {
            // ImagePlugin has already populated this store when DefaultPlugins
            // is in use. Reinitializing it drops Bevy's fallback textures.
            if !app
                .world()
                .contains_resource::<bevy::asset::Assets<bevy::image::Image>>()
            {
                app.init_asset::<bevy::image::Image>();
            }
            app.register_asset_loader(SvgImageLoader);
        }
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

    #[cfg(all(feature = "svg", not(target_arch = "wasm32")))]
    #[test]
    fn loads_svg_as_an_image_through_the_tilt_ui_asset_source() {
        use std::time::{Duration, SystemTime};

        use bevy::{asset::AssetServer, image::Image};

        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("tilt-ui-svg-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("mark.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="28" height="18"><rect width="28" height="18"/></svg>"#,
        )
        .unwrap();

        let mut app = App::new();
        app.add_plugins(bevy::app::TaskPoolPlugin::default());
        app.add_plugins(TiltUiAssetSourcePlugin::new(&root));
        app.add_plugins(AssetPlugin::default());
        app.add_plugins(TiltUiAssetsPlugin);
        let handle = app
            .world()
            .resource::<AssetServer>()
            .load::<Image>("tilt-ui://mark.svg");
        let mut size = None;
        for _ in 0..100 {
            app.update();
            size = app
                .world()
                .resource::<Assets<Image>>()
                .get(&handle)
                .map(|image| image.texture_descriptor.size);
            if size.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        drop(app);
        std::fs::remove_dir_all(root).unwrap();
        let size = size.expect("SVG asset should load through AssetServer");
        assert_eq!((size.width, size.height), (28, 18));
    }

    #[cfg(feature = "svg")]
    #[test]
    fn svg_loader_preserves_images_registered_before_tilt_ui() {
        use bevy::{asset::AssetApp, image::Image};

        let mut app = App::new();
        app.add_plugins(AssetPlugin::default());
        app.init_asset::<Image>();
        let existing = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::default());

        app.add_plugins(TiltUiAssetsPlugin);

        assert!(
            app.world()
                .resource::<Assets<Image>>()
                .get(&existing)
                .is_some()
        );
    }

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
