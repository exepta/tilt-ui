//! Shared overflow scrolling and CSS-styled scrollbar parts.

use bevy::{
    app::{App, Plugin, PostUpdate, Update},
    ecs::{
        component::Component,
        hierarchy::ChildOf,
        message::MessageReader,
        query::With,
        schedule::SystemSet,
        system::{Commands, Query, Res},
        world::World,
    },
    input::mouse::MouseScrollUnit,
    math::{BVec2, Vec2},
    prelude::{Entity, InheritedVisibility, IntoScheduleConfigs, Visibility},
    text::TextLayoutInfo,
    ui::{
        ComputedNode, ComputedUiRenderTargetInfo, ComputedUiTargetCamera, IgnoreScroll, Node,
        OverflowAxis, ScrollPosition, UiGlobalTransform, UiScale, UiStack, UiSystems, Val,
        widget::{TextScroll, scroll_editable_text},
    },
};
use bevy_picking::{
    Pickable,
    events::{Drag, DragEnd, Pointer, Press, Release, Scroll},
    hover::HoverMap,
    pointer::{PointerButton, PointerId},
};
use tilt_ui_css::CssOverflow;

use crate::{
    ComponentStyleOwner, ControlPart, ControlPartKind, TiltElement,
    style::CascadedStyle,
    widgets::{controls::spawn_part, state::WidgetLayoutOverride},
};

/// Registers shared wheel, scrollbar drag, and scrollbar geometry systems.
#[derive(Debug, Default, Clone, Copy)]
pub struct TiltUiScrollRuntimePlugin;

/// Orders scrollbar pointer capture before editable text selection.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ScrollSystemSet {
    Pointer,
}

impl Plugin for TiltUiScrollRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, wheel_scroll);
        app.init_resource::<crate::widgets::state::UiMotionSettings>();
        app.add_systems(
            Update,
            scrollbar_pointer_input.in_set(ScrollSystemSet::Pointer),
        );
        app.add_systems(
            PostUpdate,
            (
                animate_wheel_scroll.before(UiSystems::Layout),
                restore_user_text_scroll.after(scroll_editable_text),
                update_scrollbar_visuals
                    .after(restore_user_text_scroll)
                    .after(UiSystems::PostLayout),
            ),
        );
    }
}

#[derive(Component, Debug, Clone, Copy, Default)]
struct ScrollbarParts {
    x: Option<(Entity, Entity)>,
    y: Option<(Entity, Entity)>,
    x_mode: Option<CssOverflow>,
    y_mode: Option<CssOverflow>,
}

#[derive(Component, Debug, Clone, Copy)]
struct SmoothScrollTarget(Vec2);

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct ActiveScrollbarDrag {
    pub(crate) pointer: PointerId,
    pub(crate) vertical: bool,
    pub(crate) grab_offset: f32,
}

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct UserTextScroll(Vec2);

fn restore_user_text_scroll(
    mut editors: Query<(
        &mut UserTextScroll,
        &mut TextScroll,
        &ComputedNode,
        Option<&TextLayoutInfo>,
    )>,
) {
    for (mut requested, mut actual, node, layout) in &mut editors {
        let max = text_scroll_max(node, layout);
        let clamped = requested.0.clamp(Vec2::ZERO, max);
        if requested.0 != clamped {
            requested.0 = clamped;
        }
        if actual.0 != clamped {
            actual.0 = clamped;
        }
    }
}

fn text_scroll_max(node: &ComputedNode, layout: Option<&TextLayoutInfo>) -> Vec2 {
    let view = node.content_box().size();
    let content = layout.map_or_else(|| node.content_size(), |layout| layout.size);
    (content - view).max(Vec2::ZERO)
}

fn scrollable(mode: Option<CssOverflow>) -> bool {
    matches!(mode, Some(CssOverflow::Auto | CssOverflow::Scroll))
}

/// Creates persistent scrollbar anatomy for styled overflow elements in one component scope.
pub(crate) fn ensure_scrollbar_parts(world: &mut World, scope: Entity) -> bool {
    let candidates = {
        let mut query = world.query::<(Entity, &ComponentStyleOwner, &CascadedStyle)>();
        query
            .iter(world)
            .filter(|(entity, owner, _)| {
                owner.0 == scope
                    && (world.get::<TiltElement>(*entity).is_some()
                        || world
                            .get::<ControlPart>(*entity)
                            .is_some_and(|part| part.kind == ControlPartKind::Popup))
            })
            .map(|(entity, _, style)| (entity, style.0.overflow_x, style.0.overflow_y))
            .collect::<Vec<_>>()
    };
    let mut created = false;
    for (owner, x_mode, y_mode) in candidates {
        let mut parts = world
            .get::<ScrollbarParts>(owner)
            .copied()
            .unwrap_or_default();
        parts.x_mode = x_mode;
        parts.y_mode = y_mode;
        if scrollable(x_mode) && parts.x.is_none() {
            parts.x = Some(spawn_axis(world, owner, false));
            created = true;
        }
        if scrollable(y_mode) && parts.y.is_none() {
            parts.y = Some(spawn_axis(world, owner, true));
            created = true;
        }
        if parts.x.is_some() || parts.y.is_some() {
            if world.get::<TextScroll>(owner).is_none()
                && world.get::<ScrollPosition>(owner).is_none()
            {
                world.entity_mut(owner).insert(ScrollPosition::default());
            }
            world.entity_mut(owner).insert(parts);
        }
    }
    created
}

fn spawn_axis(world: &mut World, owner: Entity, vertical: bool) -> (Entity, Entity) {
    let (track_kind, thumb_kind) = if vertical {
        (
            ControlPartKind::ScrollbarYTrack,
            ControlPartKind::ScrollbarYThumb,
        )
    } else {
        (
            ControlPartKind::ScrollbarXTrack,
            ControlPartKind::ScrollbarXThumb,
        )
    };
    let track = spawn_part(world, owner, track_kind);
    let thumb = spawn_part(world, owner, thumb_kind);
    world.entity_mut(track).insert((
        IgnoreScroll(BVec2::TRUE),
        Pickable::default(),
        Visibility::Hidden,
    ));
    world.entity_mut(thumb).insert(Pickable::default());
    world.entity_mut(track).add_child(thumb);
    world.entity_mut(owner).add_child(track);
    (track, thumb)
}

fn wheel_scroll(
    mut wheels: Option<MessageReader<Pointer<Scroll>>>,
    hover: Option<Res<HoverMap>>,
    mut commands: Commands,
) {
    let (Some(wheels), Some(hover)) = (wheels.as_mut(), hover) else {
        return;
    };
    for wheel in wheels.read() {
        let target = hover
            .get(&wheel.pointer_id)
            .into_iter()
            .flat_map(|hits| hits.iter())
            .min_by(|(_, a), (_, b)| a.depth.total_cmp(&b.depth))
            .map(|(entity, _)| *entity);
        if target != Some(wheel.entity) {
            continue;
        }
        let scale = wheel_unit_scale(wheel.unit);
        let delta = Vec2::new(-wheel.x, -wheel.y) * scale;
        let target = wheel.entity;
        let position = wheel.pointer_location.position;
        let camera = wheel.hit.camera;
        commands.queue(move |world: &mut World| {
            let target = frontmost_scroll_target(world, target, position, camera).unwrap_or(target);
            scroll_ancestors_with_motion(world, target, delta, true);
        });
    }
}

fn wheel_unit_scale(unit: MouseScrollUnit) -> f32 {
    match unit {
        MouseScrollUnit::Line => 56.0,
        MouseScrollUnit::Pixel => 1.0,
    }
}

fn frontmost_scroll_target(
    world: &World,
    fallback: Entity,
    position: Vec2,
    camera: Entity,
) -> Option<Entity> {
    let stack = world.get_resource::<UiStack>()?;
    stack.uinodes.iter().rev().copied().find(|entity| {
        let Some(parts) = world.get::<ScrollbarParts>(*entity) else {
            return false;
        };
        let Some(node) = world.get::<ComputedNode>(*entity) else {
            return false;
        };
        let (_, max) = scroll_geometry(world, *entity, node);
        if !(scrollable(parts.x_mode) && max.x > 0.0 || scrollable(parts.y_mode) && max.y > 0.0) {
            return false;
        }
        if world
            .get::<ComputedUiTargetCamera>(*entity)
            .and_then(ComputedUiTargetCamera::get)
            .is_some_and(|target| target != camera)
        {
            return false;
        }
        if world
            .get::<InheritedVisibility>(*entity)
            .is_some_and(|visibility| !visibility.get())
        {
            return false;
        }
        scroll_region_contains(world, *entity, position)
            && (is_ancestor(world, *entity, fallback) || is_ancestor(world, fallback, *entity))
    })
}

fn is_ancestor(world: &World, ancestor: Entity, target: Entity) -> bool {
    let mut current = Some(target);
    while let Some(entity) = current {
        if entity == ancestor {
            return true;
        }
        current = world.get::<ChildOf>(entity).map(ChildOf::parent);
    }
    false
}

fn scroll_region_contains(world: &World, entity: Entity, position: Vec2) -> bool {
    let mut current = Some(entity);
    while let Some(ancestor) = current {
        current = world.get::<ChildOf>(ancestor).map(ChildOf::parent);
        let Some(style) = world.get::<Node>(ancestor) else {
            continue;
        };
        if ancestor == entity
            || style.overflow.x != OverflowAxis::Visible
            || style.overflow.y != OverflowAxis::Visible
        {
            let Some(node) = world.get::<ComputedNode>(ancestor) else {
                continue;
            };
            let Some(transform) = world.get::<UiGlobalTransform>(ancestor) else {
                continue;
            };
            let Some(target) = world.get::<ComputedUiRenderTargetInfo>(ancestor) else {
                continue;
            };
            let scale = world.get_resource::<UiScale>().map_or(1.0, |scale| scale.0);
            let Some(point) =
                node.normalize_point(*transform, position * target.scale_factor() / scale)
            else {
                return false;
            };
            if point.x.abs() > 0.5 || point.y.abs() > 0.5 {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
fn scroll_ancestors(world: &mut World, target: Entity, delta: Vec2) {
    scroll_ancestors_with_motion(world, target, delta, false);
}

fn scroll_ancestors_with_motion(world: &mut World, target: Entity, mut delta: Vec2, smooth: bool) {
    let active_modal = {
        let mut dialogs = world.query::<(Entity, &crate::DialogState)>();
        dialogs
            .iter(world)
            .filter(|(_, state)| state.open && state.renderer == crate::DialogRenderer::Bevy)
            .map(|(entity, _)| entity)
            .last()
    };
    if active_modal.is_some_and(|modal| !is_ancestor(world, modal, target)) {
        return;
    }
    let mut current = Some(target);
    while let Some(entity) = current {
        if Some(entity) == active_modal {
            break;
        }
        current = world.get::<ChildOf>(entity).map(ChildOf::parent);
        let Some(parts) = world.get::<ScrollbarParts>(entity).copied() else {
            continue;
        };
        let Some(node) = world.get::<ComputedNode>(entity).copied() else {
            continue;
        };
        let (_, max) = scroll_geometry(world, entity, &node);
        if let Some(actual) = world.get::<TextScroll>(entity).map(|scroll| scroll.0) {
            let previous = world
                .get::<SmoothScrollTarget>(entity)
                .map_or(actual, |target| target.0);
            let mut next = previous;
            let physical_delta = delta / node.inverse_scale_factor.max(f32::EPSILON);
            if scrollable(parts.x_mode) && max.x > 0.0 {
                next.x = (previous.x + physical_delta.x).clamp(0.0, max.x);
                if next.x != previous.x {
                    delta.x -= (next.x - previous.x) * node.inverse_scale_factor;
                }
            }
            if scrollable(parts.y_mode) && max.y > 0.0 {
                next.y = (previous.y + physical_delta.y).clamp(0.0, max.y);
                if next.y != previous.y {
                    delta.y -= (next.y - previous.y) * node.inverse_scale_factor;
                }
            }
            if next != previous {
                if smooth && smooth_scroll_enabled(world) {
                    world.entity_mut(entity).insert(SmoothScrollTarget(next));
                } else {
                    world.entity_mut(entity).remove::<SmoothScrollTarget>();
                    if let Some(mut scroll) = world.get_mut::<TextScroll>(entity) {
                        scroll.0 = next;
                    }
                    world.entity_mut(entity).insert(UserTextScroll(next));
                }
            }
        } else if let Some(actual) = world.get::<ScrollPosition>(entity).map(|scroll| scroll.0) {
            let previous = world
                .get::<SmoothScrollTarget>(entity)
                .map_or(actual, |target| target.0);
            let mut next = previous;
            let logical_max = max * node.inverse_scale_factor;
            if scrollable(parts.x_mode) && logical_max.x > 0.0 {
                next.x = (previous.x + delta.x).clamp(0.0, logical_max.x);
                if next.x != previous.x {
                    delta.x -= next.x - previous.x;
                }
            }
            if scrollable(parts.y_mode) && logical_max.y > 0.0 {
                next.y = (previous.y + delta.y).clamp(0.0, logical_max.y);
                if next.y != previous.y {
                    delta.y -= next.y - previous.y;
                }
            }
            if next != previous {
                if smooth && smooth_scroll_enabled(world) {
                    world.entity_mut(entity).insert(SmoothScrollTarget(next));
                } else {
                    world.entity_mut(entity).remove::<SmoothScrollTarget>();
                    if let Some(mut scroll) = world.get_mut::<ScrollPosition>(entity) {
                        scroll.0 = next;
                    }
                }
            }
        }
        if delta == Vec2::ZERO {
            break;
        }
    }
}

fn smooth_scroll_enabled(world: &World) -> bool {
    world
        .get_resource::<crate::widgets::state::UiMotionSettings>()
        .is_some_and(|settings| settings.wheel_seconds > 0.0)
}

fn animate_wheel_scroll(world: &mut World) {
    let duration = world
        .get_resource::<crate::widgets::state::UiMotionSettings>()
        .map_or(0.0, |settings| settings.wheel_seconds);
    let delta = world
        .get_resource::<bevy::time::Time>()
        .map_or(1.0 / 60.0, |time| time.delta_secs());
    let active = {
        let mut query = world.query::<(Entity, &SmoothScrollTarget)>();
        query
            .iter(world)
            .map(|(entity, target)| (entity, target.0))
            .collect::<Vec<_>>()
    };
    let amount = if duration <= 0.0 {
        1.0
    } else {
        1.0 - (-delta / duration).exp()
    };
    for (entity, target) in active {
        let Some(node) = world.get::<ComputedNode>(entity) else {
            world.entity_mut(entity).remove::<SmoothScrollTarget>();
            continue;
        };
        let (_, max) = scroll_geometry(world, entity, node);
        let text = world.get::<TextScroll>(entity).is_some();
        let max = if text {
            max
        } else {
            max * node.inverse_scale_factor
        };
        let target = target.clamp(Vec2::ZERO, max);
        let actual = if text {
            world.get::<TextScroll>(entity).map(|scroll| scroll.0)
        } else {
            world.get::<ScrollPosition>(entity).map(|scroll| scroll.0)
        };
        let Some(actual) = actual else {
            world.entity_mut(entity).remove::<SmoothScrollTarget>();
            continue;
        };
        let next = if (target - actual).length_squared() < 0.25 {
            target
        } else {
            actual.lerp(target, amount)
        };
        if text {
            if let Some(mut scroll) = world.get_mut::<TextScroll>(entity) {
                scroll.0 = next;
            }
            world.entity_mut(entity).insert(UserTextScroll(next));
        } else if let Some(mut scroll) = world.get_mut::<ScrollPosition>(entity) {
            scroll.0 = next;
        }
        if next == target {
            world.entity_mut(entity).remove::<SmoothScrollTarget>();
        }
    }
}

fn scroll_geometry(world: &World, entity: Entity, node: &ComputedNode) -> (Vec2, Vec2) {
    if world.get::<TextScroll>(entity).is_some() {
        let view = node.content_box().size();
        (
            view,
            text_scroll_max(node, world.get::<TextLayoutInfo>(entity)),
        )
    } else {
        let view = node.size();
        (
            view,
            (node.content_size() - view + node.scrollbar_size).max(Vec2::ZERO),
        )
    }
}

fn scrollbar_pointer_input(
    mut presses: Option<MessageReader<Pointer<Press>>>,
    mut drags: Option<MessageReader<Pointer<Drag>>>,
    mut ends: Option<MessageReader<Pointer<DragEnd>>>,
    mut releases: Option<MessageReader<Pointer<Release>>>,
    mut commands: Commands,
) {
    if let Some(presses) = presses.as_mut() {
        for press in presses
            .read()
            .filter(|event| event.button == PointerButton::Primary)
        {
            let target = press.entity;
            let pointer = press.pointer_id;
            let position = press.pointer_location.position;
            commands.queue(move |world: &mut World| {
                let Some((owner, kind, vertical)) = scrollbar_hit(world, target, position) else {
                    return;
                };
                let Some(parts) = world.get::<ScrollbarParts>(owner).copied() else {
                    return;
                };
                let Some((track, _)) = (if vertical { parts.y } else { parts.x }) else {
                    return;
                };
                let Some(fraction) = pointer_fraction(world, track, vertical, position) else {
                    return;
                };
                let Some(travel) = scrollbar_travel(world, owner, vertical) else {
                    return;
                };
                let current = scroll_fraction(world, owner, vertical).unwrap_or(0.0);
                let grab_offset = if matches!(
                    kind,
                    ControlPartKind::ScrollbarYThumb | ControlPartKind::ScrollbarXThumb
                ) {
                    fraction - current * travel
                } else {
                    (1.0 - travel) * 0.5
                };
                world.entity_mut(owner).insert(ActiveScrollbarDrag {
                    pointer,
                    vertical,
                    grab_offset,
                });
                set_scroll_fraction(
                    world,
                    owner,
                    vertical,
                    ((fraction - grab_offset) / travel).clamp(0.0, 1.0),
                );
            });
        }
    }
    if let Some(drags) = drags.as_mut() {
        for drag in drags
            .read()
            .filter(|event| event.button == PointerButton::Primary)
        {
            let pointer = drag.pointer_id;
            let position = drag.pointer_location.position;
            commands.queue(move |world: &mut World| {
                let active_owner = {
                    let mut query = world.query::<(Entity, &ActiveScrollbarDrag)>();
                    query
                        .iter(world)
                        .find(|(_, active)| active.pointer == pointer)
                        .map(|(entity, active)| (entity, *active))
                };
                let Some((owner, active)) = active_owner else {
                    return;
                };
                let Some(parts) = world.get::<ScrollbarParts>(owner).copied() else {
                    return;
                };
                let Some((track, _)) = (if active.vertical { parts.y } else { parts.x }) else {
                    return;
                };
                let Some(fraction) = pointer_fraction(world, track, active.vertical, position)
                else {
                    return;
                };
                let Some(travel) = scrollbar_travel(world, owner, active.vertical) else {
                    return;
                };
                set_scroll_fraction(
                    world,
                    owner,
                    active.vertical,
                    ((fraction - active.grab_offset) / travel).clamp(0.0, 1.0),
                );
            });
        }
    }
    if let Some(ends) = ends.as_mut() {
        for end in ends
            .read()
            .filter(|event| event.button == PointerButton::Primary)
        {
            queue_drag_end(&mut commands, end.pointer_id);
        }
    }
    if let Some(releases) = releases.as_mut() {
        for release in releases
            .read()
            .filter(|event| event.button == PointerButton::Primary)
        {
            queue_drag_end(&mut commands, release.pointer_id);
        }
    }
}

fn queue_drag_end(commands: &mut Commands, pointer: PointerId) {
    commands.queue(move |world: &mut World| {
        let active = {
            let mut query = world.query::<(Entity, &ActiveScrollbarDrag)>();
            query
                .iter(world)
                .filter(|(_, drag)| drag.pointer == pointer)
                .map(|(entity, _)| entity)
                .collect::<Vec<_>>()
        };
        for entity in active {
            world.entity_mut(entity).remove::<ActiveScrollbarDrag>();
        }
    });
}

fn axis_for_part(kind: ControlPartKind) -> Option<bool> {
    match kind {
        ControlPartKind::ScrollbarYTrack | ControlPartKind::ScrollbarYThumb => Some(true),
        ControlPartKind::ScrollbarXTrack | ControlPartKind::ScrollbarXThumb => Some(false),
        _ => None,
    }
}

fn scrollbar_hit(
    world: &World,
    target: Entity,
    position: Vec2,
) -> Option<(Entity, ControlPartKind, bool)> {
    if let Some(part) = world.get::<ControlPart>(target).copied()
        && let Some(vertical) = axis_for_part(part.kind)
    {
        return Some((part.owner, part.kind, vertical));
    }
    if world
        .get::<ControlPart>(target)
        .is_some_and(|part| part.kind.handles_own_pointer())
    {
        return None;
    }
    let mut current = Some(target);
    while let Some(entity) = current {
        current = world.get::<ChildOf>(entity).map(ChildOf::parent);
        let Some(parts) = world.get::<ScrollbarParts>(entity).copied() else {
            continue;
        };
        for (axis, pair) in [(true, parts.y), (false, parts.x)] {
            let Some((track, thumb)) = pair else { continue };
            if world.get::<Visibility>(track) != Some(&Visibility::Visible) {
                continue;
            }
            let hit = |part| scrollbar_contains(world, part, position);
            if hit(thumb) {
                return Some((
                    entity,
                    if axis {
                        ControlPartKind::ScrollbarYThumb
                    } else {
                        ControlPartKind::ScrollbarXThumb
                    },
                    axis,
                ));
            }
            if hit(track) {
                return Some((
                    entity,
                    if axis {
                        ControlPartKind::ScrollbarYTrack
                    } else {
                        ControlPartKind::ScrollbarXTrack
                    },
                    axis,
                ));
            }
        }
    }
    None
}

fn scrollbar_contains(world: &World, entity: Entity, position: Vec2) -> bool {
    let Some(node) = world.get::<ComputedNode>(entity) else {
        return false;
    };
    let Some(transform) = world.get::<UiGlobalTransform>(entity) else {
        return false;
    };
    let Some(target) = world.get::<ComputedUiRenderTargetInfo>(entity) else {
        return false;
    };
    let scale = world.get_resource::<UiScale>().map_or(1.0, |scale| scale.0);
    node.normalize_point(*transform, position * target.scale_factor() / scale)
        .is_some_and(|point| point.x.abs() <= 0.5 && point.y.abs() <= 0.5)
}

fn pointer_fraction(world: &World, track: Entity, vertical: bool, position: Vec2) -> Option<f32> {
    let node = world.get::<ComputedNode>(track)?;
    let transform = *world.get::<UiGlobalTransform>(track)?;
    let target = world.get::<ComputedUiRenderTargetInfo>(track)?;
    let scale = world.get_resource::<UiScale>().map_or(1.0, |scale| scale.0);
    let point = node.normalize_point(transform, position * target.scale_factor() / scale)?;
    Some(
        (if vertical {
            point.y + 0.5
        } else {
            point.x + 0.5
        })
        .clamp(0.0, 1.0),
    )
}

fn scroll_fraction(world: &World, entity: Entity, vertical: bool) -> Option<f32> {
    let node = world.get::<ComputedNode>(entity)?;
    let (_, max) = scroll_geometry(world, entity, node);
    let current = world
        .get::<TextScroll>(entity)
        .map(|scroll| scroll.0)
        .or_else(|| {
            world
                .get::<ScrollPosition>(entity)
                .map(|scroll| scroll.0 / node.inverse_scale_factor.max(f32::EPSILON))
        })?;
    let (current, max) = if vertical {
        (current.y, max.y)
    } else {
        (current.x, max.x)
    };
    Some(if max > 0.0 { current / max } else { 0.0 })
}

fn scrollbar_travel(world: &World, entity: Entity, vertical: bool) -> Option<f32> {
    let node = world.get::<ComputedNode>(entity)?;
    let (view, max) = scroll_geometry(world, entity, node);
    let content = view + max;
    let (view, content) = if vertical {
        (view.y, content.y)
    } else {
        (view.x, content.x)
    };
    if !view.is_finite() || !content.is_finite() || content <= view {
        return None;
    }
    Some(1.0 - (view / content).clamp(0.08, 1.0))
}

fn set_scroll_fraction(world: &mut World, entity: Entity, vertical: bool, fraction: f32) {
    world.entity_mut(entity).remove::<SmoothScrollTarget>();
    let Some(node) = world.get::<ComputedNode>(entity).copied() else {
        return;
    };
    let (_, max) = scroll_geometry(world, entity, &node);
    if let Some(mut text_scroll) = world.get_mut::<TextScroll>(entity) {
        if vertical {
            text_scroll.0.y = max.y * fraction;
        } else {
            text_scroll.0.x = max.x * fraction;
        }
        let requested = text_scroll.0;
        world.entity_mut(entity).insert(UserTextScroll(requested));
    } else if let Some(mut scroll) = world.get_mut::<ScrollPosition>(entity) {
        let logical_max = max * node.inverse_scale_factor;
        if vertical {
            scroll.0.y = logical_max.y * fraction;
        } else {
            scroll.0.x = logical_max.x * fraction;
        }
    }
}

fn update_scrollbar_visuals(world: &mut World) {
    let owners = {
        let mut query = world.query_filtered::<Entity, With<ScrollbarParts>>();
        query.iter(world).collect::<Vec<_>>()
    };
    for owner in owners {
        let Some(parts) = world.get::<ScrollbarParts>(owner).copied() else {
            continue;
        };
        let Some(node) = world.get::<ComputedNode>(owner).copied() else {
            continue;
        };
        let (view, max) = scroll_geometry(world, owner, &node);
        let content = view + max;
        if let Some((track, thumb)) = parts.y {
            update_axis(
                world,
                owner,
                (track, thumb),
                parts.y_mode,
                AxisVisual {
                    view: view.y,
                    content: content.y,
                    vertical: true,
                },
            );
        }
        if let Some((track, thumb)) = parts.x {
            update_axis(
                world,
                owner,
                (track, thumb),
                parts.x_mode,
                AxisVisual {
                    view: view.x,
                    content: content.x,
                    vertical: false,
                },
            );
        }
    }
}

struct AxisVisual {
    view: f32,
    content: f32,
    vertical: bool,
}

fn update_axis(
    world: &mut World,
    owner: Entity,
    (track, thumb): (Entity, Entity),
    mode: Option<CssOverflow>,
    AxisVisual {
        view,
        content,
        vertical,
    }: AxisVisual,
) {
    let show = matches!(mode, Some(CssOverflow::Scroll))
        || matches!(mode, Some(CssOverflow::Auto)) && content > view + 0.5;
    let visibility = if show {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if world.get::<Visibility>(track) != Some(&visibility) {
        world.entity_mut(track).insert(visibility);
    }
    if !show || !view.is_finite() || !content.is_finite() || content <= 0.0 {
        return;
    }
    let size = (view / content).clamp(0.08, 1.0) * 100.0;
    let offset = scroll_fraction(world, owner, vertical)
        .unwrap_or(0.0)
        .clamp(0.0, 1.0)
        * (100.0 - size);
    let next = if vertical {
        WidgetLayoutOverride {
            height: Some(Val::Percent(size)),
            top: Some(Val::Percent(offset)),
            ..Default::default()
        }
    } else {
        WidgetLayoutOverride {
            width: Some(Val::Percent(size)),
            left: Some(Val::Percent(offset)),
            ..Default::default()
        }
    };
    let same = world
        .get::<WidgetLayoutOverride>(thumb)
        .is_some_and(|current| {
            current.width == next.width
                && current.height == next.height
                && current.left == next.left
                && current.top == next.top
        });
    if !same {
        if let Some(mut node) = world.get_mut::<Node>(thumb) {
            next.apply(&mut node);
        }
        world.entity_mut(thumb).insert(next);
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        app::{App, PostUpdate, Update},
        camera::NormalizedRenderTarget,
        ecs::{change_detection::DetectChangesMut, system::Query, world::World},
        input::{mouse::MouseScrollUnit, touch::TouchPhase},
        math::Vec2,
        prelude::{InheritedVisibility, IntoScheduleConfigs, Visibility},
        sprite::BorderRect,
        text::{EditableText as NativeEditableText, TextLayoutInfo},
        ui::{
            ComputedNode, ComputedUiRenderTargetInfo, Node, ScrollPosition, UiGlobalTransform,
            UiStack, widget::TextScroll,
        },
    };
    use bevy_picking::{
        backend::HitData,
        events::{Pointer, Press, Scroll},
        hover::HoverMap,
        pointer::{Location, PointerButton, PointerId},
    };
    use tilt_ui_core::{ElementKind, InputType};
    use tilt_ui_css::{ComputedStyle, CssOverflow};

    use crate::{
        ComponentStyleOwner, ControlPart, EditableText, TiltControl, TiltElement,
        TiltUiControlRuntimePlugin, style::CascadedStyle,
    };

    use super::{
        ActiveScrollbarDrag, ScrollbarParts, SmoothScrollTarget, TiltUiScrollRuntimePlugin,
        UserTextScroll, animate_wheel_scroll, ensure_scrollbar_parts, frontmost_scroll_target,
        restore_user_text_scroll, scroll_ancestors, scroll_ancestors_with_motion, scrollbar_hit,
        set_scroll_fraction, update_scrollbar_visuals, wheel_scroll, wheel_unit_scale,
    };
    use crate::widgets::state::UiMotionSettings;

    #[test]
    fn wheel_lines_cover_more_distance_without_changing_pixel_scrolling() {
        assert_eq!(wheel_unit_scale(MouseScrollUnit::Line), 56.0);
        assert_eq!(wheel_unit_scale(MouseScrollUnit::Pixel), 1.0);
    }

    #[test]
    fn wheel_motion_eases_to_target_and_thumb_drag_cancels_it() {
        let mut world = World::new();
        world.init_resource::<UiMotionSettings>();
        let owner = world
            .spawn((
                ScrollPosition::default(),
                ScrollbarParts {
                    y_mode: Some(CssOverflow::Auto),
                    ..Default::default()
                },
                ComputedNode {
                    size: Vec2::new(100.0, 50.0),
                    content_size: Vec2::new(100.0, 150.0),
                    inverse_scale_factor: 1.0,
                    ..Default::default()
                },
            ))
            .id();
        scroll_ancestors_with_motion(&mut world, owner, Vec2::new(0.0, 60.0), true);
        assert_eq!(world.get::<ScrollPosition>(owner).unwrap().0.y, 0.0);
        animate_wheel_scroll(&mut world);
        let first = world.get::<ScrollPosition>(owner).unwrap().0.y;
        assert!(first > 0.0 && first < 60.0);
        set_scroll_fraction(&mut world, owner, true, 0.75);
        assert!(world.get::<SmoothScrollTarget>(owner).is_none());
        assert_eq!(world.get::<ScrollPosition>(owner).unwrap().0.y, 75.0);
    }

    fn native_cursor_recenter(mut editors: Query<&mut TextScroll>) {
        for mut scroll in &mut editors {
            scroll.0 = Vec2::ZERO;
        }
    }

    #[test]
    fn textarea_wheel_and_thumb_survive_native_cursor_recentering() {
        let mut app = App::new();
        app.add_systems(
            PostUpdate,
            (native_cursor_recenter, restore_user_text_scroll).chain(),
        );
        let area = app
            .world_mut()
            .spawn((
                TextScroll::default(),
                TextLayoutInfo {
                    size: Vec2::new(100.0, 150.0),
                    ..Default::default()
                },
                ComputedNode {
                    size: Vec2::new(100.0, 50.0),
                    inverse_scale_factor: 1.0,
                    ..Default::default()
                },
                ScrollbarParts {
                    y_mode: Some(CssOverflow::Auto),
                    ..Default::default()
                },
            ))
            .id();

        scroll_ancestors(app.world_mut(), area, Vec2::new(0.0, 35.0));
        app.update();
        assert_eq!(app.world().get::<TextScroll>(area).unwrap().0.y, 35.0);
        assert_eq!(app.world().get::<UserTextScroll>(area).unwrap().0.y, 35.0);

        set_scroll_fraction(app.world_mut(), area, true, 0.75);
        app.update();
        assert_eq!(app.world().get::<TextScroll>(area).unwrap().0.y, 75.0);

        app.world_mut().entity_mut(area).remove::<UserTextScroll>();
        app.update();
        assert_eq!(app.world().get::<TextScroll>(area).unwrap().0.y, 0.0);
    }

    #[test]
    fn overflow_parts_are_persistent_and_wheel_scrolls_the_nearest_container() {
        let mut world = World::new();
        let scope = world.spawn_empty().id();
        let outer = world
            .spawn((
                TiltElement {
                    kind: ElementKind::Div,
                },
                ComponentStyleOwner(scope),
                CascadedStyle(ComputedStyle {
                    overflow_y: Some(CssOverflow::Auto),
                    ..Default::default()
                }),
                Node::default(),
                ComputedNode {
                    size: Vec2::new(200.0, 100.0),
                    content_size: Vec2::new(200.0, 400.0),
                    inverse_scale_factor: 1.0,
                    ..Default::default()
                },
            ))
            .id();
        let inner = world
            .spawn((
                TiltElement {
                    kind: ElementKind::Div,
                },
                ComponentStyleOwner(scope),
                CascadedStyle(ComputedStyle {
                    overflow_y: Some(CssOverflow::Auto),
                    ..Default::default()
                }),
                Node::default(),
                ComputedNode {
                    size: Vec2::new(100.0, 50.0),
                    content_size: Vec2::new(100.0, 150.0),
                    inverse_scale_factor: 1.0,
                    ..Default::default()
                },
            ))
            .id();
        let content = world.spawn_empty().id();
        world.entity_mut(outer).add_child(inner);
        world.entity_mut(inner).add_child(content);
        assert!(ensure_scrollbar_parts(&mut world, scope));
        let inner_parts = world.get::<ScrollbarParts>(inner).copied().unwrap();
        assert_eq!(
            world
                .get::<ControlPart>(inner_parts.y.unwrap().1)
                .unwrap()
                .owner,
            inner
        );
        assert!(!ensure_scrollbar_parts(&mut world, scope));
        assert_eq!(world.get::<ScrollbarParts>(inner).unwrap().y, inner_parts.y);

        scroll_ancestors(&mut world, content, Vec2::new(0.0, 60.0));
        assert_eq!(world.get::<ScrollPosition>(inner).unwrap().0.y, 60.0);
        assert_eq!(world.get::<ScrollPosition>(outer).unwrap().0.y, 0.0);
        scroll_ancestors(&mut world, content, Vec2::new(0.0, 60.0));
        assert_eq!(world.get::<ScrollPosition>(inner).unwrap().0.y, 100.0);
        assert_eq!(world.get::<ScrollPosition>(outer).unwrap().0.y, 20.0);
        update_scrollbar_visuals(&mut world);
        assert_eq!(
            world.get::<Visibility>(inner_parts.y.unwrap().0),
            Some(&Visibility::Visible)
        );
        let thumb = world
            .get::<crate::WidgetLayoutOverride>(inner_parts.y.unwrap().1)
            .unwrap();
        assert!(
            matches!(thumb.height, Some(bevy::ui::Val::Percent(value)) if (value - 100.0 / 3.0).abs() < 0.01)
        );
        assert!(
            matches!(thumb.top, Some(bevy::ui::Val::Percent(value)) if (value - 200.0 / 3.0).abs() < 0.01)
        );
    }

    #[test]
    fn popup_parts_can_own_scrollbars() {
        let mut world = World::new();
        let scope = world.spawn_empty().id();
        let choice = world.spawn_empty().id();
        let popup = world
            .spawn((
                ControlPart {
                    owner: choice,
                    kind: crate::ControlPartKind::Popup,
                },
                ComponentStyleOwner(scope),
                CascadedStyle(ComputedStyle {
                    overflow_y: Some(CssOverflow::Auto),
                    ..Default::default()
                }),
                Node::default(),
                ComputedNode {
                    size: Vec2::new(180.0, 98.0),
                    content_size: Vec2::new(180.0, 130.0),
                    inverse_scale_factor: 1.0,
                    ..Default::default()
                },
            ))
            .id();
        world.entity_mut(choice).add_child(popup);

        assert!(ensure_scrollbar_parts(&mut world, scope));
        assert!(world.get::<ScrollbarParts>(popup).unwrap().y.is_some());
        scroll_ancestors(&mut world, popup, Vec2::new(0.0, 20.0));
        assert_eq!(world.get::<ScrollPosition>(popup).unwrap().0.y, 20.0);
    }

    #[test]
    fn auto_scrollbar_hides_without_overflow() {
        let mut world = World::new();
        let scope = world.spawn_empty().id();
        let element = world
            .spawn((
                TiltElement {
                    kind: ElementKind::Div,
                },
                ComponentStyleOwner(scope),
                CascadedStyle(ComputedStyle {
                    overflow_y: Some(CssOverflow::Auto),
                    ..Default::default()
                }),
                Node::default(),
                ComputedNode {
                    size: Vec2::new(100.0, 100.0),
                    content_size: Vec2::new(100.0, 80.0),
                    ..Default::default()
                },
            ))
            .id();
        ensure_scrollbar_parts(&mut world, scope);
        update_scrollbar_visuals(&mut world);
        let track = world.get::<ScrollbarParts>(element).unwrap().y.unwrap().0;
        assert_eq!(world.get::<Visibility>(track), Some(&Visibility::Hidden));
    }

    #[test]
    fn padding_alone_cannot_move_a_bevy_scroll_position() {
        let mut world = World::new();
        let scope = world.spawn_empty().id();
        let owner = world
            .spawn((
                TiltElement {
                    kind: ElementKind::Body,
                },
                ComponentStyleOwner(scope),
                CascadedStyle(ComputedStyle {
                    overflow_y: Some(CssOverflow::Auto),
                    ..Default::default()
                }),
                Node::default(),
                ComputedNode {
                    size: Vec2::splat(100.0),
                    content_size: Vec2::splat(100.0),
                    padding: BorderRect::all(10.0),
                    inverse_scale_factor: 1.0,
                    ..Default::default()
                },
            ))
            .id();
        ensure_scrollbar_parts(&mut world, scope);
        scroll_ancestors(&mut world, owner, Vec2::new(0.0, 40.0));
        assert_eq!(world.get::<ScrollPosition>(owner).unwrap().0.y, 0.0);
        update_scrollbar_visuals(&mut world);
        let track = world.get::<ScrollbarParts>(owner).unwrap().y.unwrap().0;
        assert_eq!(world.get::<Visibility>(track), Some(&Visibility::Hidden));
    }

    #[test]
    fn scrollbar_reacts_to_layout_growth_without_computed_node_change_ticks() {
        let mut world = World::new();
        let scope = world.spawn_empty().id();
        let owner = world
            .spawn((
                TiltElement {
                    kind: ElementKind::Div,
                },
                ComponentStyleOwner(scope),
                CascadedStyle(ComputedStyle {
                    overflow_y: Some(CssOverflow::Auto),
                    ..Default::default()
                }),
                Node::default(),
                ComputedNode {
                    size: Vec2::splat(100.0),
                    content_size: Vec2::splat(80.0),
                    ..Default::default()
                },
            ))
            .id();
        ensure_scrollbar_parts(&mut world, scope);
        let track = world.get::<ScrollbarParts>(owner).unwrap().y.unwrap().0;
        update_scrollbar_visuals(&mut world);
        assert_eq!(world.get::<Visibility>(track), Some(&Visibility::Hidden));

        world
            .get_mut::<ComputedNode>(owner)
            .unwrap()
            .bypass_change_detection()
            .content_size = Vec2::splat(200.0);
        update_scrollbar_visuals(&mut world);
        assert_eq!(world.get::<Visibility>(track), Some(&Visibility::Visible));
    }

    #[test]
    fn wheel_prefers_the_inner_scroll_box_when_body_is_the_picking_target() {
        let mut world = World::new();
        let scope = world.spawn_empty().id();
        let spawn = |world: &mut World, kind, size, content| {
            world
                .spawn((
                    TiltElement { kind },
                    ComponentStyleOwner(scope),
                    CascadedStyle(ComputedStyle {
                        overflow_y: Some(CssOverflow::Auto),
                        ..Default::default()
                    }),
                    Node::default(),
                    ComputedNode {
                        size: Vec2::splat(size),
                        content_size: Vec2::splat(content),
                        inverse_scale_factor: 1.0,
                        ..Default::default()
                    },
                    UiGlobalTransform::default(),
                    ComputedUiRenderTargetInfo::default(),
                    InheritedVisibility::VISIBLE,
                ))
                .id()
        };
        let body = spawn(&mut world, ElementKind::Body, 200.0, 400.0);
        let inner = spawn(&mut world, ElementKind::Div, 80.0, 180.0);
        world.entity_mut(body).add_child(inner);
        ensure_scrollbar_parts(&mut world, scope);
        world.insert_resource(UiStack {
            partition: Vec::new(),
            uinodes: vec![body, inner],
        });
        let target = frontmost_scroll_target(&world, body, Vec2::ZERO, body).unwrap();
        assert_eq!(target, inner);
        scroll_ancestors(&mut world, target, Vec2::new(0.0, 20.0));
        assert_eq!(world.get::<ScrollPosition>(inner).unwrap().0.y, 20.0);
        assert_eq!(world.get::<ScrollPosition>(body).unwrap().0.y, 0.0);
    }

    #[test]
    fn scrollbar_gutter_resolves_to_track_even_when_the_owner_was_picked() {
        let mut world = World::new();
        let owner = world.spawn_empty().id();
        let track = world
            .spawn((
                ComputedNode {
                    size: Vec2::new(12.0, 100.0),
                    ..Default::default()
                },
                ComputedUiRenderTargetInfo::default(),
                UiGlobalTransform::default(),
                Visibility::Visible,
            ))
            .id();
        let thumb = world.spawn_empty().id();
        world.entity_mut(owner).add_child(track);
        world.entity_mut(track).add_child(thumb);
        world.entity_mut(owner).insert(ScrollbarParts {
            y: Some((track, thumb)),
            ..Default::default()
        });

        assert_eq!(
            scrollbar_hit(&world, owner, Vec2::new(4.0, 20.0)),
            Some((owner, crate::ControlPartKind::ScrollbarYTrack, true))
        );
        assert_eq!(scrollbar_hit(&world, owner, Vec2::new(20.0, 20.0)), None);

        let resize_handle = world
            .spawn(crate::ControlPart {
                owner,
                kind: crate::ControlPartKind::ResizeHandle,
            })
            .id();
        world.entity_mut(owner).add_child(resize_handle);
        assert_eq!(
            scrollbar_hit(&world, resize_handle, Vec2::new(4.0, 20.0)),
            None
        );
    }

    #[test]
    fn picked_wheel_event_scrolls_only_the_targeted_container() {
        let mut app = App::new();
        app.add_message::<Pointer<Scroll>>()
            .insert_resource(HoverMap::default())
            .add_systems(Update, wheel_scroll);
        let scope = app.world_mut().spawn_empty().id();
        let owner = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Div,
                },
                ComponentStyleOwner(scope),
                CascadedStyle(ComputedStyle {
                    overflow_y: Some(CssOverflow::Auto),
                    ..Default::default()
                }),
                Node::default(),
                ComputedNode {
                    size: Vec2::new(100.0, 50.0),
                    content_size: Vec2::new(100.0, 150.0),
                    inverse_scale_factor: 1.0,
                    ..Default::default()
                },
            ))
            .id();
        let child = app.world_mut().spawn_empty().id();
        app.world_mut().entity_mut(owner).add_child(child);
        ensure_scrollbar_parts(app.world_mut(), scope);
        let hit = HitData::new(owner, 0.0, None, None);
        app.world_mut()
            .resource_mut::<HoverMap>()
            .entry(PointerId::Mouse)
            .or_default()
            .insert(child, hit.clone());
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<Pointer<Scroll>>>()
            .write(Pointer::new(
                PointerId::Mouse,
                Location {
                    target: NormalizedRenderTarget::None {
                        width: 100,
                        height: 100,
                    },
                    position: Vec2::ZERO,
                },
                Scroll {
                    unit: MouseScrollUnit::Pixel,
                    x: 0.0,
                    y: -24.0,
                    hit,
                    phase: TouchPhase::Moved,
                },
                child,
            ));
        app.update();
        assert_eq!(app.world().get::<ScrollPosition>(owner).unwrap().0.y, 24.0);
    }

    #[test]
    fn scrollbar_press_captures_pointer_before_editor_selection() {
        let mut app = App::new();
        app.add_plugins((TiltUiControlRuntimePlugin, TiltUiScrollRuntimePlugin))
            .add_message::<Pointer<Press>>();
        let owner = app
            .world_mut()
            .spawn((
                Node::default(),
                TiltControl,
                EditableText::new("text".into(), InputType::Text, false, true),
                NativeEditableText::default(),
                TextLayoutInfo {
                    size: Vec2::new(120.0, 150.0),
                    ..Default::default()
                },
                TextScroll::default(),
                ComputedNode {
                    size: Vec2::new(120.0, 50.0),
                    content_size: Vec2::new(120.0, 150.0),
                    inverse_scale_factor: 1.0,
                    ..Default::default()
                },
                ComputedUiRenderTargetInfo::default(),
                UiGlobalTransform::default(),
            ))
            .id();
        let track = app
            .world_mut()
            .spawn((
                ControlPart {
                    owner,
                    kind: crate::ControlPartKind::ScrollbarYTrack,
                },
                ComputedNode {
                    size: Vec2::new(10.0, 50.0),
                    ..Default::default()
                },
                ComputedUiRenderTargetInfo::default(),
                UiGlobalTransform::default(),
                Visibility::Visible,
            ))
            .id();
        let thumb = app.world_mut().spawn_empty().id();
        app.world_mut().entity_mut(owner).add_child(track);
        app.world_mut().entity_mut(track).add_child(thumb);
        app.world_mut().entity_mut(owner).insert(ScrollbarParts {
            y: Some((track, thumb)),
            y_mode: Some(CssOverflow::Auto),
            ..Default::default()
        });
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<Pointer<Press>>>()
            .write(Pointer::new(
                PointerId::Mouse,
                Location {
                    target: NormalizedRenderTarget::None {
                        width: 120,
                        height: 50,
                    },
                    position: Vec2::ZERO,
                },
                Press {
                    button: PointerButton::Primary,
                    hit: HitData::new(owner, 0.0, None, None),
                    count: 1,
                },
                owner,
            ));
        app.update();
        assert!(app.world().get::<ActiveScrollbarDrag>(owner).is_some());
        assert!(
            app.world()
                .get::<NativeEditableText>(owner)
                .unwrap()
                .pending_edits
                .is_empty()
        );
    }
}
