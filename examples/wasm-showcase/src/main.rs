use bevy::prelude::*;
use tilt_ui::TiltUiPlugin;

tilt_ui::include_components!();

#[cfg(target_arch = "wasm32")]
const UI_SOURCE_ROOT: &str = "src-ui";
#[cfg(not(target_arch = "wasm32"))]
const UI_SOURCE_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src-ui");

fn main() {
    App::new()
        .add_plugins(
            TiltUiPlugin::new(tilt_ui_component_catalog()).with_source_root(UI_SOURCE_ROOT),
        )
        .add_plugins(DefaultPlugins)
        .run();
}
