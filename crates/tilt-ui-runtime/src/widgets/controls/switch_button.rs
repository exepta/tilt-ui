//! Runtime semantics specific to the TiltUI switch-button element.

use bevy::ecs::{component::Component, entity::Entity, world::World};

use crate::ControlPartKind;

use super::spawn_part;

/// Identifies a materialized switch button control.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TiltSwitchButton;

pub(crate) fn materialize_parts(world: &mut World, owner: Entity) {
    let track = spawn_part(world, owner, ControlPartKind::Track);
    let thumb = spawn_part(world, owner, ControlPartKind::Thumb);
    world.entity_mut(track).add_child(thumb);
    world.entity_mut(owner).add_child(track);
}
