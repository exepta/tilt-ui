//! Template-backed in-window and native message dialogs.

#[cfg(feature = "file-dialog")]
use std::sync::{
    Mutex,
    mpsc::{Receiver, TryRecvError, channel},
};

use bevy::{
    app::{App, Update},
    ecs::{
        component::Component,
        entity::Entity,
        hierarchy::{ChildOf, Children},
        message::{Message, MessageReader, Messages},
        system::Commands,
        world::World,
    },
    input::{ButtonInput, keyboard::KeyCode},
    prelude::IntoScheduleConfigs,
    ui::{
        FocusPolicy, GlobalZIndex, Node,
        widget::{Button, Text},
    },
};
use bevy_input_focus::{
    FocusCause, InputFocus,
    tab_navigation::{TabGroup, TabIndex},
};
use bevy_picking::{
    Pickable,
    events::{Click, Pointer},
    pointer::PointerButton,
};
use tilt_ui_core::{ElementKind, TemplateAttribute};

use crate::{
    ComponentElementIds, ComponentRoot, ComponentStyleOwner, ControlActivated, ControlPart,
    ControlPartKind, ElementClasses, ElementState, StaticAttribute, StaticAttributes, StyleDirty,
    TiltControl, TiltElement, TiltText,
    widgets::{
        controls::{spawn_part, spawn_text_part},
        state::set_widget_display,
    },
};

/// Content and presentation of a dialog created from Rust rather than HTML.
#[derive(Debug, Clone)]
pub struct DialogConfig {
    pub title: String,
    pub content: String,
    pub renderer: DialogRenderer,
    pub layout: DialogLayout,
    pub kind: DialogKind,
}

impl DialogConfig {
    pub fn info(title: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            content: content.into(),
            renderer: DialogRenderer::Bevy,
            layout: DialogLayout::FloatingPanel,
            kind: DialogKind::Info,
        }
    }

    pub fn warning(title: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            kind: DialogKind::Warning,
            ..Self::info(title, content)
        }
    }

    pub fn error(title: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            kind: DialogKind::Error,
            ..Self::info(title, content)
        }
    }

    pub fn question(title: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            kind: DialogKind::Question,
            ..Self::info(title, content)
        }
    }

    pub fn blank(layout: DialogLayout) -> Self {
        Self {
            layout,
            kind: DialogKind::Blank,
            ..Self::info("", "")
        }
    }

    pub fn with_renderer(mut self, renderer: DialogRenderer) -> Self {
        self.renderer = renderer;
        self
    }

    pub fn with_layout(mut self, layout: DialogLayout) -> Self {
        self.layout = layout;
        self
    }
}

/// Requests a new dialog from a normal Bevy system or `#[html_fn]` handler.
#[derive(Message, Debug, Clone)]
pub struct ShowDialog {
    pub parent: Entity,
    pub config: DialogConfig,
}

/// Emitted after a Rust-requested dialog has been added to the UI tree.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialogSpawned {
    pub entity: Entity,
}

/// Where a dialog is displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogRenderer {
    Bevy,
    System,
}

/// Placement of an in-window dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogLayout {
    FloatingPanel,
    BottomSheet,
}

/// Semantic style and native icon of a dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogKind {
    Info,
    Warning,
    Error,
    Question,
    Blank,
}

/// Result produced when a dialog closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogResult {
    Closed,
    Confirmed,
    Cancelled,
}

/// Emitted once whenever an open dialog is dismissed.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialogClosed {
    pub entity: Entity,
    pub result: DialogResult,
}

/// Persistent state of a `<dialog>` widget.
#[derive(Component, Debug, Clone)]
pub struct DialogState {
    pub renderer: DialogRenderer,
    pub layout: DialogLayout,
    pub kind: DialogKind,
    pub title: String,
    pub open: bool,
    pub panel: Entity,
    trigger_id: Option<String>,
    ephemeral: bool,
    close_button: Entity,
    previous_focus: Option<Entity>,
}

#[derive(Component, Default)]
struct DialogTriggers(Vec<Entity>);

#[derive(Component)]
struct DialogCloseButton;

#[derive(Component)]
struct DialogAction(DialogResult);

#[cfg(feature = "file-dialog")]
#[derive(Component)]
struct PendingNativeDialog(Mutex<Receiver<DialogResult>>);

fn attribute<'a>(attributes: &'a [TemplateAttribute], name: &str) -> Option<&'a str> {
    crate::component::static_attribute_value(attributes, name)
}

fn localized_action_label(world: &World, fallback: &str) -> String {
    let key = match fallback {
        "Close" => "dialog-action-close",
        "OK" => "dialog-action-ok",
        "Cancel" => "dialog-action-cancel",
        "Confirm" => "dialog-action-confirm",
        _ => return fallback.to_owned(),
    };
    #[cfg(not(feature = "fluent"))]
    let _ = (world, key);
    #[cfg(feature = "fluent")]
    if let Some(value) = world
        .get_resource::<crate::UiLocalization>()
        .and_then(|locale| locale.translate(key, None))
    {
        return value;
    }
    fallback.to_owned()
}

pub(crate) fn materialize(world: &mut World, entity: Entity, attributes: &[TemplateAttribute]) {
    let renderer = match attribute(attributes, "renderer") {
        Some("system") if cfg!(feature = "file-dialog") => DialogRenderer::System,
        _ => DialogRenderer::Bevy,
    };
    let layout = match attribute(attributes, "layout") {
        Some("bottom-sheet" | "bottom") => DialogLayout::BottomSheet,
        _ => DialogLayout::FloatingPanel,
    };
    let kind = match attribute(attributes, "type") {
        Some("warn" | "warning") => DialogKind::Warning,
        Some("error" | "failure") => DialogKind::Error,
        Some("question") => DialogKind::Question,
        Some("blank") => DialogKind::Blank,
        _ => DialogKind::Info,
    };
    let open = attribute(attributes, "open").is_some_and(|value| value != "false");
    let title = attribute(attributes, "title")
        .unwrap_or_default()
        .to_owned();
    let panel = spawn_part(world, entity, ControlPartKind::Popup);
    world.entity_mut(panel).insert(Pickable::default());
    world.entity_mut(entity).add_child(panel);
    if !title.is_empty() {
        let label = spawn_text_part(world, entity, ControlPartKind::Label, title.clone());
        world.entity_mut(panel).add_child(label);
    }
    let close = spawn_text_part(world, entity, ControlPartKind::Indicator, "×");
    world.entity_mut(close).insert((
        Button,
        TiltControl,
        ElementState::default(),
        TabIndex(0),
        DialogCloseButton,
        Pickable::default(),
    ));
    world.entity_mut(panel).add_child(close);
    world.entity_mut(entity).insert((
        DialogState {
            renderer,
            layout,
            kind,
            title,
            open,
            panel,
            trigger_id: attribute(attributes, "trigger")
                .or_else(|| attribute(attributes, "triggger"))
                .map(str::to_owned),
            ephemeral: false,
            close_button: close,
            previous_focus: None,
        },
        ElementState {
            open,
            ..Default::default()
        },
        FocusPolicy::Block,
        TabGroup::modal(),
        GlobalZIndex(30_000),
        Pickable::default(),
    ));
    set_widget_display(world, entity, open && renderer == DialogRenderer::Bevy);
}

pub(crate) fn panel(world: &World, dialog: Entity) -> Entity {
    world
        .get::<DialogState>(dialog)
        .map_or(dialog, |state| state.panel)
}

/// Updates a dialog title without reopening the dialog.
pub(crate) fn set_dialog_title(world: &mut World, dialog: Entity, title: String) {
    let Some(state) = world.get::<DialogState>(dialog) else {
        return;
    };
    let panel = state.panel;
    if state.title == title {
        return;
    }
    world.get_mut::<DialogState>(dialog).unwrap().title = title.clone();
    let label = world.get::<Children>(panel).and_then(|children| {
        children.iter().copied().find(|child| {
            world
                .get::<ControlPart>(*child)
                .is_some_and(|part| part.kind == ControlPartKind::Label)
        })
    });
    if let Some(label) = label {
        if let Some(mut text) = world.get_mut::<Text>(label) {
            text.0 = title;
        }
    } else if !title.is_empty() {
        let label = spawn_text_part(world, dialog, ControlPartKind::Label, title);
        world.entity_mut(panel).add_child(label);
        if let Some(scope) = world
            .get::<ComponentStyleOwner>(dialog)
            .map(|owner| owner.0)
        {
            world.entity_mut(scope).insert(StyleDirty);
        }
    }
}

fn enclosing_body(world: &World, start: Entity) -> Option<Entity> {
    let mut current = Some(start);
    while let Some(entity) = current {
        if world
            .get::<TiltElement>(entity)
            .is_some_and(|element| element.kind == ElementKind::Body)
        {
            return Some(entity);
        }
        current = world.get::<ChildOf>(entity).map(ChildOf::parent);
    }
    None
}

fn dialog_mount(world: &World, start: Entity) -> Option<Entity> {
    let mut current = Some(start);
    let mut outermost = None;
    while let Some(entity) = current {
        if world.get::<ComponentRoot>(entity).is_some() {
            outermost = Some(entity);
        }
        current = world.get::<ChildOf>(entity).map(ChildOf::parent);
    }
    outermost.or_else(|| enclosing_body(world, start))
}

/// Creates and opens a styled modal under an existing component's UI tree.
///
/// `parent` can be any materialized element in that component. Returns `None`
/// if it has no component style scope.
pub fn spawn_dialog(world: &mut World, parent: Entity, config: DialogConfig) -> Option<Entity> {
    let mut ancestor = Some(parent);
    let scope = loop {
        let entity = ancestor?;
        if let Some(owner) = world.get::<ComponentStyleOwner>(entity) {
            break owner.0;
        }
        ancestor = world.get::<ChildOf>(entity).map(ChildOf::parent);
    };
    if world.get_entity(scope).is_err() {
        return None;
    }
    let mount = dialog_mount(world, parent).unwrap_or_else(|| {
        let mut query = world.query::<(Entity, &TiltElement, &ComponentStyleOwner)>();
        query
            .iter(world)
            .find(|(_, element, owner)| owner.0 == scope && element.kind == ElementKind::Body)
            .map(|(entity, _, _)| entity)
            .unwrap_or(parent)
    });
    let renderer = if config.renderer == DialogRenderer::System && !cfg!(feature = "file-dialog") {
        DialogRenderer::Bevy
    } else {
        config.renderer
    };
    let layout = match config.layout {
        DialogLayout::FloatingPanel => "floating-panel",
        DialogLayout::BottomSheet => "bottom-sheet",
    };
    let kind = match config.kind {
        DialogKind::Info => "info",
        DialogKind::Warning => "warning",
        DialogKind::Error => "error",
        DialogKind::Question => "question",
        DialogKind::Blank => "blank",
    };
    let attributes = [
        TemplateAttribute::Static {
            name: "title".into(),
            value: config.title.clone(),
        },
        TemplateAttribute::Static {
            name: "layout".into(),
            value: layout.into(),
        },
        TemplateAttribute::Static {
            name: "type".into(),
            value: kind.into(),
        },
        TemplateAttribute::Static {
            name: "renderer".into(),
            value: if renderer == DialogRenderer::System {
                "system"
            } else {
                "bevy-app"
            }
            .into(),
        },
    ];
    let dialog = world
        .spawn((
            Node::default(),
            TiltElement {
                kind: ElementKind::Dialog,
            },
            ComponentStyleOwner(scope),
            StaticAttributes {
                attributes: attributes
                    .iter()
                    .filter_map(|attribute| match attribute {
                        TemplateAttribute::Static { name, value } => Some(StaticAttribute {
                            name: name.clone(),
                            value: value.clone(),
                        }),
                        _ => None,
                    })
                    .collect(),
            },
        ))
        .id();
    materialize(world, dialog, &attributes);
    world.get_mut::<DialogState>(dialog).unwrap().ephemeral = true;
    let panel = panel(world, dialog);
    if !config.content.is_empty() {
        let content = spawn_text_part(
            world,
            dialog,
            ControlPartKind::Value,
            config.content.clone(),
        );
        world.entity_mut(content).insert(TiltText {
            value: config.content,
        });
        world.entity_mut(panel).add_child(content);
    }
    if renderer == DialogRenderer::Bevy {
        let actions: &[(&str, DialogResult)] = match config.kind {
            DialogKind::Info => &[("Close", DialogResult::Closed)],
            DialogKind::Warning | DialogKind::Error => &[("OK", DialogResult::Confirmed)],
            DialogKind::Question => &[
                ("Cancel", DialogResult::Cancelled),
                ("Confirm", DialogResult::Confirmed),
            ],
            DialogKind::Blank => &[],
        };
        if !actions.is_empty() {
            let footer = world
                .spawn((
                    Node::default(),
                    TiltElement {
                        kind: ElementKind::Div,
                    },
                    ElementClasses {
                        classes: vec!["dialog-actions".into()],
                    },
                    ComponentStyleOwner(scope),
                ))
                .id();
            world.entity_mut(panel).add_child(footer);
            for (label, result) in actions {
                let label = localized_action_label(world, label);
                let button = world
                    .spawn((
                        TiltElement {
                            kind: ElementKind::Button,
                        },
                        ComponentStyleOwner(scope),
                        DialogAction(*result),
                    ))
                    .id();
                crate::render::materialize_element(world, button, ElementKind::Button, &[]);
                world.entity_mut(footer).add_child(button);
                let text = world.spawn(ComponentStyleOwner(scope)).id();
                crate::render::materialize_text(world, text, &label);
                world.entity_mut(button).add_child(text);
            }
        }
    }
    world.entity_mut(mount).add_child(dialog);
    world.entity_mut(scope).insert(StyleDirty);
    open_dialog(world, dialog);
    if let Some(mut messages) = world.get_resource_mut::<Messages<DialogSpawned>>() {
        messages.write(DialogSpawned { entity: dialog });
    }
    Some(dialog)
}

pub(crate) fn resolve_targets(world: &mut World, scope: Entity) {
    let entries = {
        let mut query = world.query::<(Entity, &DialogState, &ComponentStyleOwner)>();
        query
            .iter(world)
            .filter(|(_, _, owner)| owner.0 == scope)
            .map(|(entity, state, _)| (entity, state.trigger_id.clone()))
            .collect::<Vec<_>>()
    };
    for (dialog, trigger_id) in entries {
        let open = world
            .get::<DialogState>(dialog)
            .is_some_and(|state| state.open);
        if let Some(mut css) = world.get_mut::<ElementState>(dialog) {
            css.open = open;
        }
        if open
            && world
                .get::<DialogState>(dialog)
                .is_some_and(|state| state.renderer == DialogRenderer::Bevy)
        {
            focus_dialog(world, dialog);
        }
        let mount = dialog_mount(world, dialog).or_else(|| {
            let mut query = world.query::<(Entity, &TiltElement, &ComponentStyleOwner)>();
            query
                .iter(world)
                .find(|(_, element, owner)| owner.0 == scope && element.kind == ElementKind::Body)
                .map(|(entity, _, _)| entity)
        });
        if let Some(mount) = mount {
            world.entity_mut(mount).add_child(dialog);
        }
        if let Some(target) = trigger_id
            .as_deref()
            .and_then(|id| world.get::<ComponentElementIds>(scope)?.get(id))
        {
            if let Some(mut triggers) = world.get_mut::<DialogTriggers>(target) {
                triggers.0.push(dialog);
            } else {
                world
                    .entity_mut(target)
                    .insert(DialogTriggers(vec![dialog]));
            }
        }
    }
}

fn focus_dialog(world: &mut World, entity: Entity) {
    let previous = world.get_resource::<InputFocus>().and_then(InputFocus::get);
    let close = world
        .get::<DialogState>(entity)
        .map(|state| state.close_button);
    if let Some(mut state) = world.get_mut::<DialogState>(entity) {
        state.previous_focus = previous;
    }
    if let (Some(close), Some(mut focus)) = (close, world.get_resource_mut::<InputFocus>()) {
        focus.set(close, FocusCause::Navigated);
    }
}

/// Changes visibility without rebuilding a dialog's authored content.
pub fn open_dialog(world: &mut World, entity: Entity) -> bool {
    let Some(mut state) = world.get_mut::<DialogState>(entity) else {
        return false;
    };
    if state.open {
        return false;
    }
    state.open = true;
    let renderer = state.renderer;
    if let Some(mut css) = world.get_mut::<ElementState>(entity) {
        css.open = true;
    }
    set_widget_display(world, entity, renderer == DialogRenderer::Bevy);
    if renderer == DialogRenderer::Bevy {
        focus_dialog(world, entity);
    }
    true
}

/// Closes a dialog and emits [`DialogClosed`] when it was open.
pub fn close_dialog(world: &mut World, entity: Entity, result: DialogResult) -> bool {
    let Some(mut state) = world.get_mut::<DialogState>(entity) else {
        return false;
    };
    if !state.open {
        return false;
    }
    state.open = false;
    let ephemeral = state.ephemeral;
    let previous_focus = state.previous_focus.take();
    let renderer = state.renderer;
    if let Some(mut css) = world.get_mut::<ElementState>(entity) {
        css.open = false;
    }
    set_widget_display(world, entity, false);
    if renderer == DialogRenderer::Bevy {
        let focused_inside = world
            .get_resource::<InputFocus>()
            .and_then(InputFocus::get)
            .is_some_and(|focused| ancestor_dialog(world, focused) == Some(entity));
        if focused_inside {
            let restore = previous_focus.filter(|previous| world.get_entity(*previous).is_ok());
            if let Some(mut focus) = world.get_resource_mut::<InputFocus>() {
                if let Some(previous) = restore {
                    focus.set(previous, FocusCause::Navigated);
                } else {
                    focus.clear();
                }
            }
        }
    }
    if let Some(mut messages) = world.get_resource_mut::<Messages<DialogClosed>>() {
        messages.write(DialogClosed { entity, result });
    }
    if ephemeral {
        let scope = world
            .get::<ComponentStyleOwner>(entity)
            .map(|owner| owner.0);
        world.entity_mut(entity).despawn();
        if let Some(scope) = scope {
            if let Ok(mut owner) = world.get_entity_mut(scope) {
                owner.insert(StyleDirty);
            }
        }
    }
    true
}

fn ancestor_dialog(world: &World, mut entity: Entity) -> Option<Entity> {
    loop {
        if world.get::<DialogState>(entity).is_some() {
            return Some(entity);
        }
        entity = world.get::<ChildOf>(entity)?.parent();
    }
}

fn close_result(attributes: &StaticAttributes) -> Option<DialogResult> {
    let action = attributes
        .attributes
        .iter()
        .find(|attribute| attribute.name == "dialog-close")?;
    Some(match action.value.as_str() {
        "confirm" | "ok" | "yes" => DialogResult::Confirmed,
        "cancel" | "no" => DialogResult::Cancelled,
        _ => DialogResult::Closed,
    })
}

fn process_activations(mut events: MessageReader<ControlActivated>, mut commands: Commands) {
    for event in events.read() {
        let target = event.entity;
        commands.queue(move |world: &mut World| {
            if world.get::<DialogCloseButton>(target).is_some() {
                if let Some(dialog) = ancestor_dialog(world, target) {
                    close_dialog(world, dialog, DialogResult::Closed);
                }
                return;
            }
            if let Some(result) = world.get::<DialogAction>(target).map(|action| action.0) {
                if let Some(dialog) = ancestor_dialog(world, target) {
                    close_dialog(world, dialog, result);
                }
                return;
            }
            if let Some(result) = world.get::<StaticAttributes>(target).and_then(close_result) {
                if let Some(dialog) = ancestor_dialog(world, target) {
                    close_dialog(world, dialog, result);
                }
                return;
            }
            let dialogs = world
                .get::<DialogTriggers>(target)
                .map(|triggers| triggers.0.clone());
            if let Some(dialogs) = dialogs {
                for dialog in dialogs {
                    open_dialog(world, dialog);
                }
            }
        });
    }
}

fn spawn_requested_dialogs(mut requests: MessageReader<ShowDialog>, mut commands: Commands) {
    for request in requests.read() {
        let parent = request.parent;
        let config = request.config.clone();
        commands.queue(move |world: &mut World| {
            spawn_dialog(world, parent, config);
        });
    }
}

fn remove_orphan_dialogs(world: &mut World) {
    let stale = {
        let mut query = world.query::<(Entity, &DialogState, &ComponentStyleOwner)>();
        query
            .iter(world)
            .filter(|(_, _, owner)| world.get_entity(owner.0).is_err())
            .map(|(entity, _, _)| entity)
            .collect::<Vec<_>>()
    };
    for entity in stale {
        world.entity_mut(entity).despawn();
    }
}

fn dismiss_on_escape(world: &mut World) {
    if !world
        .get_resource::<ButtonInput<KeyCode>>()
        .is_some_and(|keys| keys.just_pressed(KeyCode::Escape))
    {
        return;
    }
    let top = {
        let mut query = world.query::<(Entity, &DialogState)>();
        query
            .iter(world)
            .filter(|(_, state)| state.open && state.renderer == DialogRenderer::Bevy)
            .map(|(entity, _)| entity)
            .last()
    };
    if let Some(entity) = top {
        close_dialog(world, entity, DialogResult::Cancelled);
    }
}

fn dismiss_on_backdrop(mut clicks: Option<MessageReader<Pointer<Click>>>, mut commands: Commands) {
    let Some(clicks) = clicks.as_mut() else {
        return;
    };
    for click in clicks.read() {
        if click.button != PointerButton::Primary {
            continue;
        }
        let target = click.entity;
        commands.queue(move |world: &mut World| {
            if world.get::<DialogState>(target).is_some() {
                close_dialog(world, target, DialogResult::Cancelled);
            }
        });
    }
}

#[cfg(feature = "file-dialog")]
fn native_text(world: &World, dialog: Entity) -> String {
    fn collect(world: &World, entity: Entity, parts: &mut Vec<String>) {
        if world.get::<TiltText>(entity).is_some() {
            if let Some(text) = world.get::<Text>(entity) {
                let value = text.0.trim();
                if !value.is_empty() {
                    parts.push(value.to_owned());
                }
            }
        }
        if let Some(children) = world.get::<Children>(entity) {
            for child in children {
                collect(world, *child, parts);
            }
        }
    }
    let mut parts = Vec::new();
    collect(world, dialog, &mut parts);
    parts.join("\n")
}

#[cfg(feature = "file-dialog")]
fn native_message(
    state: &DialogState,
    body: String,
) -> (String, String, rfd::MessageLevel, rfd::MessageButtons) {
    use rfd::{MessageButtons, MessageLevel};
    let level = match state.kind {
        DialogKind::Warning => MessageLevel::Warning,
        DialogKind::Error => MessageLevel::Error,
        _ => MessageLevel::Info,
    };
    let buttons = if state.kind == DialogKind::Question {
        MessageButtons::YesNoCancel
    } else {
        MessageButtons::Ok
    };
    (
        if state.title.is_empty() {
            "TiltUI".to_owned()
        } else {
            state.title.clone()
        },
        body,
        level,
        buttons,
    )
}

#[cfg(feature = "file-dialog")]
fn native_result(result: rfd::MessageDialogResult) -> DialogResult {
    match result {
        rfd::MessageDialogResult::Yes | rfd::MessageDialogResult::Ok => DialogResult::Confirmed,
        _ => DialogResult::Cancelled,
    }
}

#[cfg(feature = "file-dialog")]
fn launch_native_dialogs(world: &mut World) {
    let mut query = world.query::<(Entity, &DialogState, Option<&PendingNativeDialog>)>();
    let mut pending = false;
    let mut next = None;
    for (entity, state, task) in query.iter(world) {
        pending |= task.is_some();
        if state.open && state.renderer == DialogRenderer::System && task.is_none() {
            next.get_or_insert(entity);
        }
    }
    if pending {
        return;
    }
    let Some(entity) = next else {
        return;
    };
    let state = world.get::<DialogState>(entity).unwrap().clone();
    let (title, body, level, buttons) = native_message(&state, native_text(world, entity));
    let (sender, receiver) = channel();
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(move || {
        let result = rfd::MessageDialog::new()
            .set_title(title)
            .set_description(body)
            .set_level(level)
            .set_buttons(buttons)
            .show();
        let _ = sender.send(native_result(result));
    });
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(async move {
        let dialog = rfd::AsyncMessageDialog::new()
            .set_title(title)
            .set_description(body)
            .set_level(level)
            .set_buttons(buttons);
        let _ = sender.send(native_result(dialog.show().await));
    });
    world
        .entity_mut(entity)
        .insert(PendingNativeDialog(Mutex::new(receiver)));
}

#[cfg(feature = "file-dialog")]
fn finish_native_dialogs(world: &mut World) {
    let finished = {
        let mut query = world.query::<(Entity, &PendingNativeDialog)>();
        query
            .iter(world)
            .filter_map(
                |(entity, pending)| match pending.0.lock().ok()?.try_recv() {
                    Ok(result) => Some((entity, result)),
                    Err(TryRecvError::Disconnected) => Some((entity, DialogResult::Cancelled)),
                    Err(TryRecvError::Empty) => None,
                },
            )
            .collect::<Vec<_>>()
    };
    for (entity, result) in finished {
        world.entity_mut(entity).remove::<PendingNativeDialog>();
        close_dialog(world, entity, result);
    }
}

pub(crate) fn install(app: &mut App) {
    use crate::control::TiltControlSystems;
    app.add_message::<DialogClosed>()
        .add_message::<DialogSpawned>()
        .add_message::<ShowDialog>()
        .add_systems(
            Update,
            (
                spawn_requested_dialogs,
                process_activations,
                dismiss_on_escape,
                dismiss_on_backdrop,
                remove_orphan_dialogs,
            )
                .chain()
                .after(TiltControlSystems::Selection),
        );
    #[cfg(feature = "file-dialog")]
    app.add_systems(
        Update,
        (launch_native_dialogs, finish_native_dialogs)
            .chain()
            .after(process_activations),
    );
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::{
        app::App,
        camera::NormalizedRenderTarget,
        ecs::{hierarchy::Children, message::Messages},
        math::Vec2,
        ui::{Display, Node, widget::Text},
    };
    use bevy_input_focus::{InputFocus, tab_navigation::TabGroup};
    use bevy_picking::{
        backend::HitData,
        events::{Click, Pointer},
        pointer::{Location, PointerButton, PointerId},
    };
    use tilt_ui_core::{ElementKind, TemplateAttribute};

    use super::{
        DialogAction, DialogClosed, DialogConfig, DialogKind, DialogLayout, DialogRenderer,
        DialogResult, DialogSpawned, DialogState, ShowDialog, close_dialog, install, materialize,
        open_dialog, resolve_targets, set_dialog_title,
    };
    use crate::{
        ComponentElementIds, ComponentRoot, ComponentStyleOwner, ControlActivated, ControlPart,
        ControlPartKind, ElementId, StaticAttribute, StaticAttributes, TiltElement,
    };

    fn setup() -> (App, bevy::ecs::entity::Entity, bevy::ecs::entity::Entity) {
        let mut app = App::new();
        app.add_message::<ControlActivated>()
            .add_message::<Pointer<Click>>();
        install(&mut app);
        let scope = app.world_mut().spawn(ComponentElementIds::default()).id();
        let body = app
            .world_mut()
            .spawn((
                Node::default(),
                TiltElement {
                    kind: ElementKind::Body,
                },
                ComponentStyleOwner(scope),
            ))
            .id();
        let trigger = app.world_mut().spawn(ElementId("open".into())).id();
        app.world_mut()
            .get_mut::<ComponentElementIds>(scope)
            .unwrap()
            .insert("open".into(), trigger);
        let dialog = app
            .world_mut()
            .spawn((Node::default(), ComponentStyleOwner(scope)))
            .id();
        materialize(
            app.world_mut(),
            dialog,
            &[
                TemplateAttribute::Static {
                    name: "trigger".into(),
                    value: "open".into(),
                },
                TemplateAttribute::Static {
                    name: "title".into(),
                    value: "Example".into(),
                },
                TemplateAttribute::Static {
                    name: "layout".into(),
                    value: "bottom-sheet".into(),
                },
            ],
        );
        app.world_mut().entity_mut(body).add_child(dialog);
        resolve_targets(app.world_mut(), scope);
        (app, dialog, trigger)
    }

    #[test]
    fn bound_title_updates_the_existing_dialog_label() {
        let (mut app, dialog, _) = setup();
        let panel = app.world().get::<DialogState>(dialog).unwrap().panel;
        let label = app
            .world()
            .get::<Children>(panel)
            .unwrap()
            .iter()
            .copied()
            .find(|child| {
                app.world()
                    .get::<ControlPart>(*child)
                    .is_some_and(|part| part.kind == ControlPartKind::Label)
            })
            .unwrap();

        set_dialog_title(app.world_mut(), dialog, "Übersetzter Titel".into());
        assert_eq!(
            app.world().get::<DialogState>(dialog).unwrap().title,
            "Übersetzter Titel"
        );
        assert_eq!(
            app.world().get::<Text>(label).unwrap().0,
            "Übersetzter Titel"
        );
    }

    #[test]
    fn trigger_opens_persistent_bottom_sheet_and_close_emits_once() {
        let (mut app, dialog, trigger) = setup();
        let state = app.world().get::<DialogState>(dialog).unwrap();
        assert_eq!(state.renderer, DialogRenderer::Bevy);
        assert_eq!(state.layout, DialogLayout::BottomSheet);
        assert_eq!(state.kind, DialogKind::Info);
        assert_eq!(
            app.world().get::<Node>(dialog).unwrap().display,
            Display::None
        );
        let panel = state.panel;

        app.world_mut()
            .resource_mut::<Messages<ControlActivated>>()
            .write(ControlActivated { entity: trigger });
        app.update();
        assert!(app.world().get::<DialogState>(dialog).unwrap().open);
        assert_eq!(app.world().get::<DialogState>(dialog).unwrap().panel, panel);
        assert_ne!(
            app.world().get::<Node>(dialog).unwrap().display,
            Display::None
        );

        assert!(close_dialog(
            app.world_mut(),
            dialog,
            DialogResult::Confirmed
        ));
        assert!(!close_dialog(
            app.world_mut(),
            dialog,
            DialogResult::Confirmed
        ));
        assert_eq!(app.world().resource::<Messages<DialogClosed>>().len(), 1);
        assert!(!app.world().get::<DialogState>(dialog).unwrap().open);
        assert!(open_dialog(app.world_mut(), dialog));
        assert_eq!(app.world().get::<DialogState>(dialog).unwrap().panel, panel);
    }

    #[test]
    fn authored_action_closes_dialog_with_its_result() {
        let (mut app, dialog, _) = setup();
        let panel = app.world().get::<DialogState>(dialog).unwrap().panel;
        let action = app
            .world_mut()
            .spawn(StaticAttributes {
                attributes: vec![StaticAttribute {
                    name: "dialog-close".into(),
                    value: "cancel".into(),
                }],
            })
            .id();
        app.world_mut().entity_mut(panel).add_child(action);
        open_dialog(app.world_mut(), dialog);
        app.world_mut()
            .resource_mut::<Messages<ControlActivated>>()
            .write(ControlActivated { entity: action });
        app.update();
        assert!(!app.world().get::<DialogState>(dialog).unwrap().open);
        let messages = app.world().resource::<Messages<DialogClosed>>();
        let mut cursor = messages.get_cursor();
        assert_eq!(
            cursor.read(messages).next(),
            Some(&DialogClosed {
                entity: dialog,
                result: DialogResult::Cancelled,
            })
        );
    }

    #[test]
    fn native_renderer_stays_out_of_bevy_layout() {
        let mut world = bevy::ecs::world::World::new();
        let dialog = world.spawn(Node::default()).id();
        materialize(
            &mut world,
            dialog,
            &[
                TemplateAttribute::Static {
                    name: "renderer".into(),
                    value: "system".into(),
                },
                TemplateAttribute::Static {
                    name: "type".into(),
                    value: "question".into(),
                },
            ],
        );
        assert_eq!(
            world.get::<DialogState>(dialog).unwrap().kind,
            DialogKind::Question
        );
        #[cfg(feature = "file-dialog")]
        {
            assert_eq!(
                world.get::<DialogState>(dialog).unwrap().renderer,
                DialogRenderer::System
            );
            assert!(open_dialog(&mut world, dialog));
            assert_eq!(world.get::<Node>(dialog).unwrap().display, Display::None);
        }
        #[cfg(not(feature = "file-dialog"))]
        assert_eq!(
            world.get::<DialogState>(dialog).unwrap().renderer,
            DialogRenderer::Bevy
        );
    }

    #[test]
    fn backdrop_click_closes_open_dialog() {
        let (mut app, dialog, _) = setup();
        open_dialog(app.world_mut(), dialog);
        app.world_mut()
            .resource_mut::<Messages<Pointer<Click>>>()
            .write(Pointer::new(
                PointerId::Mouse,
                Location {
                    target: NormalizedRenderTarget::None {
                        width: 400,
                        height: 300,
                    },
                    position: Vec2::new(12.0, 12.0),
                },
                Click {
                    button: PointerButton::Primary,
                    hit: HitData::new(dialog, 0.0, None, None),
                    duration: Duration::from_millis(20),
                    count: 1,
                },
                dialog,
            ));
        app.update();
        assert!(!app.world().get::<DialogState>(dialog).unwrap().open);
        let messages = app.world().resource::<Messages<DialogClosed>>();
        let mut cursor = messages.get_cursor();
        assert_eq!(
            cursor.read(messages).next().unwrap().result,
            DialogResult::Cancelled
        );
    }

    #[test]
    fn modal_keeps_tab_navigation_scoped_and_restores_focus() {
        let (mut app, dialog, trigger) = setup();
        app.world_mut()
            .insert_resource(InputFocus::from_entity(trigger));
        assert!(app.world().get::<TabGroup>(dialog).unwrap().modal);
        let close = app.world().get::<DialogState>(dialog).unwrap().close_button;
        open_dialog(app.world_mut(), dialog);
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(close));
        close_dialog(app.world_mut(), dialog, DialogResult::Closed);
        assert_eq!(app.world().resource::<InputFocus>().get(), Some(trigger));
    }

    #[test]
    fn rust_request_spawns_and_releases_a_question_modal() {
        let (mut app, authored, _) = setup();
        let body = app
            .world()
            .get::<bevy::ecs::hierarchy::ChildOf>(authored)
            .unwrap()
            .parent();
        app.world_mut()
            .resource_mut::<Messages<ShowDialog>>()
            .write(ShowDialog {
                parent: body,
                config: DialogConfig::question("Continue?", "Apply the changes?"),
            });
        app.update();
        let spawned = app.world().resource::<Messages<DialogSpawned>>();
        let mut cursor = spawned.get_cursor();
        let dialog = cursor.read(spawned).next().unwrap().entity;
        assert!(app.world().get::<DialogState>(dialog).unwrap().open);
        let panel = app.world().get::<DialogState>(dialog).unwrap().panel;
        let confirm = {
            let mut query = app
                .world_mut()
                .query::<(bevy::ecs::entity::Entity, &DialogAction)>();
            query
                .iter(app.world())
                .find(|(_, action)| action.0 == DialogResult::Confirmed)
                .map(|(entity, _)| entity)
                .unwrap()
        };
        app.world_mut()
            .resource_mut::<Messages<ControlActivated>>()
            .write(ControlActivated { entity: confirm });
        app.update();
        assert!(app.world().get_entity(dialog).is_err());
        assert!(app.world().get_entity(panel).is_err());
        let closed = app.world().resource::<Messages<DialogClosed>>();
        let mut cursor = closed.get_cursor();
        assert_eq!(
            cursor.read(closed).next().unwrap().result,
            DialogResult::Confirmed
        );
    }

    #[test]
    fn nested_dialog_mounts_outside_the_scrollable_body() {
        let mut world = bevy::ecs::world::World::new();
        let outer_scope = world.spawn_empty().id();
        let root = world
            .spawn(ComponentRoot {
                component: tilt_ui_core::ComponentId(0),
            })
            .id();
        let body = world
            .spawn((
                Node::default(),
                TiltElement {
                    kind: ElementKind::Body,
                },
                ComponentStyleOwner(outer_scope),
            ))
            .id();
        world.entity_mut(root).add_child(body);
        let inner_scope = world.spawn(ComponentElementIds::default()).id();
        world.entity_mut(body).add_child(inner_scope);
        let wrapper = world
            .spawn((Node::default(), ComponentStyleOwner(inner_scope)))
            .id();
        world.entity_mut(inner_scope).add_child(wrapper);
        let dialog = world
            .spawn((Node::default(), ComponentStyleOwner(inner_scope)))
            .id();
        materialize(&mut world, dialog, &[]);
        world.entity_mut(wrapper).add_child(dialog);
        resolve_targets(&mut world, inner_scope);
        assert_eq!(
            world
                .get::<bevy::ecs::hierarchy::ChildOf>(dialog)
                .unwrap()
                .parent(),
            root
        );
    }

    #[test]
    fn removing_component_owner_releases_its_detached_dialog() {
        let (mut app, dialog, _) = setup();
        let owner = app.world().get::<ComponentStyleOwner>(dialog).unwrap().0;
        app.world_mut().entity_mut(owner).despawn();
        app.update();
        assert!(app.world().get_entity(dialog).is_err());
    }
}
