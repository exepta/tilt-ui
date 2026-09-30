//! Submit and reset behavior for TiltUI forms.

use std::collections::{BTreeMap, HashMap, HashSet};

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

use crate::TiltElement;
use crate::widgets::controls::{
    button::FileUploadButton, input::FileInputSelection, option::OptionData,
};
use crate::{
    ControlActivated, ControlChecked, ControlCheckedChanged, EditableText, EditableTextChanged,
    EditableTextCommitted, EditableTextOptions, ElementState, OptionSelectionChanged,
    StaticAttributes, set_control_checked, set_editable_text, widgets::state::editable_invalid,
};
use tilt_ui_core::ElementKind;

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
                Some("file") => ButtonType::Button,
                Some("loading") => ButtonType::Button,
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
    pub data: FormData,
}

/// All successful values of a form, preserving repeated names and selection order.
pub type FormData = BTreeMap<String, Vec<FormValue>>;

/// One submitted text, option, or file value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormValue {
    Text(String),
    File(FormFile),
}

/// Selected file metadata; the native path is absent in browser builds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormFile {
    pub name: String,
    pub size_bytes: Option<u64>,
    pub native_path: Option<std::path::PathBuf>,
}

impl From<&FileInputSelection> for FormFile {
    fn from(selection: &FileInputSelection) -> Self {
        Self {
            name: selection.name.clone(),
            size_bytes: selection.size_bytes,
            native_path: selection.native_path.clone(),
        }
    }
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

#[derive(Default)]
struct ValidationProgress {
    interacted: HashSet<Entity>,
    attempted: bool,
    initialized: bool,
    mode: Option<FormValidationMode>,
}

#[derive(Resource, Default)]
struct FormValidationRuntime {
    forms: HashMap<Entity, ValidationProgress>,
    dirty: HashSet<Entity>,
    edited: MessageCursor<EditableTextChanged>,
    committed: MessageCursor<EditableTextCommitted>,
    checked: MessageCursor<ControlCheckedChanged>,
    selected: MessageCursor<OptionSelectionChanged>,
}

pub(crate) fn install(app: &mut bevy::app::App) {
    app.add_message::<FormSubmitted>()
        .add_message::<FormValidationFailed>()
        .init_resource::<FormActivationCursor>()
        .init_resource::<FormValidationRuntime>()
        .add_systems(
            bevy::app::Update,
            process_form_activation.after(crate::control::TiltControlSystems::Activation),
        )
        .add_systems(
            bevy::app::Update,
            update_form_validation.after(process_form_activation),
        );
}

pub(crate) fn mark_form_dirty(world: &mut World, field: Entity) {
    if let Some(form) = parent_form(world, field)
        && let Some(mut runtime) = world.get_resource_mut::<FormValidationRuntime>()
    {
        runtime.dirty.insert(form);
    }
}

fn field_invalid(world: &World, entity: Entity, form: Entity) -> Option<bool> {
    if let Some(upload) = world.get::<FileUploadButton>(entity) {
        return upload
            .required
            .then(|| world.get::<FileInputSelection>(entity).is_none());
    }
    if let (Some(text), Some(options)) = (
        world.get::<EditableText>(entity),
        world.get::<EditableTextOptions>(entity),
    ) {
        return Some(editable_invalid(&text.value, text.input_type, options));
    }
    let element = world.get::<TiltElement>(entity)?;
    let attrs = world.get::<StaticAttributes>(entity);
    let required = attribute(attrs, "required").is_some_and(|value| {
        crate::component::boolean_attribute_value("required", value, "required")
    });
    if matches!(element.kind, ElementKind::ChoiceBox | ElementKind::ListBox) {
        return required.then(|| {
            !form_descendants(world, entity).into_iter().any(|child| {
                world.get::<OptionData>(child).is_some()
                    && !disabled_in_form(world, child, form)
                    && world
                        .get::<ControlChecked>(child)
                        .is_some_and(|checked| checked.0)
            })
        });
    }
    if required && world.get::<ControlChecked>(entity).is_some() {
        if element.kind == ElementKind::RadioButton {
            let name = attribute(attrs, "name");
            return Some(!form_descendants(world, form).into_iter().any(|other| {
                world
                    .get::<TiltElement>(other)
                    .is_some_and(|other_element| other_element.kind == ElementKind::RadioButton)
                    && attribute(world.get::<StaticAttributes>(other), "name") == name
                    && !disabled_in_form(world, other, form)
                    && world
                        .get::<ControlChecked>(other)
                        .is_some_and(|checked| checked.0)
            }));
        }
        return Some(
            !world
                .get::<ControlChecked>(entity)
                .is_some_and(|checked| checked.0),
        );
    }
    None
}

fn update_form_validation(world: &mut World) {
    let mut runtime = world
        .remove_resource::<FormValidationRuntime>()
        .unwrap_or_default();
    let mut touched = Vec::new();
    if let Some(events) = world.get_resource::<Messages<EditableTextChanged>>() {
        touched.extend(runtime.edited.read(events).map(|event| event.entity));
    }
    if let Some(events) = world.get_resource::<Messages<EditableTextCommitted>>() {
        touched.extend(runtime.committed.read(events).map(|event| event.entity));
    }
    if let Some(events) = world.get_resource::<Messages<ControlCheckedChanged>>() {
        touched.extend(runtime.checked.read(events).map(|event| event.entity));
    }
    if let Some(events) = world.get_resource::<Messages<OptionSelectionChanged>>() {
        touched.extend(runtime.selected.read(events).map(|event| event.control));
    }
    for field in touched {
        if let Some(form) = parent_form(world, field) {
            runtime
                .forms
                .entry(form)
                .or_default()
                .interacted
                .insert(field);
            runtime.dirty.insert(form);
        }
    }
    let forms = {
        let mut query = world.query::<(Entity, &FormSettings)>();
        query
            .iter(world)
            .map(|(entity, settings)| (entity, settings.validation))
            .collect::<Vec<_>>()
    };
    runtime
        .forms
        .retain(|form, _| forms.iter().any(|(entity, _)| entity == form));
    for (form, mode) in forms {
        let progress = runtime.forms.entry(form).or_default();
        let dirty = runtime.dirty.remove(&form);
        if progress.initialized && progress.mode == Some(mode) && !dirty {
            continue;
        }
        progress.initialized = true;
        progress.mode = Some(mode);
        for field in form_descendants(world, form) {
            if disabled_in_form(world, field, form) {
                if let Some(mut state) = world.get_mut::<ElementState>(field) {
                    state.invalid = false;
                }
                continue;
            }
            let Some(invalid) = field_invalid(world, field, form) else {
                continue;
            };
            let visible = match mode {
                FormValidationMode::Always => true,
                FormValidationMode::Interact => {
                    progress.interacted.contains(&field) || progress.attempted
                }
                FormValidationMode::Send => progress.attempted,
            };
            if let Some(mut state) = world.get_mut::<ElementState>(field) {
                state.invalid = invalid && visible;
            }
        }
    }
    world.insert_resource(runtime);
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

pub(crate) fn parent_form(world: &World, entity: Entity) -> Option<Entity> {
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
            pending.extend(children.iter().rev());
        }
    }
    result
}

fn disabled_in_form(world: &World, mut entity: Entity, form: Entity) -> bool {
    loop {
        if world
            .get::<ElementState>(entity)
            .is_some_and(|state| state.disabled)
            || world.get::<bevy::ui::InteractionDisabled>(entity).is_some()
        {
            return true;
        }
        if entity == form {
            return false;
        }
        let Some(parent) = world.get::<ChildOf>(entity) else {
            return false;
        };
        entity = parent.parent();
    }
}

fn submit_form(world: &mut World, form: Entity, submitter: Entity) {
    if let Some(mut runtime) = world.get_resource_mut::<FormValidationRuntime>() {
        runtime.forms.entry(form).or_default().attempted = true;
        runtime.dirty.insert(form);
    }
    let action = world
        .get::<FormSettings>(form)
        .and_then(|settings| settings.action.clone());
    let mut data = FormData::new();
    let mut invalid = Vec::new();
    for entity in form_descendants(world, form) {
        if disabled_in_form(world, entity, form) {
            continue;
        }
        if field_invalid(world, entity, form) == Some(true) {
            invalid.push(entity);
        }
        if let (Some(text), Some(options)) = (
            world.get::<EditableText>(entity),
            world.get::<EditableTextOptions>(entity),
        ) {
            if let Some(name) = options.name.as_ref().filter(|name| !name.is_empty()) {
                if text.input_type == tilt_ui_core::InputType::File {
                    if let Some(selection) = world.get::<FileInputSelection>(entity) {
                        data.entry(name.clone())
                            .or_default()
                            .push(FormValue::File(FormFile::from(selection)));
                    }
                } else {
                    let value = if text.input_type == tilt_ui_core::InputType::Number {
                        crate::widgets::controls::input::number_expression(&text.value)
                            .map(|number| number.to_string())
                            .unwrap_or_else(|| text.value.clone())
                    } else {
                        text.value.clone()
                    };
                    data.entry(name.clone())
                        .or_default()
                        .push(FormValue::Text(value));
                }
            }
        } else if let Some(upload) = world.get::<FileUploadButton>(entity) {
            if let (Some(name), Some(selection)) = (
                upload.name.as_ref(),
                world.get::<FileInputSelection>(entity),
            ) {
                data.entry(name.clone())
                    .or_default()
                    .push(FormValue::File(FormFile::from(selection)));
            }
        } else if world.get::<TiltElement>(entity).is_some_and(|element| {
            matches!(element.kind, ElementKind::ChoiceBox | ElementKind::ListBox)
        }) {
            if let Some(name) = attribute(world.get::<StaticAttributes>(entity), "name") {
                for option in form_descendants(world, entity) {
                    if disabled_in_form(world, option, form) {
                        continue;
                    }
                    if let (Some(value), Some(option)) = (
                        world.get::<ControlChecked>(option),
                        world.get::<OptionData>(option),
                    ) && value.0
                    {
                        data.entry(name.to_owned())
                            .or_default()
                            .push(FormValue::Text(option.value.clone()));
                    }
                }
            }
        } else if world
            .get::<ControlChecked>(entity)
            .is_some_and(|checked| checked.0)
        {
            if world.get::<OptionData>(entity).is_some() {
                continue;
            }
            let attributes = world.get::<StaticAttributes>(entity);
            if let Some(name) = attribute(attributes, "name") {
                data.entry(name.to_owned())
                    .or_default()
                    .push(FormValue::Text(
                        attribute(attributes, "value").unwrap_or("on").to_owned(),
                    ));
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
    if let Some(mut runtime) = world.get_resource_mut::<FormValidationRuntime>() {
        runtime.forms.remove(&form);
        runtime.dirty.insert(form);
    }
    for entity in form_descendants(world, form) {
        if world.get::<FileInputSelection>(entity).is_some() {
            world.entity_mut(entity).remove::<FileInputSelection>();
        }
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
    fn upload_button_contributes_file_data_and_resets_selection() {
        let mut world = World::new();
        world.init_resource::<Messages<FormSubmitted>>();
        world.init_resource::<Messages<FormValidationFailed>>();
        let form = world
            .spawn(FormSettings {
                action: None,
                validation: FormValidationMode::Send,
            })
            .id();
        let upload = world
            .spawn((
                FileUploadButton {
                    name: Some("attachment".into()),
                    required: true,
                },
                ElementState::default(),
                StaticAttributes::default(),
                FormButton(ButtonType::Button),
            ))
            .id();
        let submitter = world.spawn(FormButton(ButtonType::Submit)).id();
        world.entity_mut(form).add_children(&[upload, submitter]);

        submit_form(&mut world, form, submitter);
        assert!(world.resource::<Messages<FormSubmitted>>().is_empty());
        assert!(world.get::<ElementState>(upload).unwrap().invalid);

        world.entity_mut(upload).insert(FileInputSelection {
            name: "report.pdf".into(),
            size_bytes: Some(42),
            native_path: None,
        });
        submit_form(&mut world, form, submitter);
        let mut cursor = MessageCursor::<FormSubmitted>::default();
        let events = cursor
            .read(world.resource::<Messages<FormSubmitted>>())
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].data["attachment"],
            vec![FormValue::File(FormFile {
                name: "report.pdf".into(),
                size_bytes: Some(42),
                native_path: None,
            })]
        );

        reset_form(&mut world, form);
        assert!(world.get::<FileInputSelection>(upload).is_none());
        assert_eq!(field_invalid(&world, upload, form), Some(true));
    }

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
            events[0]
                .data
                .get("title")
                .and_then(|values| values.first()),
            Some(&FormValue::Text("Draft".into()))
        );

        reset_form(&mut world, form);
        assert_eq!(world.get::<EditableText>(input).unwrap().value, "");
        assert!(!world.get::<ControlChecked>(check).unwrap().0);
    }

    #[test]
    fn validation_modes_reveal_errors_at_the_configured_time() {
        let mut world = World::new();
        world.init_resource::<FormValidationRuntime>();
        world.init_resource::<Messages<EditableTextChanged>>();
        world.init_resource::<Messages<FormSubmitted>>();
        world.init_resource::<Messages<FormValidationFailed>>();
        let mut entries = Vec::new();
        for mode in [
            FormValidationMode::Always,
            FormValidationMode::Interact,
            FormValidationMode::Send,
        ] {
            let form = world
                .spawn(FormSettings {
                    action: None,
                    validation: mode,
                })
                .id();
            let field = world
                .spawn((
                    EditableText::new(String::new(), InputType::Text, false, false),
                    EditableTextOptions {
                        required: true,
                        ..Default::default()
                    },
                    ElementState::default(),
                ))
                .id();
            let submitter = world.spawn(FormButton(ButtonType::Submit)).id();
            world.entity_mut(form).add_children(&[field, submitter]);
            entries.push((form, field, submitter));
        }
        update_form_validation(&mut world);
        assert!(world.get::<ElementState>(entries[0].1).unwrap().invalid);
        assert!(!world.get::<ElementState>(entries[1].1).unwrap().invalid);
        assert!(!world.get::<ElementState>(entries[2].1).unwrap().invalid);

        assert!(set_editable_text(&mut world, entries[2].1, "filled"));
        assert!(set_editable_text(&mut world, entries[2].1, ""));
        assert!(!world.get::<ElementState>(entries[2].1).unwrap().invalid);
        update_form_validation(&mut world);
        assert!(!world.get::<ElementState>(entries[2].1).unwrap().invalid);

        world
            .resource_mut::<Messages<EditableTextChanged>>()
            .write(EditableTextChanged {
                entity: entries[1].1,
                name: None,
                value: String::new(),
            });
        update_form_validation(&mut world);
        assert!(world.get::<ElementState>(entries[1].1).unwrap().invalid);
        assert!(!world.get::<ElementState>(entries[2].1).unwrap().invalid);

        submit_form(&mut world, entries[2].0, entries[2].2);
        update_form_validation(&mut world);
        assert!(world.get::<ElementState>(entries[2].1).unwrap().invalid);
        reset_form(&mut world, entries[2].0);
        update_form_validation(&mut world);
        assert!(!world.get::<ElementState>(entries[2].1).unwrap().invalid);
    }

    #[test]
    fn submission_keeps_repeated_options_and_file_metadata() {
        let mut world = World::new();
        world.init_resource::<Messages<FormSubmitted>>();
        world.init_resource::<Messages<FormValidationFailed>>();
        let attrs = |name: &str| StaticAttributes {
            attributes: vec![crate::StaticAttribute {
                name: "name".into(),
                value: name.into(),
            }],
        };
        let form = world
            .spawn(FormSettings {
                action: None,
                validation: FormValidationMode::Send,
            })
            .id();
        let first = world
            .spawn((
                EditableText::new("one".into(), InputType::Text, false, false),
                EditableTextOptions {
                    name: Some("tag".into()),
                    ..Default::default()
                },
            ))
            .id();
        let second = world
            .spawn((
                EditableText::new("two".into(), InputType::Text, false, false),
                EditableTextOptions {
                    name: Some("tag".into()),
                    ..Default::default()
                },
            ))
            .id();
        let list = world
            .spawn((
                TiltElement {
                    kind: ElementKind::ListBox,
                },
                attrs("choices"),
            ))
            .id();
        let group = world
            .spawn(TiltElement {
                kind: ElementKind::Div,
            })
            .id();
        let option = |world: &mut World, value: &str| {
            world
                .spawn((
                    OptionData {
                        value: value.into(),
                        label: value.into(),
                    },
                    ControlChecked(true),
                ))
                .id()
        };
        let a = option(&mut world, "a");
        let b = option(&mut world, "b");
        world.entity_mut(group).add_children(&[a, b]);
        world.entity_mut(list).add_child(group);
        let file = world
            .spawn((
                EditableText::new("report.pdf".into(), InputType::File, false, false),
                EditableTextOptions {
                    name: Some("upload".into()),
                    ..Default::default()
                },
                FileInputSelection {
                    name: "report.pdf".into(),
                    size_bytes: Some(42),
                    native_path: None,
                },
            ))
            .id();
        let calculation = world
            .spawn((
                EditableText::new("2+3*4".into(), InputType::Number, false, false),
                EditableTextOptions {
                    name: Some("quantity".into()),
                    ..Default::default()
                },
            ))
            .id();
        let submitter = world.spawn(FormButton(ButtonType::Submit)).id();
        world
            .entity_mut(form)
            .add_children(&[first, second, list, file, calculation, submitter]);
        submit_form(&mut world, form, submitter);
        let mut cursor = MessageCursor::<FormSubmitted>::default();
        let events = cursor
            .read(world.resource::<Messages<FormSubmitted>>())
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].data["tag"],
            vec![FormValue::Text("one".into()), FormValue::Text("two".into())]
        );
        assert_eq!(
            events[0].data["choices"],
            vec![FormValue::Text("a".into()), FormValue::Text("b".into())]
        );
        assert_eq!(
            events[0].data["upload"],
            vec![FormValue::File(FormFile {
                name: "report.pdf".into(),
                size_bytes: Some(42),
                native_path: None,
            })]
        );
        assert_eq!(
            events[0].data["quantity"],
            vec![FormValue::Text("14".into())]
        );
    }
}
