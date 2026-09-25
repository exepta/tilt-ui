//! Image-based avatar semantics with a persistent text fallback.

use bevy::{
    ecs::{component::Component, entity::Entity, world::World},
    prelude::Visibility,
};
use tilt_ui_core::TemplateAttribute;

use crate::ControlPartKind;

/// References the avatar's persistent fallback text part.
#[derive(Component, Debug, Clone, Copy)]
pub struct AvatarFallback(pub Entity);

pub(crate) fn materialize(world: &mut World, owner: Entity, attributes: &[TemplateAttribute]) {
    let alt = crate::component::static_attribute_value(attributes, "alt").unwrap_or_default();
    let initials = alt
        .split_whitespace()
        .filter_map(|word| word.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase();
    let fallback = crate::widgets::controls::spawn_text_part(
        world,
        owner,
        ControlPartKind::Placeholder,
        initials,
    );
    if crate::component::static_attribute_value(attributes, "src").is_some() {
        world.entity_mut(fallback).insert(Visibility::Hidden);
    }
    world.entity_mut(owner).add_child(fallback);
    world.entity_mut(owner).insert(AvatarFallback(fallback));
}

/// Changes an avatar's image source and updates its persistent fallback visibility.
pub fn set_avatar_source(world: &mut World, entity: Entity, source: Option<String>) -> bool {
    let changed = super::image::set_image_source(world, entity, source.clone());
    if changed && let Some(fallback) = world.get::<AvatarFallback>(entity).copied() {
        world.entity_mut(fallback.0).insert(if source.is_some() {
            Visibility::Hidden
        } else {
            Visibility::Visible
        });
    }
    changed
}
