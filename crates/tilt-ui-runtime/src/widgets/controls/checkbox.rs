//! Runtime semantics specific to the TiltUI checkbox element.

use bevy::{
    ecs::{component::Component, entity::Entity, world::World},
    ui::widget::Text,
};

use crate::ControlPartKind;

use super::spawn_part;

/// Identifies a materialized checkbox control.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TiltCheckbox;

pub(crate) fn materialize_parts(world: &mut World, owner: Entity) {
    let indicator = spawn_part(world, owner, ControlPartKind::Indicator);
    let mark = spawn_part(world, owner, ControlPartKind::Mark);
    world.entity_mut(mark).insert(Text::new("✓"));
    world.entity_mut(indicator).add_child(mark);
    world.entity_mut(owner).add_child(indicator);
}
