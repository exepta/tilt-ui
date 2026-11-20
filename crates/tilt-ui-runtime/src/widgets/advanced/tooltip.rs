//! Targeted pointer triggers and viewport-aware placement for TiltUI tooltips.

use bevy::{
    app::{App, Plugin},
    asset::{Asset, Assets, Handle},
    ecs::{
        component::Component,
        entity::Entity,
        hierarchy::{ChildOf, Children},
        message::MessageReader,
        system::Commands,
        world::World,
    },
    math::{Vec2, Vec4},
    reflect::TypePath,
    render::{RenderApp, render_resource::AsBindGroup},
    shader::{Shader, ShaderRef},
    ui::{
        BackgroundColor, ComputedNode, ComputedUiRenderTargetInfo, GlobalZIndex,
        InteractionDisabled, Node, PositionType, UiGlobalTransform, UiRect, UiScale, Val,
    },
    ui_render::{
        UiMaterialPlugin,
        ui_material::{MaterialNode, UiMaterial},
    },
    window::{PrimaryWindow, Window},
};
use bevy_picking::{
    Pickable,
    events::{
        PointerClick, PointerDragEnd, PointerDragStart, PointerMove, PointerOut, PointerOver,
    },
    hover::HoverMap,
    pointer::{PointerButton, PointerId, PointerLocation},
};
use tilt_ui_core::{TemplateAttribute, ToolTipAlignment, ToolTipPriority};

use crate::{
    ComponentElementIds, ComponentStyleOwner, ControlPartKind, WidgetLayoutOverride,
    widgets::state::set_widget_display,
};

/// Selects pointer-following or target-anchored tooltip placement.
pub use tilt_ui_core::ToolTipVariant as TooltipVariant;

const NOSE_SHADER: Handle<Shader> =
    bevy::asset::uuid_handle!("4bb68526-e1cf-4a32-a147-cc264fd9b401");

#[derive(Asset, TypePath, AsBindGroup, Clone)]
struct TooltipNoseMaterial {
    #[uniform(0)]
    fill: Vec4,
    #[uniform(1)]
    side: Vec4,
}

impl UiMaterial for TooltipNoseMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(NOSE_SHADER)
    }
}

pub(crate) struct TooltipNosePlugin;

impl Plugin for TooltipNosePlugin {
    fn build(&self, app: &mut App) {
        if app.get_sub_app(RenderApp).is_none() {
            return;
        }
        app.world_mut()
            .resource_mut::<Assets<Shader>>()
            .insert(
                NOSE_SHADER.id(),
                Shader::from_wesl(
                    include_str!("tooltip_nose.wesl"),
                    "tilt_ui_tooltip_nose.wesl",
                ),
            )
            .expect("unique embedded tooltip nose shader");
        app.add_plugins(UiMaterialPlugin::<TooltipNoseMaterial>::default());
    }
}

/// Stores one tooltip's component-local target and trigger configuration.
#[derive(Component, Debug, Clone)]
pub struct TooltipSettings {
    /// Target resolved after the component template is materialized.
    pub target: Option<Entity>,
    /// Popup placement behavior.
    pub variant: TooltipVariant,
    /// Preferred target side for anchored placement.
    pub priority: ToolTipPriority,
    /// Preferred placement axis retained for future automatic placement.
    pub alignment: ToolTipAlignment,
    /// Shows while the pointer hovers over the target.
    pub hover: bool,
    /// Toggles when the target receives a primary click.
    pub click: bool,
    /// Shows while a primary pointer drag is active on the target.
    pub drag: bool,
    target_id: Option<String>,
}

#[derive(Component, Debug, Default)]
struct TooltipTargets(Vec<Entity>);

#[derive(Component, Debug, Clone, Copy, PartialEq)]
struct OpenTooltip {
    pointer_id: PointerId,
    pointer: Vec2,
}

pub(crate) fn materialize(world: &mut World, entity: Entity, attributes: &[TemplateAttribute]) {
    let attr = |name| crate::component::static_attribute_value(attributes, name);
    let trigger = attr("trigger").unwrap_or("hover");
    let alignment = match attr("alignment") {
        Some("vertical") => ToolTipAlignment::Vertical,
        _ => ToolTipAlignment::Horizontal,
    };
    let priority = match attr("prio").or_else(|| attr("priority")) {
        Some("top") => ToolTipPriority::Top,
        Some("bottom") | Some("down") => ToolTipPriority::Bottom,
        Some("left") => ToolTipPriority::Left,
        Some("right") => ToolTipPriority::Right,
        _ if alignment == ToolTipAlignment::Vertical => ToolTipPriority::Top,
        _ => ToolTipPriority::Right,
    };
    let settings = TooltipSettings {
        target: None,
        variant: if attr("variant") == Some("point") {
            TooltipVariant::Point
        } else {
            TooltipVariant::Follow
        },
        priority,
        alignment,
        hover: trigger.split(['|', ',', ' ']).any(|item| item == "hover"),
        click: trigger.split(['|', ',', ' ']).any(|item| item == "click"),
        drag: trigger.split(['|', ',', ' ']).any(|item| item == "drag"),
        target_id: attr("for").map(str::to_owned),
    };
    let pointed = settings.variant == TooltipVariant::Point;
    world
        .entity_mut(entity)
        .insert((settings, Pickable::IGNORE, GlobalZIndex(10_000)));
    let nose = crate::widgets::controls::spawn_part(world, entity, ControlPartKind::Indicator);
    if let Some(mut materials) = world.get_resource_mut::<Assets<TooltipNoseMaterial>>() {
        let handle = materials.add(TooltipNoseMaterial {
            fill: Vec4::new(0.14, 0.13, 0.29, 1.0),
            side: Vec4::ZERO,
        });
        world.entity_mut(nose).insert(MaterialNode(handle));
    }
    world.entity_mut(nose).insert(if pointed {
        bevy::prelude::Visibility::Visible
    } else {
        bevy::prelude::Visibility::Hidden
    });
    world.entity_mut(entity).add_child(nose);
    set_widget_display(world, entity, false);
    if attr("open") == Some("true") {
        set_tooltip_open(world, entity, true, PointerId::Mouse, Vec2::ZERO);
    }
}

pub(crate) fn resolve_targets(world: &mut World, scope: Entity) {
    let entries = {
        let mut query = world.query::<(Entity, &TooltipSettings, &ComponentStyleOwner)>();
        query
            .iter(world)
            .filter(|(_, _, owner)| owner.0 == scope)
            .map(|(entity, settings, _)| (entity, settings.target_id.clone()))
            .collect::<Vec<_>>()
    };
    for (entity, target_id) in entries {
        let target = target_id
            .as_deref()
            .and_then(|id| {
                world
                    .get::<ComponentElementIds>(scope)
                    .and_then(|ids| ids.get(id))
            })
            .or_else(|| {
                target_id
                    .is_none()
                    .then(|| world.get::<ChildOf>(entity).map(ChildOf::parent))
                    .flatten()
            });
        if let Some(mut settings) = world.get_mut::<TooltipSettings>(entity) {
            settings.target = target;
        }
        if let Some(target) = target {
            if let Some(mut targets) = world.get_mut::<TooltipTargets>(target) {
                if !targets.0.contains(&entity) {
                    targets.0.push(entity);
                }
            } else {
                world
                    .entity_mut(target)
                    .insert(TooltipTargets(vec![entity]));
            }
        }
        let mut pending = vec![entity];
        while let Some(node) = pending.pop() {
            if let Some(children) = world.get::<Children>(node) {
                pending.extend(children.iter().copied());
            }
            world.entity_mut(node).insert(Pickable::IGNORE);
        }
    }
}

pub(crate) fn reconfigure(world: &mut World, entity: Entity, name: &str, value: &str) {
    let Some(mut settings) = world.get::<TooltipSettings>(entity).cloned() else {
        return;
    };
    match name {
        "for" => {
            let next = (!value.is_empty()).then(|| value.trim_start_matches('#').to_owned());
            if settings.target_id == next {
                return;
            }
            if let Some(old) = settings.target {
                if let Some(mut targets) = world.get_mut::<TooltipTargets>(old) {
                    targets.0.retain(|target| *target != entity);
                }
            }
            settings.target_id = next;
            settings.target = None;
            world.entity_mut(entity).insert(settings);
            if let Some(scope) = world
                .get::<ComponentStyleOwner>(entity)
                .map(|owner| owner.0)
            {
                resolve_targets(world, scope);
            }
            return;
        }
        "trigger" => {
            settings.hover = value.split(['|', ',', ' ']).any(|item| item == "hover");
            settings.click = value.split(['|', ',', ' ']).any(|item| item == "click");
            settings.drag = value.split(['|', ',', ' ']).any(|item| item == "drag");
        }
        "variant" => {
            settings.variant = if value == "point" {
                TooltipVariant::Point
            } else {
                TooltipVariant::Follow
            }
        }
        "alignment" => {
            settings.alignment = if value == "vertical" {
                ToolTipAlignment::Vertical
            } else {
                ToolTipAlignment::Horizontal
            }
        }
        "prio" | "priority" => {
            settings.priority = match value {
                "top" => ToolTipPriority::Top,
                "bottom" | "down" => ToolTipPriority::Bottom,
                "left" => ToolTipPriority::Left,
                "right" => ToolTipPriority::Right,
                _ => return,
            }
        }
        _ => return,
    }
    if let Some(nose) = world.get::<Children>(entity).and_then(|children| {
        children.iter().copied().find(|child| {
            world
                .get::<crate::ControlPart>(*child)
                .is_some_and(|part| part.kind == ControlPartKind::Indicator)
        })
    }) {
        world
            .entity_mut(nose)
            .insert(if settings.variant == TooltipVariant::Point {
                bevy::prelude::Visibility::Visible
            } else {
                bevy::prelude::Visibility::Hidden
            });
    }
    world.entity_mut(entity).insert(settings);
}

fn target_contains(world: &World, target: Entity, hit: Entity) -> bool {
    let mut current = Some(hit);
    while let Some(entity) = current {
        if entity == target {
            return true;
        }
        current = world.get::<ChildOf>(entity).map(ChildOf::parent);
    }
    false
}

fn hovered_target(world: &World, target: Entity, pointer: PointerId) -> bool {
    world
        .get_resource::<HoverMap>()
        .and_then(|hover| hover.get(&pointer))
        .is_some_and(|hits| hits.keys().any(|hit| target_contains(world, target, *hit)))
}

fn tooltips_for_hit(world: &World, hit: Entity) -> Vec<Entity> {
    let mut current = Some(hit);
    let mut tooltips = Vec::new();
    while let Some(entity) = current {
        if let Some(targets) = world.get::<TooltipTargets>(entity) {
            tooltips.extend(targets.0.iter().copied());
        }
        current = world.get::<ChildOf>(entity).map(ChildOf::parent);
    }
    tooltips
}

fn set_tooltip_open(
    world: &mut World,
    entity: Entity,
    open: bool,
    pointer_id: PointerId,
    pointer: Vec2,
) {
    let open = open && world.get::<InteractionDisabled>(entity).is_none();
    let current = world.get::<OpenTooltip>(entity).copied();
    let next = OpenTooltip {
        pointer_id,
        pointer,
    };
    if current.is_some() == open {
        if open && current != Some(next) {
            *world.get_mut::<OpenTooltip>(entity).unwrap() = next;
        }
        return;
    }
    if open {
        world.entity_mut(entity).insert(next);
    } else {
        world.entity_mut(entity).remove::<OpenTooltip>();
    }
    set_widget_display(world, entity, open);
}

pub(crate) fn set_bound_open(world: &mut World, entity: Entity, open: bool) {
    if world.get::<TooltipSettings>(entity).is_none()
        || (world.get::<InteractionDisabled>(entity).is_none()
            && world.get::<OpenTooltip>(entity).is_some() == open)
    {
        return;
    }
    set_tooltip_open(world, entity, open, PointerId::Mouse, Vec2::ZERO);
}

fn handle_target_event(
    world: &mut World,
    hit: Entity,
    pointer: PointerId,
    position: Vec2,
    action: u8,
) {
    for tooltip in tooltips_for_hit(world, hit) {
        let Some(settings) = world.get::<TooltipSettings>(tooltip).cloned() else {
            continue;
        };
        let open = world.get::<OpenTooltip>(tooltip).is_some();
        if world.get::<InteractionDisabled>(tooltip).is_some() {
            set_tooltip_open(world, tooltip, false, pointer, position);
            continue;
        }
        match action {
            0 if settings.hover => set_tooltip_open(world, tooltip, true, pointer, position),
            1 if settings.hover
                && !hovered_target(world, settings.target.unwrap_or(hit), pointer) =>
            {
                set_tooltip_open(world, tooltip, false, pointer, position);
            }
            2 if settings.click => set_tooltip_open(world, tooltip, !open, pointer, position),
            3 if settings.drag => set_tooltip_open(world, tooltip, true, pointer, position),
            4 if settings.drag => set_tooltip_open(world, tooltip, false, pointer, position),
            _ => {}
        }
    }
}

fn refresh_hover(world: &mut World) {
    let pointers = {
        let mut query = world.query::<(&PointerId, &PointerLocation)>();
        query
            .iter(world)
            .filter_map(|(id, location)| {
                location
                    .location()
                    .filter(|location| location.position.is_finite())
                    .map(|location| (*id, location.position))
            })
            .collect::<Vec<_>>()
    };
    let tooltips = {
        let mut query = world.query::<(
            Entity,
            &TooltipSettings,
            Option<&OpenTooltip>,
            Option<&InteractionDisabled>,
        )>();
        query
            .iter(world)
            .filter_map(|(entity, settings, open, disabled)| {
                let hover_only = settings.hover && !settings.click && !settings.drag;
                (hover_only || (disabled.is_some() && open.is_some())).then_some((
                    entity,
                    settings.target,
                    open.copied(),
                    disabled.is_some(),
                ))
            })
            .collect::<Vec<_>>()
    };
    let has_hover = world.contains_resource::<HoverMap>();
    for (entity, target, open, disabled) in tooltips {
        if disabled {
            set_tooltip_open(world, entity, false, PointerId::Mouse, Vec2::ZERO);
            continue;
        }
        if !has_hover {
            continue;
        }
        // Retain the active pointer when multiple pointers hover the same target.
        let hovered = target.and_then(|target| {
            open.and_then(|open| {
                pointers
                    .iter()
                    .find(|(id, _)| *id == open.pointer_id && hovered_target(world, target, *id))
            })
            .or_else(|| {
                pointers
                    .iter()
                    .find(|(id, _)| *id == PointerId::Mouse && hovered_target(world, target, *id))
            })
            .or_else(|| {
                pointers
                    .iter()
                    .find(|(id, _)| hovered_target(world, target, *id))
            })
            .copied()
        });
        if let Some((pointer, position)) = hovered {
            set_tooltip_open(world, entity, true, pointer, position);
        } else {
            set_tooltip_open(world, entity, false, PointerId::Mouse, Vec2::ZERO);
        }
    }
}

pub(crate) fn tooltip_pointer_input(
    mut over: Option<MessageReader<PointerOver>>,
    mut out: Option<MessageReader<PointerOut>>,
    mut clicks: Option<MessageReader<PointerClick>>,
    mut moves: Option<MessageReader<PointerMove>>,
    mut starts: Option<MessageReader<PointerDragStart>>,
    mut ends: Option<MessageReader<PointerDragEnd>>,
    mut commands: Commands,
) {
    if let Some(over) = over.as_mut() {
        for event in over.read() {
            let (hit, pointer, position) = (event.entity, event.pointer.id, event.pointer.position);
            commands.queue(move |world: &mut World| {
                handle_target_event(world, hit, pointer, position, 0)
            });
        }
    }
    if let Some(out) = out.as_mut() {
        for event in out.read() {
            let (hit, pointer, position) = (event.entity, event.pointer.id, event.pointer.position);
            commands.queue(move |world: &mut World| {
                handle_target_event(world, hit, pointer, position, 1)
            });
        }
    }
    if let Some(clicks) = clicks.as_mut() {
        for event in clicks.read() {
            if event.button != PointerButton::Primary {
                continue;
            }
            let (hit, pointer, position) = (event.entity, event.pointer.id, event.pointer.position);
            commands.queue(move |world: &mut World| {
                handle_target_event(world, hit, pointer, position, 2)
            });
        }
    }
    if let Some(starts) = starts.as_mut() {
        for event in starts.read() {
            if event.button != PointerButton::Primary {
                continue;
            }
            let (hit, pointer, position) = (event.entity, event.pointer.id, event.pointer.position);
            commands.queue(move |world: &mut World| {
                handle_target_event(world, hit, pointer, position, 3)
            });
        }
    }
    if let Some(ends) = ends.as_mut() {
        for event in ends.read() {
            if event.button != PointerButton::Primary {
                continue;
            }
            let (hit, pointer, position) = (event.entity, event.pointer.id, event.pointer.position);
            commands.queue(move |world: &mut World| {
                handle_target_event(world, hit, pointer, position, 4)
            });
        }
    }
    if let Some(moves) = moves.as_mut() {
        for event in moves.read() {
            let position = event.pointer.position;
            let pointer = event.pointer.id;
            commands.queue(move |world: &mut World| {
                let mut query = world.query::<(Entity, &TooltipSettings, &OpenTooltip)>();
                let follows = query
                    .iter(world)
                    .filter(|(_, settings, open)| {
                        settings.variant == TooltipVariant::Follow && open.pointer_id == pointer
                    })
                    .map(|(entity, _, _)| entity)
                    .collect::<Vec<_>>();
                for entity in follows {
                    if let Some(mut open) = world.get_mut::<OpenTooltip>(entity) {
                        open.pointer = position;
                    }
                }
            });
        }
    }
}

fn clamp_to_viewport(mut top_left: Vec2, size: Vec2, viewport: Vec2, margin: f32) -> Vec2 {
    top_left.x = top_left
        .x
        .clamp(margin, (viewport.x - size.x - margin).max(margin));
    top_left.y = top_left
        .y
        .clamp(margin, (viewport.y - size.y - margin).max(margin));
    top_left
}

fn follow_position(pointer: Vec2, size: Vec2, viewport: Vec2, gap: f32) -> Vec2 {
    let mut result = pointer + Vec2::splat(gap);
    for axis in 0..2 {
        if result[axis] + size[axis] > viewport[axis] - gap {
            result[axis] = pointer[axis] - gap - size[axis];
        }
    }
    result.clamp(Vec2::ZERO, (viewport - size).max(Vec2::ZERO))
}

#[cfg(test)]
fn point_position(
    target: Vec2,
    target_size: Vec2,
    size: Vec2,
    viewport: Vec2,
    priority: ToolTipPriority,
    margin: f32,
    gap: f32,
) -> Vec2 {
    point_placement(target, target_size, size, viewport, priority, margin, gap).0
}

fn point_placement(
    target: Vec2,
    target_size: Vec2,
    size: Vec2,
    viewport: Vec2,
    priority: ToolTipPriority,
    margin: f32,
    gap: f32,
) -> (Vec2, ToolTipPriority) {
    let place = |side| match side {
        ToolTipPriority::Left => Vec2::new(
            target.x - size.x - gap,
            target.y + (target_size.y - size.y) * 0.5,
        ),
        ToolTipPriority::Right => Vec2::new(
            target.x + target_size.x + gap,
            target.y + (target_size.y - size.y) * 0.5,
        ),
        ToolTipPriority::Top => Vec2::new(
            target.x + (target_size.x - size.x) * 0.5,
            target.y - size.y - gap,
        ),
        ToolTipPriority::Bottom => Vec2::new(
            target.x + (target_size.x - size.x) * 0.5,
            target.y + target_size.y + gap,
        ),
    };
    let opposite = match priority {
        ToolTipPriority::Left => ToolTipPriority::Right,
        ToolTipPriority::Right => ToolTipPriority::Left,
        ToolTipPriority::Top => ToolTipPriority::Bottom,
        ToolTipPriority::Bottom => ToolTipPriority::Top,
    };
    let first = place(priority);
    let overflows = match priority {
        ToolTipPriority::Left => first.x < margin,
        ToolTipPriority::Right => first.x + size.x > viewport.x - margin,
        ToolTipPriority::Top => first.y < margin,
        ToolTipPriority::Bottom => first.y + size.y > viewport.y - margin,
    };
    let actual = if overflows { opposite } else { priority };
    (
        clamp_to_viewport(place(actual), size, viewport, margin),
        actual,
    )
}

fn update_nose(world: &mut World, tooltip: Entity, side: ToolTipPriority) {
    let Some(nose) = world
        .get::<Children>(tooltip)
        .into_iter()
        .flatten()
        .copied()
        .find(|child| {
            world.get::<crate::ControlPart>(*child).is_some_and(|part| {
                part.owner == tooltip && part.kind == ControlPartKind::Indicator
            })
        })
    else {
        return;
    };
    let direction = match side {
        ToolTipPriority::Top => 0.0,
        ToolTipPriority::Bottom => 1.0,
        ToolTipPriority::Right => 2.0,
        ToolTipPriority::Left => 3.0,
    };
    let mut desired = Node {
        position_type: PositionType::Absolute,
        margin: UiRect::all(Val::Px(0.0)),
        ..Default::default()
    };
    match side {
        ToolTipPriority::Top => {
            desired.width = Val::Px(14.0);
            desired.height = Val::Px(8.0);
            desired.left = Val::Percent(50.0);
            desired.bottom = Val::Px(-7.0);
            desired.margin.left = Val::Px(-7.0);
        }
        ToolTipPriority::Bottom => {
            desired.width = Val::Px(14.0);
            desired.height = Val::Px(8.0);
            desired.left = Val::Percent(50.0);
            desired.top = Val::Px(-7.0);
            desired.margin.left = Val::Px(-7.0);
        }
        ToolTipPriority::Left => {
            desired.width = Val::Px(8.0);
            desired.height = Val::Px(14.0);
            desired.right = Val::Px(-7.0);
            desired.top = Val::Percent(50.0);
            desired.margin.top = Val::Px(-7.0);
        }
        ToolTipPriority::Right => {
            desired.width = Val::Px(8.0);
            desired.height = Val::Px(14.0);
            desired.left = Val::Px(-7.0);
            desired.top = Val::Percent(50.0);
            desired.margin.top = Val::Px(-7.0);
        }
    }
    let needs_layout = world.get::<Node>(nose).is_some_and(|node| {
        node.position_type != desired.position_type
            || node.width != desired.width
            || node.height != desired.height
            || node.left != desired.left
            || node.right != desired.right
            || node.top != desired.top
            || node.bottom != desired.bottom
            || node.margin != desired.margin
    });
    if needs_layout && let Some(mut node) = world.get_mut::<Node>(nose) {
        node.position_type = desired.position_type;
        node.width = desired.width;
        node.height = desired.height;
        node.left = desired.left;
        node.right = desired.right;
        node.top = desired.top;
        node.bottom = desired.bottom;
        node.margin = desired.margin;
    }
    let Some(handle) = world
        .get::<MaterialNode<TooltipNoseMaterial>>(nose)
        .map(|node| node.0.clone())
    else {
        return;
    };
    let tint = world
        .get::<BackgroundColor>(tooltip)
        .map_or(bevy::color::Color::WHITE, |color| color.0)
        .to_linear();
    let fill = Vec4::new(tint.red, tint.green, tint.blue, tint.alpha);
    let needs_material = world
        .get_resource::<Assets<TooltipNoseMaterial>>()
        .and_then(|materials| materials.get(&handle))
        .is_some_and(|material| material.fill != fill || material.side.x != direction);
    if needs_material
        && let Some(mut materials) = world.get_resource_mut::<Assets<TooltipNoseMaterial>>()
        && let Some(mut material) = materials.get_mut(&handle)
    {
        material.fill = fill;
        material.side.x = direction;
    }
}

pub(crate) fn place_open_tooltips(world: &mut World) {
    refresh_hover(world);
    let open = {
        let mut query = world.query::<(Entity, &TooltipSettings, &OpenTooltip)>();
        query
            .iter(world)
            .map(|(entity, settings, open)| (entity, settings.clone(), *open))
            .collect::<Vec<_>>()
    };
    let viewport = {
        let mut windows = world.query_filtered::<&Window, bevy::ecs::query::With<PrimaryWindow>>();
        windows.iter(world).next().map(|window| {
            Vec2::new(
                window.physical_width() as f32,
                window.physical_height() as f32,
            )
        })
    };
    for (entity, settings, open) in open {
        crate::style::refresh_inherited_opacity(world, entity);
        let Some(parent) = world.get::<ChildOf>(entity).map(ChildOf::parent) else {
            continue;
        };
        let (Some(parent_transform), Some(parent_node), Some(tip_node)) = (
            world.get::<UiGlobalTransform>(parent),
            world.get::<ComputedNode>(parent),
            world.get::<ComputedNode>(entity),
        ) else {
            continue;
        };
        let Some(inverse) = parent_transform.try_inverse() else {
            continue;
        };
        let parent_size = parent_node.size();
        let inverse_scale = tip_node.inverse_scale_factor;
        if !inverse_scale.is_finite() || inverse_scale <= 0.0 {
            continue;
        }
        let scale = world
            .get::<ComputedUiRenderTargetInfo>(entity)
            .map_or(1.0, |target| target.scale_factor())
            / world.get_resource::<UiScale>().map_or(1.0, |scale| scale.0);
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let viewport = world
            .get::<ComputedUiRenderTargetInfo>(entity)
            .map(|target| target.physical_size().as_vec2())
            .filter(|size| size.min_element() > 0.0)
            .or(viewport)
            .unwrap_or(Vec2::new(1920.0, 1080.0) * scale);
        let size = tip_node.size();
        if size.x <= 0.0 || size.y <= 0.0 {
            continue;
        }
        let (position, nose_side) = match settings.variant {
            TooltipVariant::Follow => (
                follow_position(open.pointer * scale, size, viewport, 10.0 / inverse_scale),
                None,
            ),
            TooltipVariant::Point => {
                let Some(target) = settings.target else {
                    continue;
                };
                let (Some(transform), Some(node)) = (
                    world.get::<UiGlobalTransform>(target),
                    world.get::<ComputedNode>(target),
                ) else {
                    continue;
                };
                let top_left = transform.affine().translation - node.size() * 0.5;
                let (position, side) = point_placement(
                    top_left,
                    node.size(),
                    size,
                    viewport,
                    settings.priority,
                    6.0 * scale,
                    8.0 * scale,
                );
                (position, Some(side))
            }
        };
        if let Some(side) = nose_side {
            update_nose(world, entity, side);
        }
        let local = (inverse.transform_point2(position) + parent_size * 0.5) * inverse_scale;
        let layout = WidgetLayoutOverride {
            left: Some(Val::Px(local.x)),
            top: Some(Val::Px(local.y)),
            ..Default::default()
        };
        if world
            .get::<WidgetLayoutOverride>(entity)
            .is_some_and(|old| old.left == layout.left && old.top == layout.top)
        {
            continue;
        }
        if let Some(mut node) = world.get_mut::<Node>(entity) {
            layout.apply(&mut node);
        }
        world.entity_mut(entity).insert(layout);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        TooltipSettings, follow_position, point_placement, point_position, resolve_targets,
        tooltips_for_hit,
    };
    use crate::ComponentStyleOwner;
    use bevy::{
        camera::NormalizedRenderTarget,
        ecs::world::World,
        ecs::{change_detection::DetectChanges, entity::Entity},
        math::Vec2,
        ui::{Display, InteractionDisabled, Node},
    };
    use bevy_picking::{
        Pickable,
        backend::HitData,
        hover::HoverMap,
        pointer::{Location, PointerId, PointerLocation},
    };
    use tilt_ui_core::{TemplateAttribute, ToolTipPriority};

    #[test]
    fn follow_flips_inside_the_viewport() {
        let position = follow_position(
            Vec2::new(190.0, 90.0),
            Vec2::new(45.0, 25.0),
            Vec2::new(200.0, 100.0),
            8.0,
        );
        assert!(position.x < 190.0 && position.y < 90.0);
        assert!(position.x >= 4.0 && position.y >= 4.0);
    }

    #[test]
    fn scaled_follow_placement_matches_css_layout_and_stays_clean_at_rest() {
        use bevy::{
            app::Propagate,
            camera::{Camera, ComputedCameraValues, RenderTargetInfo},
            ecs::system::RunSystemOnce,
            math::Affine2,
            ui::{
                ComputedNode, ComputedUiRenderTargetInfo, UiGlobalTransform, UiScale,
                UiTargetCamera, Val,
            },
        };
        for ui_scale in [0.8, 1.5] {
            for dpi in [1.0, 2.0] {
                for pointer in [Vec2::new(100.0, 120.0), Vec2::new(795.0, 595.0)] {
                    let mut world = World::new();
                    world.insert_resource(UiScale(ui_scale));
                    let viewport = Vec2::new(800.0, 600.0) * dpi;
                    let camera = world
                        .spawn(Camera {
                            computed: ComputedCameraValues {
                                target_info: Some(RenderTargetInfo {
                                    physical_size: viewport.as_uvec2(),
                                    scale_factor: dpi,
                                }),
                                ..Default::default()
                            },
                            ..Default::default()
                        })
                        .id();
                    let origin = Vec2::new(13.0, 17.0);
                    let transform =
                        UiGlobalTransform::from(Affine2::from_translation(viewport * 0.5 + origin));
                    let root = world
                        .spawn((
                            Node::default(),
                            UiTargetCamera(camera),
                            ComputedNode {
                                size: viewport,
                                ..Default::default()
                            },
                            transform,
                        ))
                        .id();
                    world
                        .run_system_once(bevy::ui::update::propagate_ui_target_cameras)
                        .unwrap();
                    let target = world
                        .get::<Propagate<ComputedUiRenderTargetInfo>>(root)
                        .unwrap()
                        .0;
                    let inverse_scale = 1.0 / (dpi * ui_scale);
                    let size = Vec2::new(180.0, 80.0) / inverse_scale;
                    let popup = world
                        .spawn((
                            Node::default(),
                            ComputedNode {
                                size,
                                inverse_scale_factor: inverse_scale,
                                ..Default::default()
                            },
                            target,
                        ))
                        .id();
                    world.entity_mut(root).add_child(popup);
                    super::materialize(
                        &mut world,
                        popup,
                        &[TemplateAttribute::Static {
                            name: "trigger".into(),
                            value: "click".into(),
                        }],
                    );
                    super::set_tooltip_open(&mut world, popup, true, PointerId::Mouse, pointer);
                    super::place_open_tooltips(&mut world);
                    let gap = 10.0 / inverse_scale;
                    let position = if pointer.x < 400.0 {
                        pointer * dpi + Vec2::splat(gap)
                    } else {
                        pointer * dpi - Vec2::splat(gap) - size
                    };
                    let local = (transform.try_inverse().unwrap().transform_point2(position)
                        + viewport * 0.5)
                        * inverse_scale;
                    let layout = world.get::<crate::WidgetLayoutOverride>(popup).unwrap();
                    assert_eq!(layout.left, Some(Val::Px(local.x)));
                    assert_eq!(layout.top, Some(Val::Px(local.y)));
                    world.clear_trackers();
                    for _ in 0..2 {
                        super::place_open_tooltips(&mut world);
                        assert!(!world.entity(popup).get_ref::<Node>().unwrap().is_changed());
                        assert!(
                            !world
                                .entity(popup)
                                .get_ref::<crate::WidgetLayoutOverride>()
                                .unwrap()
                                .is_changed()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn point_uses_requested_side_and_opposite_on_overflow() {
        let size = Vec2::new(40.0, 20.0);
        let viewport = Vec2::new(200.0, 120.0);
        let right = point_position(
            Vec2::new(50.0, 50.0),
            Vec2::new(20.0, 20.0),
            size,
            viewport,
            ToolTipPriority::Right,
            4.0,
            6.0,
        );
        assert_eq!(right.x, 76.0);
        let flipped = point_position(
            Vec2::new(180.0, 50.0),
            Vec2::new(20.0, 20.0),
            size,
            viewport,
            ToolTipPriority::Right,
            4.0,
            6.0,
        );
        assert_eq!(flipped.x, 134.0);
        assert_eq!(
            point_placement(
                Vec2::new(180.0, 50.0),
                Vec2::new(20.0, 20.0),
                size,
                viewport,
                ToolTipPriority::Right,
                4.0,
                6.0,
            )
            .1,
            ToolTipPriority::Left
        );
        let top = point_position(
            Vec2::new(50.0, 2.0),
            Vec2::new(20.0, 20.0),
            size,
            viewport,
            ToolTipPriority::Top,
            4.0,
            6.0,
        );
        assert_eq!(top.y, 28.0);
    }

    #[test]
    fn triangle_nose_moves_to_the_side_facing_its_target() {
        use crate::ControlPartKind;
        let mut world = World::new();
        let tooltip = world.spawn(Node::default()).id();
        let nose =
            crate::widgets::controls::spawn_part(&mut world, tooltip, ControlPartKind::Indicator);
        world.entity_mut(tooltip).add_child(nose);
        super::update_nose(&mut world, tooltip, ToolTipPriority::Top);
        let node = world.get::<Node>(nose).unwrap();
        assert_eq!(node.bottom, bevy::ui::Val::Px(-7.0));
        assert_eq!(node.width, bevy::ui::Val::Px(14.0));
        super::update_nose(&mut world, tooltip, ToolTipPriority::Right);
        let node = world.get::<Node>(nose).unwrap();
        assert_eq!(node.left, bevy::ui::Val::Px(-7.0));
        assert_eq!(node.height, bevy::ui::Val::Px(14.0));
    }

    #[test]
    fn implicit_target_and_text_children_do_not_steal_pointer_hover() {
        let mut world = World::new();
        let scope = world.spawn_empty().id();
        let target = world.spawn(Node::default()).id();
        let tooltip = world
            .spawn((Node::default(), ComponentStyleOwner(scope)))
            .id();
        let text = world.spawn(Node::default()).id();
        world.entity_mut(scope).add_child(target);
        world.entity_mut(target).add_child(tooltip);
        world.entity_mut(tooltip).add_child(text);
        super::materialize(
            &mut world,
            tooltip,
            &[
                TemplateAttribute::Static {
                    name: "variant".into(),
                    value: "point".into(),
                },
                TemplateAttribute::Static {
                    name: "prio".into(),
                    value: "left".into(),
                },
            ],
        );
        resolve_targets(&mut world, scope);
        assert_eq!(
            world.get::<TooltipSettings>(tooltip).unwrap().target,
            Some(target)
        );
        assert_eq!(
            world.get::<TooltipSettings>(tooltip).unwrap().priority,
            ToolTipPriority::Left
        );
        assert_eq!(world.get::<Node>(tooltip).unwrap().display, Display::None);
        assert_eq!(world.get::<Pickable>(tooltip), Some(&Pickable::IGNORE));
        assert_eq!(world.get::<Pickable>(text), Some(&Pickable::IGNORE));
        assert_eq!(tooltips_for_hit(&world, target), vec![tooltip]);
    }

    #[test]
    fn changing_target_unregisters_the_old_pointer_target() {
        let mut world = World::new();
        let scope = world.spawn(crate::ComponentElementIds::default()).id();
        let first = world.spawn(Node::default()).id();
        let second = world.spawn(Node::default()).id();
        world
            .get_mut::<crate::ComponentElementIds>(scope)
            .unwrap()
            .insert("first".into(), first);
        world
            .get_mut::<crate::ComponentElementIds>(scope)
            .unwrap()
            .insert("second".into(), second);
        let tooltip = world
            .spawn((Node::default(), ComponentStyleOwner(scope)))
            .id();
        super::materialize(
            &mut world,
            tooltip,
            &[TemplateAttribute::Static {
                name: "for".into(),
                value: "first".into(),
            }],
        );
        resolve_targets(&mut world, scope);
        assert_eq!(
            world.get::<TooltipSettings>(tooltip).unwrap().target,
            Some(first)
        );
        super::reconfigure(&mut world, tooltip, "for", "second");
        assert_eq!(
            world.get::<TooltipSettings>(tooltip).unwrap().target,
            Some(second)
        );
        assert!(tooltips_for_hit(&world, first).is_empty());
        assert_eq!(tooltips_for_hit(&world, second), vec![tooltip]);
    }

    #[test]
    fn hover_and_click_triggers_reuse_the_same_tooltip_entity() {
        let mut world = World::new();
        let scope = world.spawn_empty().id();
        let target = world.spawn(Node::default()).id();
        let hover = world
            .spawn((Node::default(), ComponentStyleOwner(scope)))
            .id();
        let click = world
            .spawn((Node::default(), ComponentStyleOwner(scope)))
            .id();
        let label = world.spawn(Node::default()).id();
        world.entity_mut(scope).add_child(target);
        world.entity_mut(target).add_child(hover).add_child(click);
        world.entity_mut(hover).add_child(label);
        super::materialize(&mut world, hover, &[]);
        super::materialize(
            &mut world,
            click,
            &[TemplateAttribute::Static {
                name: "trigger".into(),
                value: "click".into(),
            }],
        );
        resolve_targets(&mut world, scope);

        super::handle_target_event(
            &mut world,
            target,
            PointerId::Mouse,
            Vec2::new(40.0, 50.0),
            0,
        );
        assert!(world.get::<super::OpenTooltip>(hover).is_some());
        assert!(world.get::<super::OpenTooltip>(click).is_none());
        assert_ne!(world.get::<Node>(hover).unwrap().display, Display::None);
        super::handle_target_event(
            &mut world,
            target,
            PointerId::Mouse,
            Vec2::new(40.0, 50.0),
            1,
        );
        assert_eq!(world.get::<Node>(hover).unwrap().display, Display::None);

        super::handle_target_event(
            &mut world,
            target,
            PointerId::Mouse,
            Vec2::new(40.0, 50.0),
            2,
        );
        assert_ne!(world.get::<Node>(click).unwrap().display, Display::None);
        super::handle_target_event(
            &mut world,
            target,
            PointerId::Mouse,
            Vec2::new(40.0, 50.0),
            2,
        );
        assert_eq!(world.get::<Node>(click).unwrap().display, Display::None);
        assert!(world.get_entity(hover).is_ok());
        assert!(world.get_entity(label).is_ok());
    }

    fn current_hover(world: &mut World, hit: Entity, position: Vec2) -> Entity {
        world.init_resource::<bevy_picking::pointer::PointerMap>();
        world.insert_resource(HoverMap::default());
        world
            .resource_mut::<HoverMap>()
            .entry(PointerId::Mouse)
            .or_default()
            .insert(hit, HitData::new(hit, 0.0, None, None));
        world
            .spawn((
                PointerId::Mouse,
                PointerLocation::new(Location {
                    target: NormalizedRenderTarget::None {
                        width: 800,
                        height: 600,
                    },
                    position,
                }),
            ))
            .id()
    }

    fn persistent_tip(world: &mut World, trigger: &str) -> (Entity, Entity) {
        let scope = world.spawn_empty().id();
        let target = world.spawn(Node::default()).id();
        let tooltip = world
            .spawn((Node::default(), ComponentStyleOwner(scope)))
            .id();
        world.entity_mut(scope).add_child(target);
        world.entity_mut(target).add_child(tooltip);
        super::materialize(
            world,
            tooltip,
            &[TemplateAttribute::Static {
                name: "trigger".into(),
                value: trigger.into(),
            }],
        );
        resolve_targets(world, scope);
        (target, tooltip)
    }

    #[test]
    fn newly_materialized_hover_tooltip_opens_under_an_already_stationary_pointer() {
        let mut world = World::new();
        let scope = world.spawn_empty().id();
        let target = world.spawn(Node::default()).id();
        let label = world.spawn(Node::default()).id();
        world.entity_mut(scope).add_child(target);
        world.entity_mut(target).add_child(label);
        let position = Vec2::new(42.0, 57.0);
        current_hover(&mut world, label, position);
        super::handle_target_event(&mut world, label, PointerId::Mouse, position, 0);

        let tooltip = world
            .spawn((Node::default(), ComponentStyleOwner(scope)))
            .id();
        world.entity_mut(target).add_child(tooltip);
        super::materialize(&mut world, tooltip, &[]);
        resolve_targets(&mut world, scope);
        super::place_open_tooltips(&mut world);
        let open = world.get::<super::OpenTooltip>(tooltip).unwrap();
        assert_eq!(open.pointer_id, PointerId::Mouse);
        assert_eq!(open.pointer, position);
        assert_ne!(world.get::<Node>(tooltip).unwrap().display, Display::None);
        world.resource_mut::<HoverMap>().clear();
        super::place_open_tooltips(&mut world);
        assert!(world.get::<super::OpenTooltip>(tooltip).is_none());
    }

    #[test]
    fn disabled_empty_slot_tooltip_reenables_without_a_click_or_new_pointer_enter() {
        let mut world = World::new();
        let (target, tooltip) = persistent_tip(&mut world, "hover");
        let position = Vec2::new(84.0, 129.0);
        current_hover(&mut world, target, position);
        world.entity_mut(tooltip).insert(InteractionDisabled);
        super::set_bound_open(&mut world, tooltip, true);
        super::place_open_tooltips(&mut world);
        assert!(world.get::<super::OpenTooltip>(tooltip).is_none());
        assert_eq!(world.get::<Node>(tooltip).unwrap().display, Display::None);
        world.entity_mut(tooltip).remove::<InteractionDisabled>();
        super::place_open_tooltips(&mut world);
        assert_eq!(
            world.get::<super::OpenTooltip>(tooltip).unwrap().pointer,
            position
        );

        world.entity_mut(tooltip).insert(InteractionDisabled);
        super::place_open_tooltips(&mut world);
        assert!(world.get::<super::OpenTooltip>(tooltip).is_none());
        world.entity_mut(tooltip).remove::<InteractionDisabled>();
        super::place_open_tooltips(&mut world);
        assert_eq!(
            world.get::<super::OpenTooltip>(tooltip).unwrap().pointer,
            position
        );
    }

    #[test]
    fn disabling_click_or_drag_tooltips_closes_them_and_blocks_every_open_path() {
        for (trigger, action) in [("click", 2), ("drag", 3), ("hover|click|drag", 3)] {
            let mut world = World::new();
            let (target, tooltip) = persistent_tip(&mut world, trigger);
            current_hover(&mut world, target, Vec2::ONE);
            super::handle_target_event(&mut world, target, PointerId::Mouse, Vec2::ONE, action);
            assert!(world.get::<super::OpenTooltip>(tooltip).is_some());
            world.entity_mut(tooltip).insert(InteractionDisabled);
            super::place_open_tooltips(&mut world);
            assert!(world.get::<super::OpenTooltip>(tooltip).is_none());
            for action in [0, 2, 3] {
                super::handle_target_event(&mut world, target, PointerId::Mouse, Vec2::ONE, action);
                assert!(world.get::<super::OpenTooltip>(tooltip).is_none());
            }
            super::set_bound_open(&mut world, tooltip, true);
            assert!(world.get::<super::OpenTooltip>(tooltip).is_none());
            world.entity_mut(tooltip).remove::<InteractionDisabled>();
            super::place_open_tooltips(&mut world);
            assert!(world.get::<super::OpenTooltip>(tooltip).is_none());
        }
    }

    #[test]
    fn unchanged_hover_sync_does_not_dirty_tooltip_layout_or_material_resources() {
        let mut world = World::new();
        world.init_resource::<bevy::asset::Assets<super::TooltipNoseMaterial>>();
        let (target, tooltip) = persistent_tip(&mut world, "hover");
        current_hover(&mut world, target, Vec2::ONE);
        world.resource_mut::<HoverMap>().clear();
        world.clear_trackers();
        for _ in 0..3 {
            super::place_open_tooltips(&mut world);
        }
        assert!(
            !world
                .entity(tooltip)
                .get_ref::<Node>()
                .unwrap()
                .is_changed()
        );
        assert!(
            !world
                .entity(tooltip)
                .get_ref::<crate::widgets::state::WidgetDisplayOverride>()
                .unwrap()
                .is_changed()
        );
        assert!(
            !world
                .get_resource_ref::<bevy::asset::Assets<super::TooltipNoseMaterial>>()
                .unwrap()
                .is_changed()
        );

        world
            .resource_mut::<HoverMap>()
            .entry(PointerId::Mouse)
            .or_default()
            .insert(target, HitData::new(target, 0.0, None, None));
        super::place_open_tooltips(&mut world);
        world.clear_trackers();
        for _ in 0..3 {
            super::place_open_tooltips(&mut world);
        }
        assert!(
            !world
                .entity(tooltip)
                .get_ref::<Node>()
                .unwrap()
                .is_changed()
        );
        assert!(
            !world
                .entity(tooltip)
                .get_ref::<super::OpenTooltip>()
                .unwrap()
                .is_changed()
        );
        assert!(
            !world
                .entity(tooltip)
                .get_ref::<crate::widgets::state::WidgetDisplayOverride>()
                .unwrap()
                .is_changed()
        );
        assert!(
            !world
                .get_resource_ref::<bevy::asset::Assets<super::TooltipNoseMaterial>>()
                .unwrap()
                .is_changed()
        );
    }
}
