//! Runtime semantics specific to the TiltUI headline element.

use bevy::ecs::{component::Component, entity::Entity, world::World};
use tilt_ui_core::{HeadlineType, TemplateAttribute};

/// Preserves the semantic level of a materialized heading.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeadlineLevel(pub HeadlineType);

pub(crate) fn materialize(world: &mut World, entity: Entity, attributes: &[TemplateAttribute]) {
    let level = match crate::component::static_attribute_value(attributes, "level") {
        Some("2") => HeadlineType::H2,
        Some("3") => HeadlineType::H3,
        Some("4") => HeadlineType::H4,
        Some("5") => HeadlineType::H5,
        Some("6") => HeadlineType::H6,
        _ => HeadlineType::H1,
    };
    world.entity_mut(entity).insert(HeadlineLevel(level));
}
