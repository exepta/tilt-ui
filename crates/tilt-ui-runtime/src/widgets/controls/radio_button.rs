//! Runtime semantics specific to the TiltUI radio-button element.

use bevy::ecs::{component::Component, entity::Entity, world::World};

use crate::ControlPartKind;

use super::spawn_part;

/// Identifies a materialized radio button control.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TiltRadioButton;

pub(crate) fn materialize_parts(world: &mut World, owner: Entity) {
    let indicator = spawn_part(world, owner, ControlPartKind::Indicator);
    let mark = spawn_part(world, owner, ControlPartKind::Mark);
    world.entity_mut(indicator).add_child(mark);
    world.entity_mut(owner).add_child(indicator);
}
