//! Pointer resize interaction for TextArea's persistent lower-right handle.

use bevy::{
    ecs::{message::MessageReader, system::Commands, world::World},
    math::Vec2,
    ui::{ComputedNode, ComputedUiRenderTargetInfo, InteractionDisabled, UiScale},
};
use bevy_picking::{
    events::{Drag, DragEnd, Pointer, Press, Release},
    pointer::PointerButton,
};

use crate::{ControlPart, ControlPartKind, set_text_area_size};

use super::drag::{ActiveControlDrag, DragKind};

/// Routes lower-right handle gestures through the shared active-drag state.
pub(crate) fn text_area_resize_input(
    mut presses: Option<MessageReader<Pointer<Press>>>,
    mut drags: Option<MessageReader<Pointer<Drag>>>,
    mut ends: Option<MessageReader<Pointer<DragEnd>>>,
    mut releases: Option<MessageReader<Pointer<Release>>>,
    mut commands: Commands,
) {
    if let Some(presses) = presses.as_mut() {
        for press in presses.read() {
            if press.button != PointerButton::Primary {
                continue;
            }
            let target = press.entity;
            let position = press.pointer_location.position;
            let pointer = press.pointer_id;
            commands.queue(move |world: &mut World| {
                let Some(part) = world
                    .get::<ControlPart>(target)
                    .copied()
                    .filter(|part| part.kind == ControlPartKind::ResizeHandle)
                else {
                    return;
                };
                if world.get::<InteractionDisabled>(part.owner).is_some() {
                    return;
                }
                let Some(node) = world.get::<ComputedNode>(part.owner) else {
                    return;
                };
                let Some(render_target) = world.get::<ComputedUiRenderTargetInfo>(part.owner)
                else {
                    return;
                };
                let ui_scale = world.get_resource::<UiScale>().map_or(1.0, |scale| scale.0);
                let size = node.size * ui_scale / render_target.scale_factor();
                world.entity_mut(part.owner).insert(ActiveControlDrag {
                    pointer,
                    kind: DragKind::TextAreaResize {
                        initial_size: size,
                        initial_pointer: position,
                    },
                });
            });
        }
    }
    if let Some(drags) = drags.as_mut() {
        for drag in drags.read() {
            if drag.button != PointerButton::Primary {
                continue;
            }
            let target = drag.entity;
            let position = drag.pointer_location.position;
            let pointer = drag.pointer_id;
            commands.queue(move |world: &mut World| {
                let Some(part) = world
                    .get::<ControlPart>(target)
                    .copied()
                    .filter(|part| part.kind == ControlPartKind::ResizeHandle)
                else {
                    return;
                };
                let Some(ActiveControlDrag {
                    kind:
                        DragKind::TextAreaResize {
                            initial_size,
                            initial_pointer,
                        },
                    ..
                }) = world
                    .get::<ActiveControlDrag>(part.owner)
                    .copied()
                    .filter(|drag| drag.pointer == pointer)
                else {
                    return;
                };
                let delta: Vec2 = position - initial_pointer;
                set_text_area_size(
                    world,
                    part.owner,
                    initial_size.x + delta.x,
                    initial_size.y + delta.y,
                );
            });
        }
    }
    if let Some(ends) = ends.as_mut() {
        for end in ends.read() {
            if end.button == PointerButton::Primary {
                queue_end(&mut commands, end.entity, end.pointer_id);
            }
        }
    }
    if let Some(releases) = releases.as_mut() {
        for release in releases.read() {
            if release.button == PointerButton::Primary {
                queue_end(&mut commands, release.entity, release.pointer_id);
            }
        }
    }
}

fn queue_end(
    commands: &mut Commands,
    target: bevy::ecs::entity::Entity,
    pointer: bevy_picking::pointer::PointerId,
) {
    commands.queue(move |world: &mut World| {
        let Some(part) = world
            .get::<ControlPart>(target)
            .copied()
            .filter(|part| part.kind == ControlPartKind::ResizeHandle)
        else {
            return;
        };
        if world
            .get::<ActiveControlDrag>(part.owner)
            .is_some_and(|drag| {
                drag.pointer == pointer && matches!(drag.kind, DragKind::TextAreaResize { .. })
            })
        {
            world.entity_mut(part.owner).remove::<ActiveControlDrag>();
        }
    });
}
