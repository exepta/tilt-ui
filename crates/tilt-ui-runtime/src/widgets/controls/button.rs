//! Runtime semantics specific to the TiltUI button element.

use bevy::ecs::component::Component;
use bevy::ecs::{entity::Entity, world::World};
use tilt_ui_core::TemplateAttribute;

use crate::{ControlPartKind, ElementState};

use super::input::FileInputOptions;
use super::spawn_part;

/// Marks a materialized TiltUI control with native button semantics.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TiltButton;

/// A button that opens the same file picker as `<input type="file">`.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct FileUploadButton {
    /// Form field name used when the button has a selected file.
    pub name: Option<String>,
    /// Whether a form submission requires a selection.
    pub required: bool,
}

/// A button with a persistent CSS-stylable spinner. While active, user clicks are ignored.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadingButton {
    /// Whether the button currently shows its spinner and blocks activation.
    pub active: bool,
    /// Entity of the generated `::spinner` part.
    pub spinner: Entity,
}

/// Changes the loading state of a `<button type="loading">`.
///
/// Returns `true` only when the entity is a loading button and its state changed.
pub fn set_button_loading(world: &mut World, entity: Entity, active: bool) -> bool {
    let Some(mut button) = world.get_mut::<LoadingButton>(entity) else {
        return false;
    };
    if button.active == active {
        return false;
    }
    button.active = active;
    if let Some(mut state) = world.get_mut::<ElementState>(entity) {
        state.loading = active;
    }
    true
}

pub(crate) fn materialize(world: &mut World, entity: Entity, attributes: &[TemplateAttribute]) {
    match crate::component::static_attribute_value(attributes, "type") {
        Some("loading") => {
            let spinner = spawn_part(world, entity, ControlPartKind::Spinner);
            world.entity_mut(entity).add_child(spinner);
            world.entity_mut(entity).insert(LoadingButton {
                active: true,
                spinner,
            });
            if let Some(mut state) = world.get_mut::<ElementState>(entity) {
                state.loading = true;
            }
        }
        Some("file") => {
            world.entity_mut(entity).insert((
                FileUploadButton {
                    name: crate::component::static_attribute_value(attributes, "name")
                        .filter(|name| !name.is_empty())
                        .map(str::to_owned),
                    required: crate::component::has_boolean_static_attribute(
                        attributes, "required",
                    ),
                },
                FileInputOptions::from_attributes(attributes),
            ));
        }
        _ => {}
    }
}
