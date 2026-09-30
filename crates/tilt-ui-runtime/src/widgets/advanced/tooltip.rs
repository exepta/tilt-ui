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
        BackgroundColor, ComputedNode, ComputedUiRenderTargetInfo, GlobalZIndex, Node,
        PositionType, UiGlobalTransform, UiRect, UiScale, Val,
    },
    ui_render::{
        UiMaterialPlugin,
        ui_material::{MaterialNode, UiMaterial},
    },
    window::{PrimaryWindow, Window},
};
use bevy_picking::{
    Pickable,
    events::{Click, DragEnd, DragStart, Move, Out, Over, Pointer},
    hover::HoverMap,
    pointer::{PointerButton, PointerId},
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
                Shader::from_wgsl(
                    include_str!("tooltip_nose.wgsl"),
                    "tilt_ui_tooltip_nose.wgsl",
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

#[derive(Component, Debug, Clone, Copy)]
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
    if open {
        world.entity_mut(entity).insert(OpenTooltip {
            pointer_id,
            pointer,
        });
    } else {
        world.entity_mut(entity).remove::<OpenTooltip>();
    }
    set_widget_display(world, entity, open);
}

pub(crate) fn set_bound_open(world: &mut World, entity: Entity, open: bool) {
    if world.get::<TooltipSettings>(entity).is_none()
        || world.get::<OpenTooltip>(entity).is_some() == open
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

pub(crate) fn tooltip_pointer_input(
    mut over: Option<MessageReader<Pointer<Over>>>,
    mut out: Option<MessageReader<Pointer<Out>>>,
    mut clicks: Option<MessageReader<Pointer<Click>>>,
    mut moves: Option<MessageReader<Pointer<Move>>>,
    mut starts: Option<MessageReader<Pointer<DragStart>>>,
    mut ends: Option<MessageReader<Pointer<DragEnd>>>,
    mut commands: Commands,
) {
    if let Some(over) = over.as_mut() {
        for event in over.read() {
            let (hit, pointer, position) = (
                event.entity,
                event.pointer_id,
                event.pointer_location.position,
            );
            commands.queue(move |world: &mut World| {
                handle_target_event(world, hit, pointer, position, 0)
            });
        }
    }
    if let Some(out) = out.as_mut() {
        for event in out.read() {
            let (hit, pointer, position) = (
                event.entity,
                event.pointer_id,
                event.pointer_location.position,
            );
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
            let (hit, pointer, position) = (
                event.entity,
                event.pointer_id,
                event.pointer_location.position,
            );
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
            let (hit, pointer, position) = (
                event.entity,
                event.pointer_id,
                event.pointer_location.position,
            );
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
            let (hit, pointer, position) = (
                event.entity,
                event.pointer_id,
                event.pointer_location.position,
            );
            commands.queue(move |world: &mut World| {
                handle_target_event(world, hit, pointer, position, 4)
            });
        }
    }
    if let Some(moves) = moves.as_mut() {
        for event in moves.read() {
            let position = event.pointer_location.position;
            let pointer = event.pointer_id;
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

fn follow_position(pointer: Vec2, size: Vec2, viewport: Vec2, margin: f32, gap: f32) -> Vec2 {
    let x = if pointer.x + gap + size.x > viewport.x - margin {
        pointer.x - gap - size.x
    } else {
        pointer.x + gap
    };
    let y = if pointer.y + gap + size.y > viewport.y - margin {
        pointer.y - gap - size.y
    } else {
        pointer.y + gap
    };
    clamp_to_viewport(Vec2::new(x, y), size, viewport, margin)
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
    let open = {
        let mut query = world.query::<(Entity, &TooltipSettings, &OpenTooltip)>();
        query
            .iter(world)
            .map(|(entity, settings, open)| (entity, settings.clone(), *open))
            .collect::<Vec<_>>()
    };
    let viewport = {
        let mut windows = world.query_filtered::<&Window, bevy::ecs::query::With<PrimaryWindow>>();
        windows
            .iter(world)
            .next()
            .map(|window| Vec2::new(window.width(), window.height()))
    };
    for (entity, settings, open) in open {
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
        let scale = world
            .get::<ComputedUiRenderTargetInfo>(entity)
            .map_or(1.0, |target| target.scale_factor())
            / world.get_resource::<UiScale>().map_or(1.0, |scale| scale.0);
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let viewport = viewport.unwrap_or(Vec2::new(1920.0, 1080.0)) * scale;
        let size = tip_node.size();
        if size.x <= 0.0 || size.y <= 0.0 {
            continue;
        }
        let (position, nose_side) = match settings.variant {
            TooltipVariant::Follow => (
                follow_position(
                    open.pointer * scale,
                    size,
                    viewport,
                    6.0 * scale,
                    10.0 * scale,
                ),
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
        let local = inverse.transform_point2(position) + parent_size * 0.5;
        let layout = WidgetLayoutOverride {
            left: Some(Val::Px(local.x / scale)),
            top: Some(Val::Px(local.y / scale)),
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
        ecs::world::World,
        math::Vec2,
        ui::{Display, Node},
    };
    use bevy_picking::{Pickable, pointer::PointerId};
    use tilt_ui_core::{TemplateAttribute, ToolTipPriority};

    #[test]
    fn follow_flips_inside_the_viewport() {
        let position = follow_position(
            Vec2::new(190.0, 90.0),
            Vec2::new(45.0, 25.0),
            Vec2::new(200.0, 100.0),
            4.0,
            8.0,
        );
        assert!(position.x < 190.0 && position.y < 90.0);
        assert!(position.x >= 4.0 && position.y >= 4.0);
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
}
