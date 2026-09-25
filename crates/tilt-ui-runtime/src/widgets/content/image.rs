//! Runtime semantics specific to the TiltUI image element.

use bevy::{
    asset::AssetServer,
    ecs::{component::Component, entity::Entity, world::World},
    image::Image,
    ui::widget::ImageNode,
};
use tilt_ui_core::TemplateAttribute;

/// Stores semantic image metadata retained independently of Bevy image loading.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct ImageMetadata {
    /// Source path declared by the template, when present.
    pub source: Option<String>,
    /// Alternative text declared by the template, when present.
    pub alt: Option<String>,
}

pub(crate) fn materialize_metadata(
    world: &mut World,
    entity: Entity,
    attributes: &[TemplateAttribute],
) {
    let mut metadata = ImageMetadata::default();
    for attribute in attributes {
        let TemplateAttribute::Static { name, value } = attribute else {
            continue;
        };
        match name.as_str() {
            "src" => metadata.source = Some(value.clone()),
            "alt" => metadata.alt = Some(value.clone()),
            _ => {}
        }
    }
    world.entity_mut(entity).insert(metadata);
}

/// Updates an image source on its existing entity without rebuilding children.
pub fn set_image_source(world: &mut World, entity: Entity, source: Option<String>) -> bool {
    let Some(metadata) = world.get::<ImageMetadata>(entity) else {
        return false;
    };
    if metadata.source == source {
        return false;
    }
    let image = source.as_ref().and_then(|source| {
        world
            .get_resource::<AssetServer>()
            .map(|server| server.load::<Image>(source.clone()))
    });
    world.get_mut::<ImageMetadata>(entity).unwrap().source = source;
    if let Some(mut node) = world.get_mut::<ImageNode>(entity) {
        node.image = image.unwrap_or_default();
    }
    true
}
