use bevy::prelude::*;
use tilt_ui::TiltUiPlugin;

tilt_ui::include_components!();

fn main() {
    App::new()
        .add_plugins(
            TiltUiPlugin::new(tilt_ui_component_catalog())
                .with_source_root(concat!(env!("CARGO_MANIFEST_DIR"), "/src-ui")),
        )
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "TiltUI widget showcase".into(),
                resolution: (1440, 1060).into(),
                ..default()
            }),
            ..default()
        }))
        .run();
}
