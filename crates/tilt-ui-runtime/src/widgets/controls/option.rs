//! Semantic value and source label for one selectable option.

use bevy::{
    ecs::{component::Component, entity::Entity, world::World},
    ui::{Checkable, Checked},
};
use tilt_ui_core::TemplateAttribute;

use crate::{ControlChecked, ElementState, TiltText};

/// Finds selectable options through authored grouping containers in document order.
pub(crate) fn descendants(world: &World, root: Entity) -> Vec<Entity> {
    let mut result = Vec::new();
    let mut pending = vec![root];
    while let Some(entity) = pending.pop() {
        if entity != root && world.get::<OptionData>(entity).is_some() {
            result.push(entity);
            continue;
        }
        if let Some(children) = world.get::<bevy::ecs::hierarchy::Children>(entity) {
            pending.extend(children.iter().rev());
        }
    }
    result
}

/// Stores an option's value and authored display label.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct OptionData {
    /// Value emitted when the option is selected.
    pub value: String,
    /// Text derived from the option's template children.
    pub label: String,
}

pub(crate) fn materialize(world: &mut World, entity: Entity, attributes: &[TemplateAttribute]) {
    let selected = crate::component::has_boolean_static_attribute(attributes, "selected");
    let value = crate::component::static_attribute_value(attributes, "value")
        .unwrap_or_default()
        .to_owned();
    world.entity_mut(entity).insert((
        OptionData {
            value,
            label: String::new(),
        },
        ControlChecked(selected),
        Checkable,
    ));
    if selected {
        world.entity_mut(entity).insert(Checked);
    }
    if let Some(mut state) = world.get_mut::<ElementState>(entity) {
        state.checked = selected;
    }
}

pub(crate) fn finish(world: &mut World, entity: Entity) {
    let label = world
        .get::<bevy::ecs::hierarchy::Children>(entity)
        .map(|children| {
            children
                .iter()
                .copied()
                .filter_map(|child| world.get::<TiltText>(child))
                .map(|text| text.value.trim())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    if let Some(mut option) = world.get_mut::<OptionData>(entity) {
        option.label = if label.is_empty() {
            option.value.clone()
        } else {
            label
        };
        if option.value.is_empty() {
            option.value = option.label.clone();
        }
    }
}
