//! One-plugin setup for the TiltUI asset source, runtime, and optional UI camera.

use std::{
    num::NonZeroU32,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use bevy::{
    app::{App, First, Last, Plugin, PostStartup},
    asset::AssetServer,
    camera::{Camera, Camera2d},
    ecs::{
        query::Added,
        resource::Resource,
        system::{Commands, Query, Res},
    },
    prelude::With,
    ui::IsDefaultUiCamera,
    window::{PresentMode, Window},
    winit::{UpdateMode, WinitSettings},
};
use tilt_ui_runtime::{ComponentCatalog, TiltUiComponentRuntimePlugin};

use crate::{TiltUiAssetSourcePlugin, TiltUiAssetsPlugin};
#[cfg(feature = "fluent")]
use tilt_ui_runtime::{UiFluentConfig, UiFluentPlugin};

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

/// Maximum cadence for the Bevy window loop that renders TiltUI.
///
/// A slow frame or platform presentation fallback can reduce the achieved
/// rate. This affects the whole Bevy app, not only UI.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiFrameRate {
    Fps30,
    Fps45,
    Fps60,
    Custom(NonZeroU32),
}

impl UiFrameRate {
    /// Creates a custom target; zero is not a valid frame rate.
    pub fn custom(fps: u32) -> Option<Self> {
        NonZeroU32::new(fps).map(Self::Custom)
    }

    /// Returns the target updates per second.
    pub const fn fps(self) -> u32 {
        match self {
            Self::Fps30 => 30,
            Self::Fps45 => 45,
            Self::Fps60 => 60,
            Self::Custom(fps) => fps.get(),
        }
    }

    fn interval(self) -> Duration {
        Duration::from_secs_f64(1.0 / f64::from(self.fps()))
    }
}

fn winit_settings() -> WinitSettings {
    WinitSettings {
        focused_mode: UpdateMode::Continuous,
        unfocused_mode: UpdateMode::reactive_low_power(Duration::from_secs(1)),
    }
}

#[derive(Resource, Default)]
struct FramePacer {
    last_frame: Option<Instant>,
}

impl FramePacer {
    fn wait_duration(&self, now: Instant, frame_rate: UiFrameRate) -> Duration {
        self.last_frame
            .and_then(|last| last.checked_add(frame_rate.interval()))
            .map_or(Duration::ZERO, |deadline| {
                deadline.saturating_duration_since(now)
            })
    }
}

fn pace_ui_frames(frame_rate: Res<UiFrameRate>, mut pacer: bevy::ecs::system::ResMut<FramePacer>) {
    let wait = pacer.wait_duration(Instant::now(), *frame_rate);
    if !wait.is_zero() {
        thread::sleep(wait);
    }
    pacer.last_frame = Some(Instant::now());
}

fn configure_frame_presentation(mut windows: Query<&mut Window, Added<Window>>) {
    for mut window in &mut windows {
        window.present_mode = PresentMode::AutoNoVsync;
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
    frame_rate: Option<UiFrameRate>,
    #[cfg(feature = "fluent")]
    fluent: Option<UiFluentConfig>,
}

impl TiltUiPlugin {
    /// Creates a one-plugin setup using the default `src-ui` source root.
    pub fn new(catalog: ComponentCatalog) -> Self {
        Self {
            catalog,
            source_root: PathBuf::from(tilt_ui_runtime::DEFAULT_TILT_UI_ASSET_PATH),
            camera: TiltUiCameraMode::default(),
            frame_rate: None,
            #[cfg(feature = "fluent")]
            fluent: None,
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

    /// Caps the focused window loop to 30, 45, 60, or a custom FPS.
    /// Uses non-VSync presentation so targets above the display refresh rate
    /// are possible when the platform supports them.
    /// Without this option, Bevy's own frame pacing remains unchanged.
    pub fn with_ui_fps(mut self, frame_rate: UiFrameRate) -> Self {
        self.frame_rate = Some(frame_rate);
        self
    }

    /// Enables Fluent catalogs configured under the UI asset source.
    #[cfg(feature = "fluent")]
    pub fn with_localization(mut self, config: UiFluentConfig) -> Self {
        self.fluent = Some(config);
        self
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
        if let Some(frame_rate) = self.frame_rate {
            app.insert_resource(frame_rate)
                .insert_resource(winit_settings())
                .init_resource::<FramePacer>()
                .add_systems(First, configure_frame_presentation)
                .add_systems(Last, pace_ui_frames);
        }
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
        #[cfg(feature = "fluent")]
        if let Some(config) = &self.fluent {
            app.add_plugins(UiFluentPlugin::new(config.clone()));
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
    use std::time::{Duration, Instant};

    use bevy::{
        app::{App, First, Last, PostStartup},
        asset::{AssetPlugin, Assets},
        camera::{Camera, Camera2d},
        prelude::With,
        ui::IsDefaultUiCamera,
        window::{PresentMode, Window},
        winit::{UpdateMode, WinitSettings},
    };
    use tilt_ui_runtime::UiTemplateAsset;
    #[cfg(feature = "fluent")]
    use tilt_ui_runtime::{UiFluentAsset, UiFluentConfig, UiLocalization};

    use super::{
        CameraSetup, FramePacer, TiltUiCameraMode, TiltUiPlugin, UiFrameRate,
        configure_frame_presentation, ensure_ui_camera, pace_ui_frames, winit_settings,
    };

    #[test]
    fn one_plugin_installs_asset_types_after_asset_plugin() {
        let mut app = App::new();
        app.add_plugins(
            TiltUiPlugin::new(Default::default())
                .with_source_root("test-src-ui")
                .with_ui_fps(UiFrameRate::Fps45),
        );
        app.add_plugins(AssetPlugin::default());
        app.finish();
        assert!(app.world().contains_resource::<Assets<UiTemplateAsset>>());
        assert_eq!(
            app.world().resource::<WinitSettings>().focused_mode,
            UpdateMode::Continuous
        );
        assert!(app.world().contains_resource::<FramePacer>());
    }

    #[cfg(feature = "fluent")]
    #[test]
    fn one_plugin_installs_optional_fluent_assets() {
        let config = UiFluentConfig::new("en-US")
            .unwrap()
            .with_catalog("en-US", "locales/en-US.ftl")
            .unwrap();
        let mut app = App::new();
        app.add_plugins(
            TiltUiPlugin::new(Default::default())
                .with_source_root("test-src-ui")
                .with_localization(config),
        );
        app.add_plugins(bevy::app::TaskPoolPlugin::default());
        app.add_plugins(AssetPlugin::default());
        app.finish();
        assert!(app.world().contains_resource::<Assets<UiFluentAsset>>());
        assert_eq!(
            app.world()
                .resource::<UiLocalization>()
                .locale()
                .to_string(),
            "en-US"
        );
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

    #[test]
    fn frame_rate_presets_and_custom_values_set_distinct_deadlines() {
        let last = Instant::now();
        let pacer = FramePacer {
            last_frame: Some(last),
        };
        for (frame_rate, fps) in [
            (UiFrameRate::custom(5).unwrap(), 5),
            (UiFrameRate::custom(10).unwrap(), 10),
            (UiFrameRate::Fps30, 30),
            (UiFrameRate::Fps45, 45),
            (UiFrameRate::Fps60, 60),
            (UiFrameRate::custom(120).unwrap(), 120),
        ] {
            assert_eq!(frame_rate.fps(), fps);
            assert_eq!(
                pacer.wait_duration(last + Duration::from_millis(1), frame_rate),
                frame_rate.interval() - Duration::from_millis(1)
            );
        }
        assert_eq!(winit_settings().focused_mode, UpdateMode::Continuous);
        assert!(UiFrameRate::custom(0).is_none());
    }

    #[test]
    fn input_bursts_and_runtime_changes_still_obey_the_cap() {
        let mut app = App::new();
        app.insert_resource(UiFrameRate::Fps60)
            .init_resource::<FramePacer>()
            .add_systems(Last, pace_ui_frames);
        app.update();
        app.world_mut()
            .insert_resource(UiFrameRate::custom(5).unwrap());
        let started = Instant::now();
        app.update();
        assert!(started.elapsed() >= Duration::from_millis(185));
    }

    #[test]
    fn configured_fps_uses_non_vsync_presentation() {
        let mut app = App::new();
        app.add_systems(First, configure_frame_presentation);
        let window = app.world_mut().spawn(Window::default()).id();
        app.update();
        assert_eq!(
            app.world().get::<Window>(window).unwrap().present_mode,
            PresentMode::AutoNoVsync
        );
    }
}
