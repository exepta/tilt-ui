use bevy::prelude::*;
use tilt_ui::{TiltUiPlugin, UiFrameRate, UiRuntimeConfiguration};

tilt_ui::include_components!();

fn main() {
    let ui_files = UiRuntimeConfiguration::default()
        .with_themes_path("themes")
        .expect("valid theme directory")
        .with_theme_names(["light", "dark"]);

    App::new()
        .add_plugins(
            TiltUiPlugin::new(tilt_ui_component_catalog())
                .with_source_root(concat!(env!("CARGO_MANIFEST_DIR"), "/src-ui"))
                .with_runtime_configuration(ui_files)
                .with_ui_fps(UiFrameRate::Fps60),
        )
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "TiltUI · Icon Atlas".into(),
                resolution: (1440, 980).into(),
                ..default()
            }),
            ..default()
        }))
        .run();
}
