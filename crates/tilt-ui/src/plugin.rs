//! One-plugin setup for the TiltUI asset source, runtime, and optional UI camera.

use std::path::{Path, PathBuf};

use bevy::{
    app::{App, Plugin, PostStartup},
    asset::AssetServer,
    camera::{Camera, Camera2d},
    ecs::{
        resource::Resource,
        system::{Commands, Query, Res},
    },
    prelude::With,
    ui::IsDefaultUiCamera,
};
use tilt_ui_runtime::{ComponentCatalog, TiltUiComponentRuntimePlugin};

use crate::{TiltUiAssetSourcePlugin, TiltUiAssetsPlugin};

/// Controls whether TiltUI creates a default 2D UI camera.
#[derive(Debug, Clone)]
pub enum TiltUiCameraMode {
    /// Spawn a camera after user startup systems, unless one is already marked default.
    Automatic(Camera),
    /// Use a camera supplied by the application. Mark it with `IsDefaultUiCamera`.
    Manual,
}

impl Default for TiltUiCameraMode {
    fn default() -> Self {
        Self::Automatic(Camera::default())
    }
}

#[derive(Resource, Clone)]
struct CameraSetup(TiltUiCameraMode);

/// Installs the TiltUI asset source, asset loaders, component runtime, and controls.
///
/// Add this plugin before `DefaultPlugins` so Bevy can register the `tilt-ui://`
/// asset source before it creates the asset server.
#[derive(Debug, Clone)]
pub struct TiltUiPlugin {
    catalog: ComponentCatalog,
    source_root: PathBuf,
    camera: TiltUiCameraMode,
}

impl TiltUiPlugin {
    /// Creates a one-plugin setup using the default `src-ui` source root.
    pub fn new(catalog: ComponentCatalog) -> Self {
        Self {
            catalog,
            source_root: PathBuf::from(tilt_ui_runtime::DEFAULT_TILT_UI_ASSET_PATH),
            camera: TiltUiCameraMode::default(),
        }
    }

    /// Sets the directory containing component triplets and other UI assets.
    pub fn with_source_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.source_root = root.into();
        self
    }

    /// Configures automatic or application-managed UI camera creation.
    pub fn with_camera_mode(mut self, mode: TiltUiCameraMode) -> Self {
        self.camera = mode;
        self
    }

    /// Uses this Bevy camera configuration if TiltUI creates the UI camera.
    pub fn with_camera(self, camera: Camera) -> Self {
        self.with_camera_mode(TiltUiCameraMode::Automatic(camera))
    }

    /// Leaves camera creation to the application.
    pub fn without_camera(self) -> Self {
        self.with_camera_mode(TiltUiCameraMode::Manual)
    }

    /// Returns the configured UI source directory.
    pub fn source_root(&self) -> &Path {
        &self.source_root
    }
}

impl Plugin for TiltUiPlugin {
    fn build(&self, app: &mut App) {
        assert!(
            !app.world().contains_resource::<AssetServer>(),
            "TiltUiPlugin must be added before DefaultPlugins or AssetPlugin"
        );
        TiltUiAssetSourcePlugin::new(self.source_root.clone()).build(app);
        app.insert_resource(CameraSetup(self.camera.clone()))
            .add_systems(PostStartup, ensure_ui_camera);
    }

    fn finish(&self, app: &mut App) {
        assert!(
            app.world().contains_resource::<AssetServer>(),
            "TiltUiPlugin requires DefaultPlugins or AssetPlugin after it"
        );
        if !app.is_plugin_added::<TiltUiAssetsPlugin>() {
            app.add_plugins(TiltUiAssetsPlugin);
        }
        if !app.is_plugin_added::<TiltUiComponentRuntimePlugin>() {
            app.add_plugins(TiltUiComponentRuntimePlugin::new(self.catalog));
        }
    }
}

fn ensure_ui_camera(
    mut commands: Commands,
    existing: Query<(), With<IsDefaultUiCamera>>,
    setup: Res<CameraSetup>,
) {
    if !existing.is_empty() {
        return;
    }
    if let TiltUiCameraMode::Automatic(camera) = &setup.0 {
        commands.spawn((Camera2d, camera.clone(), IsDefaultUiCamera));
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        app::{App, PostStartup},
        asset::{AssetPlugin, Assets},
        camera::{Camera, Camera2d},
        prelude::With,
        ui::IsDefaultUiCamera,
    };
    use tilt_ui_runtime::UiTemplateAsset;

    use super::{CameraSetup, TiltUiCameraMode, TiltUiPlugin, ensure_ui_camera};

    #[test]
    fn one_plugin_installs_asset_types_after_asset_plugin() {
        let mut app = App::new();
        app.add_plugins(TiltUiPlugin::new(Default::default()).with_source_root("test-src-ui"));
        app.add_plugins(AssetPlugin::default());
        app.finish();
        assert!(app.world().contains_resource::<Assets<UiTemplateAsset>>());
    }

    #[test]
    fn automatic_camera_respects_an_existing_default_camera() {
        let mut app = App::new();
        app.insert_resource(CameraSetup(TiltUiCameraMode::Automatic(Camera::default())))
            .add_systems(PostStartup, ensure_ui_camera);
        app.world_mut().spawn((Camera2d, IsDefaultUiCamera));
        app.update();
        let mut cameras = app
            .world_mut()
            .query_filtered::<(), With<IsDefaultUiCamera>>();
        assert_eq!(cameras.iter(app.world()).count(), 1);
    }
}
