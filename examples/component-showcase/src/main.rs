use bevy::prelude::*;
use tilt_ui::{TiltUiPlugin, UiFrameRate, UiLang, UiRuntimeConfiguration};

tilt_ui::include_components!();

fn main() {
    let resolution = std::env::var("TILT_UI_SHOWCASE_SIZE")
        .ok()
        .and_then(|size| {
            let (width, height) = size.split_once('x')?;
            Some((width.parse::<u32>().ok()?, height.parse::<u32>().ok()?))
        })
        .unwrap_or((1440, 1060));
    let ui_files = UiRuntimeConfiguration::default()
        .with_themes_path("themes")
        .expect("valid themes path")
        .with_theme_names(["light", "dark"])
        .with_language_path("locales")
        .expect("valid language path");
    App::new()
        .insert_resource(UiLang::new("en-US"))
        .add_plugins(
            TiltUiPlugin::new(tilt_ui_component_catalog())
                .with_source_root(concat!(env!("CARGO_MANIFEST_DIR"), "/src-ui"))
                .with_ui_fps(UiFrameRate::Fps60)
                .with_runtime_configuration(ui_files),
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
