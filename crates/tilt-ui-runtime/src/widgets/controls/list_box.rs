//! Selection configuration for a visible list of authored options.

use bevy::ecs::{component::Component, entity::Entity, world::World};
use tilt_ui_core::TemplateAttribute;

use crate::ControlChecked;

/// Configures whether a ListBox permits more than one selected option.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListBoxMode {
    /// Whether repeated option activation toggles independent selections.
    pub multiple: bool,
}

pub(crate) fn materialize(world: &mut World, entity: Entity, attributes: &[TemplateAttribute]) {
    world.entity_mut(entity).insert(ListBoxMode {
        multiple: crate::component::has_boolean_static_attribute(attributes, "multiple"),
    });
}

pub(crate) fn finish(world: &mut World, entity: Entity) {
    if world
        .get::<ListBoxMode>(entity)
        .is_some_and(|mode| mode.multiple)
    {
        return;
    }
    let selected = super::option::descendants(world, entity)
        .into_iter()
        .filter(|child| {
            world
                .get::<ControlChecked>(*child)
                .is_some_and(|value| value.0)
        })
        .collect::<Vec<_>>();
    for extra in selected.into_iter().skip(1) {
        crate::set_control_checked(world, extra, false);
    }
}
