//! Transient, CSS-stylable notifications with a viewport-level stack.

use std::time::Duration;

use bevy::{
    app::{App, Update},
    ecs::{
        component::Component,
        entity::Entity,
        hierarchy::ChildOf,
        message::{Message, MessageReader, Messages},
        resource::Resource,
        system::Commands,
        world::World,
    },
    prelude::IntoScheduleConfigs,
    ui::{
        AlignItems, FlexDirection, FocusPolicy, GlobalZIndex, JustifyContent, Node, PositionType,
        UiRect, Val,
    },
    window::{PrimaryWindow, Window},
};
use bevy_picking::Pickable;
use tilt_ui_core::{ElementKind, TemplateAttribute};

use crate::{
    ComponentElementIds, ComponentRoot, ComponentStyleOwner, ControlActivated, ControlPartKind,
    ElementClasses, ElementState, StaticAttribute, StaticAttributes, StyleDirty, TiltElement,
    TiltText,
    widgets::{controls::spawn_text_part, state::set_widget_display},
};

/// The semantic color and icon treatment of a toast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Success,
    Error,
    Warning,
    Info,
}

impl ToastKind {
    fn from_str(value: &str) -> Self {
        match value {
            "success" => Self::Success,
            "error" | "failure" => Self::Error,
            "warning" | "warn" => Self::Warning,
            _ => Self::Info,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }
}

/// A notification created from Rust. Authored `<toast>` children remain available for rich HTML.
#[derive(Debug, Clone)]
pub struct ToastConfig {
    pub title: String,
    pub content: String,
    pub kind: ToastKind,
    /// `None` keeps the toast visible until it is closed explicitly.
    pub duration: Option<Duration>,
    /// Optional CSS class added to the generated `<toast>` element.
    pub class: Option<String>,
}

impl ToastConfig {
    pub fn new(kind: ToastKind, title: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            content: content.into(),
            kind,
            duration: Some(Duration::from_secs(5)),
            class: None,
        }
    }

    pub fn success(title: impl Into<String>, content: impl Into<String>) -> Self {
        Self::new(ToastKind::Success, title, content)
    }

    pub fn error(title: impl Into<String>, content: impl Into<String>) -> Self {
        Self::new(ToastKind::Error, title, content)
    }

    pub fn warning(title: impl Into<String>, content: impl Into<String>) -> Self {
        Self::new(ToastKind::Warning, title, content)
    }

    pub fn info(title: impl Into<String>, content: impl Into<String>) -> Self {
        Self::new(ToastKind::Info, title, content)
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    pub fn persistent(mut self) -> Self {
        self.duration = None;
        self
    }

    pub fn with_class(mut self, class: impl Into<String>) -> Self {
        self.class = Some(class.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastPlacement {
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
}

/// Global stack layout. Change this resource at runtime to move existing toasts.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct ToastStackSettings {
    pub placement: ToastPlacement,
    pub max_visible: usize,
    pub width: f32,
    pub margin: f32,
    pub gap: f32,
}

impl Default for ToastStackSettings {
    fn default() -> Self {
        Self {
            placement: ToastPlacement::TopRight,
            max_visible: 4,
            width: 360.0,
            margin: 20.0,
            gap: 10.0,
        }
    }
}

/// Request a new toast from a Bevy system or `#[html_fn]` handler.
#[derive(Message, Debug, Clone)]
pub struct ShowToast {
    pub parent: Entity,
    pub config: ToastConfig,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToastSpawned {
    pub entity: Entity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastCloseReason {
    Manual,
    Timeout,
    Overflow,
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToastClosed {
    pub entity: Entity,
    pub reason: ToastCloseReason,
}

/// Persistent state of a template or Rust-created toast.
#[derive(Component, Debug, Clone)]
pub struct ToastState {
    pub kind: ToastKind,
    pub open: bool,
    pub closing: bool,
    pub duration: Option<Duration>,
    pub remaining: Option<f32>,
    pub ephemeral: bool,
    trigger_id: Option<String>,
    initial_open: bool,
    sequence: u64,
}

#[derive(Component)]
struct ToastStack;

#[derive(Component, Clone, Copy)]
struct ToastOrigin(Entity);

#[derive(Component)]
struct ToastResolved;

#[derive(Component, Default)]
struct ToastTriggers(Vec<Entity>);

#[derive(Component)]
struct ToastCloseButton;

#[derive(Component, Clone, Copy)]
struct ToastExitMotion {
    remaining: f32,
    reason: ToastCloseReason,
}

#[derive(Resource, Default)]
struct ToastSequence(u64);

fn attribute<'a>(attributes: &'a [TemplateAttribute], name: &str) -> Option<&'a str> {
    crate::component::static_attribute_value(attributes, name)
}

fn parse_duration(value: &str) -> Option<Option<Duration>> {
    let value = value.trim();
    if value == "0" || value.eq_ignore_ascii_case("persistent") {
        return Some(None);
    }
    let seconds = if let Some(ms) = value.strip_suffix("ms") {
        ms.trim().parse::<f32>().ok()? / 1000.0
    } else if let Some(seconds) = value.strip_suffix('s') {
        seconds.trim().parse::<f32>().ok()?
    } else {
        value.parse::<f32>().ok()? / 1000.0
    };
    (seconds.is_finite() && seconds >= 0.0)
        .then(|| (seconds > 0.0).then(|| Duration::from_secs_f32(seconds)))
}

pub(crate) fn materialize(world: &mut World, entity: Entity, attributes: &[TemplateAttribute]) {
    let kind = ToastKind::from_str(attribute(attributes, "type").unwrap_or("info"));
    let duration = attribute(attributes, "duration")
        .and_then(parse_duration)
        .unwrap_or(Some(Duration::from_secs(5)));
    let title = attribute(attributes, "title").unwrap_or_default();
    world.entity_mut(entity).insert((
        ToastState {
            kind,
            open: false,
            closing: false,
            duration,
            remaining: None,
            ephemeral: false,
            trigger_id: attribute(attributes, "trigger")
                .map(|id| id.trim_start_matches('#').to_owned()),
            initial_open: attribute(attributes, "open").is_some_and(|value| value != "false"),
            sequence: 0,
        },
        ElementState::default(),
        FocusPolicy::Pass,
        Pickable::default(),
    ));
    if !title.is_empty() {
        let label = spawn_text_part(world, entity, ControlPartKind::Label, title);
        world.entity_mut(entity).add_child(label);
    }
    let close = world
        .spawn((
            TiltElement {
                kind: ElementKind::Button,
            },
            world
                .get::<ComponentStyleOwner>(entity)
                .copied()
                .unwrap_or(ComponentStyleOwner(entity)),
            ElementClasses {
                classes: vec!["toast-close".into()],
            },
            ToastCloseButton,
        ))
        .id();
    crate::render::materialize_element(world, close, ElementKind::Button, &[]);
    let glyph = world
        .spawn(
            world
                .get::<ComponentStyleOwner>(entity)
                .copied()
                .unwrap_or(ComponentStyleOwner(entity)),
        )
        .id();
    crate::render::materialize_text(world, glyph, "×");
    world.entity_mut(close).add_child(glyph);
    world.entity_mut(entity).add_child(close);
    set_widget_display(world, entity, false);
}

fn mount_for(world: &World, start: Entity) -> Option<Entity> {
    let mut current = Some(start);
    let mut outermost = None;
    while let Some(entity) = current {
        if world.get::<ComponentRoot>(entity).is_some() {
            outermost = Some(entity);
        }
        current = world.get::<ChildOf>(entity).map(ChildOf::parent);
    }
    outermost
}

fn stack_for(world: &mut World, mount: Entity) -> Entity {
    if let Some(stack) = world
        .get::<bevy::ecs::hierarchy::Children>(mount)
        .and_then(|children| {
            children
                .iter()
                .find(|child| world.get::<ToastStack>(**child).is_some())
                .copied()
        })
    {
        return stack;
    }
    let stack = world
        .spawn((
            ToastStack,
            Node::default(),
            GlobalZIndex(20_000),
            FocusPolicy::Pass,
            Pickable::IGNORE,
        ))
        .id();
    world.entity_mut(mount).add_child(stack);
    update_stack_node(world, stack);
    stack
}

fn mount_toast(world: &mut World, toast: Entity, source_parent: Entity) -> Option<Entity> {
    let mount = mount_for(world, source_parent)?;
    let stack = stack_for(world, mount);
    world.entity_mut(stack).add_child(toast);
    world.entity_mut(toast).insert(ToastOrigin(source_parent));
    Some(stack)
}

pub(crate) fn resolve_targets(world: &mut World, scope: Entity) {
    let entries = {
        let mut query = world.query::<(Entity, &ToastState, &ComponentStyleOwner)>();
        query
            .iter(world)
            .filter(|(entity, _, owner)| {
                owner.0 == scope && world.get::<ToastResolved>(*entity).is_none()
            })
            .map(|(entity, state, _)| (entity, state.trigger_id.clone(), state.initial_open))
            .collect::<Vec<_>>()
    };
    let mut mounted_any = false;
    for (toast, trigger_id, initial_open) in entries {
        let Some(parent) = world.get::<ChildOf>(toast).map(ChildOf::parent) else {
            continue;
        };
        if mount_toast(world, toast, parent).is_none() {
            continue;
        }
        mounted_any = true;
        if let Some(target) = trigger_id.as_deref().and_then(|id| {
            world
                .get::<ComponentElementIds>(scope)
                .and_then(|ids| ids.get(id))
        }) {
            if let Some(mut triggers) = world.get_mut::<ToastTriggers>(target) {
                if !triggers.0.contains(&toast) {
                    triggers.0.push(toast);
                }
            } else {
                world.entity_mut(target).insert(ToastTriggers(vec![toast]));
            }
        }
        world.entity_mut(toast).insert(ToastResolved);
        if initial_open {
            show_toast(world, toast);
        }
    }
    if mounted_any && world.get_entity(scope).is_ok() {
        world.entity_mut(scope).insert(StyleDirty);
    }
}

/// Creates and displays a toast under the component that owns `parent`.
pub fn spawn_toast(world: &mut World, parent: Entity, config: ToastConfig) -> Option<Entity> {
    let mut current = Some(parent);
    let scope = loop {
        let entity = current?;
        if let Some(scope) = world.get::<ComponentStyleOwner>(entity) {
            break scope.0;
        }
        current = world.get::<ChildOf>(entity).map(ChildOf::parent);
    };
    if world.get_entity(scope).is_err() {
        return None;
    }
    let attributes = vec![
        TemplateAttribute::Static {
            name: "type".into(),
            value: config.kind.as_str().into(),
        },
        TemplateAttribute::Static {
            name: "title".into(),
            value: config.title.clone(),
        },
    ];
    let toast = world
        .spawn((
            Node::default(),
            TiltElement {
                kind: ElementKind::Toast,
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
    if let Some(class) = config.class {
        world.entity_mut(toast).insert(ElementClasses {
            classes: class.split_ascii_whitespace().map(str::to_owned).collect(),
        });
    }
    materialize(world, toast, &attributes);
    {
        let mut state = world.get_mut::<ToastState>(toast).unwrap();
        state.duration = config.duration;
        state.ephemeral = true;
    }
    if !config.content.is_empty() {
        let content = spawn_text_part(world, toast, ControlPartKind::Value, config.content.clone());
        world.entity_mut(content).insert(TiltText {
            value: config.content,
        });
        world.entity_mut(toast).add_child(content);
    }
    world.entity_mut(parent).add_child(toast);
    mount_toast(world, toast, parent)?;
    world.entity_mut(scope).insert(StyleDirty);
    show_toast(world, toast);
    if let Some(mut messages) = world.get_resource_mut::<Messages<ToastSpawned>>() {
        messages.write(ToastSpawned { entity: toast });
    }
    Some(toast)
}

/// Displays an authored toast again, restarting its automatic duration.
pub fn show_toast(world: &mut World, toast: Entity) -> bool {
    let Some(state) = world.get::<ToastState>(toast) else {
        return false;
    };
    if state.open {
        return false;
    }
    let stack = match world.get::<ChildOf>(toast).map(ChildOf::parent) {
        Some(stack) if world.get::<ToastStack>(stack).is_some() => stack,
        Some(parent) => match mount_toast(world, toast, parent) {
            Some(stack) => stack,
            None => return false,
        },
        None => return false,
    };
    let max_visible = world
        .get_resource::<ToastStackSettings>()
        .map_or(4, |settings| settings.max_visible.max(1));
    let mut visible = {
        let mut query = world.query::<(Entity, &ToastState, &ChildOf)>();
        query
            .iter(world)
            .filter(|(entity, state, parent)| {
                *entity != toast && parent.parent() == stack && (state.open || state.closing)
            })
            .map(|(entity, state, _)| (state.sequence, entity))
            .collect::<Vec<_>>()
    };
    visible.sort_by_key(|(sequence, _)| *sequence);
    while visible.len() >= max_visible {
        let (_, oldest) = visible.remove(0);
        finish_close(world, oldest, ToastCloseReason::Overflow);
    }
    world.init_resource::<ToastSequence>();
    let sequence = {
        let mut next = world.resource_mut::<ToastSequence>();
        next.0 = next.0.wrapping_add(1);
        next.0
    };
    {
        let mut state = world.get_mut::<ToastState>(toast).unwrap();
        state.open = true;
        state.closing = false;
        state.remaining = state.duration.map(|duration| duration.as_secs_f32());
        state.sequence = sequence;
    }
    world.entity_mut(toast).remove::<ToastExitMotion>();
    if let Some(mut state) = world.get_mut::<ElementState>(toast) {
        state.open = true;
        state.closing = false;
    }
    set_widget_display(world, toast, true);
    // Reopened authored toasts appear at the end of the active stack.
    world.entity_mut(stack).add_child(toast);
    true
}

/// Closes a toast. Rust-created toasts are removed after their exit animation.
pub fn close_toast(world: &mut World, toast: Entity) -> bool {
    close_toast_with_reason(world, toast, ToastCloseReason::Manual)
}

fn close_toast_with_reason(world: &mut World, toast: Entity, reason: ToastCloseReason) -> bool {
    let Some(state) = world.get::<ToastState>(toast) else {
        return false;
    };
    if !state.open || state.closing {
        return false;
    }
    if let Some(mut state) = world.get_mut::<ToastState>(toast) {
        state.open = false;
        state.closing = true;
        state.remaining = None;
    }
    if let Some(mut state) = world.get_mut::<ElementState>(toast) {
        state.open = false;
        state.closing = true;
    }
    world.entity_mut(toast).insert(ToastExitMotion {
        remaining: 0.16,
        reason,
    });
    true
}

fn finish_close(world: &mut World, toast: Entity, reason: ToastCloseReason) {
    let Some(state) = world.get::<ToastState>(toast) else {
        return;
    };
    if !state.open && !state.closing {
        return;
    }
    let ephemeral = state.ephemeral;
    if let Some(mut state) = world.get_mut::<ToastState>(toast) {
        state.open = false;
        state.closing = false;
        state.remaining = None;
    }
    if let Some(mut state) = world.get_mut::<ElementState>(toast) {
        state.open = false;
        state.closing = false;
    }
    world.entity_mut(toast).remove::<ToastExitMotion>();
    set_widget_display(world, toast, false);
    if let Some(mut messages) = world.get_resource_mut::<Messages<ToastClosed>>() {
        messages.write(ToastClosed {
            entity: toast,
            reason,
        });
    }
    if ephemeral {
        world.entity_mut(toast).despawn();
    }
}

fn ancestor_toast(world: &World, mut entity: Entity) -> Option<Entity> {
    loop {
        if world.get::<ToastState>(entity).is_some() {
            return Some(entity);
        }
        entity = world.get::<ChildOf>(entity)?.parent();
    }
}

fn spawn_requested_toasts(mut requests: MessageReader<ShowToast>, mut commands: Commands) {
    for request in requests.read() {
        let parent = request.parent;
        let config = request.config.clone();
        commands.queue(move |world: &mut World| {
            spawn_toast(world, parent, config);
        });
    }
}

fn process_activations(mut activations: MessageReader<ControlActivated>, mut commands: Commands) {
    for activation in activations.read() {
        let target = activation.entity;
        commands.queue(move |world: &mut World| {
            if world.get::<ToastCloseButton>(target).is_some()
                || world.get::<StaticAttributes>(target).is_some_and(|attrs| {
                    attrs
                        .attributes
                        .iter()
                        .any(|attribute| attribute.name == "toast-close")
                })
            {
                if let Some(toast) = ancestor_toast(world, target) {
                    close_toast(world, toast);
                }
                return;
            }
            if let Some(triggers) = world.get::<ToastTriggers>(target) {
                let toasts = triggers.0.clone();
                for toast in toasts {
                    show_toast(world, toast);
                }
            }
        });
    }
}

fn tick_toasts(world: &mut World) {
    let delta = world
        .get_resource::<bevy::time::Time>()
        .map_or(1.0 / 60.0, bevy::time::Time::delta_secs)
        .max(0.0);
    let active = {
        let mut query = world.query::<(Entity, &ToastState)>();
        query
            .iter(world)
            .filter(|(_, state)| state.open || state.closing)
            .map(|(entity, _)| entity)
            .collect::<Vec<_>>()
    };
    for toast in active {
        if let Some(mut motion) = world.get_mut::<ToastExitMotion>(toast) {
            motion.remaining -= delta;
            if motion.remaining <= 0.0 {
                let reason = motion.reason;
                finish_close(world, toast, reason);
            }
            continue;
        }
        let expired = if let Some(mut state) = world.get_mut::<ToastState>(toast) {
            if let Some(remaining) = state.remaining.as_mut() {
                *remaining -= delta;
                *remaining <= 0.0
            } else {
                false
            }
        } else {
            false
        };
        if expired {
            close_toast_with_reason(world, toast, ToastCloseReason::Timeout);
        }
    }
}

fn remove_orphan_toasts(world: &mut World) {
    let stale = {
        let mut query = world.query::<(
            Entity,
            &ToastState,
            &ComponentStyleOwner,
            Option<&ToastOrigin>,
        )>();
        query
            .iter(world)
            .filter(|(_, _, owner, origin)| {
                world.get_entity(owner.0).is_err()
                    || origin.is_some_and(|origin| world.get_entity(origin.0).is_err())
            })
            .map(|(entity, _, _, _)| entity)
            .collect::<Vec<_>>()
    };
    for toast in stale {
        world.entity_mut(toast).despawn();
    }
}

fn update_stack_node(world: &mut World, stack: Entity) {
    let settings = world
        .get_resource::<ToastStackSettings>()
        .copied()
        .unwrap_or_default();
    let viewport = {
        let mut windows = world.query_filtered::<&Window, bevy::ecs::query::With<PrimaryWindow>>();
        windows
            .iter(world)
            .next()
            .map(|window| window.resolution.width())
    };
    let margin = settings.margin.max(0.0);
    let width = settings
        .width
        .max(1.0)
        .min(viewport.map_or(f32::INFINITY, |width| (width - margin * 2.0).max(1.0)));
    let mut next = Node {
        position_type: PositionType::Absolute,
        display: bevy::ui::Display::Flex,
        flex_direction: FlexDirection::Column,
        justify_content: JustifyContent::Start,
        align_items: AlignItems::Stretch,
        width: Val::Px(width),
        row_gap: Val::Px(settings.gap.max(0.0)),
        padding: UiRect::ZERO,
        ..Default::default()
    };
    match settings.placement {
        ToastPlacement::TopRight => {
            next.top = Val::Px(margin);
            next.right = Val::Px(margin);
        }
        ToastPlacement::TopLeft => {
            next.top = Val::Px(margin);
            next.left = Val::Px(margin);
        }
        ToastPlacement::BottomRight => {
            next.bottom = Val::Px(margin);
            next.right = Val::Px(margin);
        }
        ToastPlacement::BottomLeft => {
            next.bottom = Val::Px(margin);
            next.left = Val::Px(margin);
        }
    }
    if world.get::<Node>(stack) != Some(&next) {
        world.entity_mut(stack).insert(next);
    }
}

fn update_stacks(world: &mut World) {
    let stacks = {
        let mut query = world.query_filtered::<Entity, bevy::ecs::query::With<ToastStack>>();
        query.iter(world).collect::<Vec<_>>()
    };
    for stack in stacks {
        update_stack_node(world, stack);
    }
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<ToastStackSettings>()
        .init_resource::<ToastSequence>()
        .add_message::<ShowToast>()
        .add_message::<ToastSpawned>()
        .add_message::<ToastClosed>()
        .add_systems(
            Update,
            (
                spawn_requested_toasts,
                process_activations,
                tick_toasts,
                remove_orphan_toasts,
                update_stacks,
            )
                .chain()
                .after(crate::control::TiltControlSystems::Selection),
        );
}

#[cfg(test)]
mod tests {
    use bevy::{
        ecs::{hierarchy::ChildOf, message::Messages},
        ui::{Display, Node},
    };
    use tilt_ui_core::ComponentId;

    use super::*;

    fn world_with_body() -> (World, Entity, Entity) {
        let mut world = World::new();
        world.init_resource::<ToastStackSettings>();
        world.init_resource::<ToastSequence>();
        world.insert_resource(Messages::<ToastClosed>::default());
        world.insert_resource(Messages::<ToastSpawned>::default());
        let root = world
            .spawn((
                ComponentRoot {
                    component: ComponentId(0),
                },
                ComponentElementIds::default(),
            ))
            .id();
        let body = world
            .spawn((
                Node::default(),
                TiltElement {
                    kind: ElementKind::Body,
                },
                ComponentStyleOwner(root),
            ))
            .id();
        world.entity_mut(root).add_child(body);
        (world, root, body)
    }

    #[test]
    fn duration_accepts_seconds_milliseconds_and_persistent() {
        assert_eq!(
            parse_duration("2.5s"),
            Some(Some(Duration::from_millis(2500)))
        );
        assert_eq!(
            parse_duration("750ms"),
            Some(Some(Duration::from_millis(750)))
        );
        assert_eq!(parse_duration("0"), Some(None));
        assert_eq!(parse_duration("nope"), None);
    }

    #[test]
    fn authored_toast_mounts_above_scroll_body_and_reopens() {
        let (mut world, root, body) = world_with_body();
        let trigger = world.spawn_empty().id();
        world.entity_mut(body).add_child(trigger);
        world
            .get_mut::<ComponentElementIds>(root)
            .unwrap()
            .insert("notify".into(), trigger);
        let toast = world
            .spawn((Node::default(), ComponentStyleOwner(root)))
            .id();
        materialize(
            &mut world,
            toast,
            &[
                TemplateAttribute::Static {
                    name: "type".into(),
                    value: "success".into(),
                },
                TemplateAttribute::Static {
                    name: "trigger".into(),
                    value: "notify".into(),
                },
                TemplateAttribute::Static {
                    name: "duration".into(),
                    value: "0".into(),
                },
            ],
        );
        world.entity_mut(body).add_child(toast);
        resolve_targets(&mut world, root);
        let stack = world.get::<ChildOf>(toast).unwrap().parent();
        assert!(world.get::<ToastStack>(stack).is_some());
        assert_eq!(world.get::<ChildOf>(stack).unwrap().parent(), root);
        assert_eq!(world.get::<ToastTriggers>(trigger).unwrap().0, vec![toast]);
        assert!(show_toast(&mut world, toast));
        assert_eq!(world.get::<Node>(toast).unwrap().display, Display::Flex);
        assert!(close_toast(&mut world, toast));
        for _ in 0..10 {
            tick_toasts(&mut world);
        }
        assert!(!world.get::<ToastState>(toast).unwrap().open);
        assert_eq!(world.get::<Node>(toast).unwrap().display, Display::None);
        assert!(show_toast(&mut world, toast));
    }

    #[test]
    fn rust_toasts_stack_and_oldest_exits_when_limit_is_reached() {
        let (mut world, _, body) = world_with_body();
        world.resource_mut::<ToastStackSettings>().max_visible = 2;
        let first = spawn_toast(&mut world, body, ToastConfig::info("First", "A")).unwrap();
        let second = spawn_toast(&mut world, body, ToastConfig::success("Second", "B")).unwrap();
        let third = spawn_toast(&mut world, body, ToastConfig::error("Third", "C")).unwrap();
        assert!(world.get_entity(first).is_err());
        assert!(world.get::<ToastState>(second).unwrap().open);
        assert!(world.get::<ToastState>(third).unwrap().open);
        let mut cursor =
            Messages::<ToastClosed>::get_cursor(world.resource::<Messages<ToastClosed>>());
        assert_eq!(
            cursor
                .read(world.resource::<Messages<ToastClosed>>())
                .next()
                .unwrap()
                .reason,
            ToastCloseReason::Overflow
        );
    }

    #[test]
    fn automatic_duration_closes_a_rust_toast() {
        let (mut world, _, body) = world_with_body();
        let toast = spawn_toast(
            &mut world,
            body,
            ToastConfig::warning("Soon", "Gone").with_duration(Duration::from_millis(1)),
        )
        .unwrap();
        tick_toasts(&mut world);
        assert!(world.get::<ToastState>(toast).unwrap().closing);
        for _ in 0..10 {
            tick_toasts(&mut world);
        }
        assert!(world.get_entity(toast).is_err());
        let mut cursor =
            Messages::<ToastClosed>::get_cursor(world.resource::<Messages<ToastClosed>>());
        assert_eq!(
            cursor
                .read(world.resource::<Messages<ToastClosed>>())
                .next()
                .unwrap()
                .reason,
            ToastCloseReason::Timeout
        );
    }
}
