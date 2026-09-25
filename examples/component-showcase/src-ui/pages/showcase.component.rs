//! State and behavior for the widget showcase component.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use tilt_ui::{
    DialogClosed, DialogConfig, DialogState, EditableTextCommitted, ElementId, HtmlClick,
    HtmlEvent, ShowDialog, SliderChanged, SliderCommitted, component_init, component_update,
    html_fn, html_shared, open_dialog, spawn_component,
};

use super::tilt_ui_component_id;

#[html_shared]
#[derive(Resource, serde::Serialize)]
struct ShowcaseState {
    progress: f32,
    progress_label: u32,
    slider_value: u32,
    #[serde(skip)]
    started_at: f32,
}

#[component_init]
fn show_page(mut commands: Commands) {
    commands.insert_resource(ShowcaseState {
        progress: 40.0,
        progress_label: 40,
        slider_value: 75,
        started_at: -2.666,
    });
    spawn_component(
        &mut commands,
        tilt_ui_component_id("showcase").expect("showcase metadata"),
    );
}

#[component_update]
fn animate_progress(time: Res<Time>, mut state: ResMut<ShowcaseState>) {
    let next = ((time.elapsed_secs() - state.started_at) * 15.0) % 100.0;
    if (state.progress - next).abs() > 0.01 {
        state.progress = next;
        state.progress_label = next.round() as u32;
    }
}

#[component_update]
fn reflect_slider_value(
    mut changes: MessageReader<SliderChanged>,
    mut state: ResMut<ShowcaseState>,
) {
    for change in changes.read() {
        if change.upper.is_none() {
            state.slider_value = change.value.round() as u32;
        }
    }
}

#[html_fn("reset_progress")]
fn reset_progress(In(_event): In<HtmlEvent>, time: Res<Time>, mut state: ResMut<ShowcaseState>) {
    state.progress = 0.0;
    state.progress_label = 0;
    state.started_at = time.elapsed_secs();
}

#[html_fn("open_rust_dialog")]
fn open_rust_dialog(In(click): In<HtmlClick>, mut dialogs: MessageWriter<ShowDialog>) {
    dialogs.write(ShowDialog {
        parent: click.target,
        config: DialogConfig::question("Continue?", "This dialog was created in Rust."),
    });
}

#[component_update]
fn log_text_commits(mut events: MessageReader<EditableTextCommitted>) {
    for event in events.read() {
        debug!(?event.entity, value = %event.value, "Input committed");
    }
}

#[component_update]
fn log_slider_commits(mut events: MessageReader<SliderCommitted>) {
    for event in events.read() {
        debug!(?event.entity, event.value, ?event.upper, "Slider committed");
    }
}

#[component_update]
fn log_dialog_results(mut events: MessageReader<DialogClosed>) {
    for event in events.read() {
        info!(?event.entity, ?event.result, "Dialog closed");
    }
}

#[component_update]
fn capture_showcase(
    mut commands: Commands,
    time: Res<Time>,
    mut captured: Local<bool>,
    mut dialog_preview_opened: Local<bool>,
    dialogs: Query<(Entity, &ElementId), With<DialogState>>,
    elements: Query<(Entity, &ElementId)>,
    mut requests: MessageWriter<ShowDialog>,
) {
    if !*dialog_preview_opened
        && time.elapsed_secs() >= 1.5
        && std::env::var_os("TILT_UI_SCREENSHOT").is_some()
        && std::env::var("TILT_UI_SCREENSHOT_DIALOG").ok().as_deref() == Some("rust")
    {
        if let Some((parent, _)) = elements.iter().find(|(_, id)| id.0 == "open-info-dialog") {
            requests.write(ShowDialog {
                parent,
                config: DialogConfig::question("Continue?", "This dialog was created in Rust."),
            });
            *dialog_preview_opened = true;
        }
    }
    if *captured || time.elapsed_secs() < 2.0 {
        return;
    }
    let Ok(path) = std::env::var("TILT_UI_SCREENSHOT") else {
        *captured = true;
        return;
    };
    if let Ok(id) = std::env::var("TILT_UI_SCREENSHOT_DIALOG") {
        if let Some((entity, _)) = dialogs.iter().find(|(_, element_id)| element_id.0 == id) {
            commands.queue(move |world: &mut World| {
                open_dialog(world, entity);
            });
        }
    }
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
    *captured = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slider_label_follows_slider_changes() {
        let mut app = App::new();
        app.add_message::<SliderChanged>()
            .insert_resource(ShowcaseState {
                progress: 40.0,
                progress_label: 40,
                slider_value: 75,
                started_at: 0.0,
            })
            .add_systems(Update, reflect_slider_value);
        let slider = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<SliderChanged>>()
            .write(SliderChanged {
                entity: slider,
                value: 42.0,
                upper: None,
            });
        app.update();
        assert_eq!(app.world().resource::<ShowcaseState>().slider_value, 42);
    }
}
