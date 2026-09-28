use __tilt_ui_generated_components::main_component::{LifecycleView, track_ui_lifecycle};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use tilt_ui::prelude::*;

tilt_ui::include_components!();

fn main() {
    let resolution = std::env::var("TILT_UI_BASIC_SIZE")
        .ok()
        .and_then(|size| {
            let (width, height) = size.split_once('x')?;
            Some((width.parse::<u32>().ok()?, height.parse::<u32>().ok()?))
        })
        .unwrap_or((1280, 900));
    App::new()
        .add_plugins(
            TiltUiPlugin::new(tilt_ui_component_catalog())
                .with_source_root(concat!(env!("CARGO_MANIFEST_DIR"), "/src-ui"))
                .with_ui_fps(UiFrameRate::Fps60),
        )
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "TiltUI basic example".into(),
                resolution: resolution.into(),
                ..default()
            }),
            ..default()
        }))
        .init_resource::<LifecycleView>()
        .add_systems(Update, track_ui_lifecycle.after(UiStateRuntimeSet::Observe))
        .add_systems(Update, capture_example)
        .run();
}

fn capture_example(mut commands: Commands, time: Res<Time>, mut captured: Local<bool>) {
    if *captured || time.elapsed_secs() < 2.0 {
        return;
    }
    if let Ok(path) = std::env::var("TILT_UI_BASIC_SCREENSHOT") {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
    *captured = true;
}
