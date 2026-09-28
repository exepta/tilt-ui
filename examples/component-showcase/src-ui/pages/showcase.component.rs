//! State and behavior for the widget showcase component.

use bevy::diagnostic::FrameCount;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use tilt_ui::{
    ChoiceBoxParts, ControlChecked, DialogClosed, DialogConfig, DialogState, EditableTextChanged,
    EditableTextCommitted, ElementId, HtmlClick, HtmlEvent, OptionData, OptionSelectionChanged,
    ShowDialog, SliderChanged, SliderCommitted, UiFrameRate, UiLocalization, UiThemes,
    component_init, component_update, html_fn, html_shared, open_dialog, set_option_selected,
    set_progress_value, spawn_component, switch_ui_theme,
};

use super::tilt_ui_component_id;

// Legacy-style metadata is optional; the build script also accepts convention-only components.
#[allow(dead_code)]
struct ShowcaseComponentMetadata {
    template_name: &'static str,
    template_file: &'static str,
    styles: &'static [&'static str],
}

#[allow(dead_code)]
const SHOWCASE_COMPONENT: ShowcaseComponentMetadata = ShowcaseComponentMetadata {
    template_name: "showcase",
    template_file: "showcase.component.html",
    styles: &["showcase.component.css", "showcase-runtime.css"],
};

#[html_shared]
#[derive(Resource, serde::Serialize)]
struct ShowcaseState {
    slider_value: u32,
    event_name: String,
    event_detail: String,
    table_feedback: String,
    custom_fps: u32,
    custom_fps_disabled: bool,
    current_fps: String,
    #[serde(skip)]
    started_at: f32,
}

#[component_init]
fn show_page(mut commands: Commands, mut localization: ResMut<UiLocalization>) {
    if let Ok(locale) = std::env::var("TILT_UI_SHOWCASE_LANG") {
        if let Err(error) = localization.set_locale(&locale) {
            warn!("Invalid showcase language {locale:?}: {error}");
        }
    }
    let start_theme = std::env::var("TILT_UI_SHOWCASE_THEME").unwrap_or_else(|_| "light".into());
    commands.queue(move |world: &mut World| {
        if let Err(error) = switch_ui_theme(world, &start_theme) {
            warn!("Invalid showcase theme {start_theme:?}: {error}");
            switch_ui_theme(world, "light").expect("registered light theme");
        }
    });
    commands.insert_resource(ShowcaseState {
        slider_value: 75,
        event_name: "init".into(),
        event_detail: String::new(),
        table_feedback: "—".into(),
        custom_fps: 120,
        custom_fps_disabled: true,
        current_fps: "--".into(),
        started_at: -2.666,
    });
    spawn_component(
        &mut commands,
        tilt_ui_component_id("showcase").expect("showcase metadata"),
    );
}

#[component_update]
fn animate_progress(
    time: Res<Time>,
    state: Res<ShowcaseState>,
    mut commands: Commands,
    elements: Query<(Entity, &ElementId)>,
    children: Query<&Children>,
    mut labels: Query<&mut Text>,
    mut targets: Local<Option<(Entity, Entity)>>,
) {
    let next = ((time.elapsed_secs() - state.started_at) * 15.0) % 100.0;
    if targets.is_none_or(|(progress, label)| {
        elements.get(progress).is_err() || labels.get(label).is_err()
    }) {
        let progress = elements
            .iter()
            .find(|(_, id)| id.0 == "demo-progress")
            .map(|(entity, _)| entity);
        let label_parent = elements
            .iter()
            .find(|(_, id)| id.0 == "demo-progress-label")
            .map(|(entity, _)| entity);
        let label = label_parent
            .and_then(|entity| children.get(entity).ok())
            .and_then(|children| children.iter().find(|child| labels.get(*child).is_ok()));
        *targets = progress.zip(label);
    }
    let Some((progress, label)) = *targets else { return; };
    commands.queue(move |world: &mut World| {
        set_progress_value(world, progress, next);
    });
    if let Ok(mut text) = labels.get_mut(label) {
        let value = format!("{}%", next.round() as u32);
        if text.0 != value {
            text.0 = value;
        }
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

#[component_update]
fn change_language(
    mut changes: MessageReader<OptionSelectionChanged>,
    ids: Query<(Entity, &ElementId)>,
    choices: Query<&ChoiceBoxParts>,
    options: Query<(&OptionData, &ControlChecked)>,
    children: Query<&Children>,
    mut localization: ResMut<UiLocalization>,
    mut commands: Commands,
) {
    for change in changes.read() {
        if change.selected
            && ids
                .get(change.control)
                .is_ok_and(|(_, id)| id.0 == "language-choice")
        {
            if let Err(error) = localization.set_locale(&change.value) {
                warn!("Could not select showcase language: {error}");
            }
        }
    }
    let selected_locale = localization.locale().to_string();
    let Some((control, _)) = ids.iter().find(|(_, id)| id.0 == "language-choice") else {
        return;
    };
    let Ok(choice) = choices.get(control) else {
        return;
    };
    let Ok(entries) = children.get(choice.popup) else {
        return;
    };
    for option in entries {
        let Ok((data, checked)) = options.get(*option) else {
            continue;
        };
        if data.value == selected_locale && !checked.0 {
            let option = *option;
            commands.queue(move |world: &mut World| {
                set_option_selected(world, option, true);
            });
            break;
        }
    }
}

#[component_update]
fn change_theme(
    mut changes: MessageReader<OptionSelectionChanged>,
    ids: Query<(Entity, &ElementId)>,
    choices: Query<&ChoiceBoxParts>,
    options: Query<(&OptionData, &ControlChecked)>,
    children: Query<&Children>,
    themes: Res<UiThemes>,
    mut commands: Commands,
    mut synced: Local<Option<(Entity, String)>>,
) {
    let mut user_selected = false;
    for change in changes.read() {
        if change.selected
            && ids
                .get(change.control)
                .is_ok_and(|(_, id)| id.0 == "theme-choice")
        {
            user_selected = true;
            let name = change.value.clone();
            commands.queue(move |world: &mut World| {
                if let Err(error) = switch_ui_theme(world, &name) {
                    warn!("Could not select showcase theme: {error}");
                }
            });
        }
    }
    if user_selected {
        return;
    }
    let Some(active) = themes.active() else {
        return;
    };
    if synced
        .as_ref()
        .is_some_and(|(entity, name)| name == active && ids.get(*entity).is_ok())
    {
        return;
    }
    let Some((control, _)) = ids.iter().find(|(_, id)| id.0 == "theme-choice") else {
        return;
    };
    let Ok(choice) = choices.get(control) else {
        return;
    };
    let Ok(entries) = children.get(choice.popup) else {
        return;
    };
    for option in entries {
        let Ok((data, checked)) = options.get(*option) else {
            continue;
        };
        if data.value == active {
            if !checked.0 {
                let option = *option;
                commands.queue(move |world: &mut World| {
                    set_option_selected(world, option, true);
                });
            }
            *synced = Some((control, active.to_owned()));
            break;
        }
    }
}

#[component_update]
fn change_ui_fps(
    mut selections: MessageReader<OptionSelectionChanged>,
    mut edits: MessageReader<EditableTextChanged>,
    ids: Query<&ElementId>,
    mut frame_rate: ResMut<UiFrameRate>,
    mut state: ResMut<ShowcaseState>,
) {
    for selection in selections.read() {
        if !selection.selected
            || !ids
                .get(selection.control)
                .is_ok_and(|id| id.0 == "fps-choice")
        {
            continue;
        }
        let next = match selection.value.as_str() {
            "30" => Some(UiFrameRate::Fps30),
            "45" => Some(UiFrameRate::Fps45),
            "60" => Some(UiFrameRate::Fps60),
            "custom" => UiFrameRate::custom(state.custom_fps),
            _ => None,
        };
        if let Some(next) = next {
            *frame_rate = next;
            state.custom_fps_disabled = selection.value != "custom";
        }
    }
    for edit in edits.read() {
        if state.custom_fps_disabled || !ids.get(edit.entity).is_ok_and(|id| id.0 == "fps-custom") {
            continue;
        }
        if let Ok(fps) = edit.value.parse::<u32>()
            && (1..=1000).contains(&fps)
        {
            state.custom_fps = fps;
            *frame_rate = UiFrameRate::custom(fps).expect("positive FPS");
        }
    }
}

#[component_update]
fn reflect_current_fps(
    time: Res<Time<Real>>,
    frame_count: Res<FrameCount>,
    frame_rate: Res<UiFrameRate>,
    mut state: ResMut<ShowcaseState>,
    mut sample: Local<Option<(u32, f64)>>,
) {
    if frame_rate.is_changed() {
        *sample = Some((frame_count.0, time.elapsed_secs_f64()));
        state.current_fps = "--".into();
        return;
    }
    if let Some(fps) = sampled_fps(&mut sample, frame_count.0, time.elapsed_secs_f64()) {
        let current = fps.to_string();
        if state.current_fps != current {
            state.current_fps = current;
        }
    }
}

fn sampled_fps(sample: &mut Option<(u32, f64)>, frame_count: u32, now: f64) -> Option<u32> {
    let Some((previous_count, previous_time)) = *sample else {
        *sample = Some((frame_count, now));
        return None;
    };
    let elapsed = now - previous_time;
    if elapsed < 0.5 {
        return None;
    }
    *sample = Some((frame_count, now));
    Some((f64::from(frame_count.wrapping_sub(previous_count)) / elapsed).round() as u32)
}

#[component_update]
fn localize_window_title(
    localization: Res<UiLocalization>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
) {
    if !localization.is_changed() {
        return;
    }
    let title = localization
        .translate("window-title", None)
        .unwrap_or_else(|| "TiltUI widget showcase".into());
    for mut window in &mut windows {
        window.title = title.clone();
    }
}

fn rust_dialog(localization: &UiLocalization) -> DialogConfig {
    let title = localization
        .translate("dialog-rust-title", None)
        .unwrap_or_else(|| "Continue?".into());
    let body = localization
        .translate("dialog-rust-body", None)
        .unwrap_or_else(|| "This dialog was created in Rust.".into());
    DialogConfig::question(title, body)
}

#[html_fn("reset_progress")]
fn reset_progress(In(_event): In<HtmlEvent>, time: Res<Time>, mut state: ResMut<ShowcaseState>) {
    state.started_at = time.elapsed_secs();
}

#[html_fn("track_event")]
fn track_event(In(event): In<HtmlEvent>, mut state: ResMut<ShowcaseState>) {
    state.event_name = event.kind.to_owned();
    state.event_detail = event
        .data
        .get("text")
        .or_else(|| event.data.get("key_code"))
        .or_else(|| event.data.get("distance_x"))
        .or_else(|| event.data.get("y"))
        .cloned()
        .unwrap_or_default();
}

#[html_fn("table_action")]
fn table_action(In(event): In<HtmlEvent>, mut state: ResMut<ShowcaseState>) {
    state.table_feedback = match event.kind {
        "click" => "✓".into(),
        "change" => event
            .value
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "—".into()),
        _ => return,
    };
}

#[html_fn("open_rust_dialog")]
fn open_rust_dialog(
    In(click): In<HtmlClick>,
    mut dialogs: MessageWriter<ShowDialog>,
    localization: Res<UiLocalization>,
) {
    dialogs.write(ShowDialog {
        parent: click.target,
        config: rust_dialog(&localization),
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
    mut screenshot_delay: Local<Option<f32>>,
    mut dialog_preview_opened: Local<bool>,
    dialogs: Query<(Entity, &ElementId), With<DialogState>>,
    elements: Query<(Entity, &ElementId)>,
    mut requests: MessageWriter<ShowDialog>,
    localization: Res<UiLocalization>,
) {
    if !*dialog_preview_opened
        && time.elapsed_secs() >= 1.5
        && std::env::var_os("TILT_UI_SCREENSHOT").is_some()
        && std::env::var("TILT_UI_SCREENSHOT_DIALOG").ok().as_deref() == Some("rust")
    {
        if let Some((parent, _)) = elements.iter().find(|(_, id)| id.0 == "open-info-dialog") {
            requests.write(ShowDialog {
                parent,
                config: rust_dialog(&localization),
            });
            *dialog_preview_opened = true;
        }
    }
    let delay = screenshot_delay.get_or_insert_with(|| {
        std::env::var("TILT_UI_SCREENSHOT_DELAY")
            .ok()
            .and_then(|value| value.parse::<f32>().ok())
            .unwrap_or(2.0)
    });
    if *captured || time.elapsed_secs() < *delay {
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
    use tilt_ui::register_ui_theme;

    #[test]
    fn table_button_and_input_actions_update_the_visible_result() {
        let mut app = App::new();
        app.insert_resource(ShowcaseState {
            slider_value: 0,
            event_name: String::new(),
            event_detail: String::new(),
            table_feedback: "—".into(),
            custom_fps: 60,
            custom_fps_disabled: true,
            current_fps: "--".into(),
            started_at: 0.0,
        });
        let handler = app.world_mut().register_system(table_action);
        let target = app.world_mut().spawn_empty().id();
        let event = |kind, value| HtmlEvent {
            target,
            kind,
            value,
            submitter: None,
            data: Default::default(),
            handler: "table_action".into(),
        };
        app.world_mut()
            .run_system_with(handler, event("click", None))
            .unwrap();
        assert_eq!(app.world().resource::<ShowcaseState>().table_feedback, "✓");
        app.world_mut()
            .run_system_with(handler, event("change", Some("Test".into())))
            .unwrap();
        assert_eq!(
            app.world().resource::<ShowcaseState>().table_feedback,
            "Test"
        );
    }

    #[test]
    fn current_fps_uses_recent_frames_after_a_rate_change() {
        let mut sample = None;
        assert_eq!(sampled_fps(&mut sample, 0, 0.0), None);
        assert_eq!(sampled_fps(&mut sample, 30, 0.5), Some(60));
        assert_eq!(sampled_fps(&mut sample, 33, 1.1), Some(5));
    }

    #[test]
    fn slider_label_follows_slider_changes() {
        let mut app = App::new();
        app.add_message::<SliderChanged>()
            .insert_resource(ShowcaseState {
                slider_value: 75,
                event_name: String::new(),
                event_detail: String::new(),
                table_feedback: "—".into(),
                custom_fps: 120,
                custom_fps_disabled: true,
                current_fps: "--".into(),
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

    #[test]
    fn progress_label_updates_on_its_text_child() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .insert_resource(ShowcaseState {
                slider_value: 75,
                event_name: String::new(),
                event_detail: String::new(),
                table_feedback: "—".into(),
                custom_fps: 120,
                custom_fps_disabled: true,
                current_fps: "--".into(),
                started_at: -4.0,
            })
            .add_systems(Update, animate_progress);
        app.world_mut()
            .spawn(ElementId("demo-progress".into()));
        let label = app
            .world_mut()
            .spawn(ElementId("demo-progress-label".into()))
            .id();
        let text = app.world_mut().spawn(Text::new("40%")).id();
        app.world_mut().entity_mut(label).add_child(text);

        app.update();

        assert_eq!(app.world().get::<Text>(text).unwrap().0, "60%");
    }

    #[test]
    fn both_showcase_catalogs_are_valid() {
        let mut localization = UiLocalization::new("en-US").unwrap();
        localization
            .insert_ftl("en-US", include_str!("../locales/en-US.ftl"))
            .unwrap();
        localization
            .insert_ftl("de-DE", include_str!("../locales/de-DE.ftl"))
            .unwrap();
        assert_eq!(
            localization.translate("site-title", None).as_deref(),
            Some("UI Widget Library")
        );
        localization.set_locale("de-DE").unwrap();
        assert_eq!(
            localization.translate("site-title", None).as_deref(),
            Some("UI-Baukasten")
        );
        assert_eq!(
            localization.translate("dialog-rust-body", None).as_deref(),
            Some("Dieser Dialog wurde in Rust erstellt.")
        );
    }

    #[test]
    fn both_named_showcase_themes_parse() {
        let mut world = World::new();
        register_ui_theme(&mut world, "light", include_str!("../themes/light.css")).unwrap();
        register_ui_theme(&mut world, "dark", include_str!("../themes/dark.css")).unwrap();
        switch_ui_theme(&mut world, "dark").unwrap();
        assert_eq!(world.resource::<tilt_ui::UiThemes>().active(), Some("dark"));
    }

    #[test]
    fn language_choice_event_switches_the_locale() {
        let mut app = App::new();
        app.add_message::<OptionSelectionChanged>()
            .insert_resource(UiLocalization::new("en-US").unwrap())
            .add_systems(Update, change_language);
        let control = app
            .world_mut()
            .spawn(ElementId("language-choice".into()))
            .id();
        let option = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<OptionSelectionChanged>>()
            .write(OptionSelectionChanged {
                control,
                option,
                value: "de-DE".into(),
                selected: true,
            });
        app.update();
        assert_eq!(
            app.world()
                .resource::<UiLocalization>()
                .locale()
                .to_string(),
            "de-DE"
        );
    }

    #[test]
    fn theme_choice_event_switches_the_named_theme() {
        let mut app = App::new();
        app.add_message::<OptionSelectionChanged>()
            .add_systems(Update, change_theme);
        register_ui_theme(app.world_mut(), "light", "button { color: #ffffff; }").unwrap();
        register_ui_theme(app.world_mut(), "dark", "button { color: #111111; }").unwrap();
        switch_ui_theme(app.world_mut(), "light").unwrap();
        let control = app.world_mut().spawn(ElementId("theme-choice".into())).id();
        let option = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<OptionSelectionChanged>>()
            .write(OptionSelectionChanged {
                control,
                option,
                value: "dark".into(),
                selected: true,
            });
        app.update();
        assert_eq!(
            app.world().resource::<tilt_ui::UiThemes>().active(),
            Some("dark")
        );
    }

    #[test]
    fn theme_choice_mounting_after_theme_selection_shows_active_theme() {
        let mut app = App::new();
        app.add_message::<OptionSelectionChanged>()
            .add_systems(Update, change_theme);
        register_ui_theme(app.world_mut(), "light", "body { color: #ffffff; }").unwrap();
        register_ui_theme(app.world_mut(), "dark", "body { color: #111111; }").unwrap();
        switch_ui_theme(app.world_mut(), "dark").unwrap();
        app.update();

        let value = app.world_mut().spawn_empty().id();
        let popup = app.world_mut().spawn_empty().id();
        let control = app
            .world_mut()
            .spawn((
                ElementId("theme-choice".into()),
                tilt_ui::TiltElement {
                    kind: tilt_ui::ElementKind::ChoiceBox,
                },
                ChoiceBoxParts {
                    value,
                    popup,
                    open: false,
                },
            ))
            .id();
        app.world_mut().entity_mut(control).add_child(popup);
        let light = app
            .world_mut()
            .spawn((
                OptionData {
                    value: "light".into(),
                    label: "Light".into(),
                },
                ControlChecked(true),
            ))
            .id();
        let dark = app
            .world_mut()
            .spawn((
                OptionData {
                    value: "dark".into(),
                    label: "Dark".into(),
                },
                ControlChecked(false),
            ))
            .id();
        app.world_mut().entity_mut(popup).add_child(light);
        app.world_mut().entity_mut(popup).add_child(dark);
        app.update();

        assert!(!app.world().get::<ControlChecked>(light).unwrap().0);
        assert!(app.world().get::<ControlChecked>(dark).unwrap().0);
    }

    #[test]
    fn fps_choice_switches_presets_and_accepts_custom_input() {
        let mut app = App::new();
        app.add_message::<OptionSelectionChanged>()
            .add_message::<EditableTextChanged>()
            .insert_resource(UiFrameRate::Fps60)
            .insert_resource(ShowcaseState {
                slider_value: 75,
                event_name: String::new(),
                event_detail: String::new(),
                table_feedback: "—".into(),
                custom_fps: 120,
                custom_fps_disabled: true,
                current_fps: "--".into(),
                started_at: 0.0,
            })
            .add_systems(Update, change_ui_fps);
        let choice = app.world_mut().spawn(ElementId("fps-choice".into())).id();
        let input = app.world_mut().spawn(ElementId("fps-custom".into())).id();
        let option = app.world_mut().spawn_empty().id();
        let select = |app: &mut App, value: &str| {
            app.world_mut()
                .resource_mut::<bevy::ecs::message::Messages<OptionSelectionChanged>>()
                .write(OptionSelectionChanged {
                    control: choice,
                    option,
                    value: value.into(),
                    selected: true,
                });
            app.update();
        };

        select(&mut app, "45");
        assert_eq!(*app.world().resource::<UiFrameRate>(), UiFrameRate::Fps45);
        assert!(app.world().resource::<ShowcaseState>().custom_fps_disabled);
        select(&mut app, "custom");
        assert_eq!(app.world().resource::<UiFrameRate>().fps(), 120);
        assert!(!app.world().resource::<ShowcaseState>().custom_fps_disabled);

        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<EditableTextChanged>>()
            .write(EditableTextChanged {
                entity: input,
                name: None,
                value: "144".into(),
            });
        app.update();
        assert_eq!(app.world().resource::<UiFrameRate>().fps(), 144);
    }
}

fn replace_showcase_content(world: &mut World, button: Entity, mode: u8) {
    let target = world
        .get::<tilt_ui::ComponentStyleOwner>(button)
        .and_then(|owner| world.get::<tilt_ui::ComponentElementIds>(owner.0))
        .and_then(|ids| ids.get("runtime-content"));
    let Some(target) = target else {
        return;
    };
    let result = match mode {
        0 => tilt_ui::set_inner_text(world, target, "<p>{{ literal }}</p>"),
        1 => tilt_ui::set_inner_html(
            world,
            target,
            "<p>{{ i18n.runtime-html-result }}</p><button onclick=\"reset_progress\">{{ i18n.reset-progress }}</button>",
        ),
        _ => tilt_ui::set_inner_bindings(
            world,
            target,
            "{{ i18n.widget-progress-bar }}: {{ demo.slider_value + 5 }}%",
        ),
    };
    if let Err(error) = result {
        warn!("Could not replace showcase content: {error}");
    }
}

#[html_fn("runtime_text")]
fn runtime_text(In(click): In<HtmlClick>, mut commands: Commands) {
    commands.queue(move |world: &mut World| replace_showcase_content(world, click.target, 0));
}

#[html_fn("runtime_html")]
fn runtime_html(In(click): In<HtmlClick>, mut commands: Commands) {
    commands.queue(move |world: &mut World| replace_showcase_content(world, click.target, 1));
}

#[html_fn("runtime_bindings")]
fn runtime_bindings(In(click): In<HtmlClick>, mut commands: Commands) {
    commands.queue(move |world: &mut World| replace_showcase_content(world, click.target, 2));
}

#[html_fn("system_cursor")]
fn system_cursor(In(event): In<HtmlEvent>, mut commands: Commands) {
    commands.entity(event.target).insert(tilt_ui::UiCursor(
        bevy::window::SystemCursorIcon::Crosshair.into(),
    ));
}

#[html_fn("custom_cursor")]
fn custom_cursor(
    In(event): In<HtmlEvent>,
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
) {
    use bevy::window::{CursorIcon, CustomCursor, CustomCursorImage};
    use bevy::{
        asset::RenderAssetUsages,
        render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    };
    let mut pixels = vec![0; 24 * 24 * 4];
    for y in 0_usize..24 {
        for x in 0_usize..24 {
            let radius = x.abs_diff(12) + y.abs_diff(12);
            if (7..=10).contains(&radius) || (x == 12 && y == 12) {
                let index = (y * 24 + x) * 4;
                pixels[index..index + 4].copy_from_slice(&[168, 85, 247, 255]);
            }
        }
    }
    let handle = images.add(Image::new(
        Extent3d {
            width: 24,
            height: 24,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::all(),
    ));
    commands
        .entity(event.target)
        .insert(tilt_ui::UiCursor(CursorIcon::Custom(CustomCursor::Image(
            CustomCursorImage {
                handle,
                hotspot: (12, 12),
                ..default()
            },
        ))));
}
