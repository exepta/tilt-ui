use bevy::{asset::AssetPlugin, prelude::*};
use tilt_ui::prelude::*;

tilt_ui::include_components!();

fn main() {
    App::new()
        .add_plugins(
            TiltUiPlugin::new(tilt_ui_component_catalog())
                .with_source_root(concat!(env!("CARGO_MANIFEST_DIR"), "/src-ui")),
        )
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            watch_for_changes_override: Some(true),
            ..default()
        }))
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    let page = tilt_ui_component_id("live").expect("live component metadata");
    spawn_component(&mut commands, page);
}
