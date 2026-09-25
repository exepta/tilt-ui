//! Runtime semantics specific to the TiltUI divider element.

use bevy::ecs::{component::Component, entity::Entity, world::World};
use tilt_ui_core::{DividerAlignment, TemplateAttribute};

/// Preserves the semantic axis of a divider.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DividerAxis(pub DividerAlignment);

pub(crate) fn materialize(world: &mut World, entity: Entity, attributes: &[TemplateAttribute]) {
    let axis = match crate::component::static_attribute_value(attributes, "alignment") {
        Some("vertical") => DividerAlignment::Vertical,
        _ => DividerAlignment::Horizontal,
    };
    world.entity_mut(entity).insert(DividerAxis(axis));
}
