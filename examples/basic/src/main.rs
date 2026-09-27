use __tilt_ui_generated_components::main_component::{LifecycleView, track_ui_lifecycle};
use bevy::prelude::*;
use tilt_ui::prelude::*;

tilt_ui::include_components!();

fn main() {
    App::new()
        .add_plugins(
            TiltUiPlugin::new(tilt_ui_component_catalog())
                .with_source_root(concat!(env!("CARGO_MANIFEST_DIR"), "/src-ui"))
                .with_ui_fps(UiFrameRate::Fps60),
        )
        .add_plugins(DefaultPlugins)
        .init_resource::<LifecycleView>()
        .add_systems(Update, track_ui_lifecycle.after(UiStateRuntimeSet::Observe))
        .run();
}
