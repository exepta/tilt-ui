use bevy::prelude::*;
use tilt_ui::{TiltUiPlugin, UiFluentConfig, UiFrameRate};

tilt_ui::include_components!();

fn main() {
    let resolution = std::env::var("TILT_UI_SHOWCASE_SIZE")
        .ok()
        .and_then(|size| {
            let (width, height) = size.split_once('x')?;
            Some((width.parse::<u32>().ok()?, height.parse::<u32>().ok()?))
        })
        .unwrap_or((1440, 1060));
    let fluent = UiFluentConfig::new("en-US")
        .expect("valid fallback locale")
        .with_catalog("en-US", "locales/en-US.ftl")
        .expect("valid English locale")
        .with_catalog("de-DE", "locales/de-DE.ftl")
        .expect("valid German locale");
    App::new()
        .add_plugins(
            TiltUiPlugin::new(tilt_ui_component_catalog())
                .with_source_root(concat!(env!("CARGO_MANIFEST_DIR"), "/src-ui"))
                .with_ui_fps(UiFrameRate::Fps60)
                .with_localization(fluent),
        )
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "TiltUI widget showcase".into(),
                resolution: resolution.into(),
                ..default()
            }),
            ..default()
        }))
        .run();
}
