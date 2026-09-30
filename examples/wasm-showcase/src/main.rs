use bevy::prelude::*;
use tilt_ui::{TiltUiPlugin, UiFluentConfig};

tilt_ui::include_components!();

#[cfg(target_arch = "wasm32")]
const UI_SOURCE_ROOT: &str = "src-ui";
#[cfg(not(target_arch = "wasm32"))]
const UI_SOURCE_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src-ui");

fn main() {
    let localization = UiFluentConfig::new("en-US")
        .expect("valid fallback")
        .with_catalog("en-US", "locales/en-US.ftl")
        .expect("valid English catalog")
        .with_catalog("de-DE", "locales/de-DE.ftl")
        .expect("valid German catalog");
    App::new()
        .add_plugins(
            TiltUiPlugin::new(tilt_ui_component_catalog())
                .with_source_root(UI_SOURCE_ROOT)
                .with_localization(localization),
        )
        .add_plugins(DefaultPlugins)
        .run();
}
