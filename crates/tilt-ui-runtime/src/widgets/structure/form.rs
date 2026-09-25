//! Submit and reset behavior for TiltUI forms.

use std::collections::BTreeMap;

use bevy::{
    ecs::{
        component::Component,
        entity::Entity,
        hierarchy::{ChildOf, Children},
        message::{Message, MessageCursor, Messages},
        resource::Resource,
        world::World,
    },
    prelude::IntoScheduleConfigs,
};
use tilt_ui_core::{ButtonType, FormValidationMode, TemplateAttribute};

use crate::{
    ControlActivated, ControlChecked, EditableText, EditableTextOptions, ElementState,
    StaticAttributes, set_control_checked, set_editable_text, widgets::state::editable_invalid,
};

/// Static behavior declared on a form element.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct FormSettings {
    /// Registered Rust handler named by the `action` attribute.
    pub action: Option<String>,
    /// Point at which validation is requested.
    pub validation: FormValidationMode,
}

impl FormSettings {
    pub(crate) fn from_attributes(attributes: &[TemplateAttribute]) -> Self {
        let value = |name| crate::component::static_attribute_value(attributes, name);
        let validation = match value("validate")
            .unwrap_or("send")
            .to_ascii_lowercase()
            .as_str()
        {
            "always" | "allways" | "all" => FormValidationMode::Always,
            "interact" => FormValidationMode::Interact,
            _ => FormValidationMode::Send,
        };
        Self {
            action: value("action").map(str::to_owned),
            validation,
        }
    }
}

/// Button behavior inside a form.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormButton(pub ButtonType);

impl FormButton {
    pub(crate) fn from_attributes(attributes: &[TemplateAttribute]) -> Self {
        Self(
            match crate::component::static_attribute_value(attributes, "type") {
                Some("button") => ButtonType::Button,
                Some("submit") => ButtonType::Submit,
                Some("reset") => ButtonType::Reset,
                _ => ButtonType::Auto,
            },
        )
    }
}

/// Emitted after a valid form submission.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct FormSubmitted {
    pub form: Entity,
    pub submitter: Entity,
    pub action: Option<String>,
    pub data: BTreeMap<String, String>,
}

/// Emitted when a submit is rejected by form validation.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct FormValidationFailed {
    pub form: Entity,
    pub submitter: Entity,
    pub invalid: Vec<Entity>,
}

#[derive(Resource, Default)]
struct FormActivationCursor(MessageCursor<ControlActivated>);

pub(crate) fn install(app: &mut bevy::app::App) {
    app.add_message::<FormSubmitted>()
        .add_message::<FormValidationFailed>()
        .init_resource::<FormActivationCursor>()
        .add_systems(
            bevy::app::Update,
            process_form_activation.after(crate::control::TiltControlSystems::Activation),
        );
}

pub(crate) fn process_form_activation(world: &mut World) {
    let activations = world.resource_scope(
        |world, mut cursor: bevy::ecs::change_detection::Mut<FormActivationCursor>| {
            cursor
                .0
                .read(world.resource::<Messages<ControlActivated>>())
                .map(|event| event.entity)
                .collect::<Vec<_>>()
        },
    );
    for submitter in activations {
        let Some(button) = world.get::<FormButton>(submitter).copied() else {
            continue;
        };
        let Some(form) = parent_form(world, submitter) else {
            continue;
        };
        match button.0 {
            ButtonType::Button => continue,
            ButtonType::Reset => reset_form(world, form),
            ButtonType::Auto | ButtonType::Submit => submit_form(world, form, submitter),
        }
    }
}

fn parent_form(world: &World, entity: Entity) -> Option<Entity> {
    let mut current = entity;
    while let Some(parent) = world.get::<ChildOf>(current) {
        current = parent.parent();
        if world.get::<FormSettings>(current).is_some() {
            return Some(current);
        }
    }
    None
}

fn form_descendants(world: &World, form: Entity) -> Vec<Entity> {
    let mut result = Vec::new();
    let mut pending = vec![form];
    while let Some(parent) = pending.pop() {
        if parent != form {
            if world.get::<FormSettings>(parent).is_some() {
                continue;
            }
            result.push(parent);
        }
        if let Some(children) = world.get::<Children>(parent) {
            pending.extend(children.iter());
        }
    }
    result
}

fn submit_form(world: &mut World, form: Entity, submitter: Entity) {
    let action = world
        .get::<FormSettings>(form)
        .and_then(|settings| settings.action.clone());
    let mut data = BTreeMap::new();
    let mut invalid = Vec::new();
    for entity in form_descendants(world, form) {
        if let (Some(text), Some(options)) = (
            world.get::<EditableText>(entity),
            world.get::<EditableTextOptions>(entity),
        ) {
            if editable_invalid(&text.value, text.input_type, options) {
                invalid.push(entity);
            }
            if let Some(name) = options.name.as_ref().filter(|name| !name.is_empty()) {
                data.insert(name.clone(), text.value.clone());
            }
        } else if world
            .get::<ControlChecked>(entity)
            .is_some_and(|checked| checked.0)
        {
            let attributes = world.get::<StaticAttributes>(entity);
            if let Some(name) = attribute(attributes, "name") {
                data.insert(
                    name.to_owned(),
                    attribute(attributes, "value").unwrap_or("on").to_owned(),
                );
            }
        }
    }
    if !invalid.is_empty() {
        for entity in &invalid {
            if let Some(mut state) = world.get_mut::<ElementState>(*entity) {
                state.invalid = true;
            }
        }
        world
            .resource_mut::<Messages<FormValidationFailed>>()
            .write(FormValidationFailed {
                form,
                submitter,
                invalid,
            });
        return;
    }
    world
        .resource_mut::<Messages<FormSubmitted>>()
        .write(FormSubmitted {
            form,
            submitter,
            action,
            data,
        });
}

fn reset_form(world: &mut World, form: Entity) {
    for entity in form_descendants(world, form) {
        let attributes = world.get::<StaticAttributes>(entity);
        let initial_value = attribute(attributes, "value")
            .unwrap_or_default()
            .to_owned();
        let initially_checked = attributes.is_some_and(|attributes| {
            attributes.attributes.iter().any(|attribute| {
                crate::component::boolean_attribute_value(
                    &attribute.name,
                    &attribute.value,
                    "checked",
                ) || crate::component::boolean_attribute_value(
                    &attribute.name,
                    &attribute.value,
                    "selected",
                )
            })
        });
        if world.get::<EditableText>(entity).is_some() {
            set_editable_text(world, entity, initial_value);
        } else if world.get::<ControlChecked>(entity).is_some() {
            set_control_checked(world, entity, initially_checked);
        }
    }
}

fn attribute<'a>(attributes: Option<&'a StaticAttributes>, name: &str) -> Option<&'a str> {
    attributes?
        .attributes
        .iter()
        .find_map(|attribute| (attribute.name == name).then_some(attribute.value.as_str()))
}

#[cfg(test)]
mod tests {
    use bevy::ecs::{
        message::{MessageCursor, Messages},
        world::World,
    };
    use tilt_ui_core::InputType;

    use super::*;

    #[test]
    fn rejects_invalid_submit_then_collects_data_and_resets() {
        let mut world = World::new();
        world.init_resource::<Messages<FormSubmitted>>();
        world.init_resource::<Messages<FormValidationFailed>>();
        let form = world
            .spawn(FormSettings {
                action: Some("save".into()),
                validation: FormValidationMode::Send,
            })
            .id();
        let input = world
            .spawn((
                EditableText::new(String::new(), InputType::Text, false, false),
                EditableTextOptions {
                    name: Some("title".into()),
                    required: true,
                    ..Default::default()
                },
                StaticAttributes::default(),
                ElementState::default(),
            ))
            .id();
        let button = world.spawn(FormButton(ButtonType::Submit)).id();
        let check = world
            .spawn((
                ControlChecked(true),
                ElementState {
                    checked: true,
                    ..Default::default()
                },
                StaticAttributes {
                    attributes: vec![crate::StaticAttribute {
                        name: "checked".into(),
                        value: "false".into(),
                    }],
                },
            ))
            .id();
        world.entity_mut(form).add_children(&[input, button, check]);

        submit_form(&mut world, form, button);
        let mut failures = MessageCursor::<FormValidationFailed>::default();
        let failed = failures
            .read(world.resource::<Messages<FormValidationFailed>>())
            .collect::<Vec<_>>();
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].invalid, vec![input]);

        assert!(set_editable_text(&mut world, input, "Draft"));
        submit_form(&mut world, form, button);
        let mut submitted = MessageCursor::<FormSubmitted>::default();
        let events = submitted
            .read(world.resource::<Messages<FormSubmitted>>())
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action.as_deref(), Some("save"));
        assert_eq!(
            events[0].data.get("title").map(String::as_str),
            Some("Draft")
        );

        reset_form(&mut world, form);
        assert_eq!(world.get::<EditableText>(input).unwrap().value, "");
        assert!(!world.get::<ControlChecked>(check).unwrap().0);
    }
}
