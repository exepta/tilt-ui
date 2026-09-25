use bevy::prelude::*;
use tilt_ui::prelude::*;
use tilt_ui::{ControlActivated, ControlCheckedChanged, HtmlEvent, component_init, html_fn};

tilt_ui::include_components!();

fn main() {
    App::new()
        .add_plugins(
            TiltUiPlugin::new(tilt_ui_component_catalog())
                .with_source_root(concat!(env!("CARGO_MANIFEST_DIR"), "/src-ui")),
        )
        .add_plugins(DefaultPlugins)
        .add_systems(Update, (log_control_activation, log_checked_change))
        .run();
}

fn log_control_activation(mut activations: MessageReader<ControlActivated>) {
    for activation in activations.read() {
        info!(?activation.entity, "Button activated");
    }
}

fn log_checked_change(mut changes: MessageReader<ControlCheckedChanged>) {
    for change in changes.read() {
        info!(?change.entity, change.checked, "Control checked state changed");
    }
}

#[component_init]
fn request_main_component(mut commands: Commands) {
    let main = tilt_ui_component_id("main").expect("generated main page metadata");
    spawn_component(&mut commands, main);
}

#[html_fn("play")]
fn play(In(event): In<HtmlEvent>) {
    info!(?event.target, "Play pressed");
}

#[cfg(feature = "extended-framework")]
#[allow(dead_code)]
#[path = "../../component-showcase/src-ui/components/test.component.rs"]
mod test_component_mod;

