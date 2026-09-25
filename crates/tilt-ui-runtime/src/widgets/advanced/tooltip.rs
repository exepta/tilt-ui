//! Targeted pointer triggers and viewport-aware placement for TiltUI tooltips.

use bevy::{
    ecs::{
        component::Component,
        entity::Entity,
        hierarchy::{ChildOf, Children},
        message::MessageReader,
        system::Commands,
        world::World,
    },
    math::Vec2,
    ui::{
        ComputedNode, ComputedUiRenderTargetInfo, GlobalZIndex, Node, UiGlobalTransform, UiScale,
        Val,
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
    world
        .entity_mut(entity)
        .insert((settings, Pickable::IGNORE, GlobalZIndex(10_000)));
    let nose = crate::widgets::controls::spawn_part(world, entity, ControlPartKind::Indicator);
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

fn point_position(
    target: Vec2,
    target_size: Vec2,
    size: Vec2,
    viewport: Vec2,
    priority: ToolTipPriority,
    margin: f32,
    gap: f32,
) -> Vec2 {
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
    clamp_to_viewport(
        if overflows { place(opposite) } else { first },
        size,
        viewport,
        margin,
    )
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
        let position = match settings.variant {
            TooltipVariant::Follow => follow_position(
                open.pointer * scale,
                size,
                viewport,
                6.0 * scale,
                10.0 * scale,
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
                point_position(
                    top_left,
                    node.size(),
                    size,
                    viewport,
                    settings.priority,
                    6.0 * scale,
                    8.0 * scale,
                )
            }
        };
        let local = inverse.transform_point2(position) + parent_node.size() * 0.5;
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
        TooltipSettings, follow_position, point_position, resolve_targets, tooltips_for_hit,
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
