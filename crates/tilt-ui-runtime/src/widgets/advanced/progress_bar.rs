//! Runtime semantics specific to the TiltUI progress-bar element.

use bevy::ecs::{entity::Entity, world::World};
use tilt_ui_core::TemplateAttribute;

use crate::{
    ControlPartKind,
    widgets::{
        controls::slider::{numeric_range, orientation},
        state::{NumericParts, update_fill_width},
    },
};

use super::super::controls::spawn_part;

/// Sets a ProgressBar value without rebuilding its persistent parts.
pub fn set_progress_value(world: &mut World, entity: Entity, value: f32) -> bool {
    if world
        .get::<NumericParts>(entity)
        .is_none_or(|parts| parts.thumb.is_some())
    {
        return false;
    }
    crate::set_numeric_value(world, entity, value)
}

pub(crate) fn materialize_parts(
    world: &mut World,
    owner: Entity,
    attributes: &[TemplateAttribute],
) {
    let range = numeric_range(attributes);
    let track = spawn_part(world, owner, ControlPartKind::Track);
    let fill = spawn_part(world, owner, ControlPartKind::Fill);
    world.entity_mut(owner).add_child(track);
    world.entity_mut(track).add_child(fill);
    world.entity_mut(owner).insert((
        range,
        NumericParts {
            track,
            fill,
            thumb: None,
            second_thumb: None,
            tip: None,
            second_tip: None,
            orientation: orientation(attributes),
        },
    ));
    update_fill_width(world, owner, range.fraction());
}
