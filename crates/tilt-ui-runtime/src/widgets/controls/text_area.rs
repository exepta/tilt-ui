//! Multiline editing and persistent resize anatomy for TextArea.

use bevy::ecs::{entity::Entity, world::World};
use bevy_picking::Pickable;
use tilt_ui_core::TemplateAttribute;

use crate::ControlPartKind;

use super::spawn_text_part;

pub(crate) fn materialize_parts(
    world: &mut World,
    owner: Entity,
    attributes: &[TemplateAttribute],
    multiline: bool,
) {
    super::input::materialize_parts(world, owner, attributes, multiline);
    let handle = spawn_text_part(world, owner, ControlPartKind::ResizeHandle, "///");
    world.entity_mut(handle).insert(Pickable::default());
    world.entity_mut(owner).add_child(handle);
}
