//! One-plugin setup for the TiltUI asset source, runtime, and optional UI camera.

use std::{
    num::NonZeroU32,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use bevy::{
    app::{App, First, Last, Plugin, PostStartup, Update},
    asset::AssetServer,
    camera::{Camera, Camera2d, Hdr, visibility::RenderLayers},
    ecs::{
        component::Component,
        entity::Entity,
        query::Added,
        resource::Resource,
        system::{Commands, Query, Res},
    },
    prelude::{DetectChanges, With},
    ui::IsDefaultUiCamera,
    window::{PresentMode, Window},
    winit::{UpdateMode, WinitSettings},
};
use tilt_ui_runtime::{ComponentCatalog, TiltUiComponentRuntimePlugin, UiRuntimeConfiguration};

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
struct CameraSetup {
    mode: TiltUiCameraMode,
}

/// Change this resource to update the camera created by TiltUI at runtime.
#[derive(Resource, Debug, Clone, Default)]
pub struct UiCameraConfiguration {
    pub render_layers: Option<RenderLayers>,
    pub hdr: bool,
}

#[derive(Component)]
struct TiltUiManagedCamera;

/// Installs the TiltUI asset source, asset loaders, component runtime, and controls.
///
/// Add this plugin before `DefaultPlugins` so Bevy can register the `tilt-ui://`
/// asset source before it creates the asset server.
#[derive(Debug, Clone)]
pub struct TiltUiPlugin {
    catalog: ComponentCatalog,
    source_root: PathBuf,
    runtime_configuration: UiRuntimeConfiguration,
    camera: TiltUiCameraMode,
    render_layers: Option<RenderLayers>,
    hdr: bool,
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
            runtime_configuration: UiRuntimeConfiguration::default(),
            camera: TiltUiCameraMode::default(),
            render_layers: None,
            hdr: false,
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

    /// Sets mutable paths for component assets, images, themes, and Fluent catalogs.
    pub fn with_runtime_configuration(mut self, config: UiRuntimeConfiguration) -> Self {
        config
            .validate()
            .expect("invalid TiltUI runtime configuration");
        self.runtime_configuration = config;
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

    /// Applies render layers to a camera created by TiltUI.
    pub fn with_render_layers(mut self, layers: RenderLayers) -> Self {
        self.render_layers = Some(layers);
        self
    }

    /// Enables or disables HDR on a camera created by TiltUI.
    pub fn with_hdr(mut self, enabled: bool) -> Self {
        self.hdr = enabled;
        self
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
        app.insert_resource(CameraSetup {
            mode: self.camera.clone(),
        })
        .insert_resource(self.runtime_configuration.clone())
        .insert_resource(UiCameraConfiguration {
            render_layers: self.render_layers.clone(),
            hdr: self.hdr,
        })
        .add_systems(PostStartup, ensure_ui_camera)
        .add_systems(Update, update_ui_camera);
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
        if !app.is_plugin_added::<UiFluentPlugin>() {
            let config = self
                .fluent
                .clone()
                .unwrap_or_else(|| UiFluentConfig::new("en-US").expect("valid default locale"));
            app.add_plugins(UiFluentPlugin::new(config));
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            app.insert_resource(crate::discovery::UiDirectoryRoot(self.source_root.clone()))
                .init_resource::<crate::discovery::DiscoveredUiFiles>();
            if let Err(error) = crate::discovery::refresh_ui_directories(app.world_mut()) {
                bevy::log::warn!("TiltUI directory discovery failed: {error}");
            }
            app.add_systems(Update, crate::discovery::refresh_changed_directories);
        }
    }
}

fn ensure_ui_camera(
    mut commands: Commands,
    existing: Query<(), With<IsDefaultUiCamera>>,
    setup: Res<CameraSetup>,
    config: Res<UiCameraConfiguration>,
) {
    if !existing.is_empty() {
        return;
    }
    if let TiltUiCameraMode::Automatic(camera) = &setup.mode {
        let mut entity = commands.spawn((
            Camera2d,
            camera.clone(),
            IsDefaultUiCamera,
            TiltUiManagedCamera,
        ));
        if let Some(layers) = &config.render_layers {
            entity.insert(layers.clone());
        }
        if config.hdr {
            entity.insert(Hdr);
        }
    }
}

fn update_ui_camera(
    mut commands: Commands,
    config: Res<UiCameraConfiguration>,
    cameras: Query<Entity, With<TiltUiManagedCamera>>,
) {
    if !config.is_changed() {
        return;
    }
    for camera in &cameras {
        let mut entity = commands.entity(camera);
        if let Some(layers) = &config.render_layers {
            entity.insert(layers.clone());
        } else {
            entity.remove::<RenderLayers>();
        }
        if config.hdr {
            entity.insert(Hdr);
        } else {
            entity.remove::<Hdr>();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use bevy::{
        app::{App, First, Last, PostStartup},
        asset::{AssetPlugin, Assets},
        camera::{Camera, Camera2d, Hdr, visibility::RenderLayers},
        prelude::{Entity, With},
        ui::IsDefaultUiCamera,
        window::{PresentMode, Window},
        winit::{UpdateMode, WinitSettings},
    };
    use tilt_ui_runtime::UiTemplateAsset;
    #[cfg(feature = "fluent")]
    use tilt_ui_runtime::{UiFluentAsset, UiFluentConfig, UiLang, UiLocalization};

    use super::{
        CameraSetup, FramePacer, TiltUiCameraMode, TiltUiPlugin, UiCameraConfiguration,
        UiFrameRate, configure_frame_presentation, ensure_ui_camera, pace_ui_frames,
        update_ui_camera, winit_settings,
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
        app.insert_resource(UiLang::new("en-US"));
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

    #[cfg(all(feature = "fluent", not(target_arch = "wasm32")))]
    #[test]
    fn one_plugin_discovers_themes_and_languages_before_startup() {
        let root =
            std::env::temp_dir().join(format!("tilt-ui-plugin-discovery-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("styles")).unwrap();
        std::fs::create_dir_all(root.join("words")).unwrap();
        std::fs::write(root.join("styles/night.css"), "body { color: #ffffff; }").unwrap();
        std::fs::write(root.join("words/de-DE.ftl"), "title = Hallo").unwrap();
        let config = tilt_ui_runtime::UiRuntimeConfiguration::default()
            .with_themes_path("styles")
            .unwrap()
            .with_language_path("words")
            .unwrap();
        let mut app = App::new();
        app.add_plugins(
            TiltUiPlugin::new(Default::default())
                .with_source_root(&root)
                .with_runtime_configuration(config),
        );
        app.add_plugins(bevy::app::TaskPoolPlugin::default());
        app.add_plugins(AssetPlugin::default());
        app.finish();
        assert!(
            app.world()
                .resource::<tilt_ui_runtime::UiThemes>()
                .get("night")
                .is_some()
        );
        app.world_mut()
            .resource_mut::<UiLocalization>()
            .set_locale("de-DE")
            .unwrap();
        assert_eq!(
            app.world()
                .resource::<UiLocalization>()
                .translate("title", None)
                .as_deref(),
            Some("Hallo")
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn automatic_camera_respects_an_existing_default_camera() {
        let mut app = App::new();
        app.insert_resource(CameraSetup {
            mode: TiltUiCameraMode::Automatic(Camera::default()),
        })
        .insert_resource(UiCameraConfiguration::default())
        .add_systems(PostStartup, ensure_ui_camera);
        app.world_mut().spawn((Camera2d, IsDefaultUiCamera));
        app.update();
        let mut cameras = app
            .world_mut()
            .query_filtered::<(), With<IsDefaultUiCamera>>();
        assert_eq!(cameras.iter(app.world()).count(), 1);
    }

    #[test]
    fn automatic_camera_receives_render_layers_and_hdr() {
        let mut app = App::new();
        let layers = RenderLayers::from_layers(&[1, 2]);
        app.insert_resource(CameraSetup {
            mode: TiltUiCameraMode::Automatic(Camera::default()),
        })
        .insert_resource(UiCameraConfiguration {
            render_layers: Some(layers.clone()),
            hdr: true,
        })
        .add_systems(PostStartup, ensure_ui_camera);
        app.update();
        let mut cameras = app
            .world_mut()
            .query_filtered::<(Entity, &RenderLayers, &Hdr), With<IsDefaultUiCamera>>();
        let entries = cameras.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(entries.len(), 1);
        assert_eq!(*entries[0].1, layers);
    }

    #[test]
    fn managed_camera_configuration_changes_at_runtime() {
        let mut app = App::new();
        app.insert_resource(CameraSetup {
            mode: TiltUiCameraMode::Automatic(Camera::default()),
        })
        .insert_resource(UiCameraConfiguration::default())
        .add_systems(PostStartup, ensure_ui_camera)
        .add_systems(bevy::app::Update, update_ui_camera);
        app.update();
        let layers = RenderLayers::layer(3);
        *app.world_mut().resource_mut::<UiCameraConfiguration>() = UiCameraConfiguration {
            render_layers: Some(layers.clone()),
            hdr: true,
        };
        app.update();
        let mut cameras = app
            .world_mut()
            .query_filtered::<(Entity, &RenderLayers, &Hdr), With<IsDefaultUiCamera>>();
        assert_eq!(cameras.iter(app.world()).count(), 1);
        *app.world_mut().resource_mut::<UiCameraConfiguration>() = UiCameraConfiguration::default();
        app.update();
        let mut cameras = app.world_mut().query_filtered::<(Entity, Option<&RenderLayers>, Option<&Hdr>), With<IsDefaultUiCamera>>();
        let (_, layers, hdr) = cameras.single(app.world()).unwrap();
        assert!(layers.is_none());
        assert!(hdr.is_none());
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
