//! Runtime semantics specific to the TiltUI input element.

use bevy::ecs::{
    component::Component,
    message::{Message, MessageReader},
};
use bevy::{
    color::Color,
    ecs::{entity::Entity, system::Commands, world::World},
    prelude::Visibility,
    text::TextLayoutInfo,
    text::{EditableText as BevyEditableText, LineBreak, TextCursorStyle, TextEdit, TextLayout},
    ui::{
        ContentSize, InteractionDisabled,
        widget::{ImageNode, TextNodeFlags, TextScroll},
    },
};
use tilt_ui_core::{InputType, TemplateAttribute};

use crate::{
    ControlActivated, ControlPartKind, EditableText, EditableTextChanged, EditableTextCommitted,
    EditableTextOptions, ElementState, TiltControl, widgets::state::EditableTextParts,
};
use bevy::ui::widget::Button;
use bevy_input_focus::tab_navigation::TabIndex;
use bevy_picking::Pickable;

use super::{spawn_part, spawn_text_part};

/// Configures native file selection for a file Input.
#[derive(Component, Debug, Clone, Default)]
pub struct FileInputOptions {
    /// Select a directory instead of a single file.
    pub folder: bool,
    /// Allowed filename extensions without leading periods.
    pub extensions: Vec<String>,
    /// Include the selected file size in the visible label.
    pub show_size: bool,
    /// Optional maximum accepted file size in bytes.
    pub max_size_bytes: Option<u64>,
}

impl FileInputOptions {
    pub(crate) fn from_attributes(attributes: &[TemplateAttribute]) -> Self {
        Self {
            folder: crate::component::has_boolean_static_attribute(attributes, "folder"),
            extensions: static_value(attributes, "extensions")
                .map_or_else(Vec::new, |raw| parse_extensions(&raw)),
            show_size: crate::component::has_boolean_static_attribute(attributes, "show-size"),
            max_size_bytes: static_value(attributes, "max-size")
                .and_then(|value| parse_file_size(&value)),
        }
    }
}

/// Optional presentation and editing behavior shared by input and textarea.
#[derive(Component, Debug, Clone, Default)]
pub struct InputFieldOptions {
    pub clear_on_blur: bool,
    pub cap_text_at: Option<usize>,
    pub cap_to_width: bool,
    pub label: Option<Entity>,
    pub icon: Option<Entity>,
}

pub(crate) fn update_input_option(world: &mut World, owner: Entity, name: &str, value: &str) {
    let Some(mut options) = world.get::<InputFieldOptions>(owner).cloned() else {
        return;
    };
    match name {
        "clear-on-blur" | "clear-on-focus-loss" | "clear_on_focus_loss" => {
            options.clear_on_blur = bool_value(value)
        }
        "cap-text-at" | "cap_text_at" => {
            options.cap_to_width = value == "width";
            options.cap_text_at = parse_cap(value);
            let max_characters = options.cap_text_at.or_else(|| {
                world
                    .get::<EditableTextOptions>(owner)
                    .and_then(|options| options.max_characters)
            });
            if let Some(mut native) = world.get_mut::<BevyEditableText>(owner) {
                native.max_characters = max_characters;
            }
        }
        "label" => {
            if let Some(label) = options.label {
                if value.is_empty() {
                    world.entity_mut(label).despawn();
                    options.label = None;
                } else if let Some(mut text) = world.get_mut::<bevy::ui::widget::Text>(label) {
                    if text.0 != value {
                        text.0 = value.to_owned();
                    }
                }
            } else if !value.is_empty() {
                let label = spawn_text_part(world, owner, ControlPartKind::Label, value);
                world.entity_mut(owner).add_child(label);
                options.label = Some(label);
            }
        }
        "icon" => {
            if let Some(icon) = options.icon.take() {
                world.entity_mut(icon).despawn();
            }
            if !value.is_empty() {
                let icon = spawn_part(world, owner, ControlPartKind::Indicator);
                let image = crate::widgets::content::image::load_image_handle(world, value)
                    .unwrap_or_default();
                world.entity_mut(icon).insert(ImageNode::new(image));
                world.entity_mut(owner).add_child(icon);
                options.icon = Some(icon);
            }
        }
        _ => return,
    }
    world.entity_mut(owner).insert(options);
}

fn bool_value(value: &str) -> bool {
    !matches!(value, "" | "false" | "0" | "off")
}

pub(crate) fn reconfigure_input(world: &mut World, owner: Entity, name: &str, value: &str) {
    let Some(editable) = world.get::<EditableText>(owner).cloned() else {
        return;
    };
    if editable.multiline {
        return;
    }
    match name {
        "type" => {
            let next = input_type(Some(value));
            if editable.input_type == next {
                return;
            }
            if let Some(mut state) = world.get_mut::<EditableText>(owner) {
                state.input_type = next;
            }
            if next == InputType::File {
                let attribute = |name: &str| {
                    world
                        .get::<crate::StaticAttributes>(owner)
                        .and_then(|attrs| {
                            attrs
                                .attributes
                                .iter()
                                .find(|attr| attr.name == name)
                                .map(|attr| attr.value.clone())
                        })
                };
                let options = FileInputOptions {
                    folder: attribute("folder")
                        .is_some_and(|value| value.is_empty() || bool_value(&value)),
                    extensions: attribute("extensions")
                        .map_or_else(Vec::new, |value| parse_extensions(&value)),
                    show_size: attribute("show-size")
                        .is_some_and(|value| value.is_empty() || bool_value(&value)),
                    max_size_bytes: attribute("max-size").and_then(|value| parse_file_size(&value)),
                };
                world.entity_mut(owner).insert(options);
            } else {
                world
                    .entity_mut(owner)
                    .remove::<(FileInputOptions, FileInputSelection)>();
            }
            let text_mode = matches!(next, InputType::Password | InputType::File);
            if let Some(parts) = world.get::<EditableTextParts>(owner).copied() {
                if text_mode && world.get::<bevy::ui::widget::Text>(parts.value).is_none() {
                    world
                        .entity_mut(parts.value)
                        .insert(bevy::ui::widget::Text::new(""));
                } else if !text_mode {
                    world
                        .entity_mut(parts.value)
                        .remove::<bevy::ui::widget::Text>();
                }
                if let Some(mut visibility) = world.get_mut::<Visibility>(parts.value) {
                    *visibility = if text_mode {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    };
                }
            }
            if text_mode {
                world
                    .entity_mut(owner)
                    .remove::<crate::control::AnimateInputText>();
            } else {
                world
                    .entity_mut(owner)
                    .insert(crate::control::AnimateInputText);
            }
            remove_step_parts(world, owner);
            if next == InputType::Number
                && world
                    .get::<crate::StaticAttributes>(owner)
                    .is_some_and(|attrs| {
                        attrs.attributes.iter().any(|attr| {
                            crate::component::boolean_attribute_value(
                                &attr.name,
                                &attr.value,
                                "show-fields",
                            )
                        })
                    })
            {
                for (kind, label, direction) in [
                    (ControlPartKind::Decrement, "−", -1.0),
                    (ControlPartKind::Increment, "+", 1.0),
                ] {
                    let part = number_step_part(world, owner, kind, label, direction);
                    world.entity_mut(owner).add_child(part);
                }
            }
            if next == InputType::File
                || next == InputType::Number && !number_characters_allowed(&editable.value)
            {
                crate::set_editable_text(world, owner, "");
            } else if let Some(parts) = world.get::<EditableTextParts>(owner).copied() {
                if let Some(mut text) = world.get_mut::<bevy::ui::widget::Text>(parts.value) {
                    text.0 = if next == InputType::Password {
                        super::super::state::masked_password(&editable.value)
                    } else {
                        editable.value.clone()
                    };
                }
            }
        }
        "show-fields" => {
            remove_step_parts(world, owner);
            if editable.input_type == InputType::Number && bool_value(value) {
                for (kind, label, direction) in [
                    (ControlPartKind::Decrement, "−", -1.0),
                    (ControlPartKind::Increment, "+", 1.0),
                ] {
                    let part = number_step_part(world, owner, kind, label, direction);
                    world.entity_mut(owner).add_child(part);
                }
            }
        }
        "folder" | "show-size" | "extensions" | "max-size" => {
            let Some(mut options) = world.get::<FileInputOptions>(owner).cloned() else {
                return;
            };
            match name {
                "folder" => options.folder = bool_value(value),
                "show-size" => options.show_size = bool_value(value),
                "extensions" => options.extensions = parse_extensions(value),
                "max-size" => options.max_size_bytes = parse_file_size(value),
                _ => {}
            }
            world.entity_mut(owner).insert(options);
        }
        _ => {}
    }
}

fn remove_step_parts(world: &mut World, owner: Entity) {
    let children = world
        .get::<bevy::ecs::hierarchy::Children>(owner)
        .map(|children| children.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default();
    for child in children {
        if world.get::<crate::ControlPart>(child).is_some_and(|part| {
            matches!(
                part.kind,
                ControlPartKind::Increment | ControlPartKind::Decrement
            )
        }) {
            world.entity_mut(child).despawn();
        }
    }
}

fn parse_extensions(value: &str) -> Vec<String> {
    value
        .trim_matches(|c| c == '[' || c == ']')
        .split(',')
        .map(|item| {
            item.trim()
                .trim_matches(['"', '\''])
                .trim_start_matches('.')
                .to_owned()
        })
        .filter(|item| !item.is_empty())
        .collect()
}

fn parse_cap(value: &str) -> Option<usize> {
    value.parse::<usize>().ok().or_else(|| {
        value
            .parse::<f64>()
            .ok()
            .filter(|number| {
                number.is_finite()
                    && *number >= 0.0
                    && number.fract() == 0.0
                    && *number <= usize::MAX as f64
            })
            .map(|number| number as usize)
    })
}

/// Caps width-bound inputs after layout; only changed values are touched.
pub(crate) fn cap_input_width(world: &mut World) {
    let entries = {
        let mut query = world.query::<(
            Entity,
            &InputFieldOptions,
            &EditableText,
            &bevy::ui::ComputedNode,
            &TextLayoutInfo,
        )>();
        query
            .iter(world)
            .filter(|(_, options, editable, node, layout)| {
                options.cap_to_width
                    && !editable.multiline
                    && node.content_box().width() > 0.0
                    && layout.size.x > node.content_box().width()
            })
            .map(|(entity, _, editable, node, layout)| {
                (
                    entity,
                    editable.value.clone(),
                    node.content_box().width(),
                    layout.size.x,
                )
            })
            .collect::<Vec<_>>()
    };
    for (entity, mut value, width, actual) in entries {
        if width <= 0.0 || actual <= width {
            continue;
        }
        let chars = value.chars().count();
        let keep = ((chars as f32 * width / actual).floor() as usize).min(chars.saturating_sub(1));
        value = value.chars().take(keep).collect();
        if crate::set_editable_text(world, entity, &value) {
            let name = world
                .get::<EditableTextOptions>(entity)
                .and_then(|options| options.name.clone());
            world
                .resource_mut::<bevy::ecs::message::Messages<EditableTextChanged>>()
                .write(EditableTextChanged {
                    entity,
                    name,
                    value,
                });
        }
    }
}

/// Stores a file or directory selected through an input or upload button.
#[derive(Component, Debug, Clone)]
pub struct FileInputSelection {
    /// User-visible filename.
    pub name: String,
    /// File size in bytes when known.
    pub size_bytes: Option<u64>,
    /// Native filesystem path, unavailable on browser targets.
    pub native_path: Option<std::path::PathBuf>,
}

/// Notification emitted when a file input or upload button completes user selection.
#[derive(Message, Debug, Clone)]
pub struct FileInputSelected {
    /// File input or upload button entity.
    pub entity: Entity,
    /// Selected metadata.
    pub selection: FileInputSelection,
}

#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct NumberStepAction {
    owner: Entity,
    direction: f64,
}

pub(crate) fn number_characters_allowed(value: &str) -> bool {
    value.bytes().all(|byte| {
        byte.is_ascii_digit() || matches!(byte, b'.' | b'+' | b'-' | b'/' | b'*' | b'%')
    })
}

/// Evaluates the operators supported by a number input, with multiplication before addition.
pub(crate) fn number_expression(value: &str) -> Option<f64> {
    struct Parser<'a> {
        bytes: &'a [u8],
        at: usize,
    }
    impl Parser<'_> {
        fn sum(&mut self) -> Option<f64> {
            let mut value = self.product()?;
            while let Some(&operator @ (b'+' | b'-')) = self.bytes.get(self.at) {
                self.at += 1;
                let rhs = self.product()?;
                value = if operator == b'+' {
                    value + rhs
                } else {
                    value - rhs
                };
            }
            Some(value)
        }
        fn product(&mut self) -> Option<f64> {
            let mut value = self.number()?;
            while let Some(&operator @ (b'*' | b'/' | b'%')) = self.bytes.get(self.at) {
                self.at += 1;
                let rhs = self.number()?;
                value = match operator {
                    b'*' => value * rhs,
                    b'/' => value / rhs,
                    _ => value % rhs,
                };
            }
            Some(value)
        }
        fn number(&mut self) -> Option<f64> {
            let sign = match self.bytes.get(self.at) {
                Some(b'+') => {
                    self.at += 1;
                    1.0
                }
                Some(b'-') => {
                    self.at += 1;
                    -1.0
                }
                _ => 1.0,
            };
            let start = self.at;
            while self
                .bytes
                .get(self.at)
                .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'.')
            {
                self.at += 1;
            }
            std::str::from_utf8(&self.bytes[start..self.at])
                .ok()?
                .parse::<f64>()
                .ok()
                .map(|number| sign * number)
        }
    }
    if !number_characters_allowed(value) || value.is_empty() {
        return None;
    }
    let mut parser = Parser {
        bytes: value.as_bytes(),
        at: 0,
    };
    parser
        .sum()
        .filter(|number| parser.at == value.len() && number.is_finite())
}

fn number_step_part(
    world: &mut World,
    owner: Entity,
    kind: ControlPartKind,
    label: &str,
    direction: f64,
) -> Entity {
    let part = spawn_part(world, owner, kind);
    let text = world
        .spawn((
            bevy::ui::Node::default(),
            bevy::ui::widget::Text::new(label),
            Pickable::IGNORE,
        ))
        .id();
    world.entity_mut(part).add_child(text);
    world.entity_mut(part).insert((
        Button,
        TiltControl,
        ElementState::default(),
        TabIndex(-1),
        Pickable::default(),
        NumberStepAction { owner, direction },
    ));
    part
}

pub(crate) fn process_number_activation(
    mut activated: MessageReader<ControlActivated>,
    mut commands: Commands,
) {
    for activation in activated.read() {
        let entity = activation.entity;
        commands.queue(move |world: &mut World| {
            let Some(action) = world.get::<NumberStepAction>(entity).copied() else {
                return;
            };
            if world.get::<InteractionDisabled>(action.owner).is_some()
                || world
                    .get::<EditableText>(action.owner)
                    .is_some_and(|text| text.readonly)
            {
                return;
            }
            let Some(text) = world.get::<EditableText>(action.owner) else {
                return;
            };
            let value = number_expression(&text.value).unwrap_or(0.0);
            let options = world.get::<EditableTextOptions>(action.owner);
            let step = options
                .and_then(|options| options.step.as_deref())
                .and_then(|step| step.parse::<f64>().ok())
                .filter(|step| *step > 0.0)
                .unwrap_or(1.0);
            let min = options
                .and_then(|options| options.min.as_deref())
                .and_then(|min| min.parse::<f64>().ok());
            let max = options
                .and_then(|options| options.max.as_deref())
                .and_then(|max| max.parse::<f64>().ok());
            let mut next = value + action.direction * step;
            if let Some(min) = min {
                next = next.max(min);
            }
            if let Some(max) = max {
                next = next.min(max);
            }
            if !next.is_finite() {
                return;
            }
            let next = next.to_string();
            if crate::set_editable_text(world, action.owner, &next) {
                let name = world
                    .get::<EditableTextOptions>(action.owner)
                    .and_then(|options| options.name.clone());
                world
                    .resource_mut::<bevy::ecs::message::Messages<EditableTextChanged>>()
                    .write(EditableTextChanged {
                        entity: action.owner,
                        name,
                        value: next,
                    });
            }
        });
    }
}

pub(crate) fn normalize_number_commit(
    mut committed: MessageReader<EditableTextCommitted>,
    mut commands: Commands,
) {
    for event in committed.read() {
        let entity = event.entity;
        commands.queue(move |world: &mut World| {
            let Some(value) = world
                .get::<EditableText>(entity)
                .filter(|text| text.input_type == InputType::Number)
                .and_then(|text| number_expression(&text.value))
            else {
                return;
            };
            let normalized = value.to_string();
            if crate::set_editable_text(world, entity, &normalized) {
                let name = world
                    .get::<EditableTextOptions>(entity)
                    .and_then(|options| options.name.clone());
                world
                    .resource_mut::<bevy::ecs::message::Messages<EditableTextChanged>>()
                    .write(EditableTextChanged {
                        entity,
                        name,
                        value: normalized,
                    });
            }
        });
    }
}

pub(crate) fn materialize_parts(
    world: &mut World,
    owner: Entity,
    attributes: &[TemplateAttribute],
    multiline: bool,
) {
    let value = static_value(attributes, "value").unwrap_or_default();
    let input_type = if multiline {
        InputType::Text
    } else {
        input_type(static_value(attributes, "type").as_deref())
    };
    if input_type == InputType::File {
        world
            .entity_mut(owner)
            .insert(FileInputOptions::from_attributes(attributes));
    }
    let readonly = crate::component::has_boolean_static_attribute(attributes, "readonly");
    let options = super::super::state::EditableTextOptions {
        name: static_value(attributes, "name"),
        required: crate::component::has_boolean_static_attribute(attributes, "required"),
        min_length: static_value(attributes, "minlength").and_then(|value| value.parse().ok()),
        max_characters: static_value(attributes, "maxlength").and_then(|value| value.parse().ok()),
        max_lines: static_value(attributes, "max-lines").and_then(|value| value.parse().ok()),
        pattern: static_value(attributes, "pattern"),
        min: static_value(attributes, "min"),
        max: static_value(attributes, "max"),
        step: static_value(attributes, "step"),
    };
    if let Some(mut css_state) = world.get_mut::<crate::ElementState>(owner) {
        css_state.readonly = readonly;
        css_state.invalid = super::super::state::editable_invalid(&value, input_type, &options);
    }
    let mut native = BevyEditableText::new(&value);
    native.cursor_width = 0.12;
    let mut semantic = EditableText::new(value.clone(), input_type, readonly, multiline);
    if multiline {
        native.queue_edit(TextEdit::TextStart(false));
        semantic.cursor = 0;
    }
    native.allow_newlines = multiline;
    native.visible_lines = Some(if multiline { 4.0 } else { 1.0 });
    let cap_text_at =
        static_value(attributes, "cap-text-at").or_else(|| static_value(attributes, "cap_text_at"));
    native.max_characters = cap_text_at
        .as_deref()
        .and_then(parse_cap)
        .or(options.max_characters);
    world.entity_mut(owner).insert((
        semantic,
        options,
        native,
        crate::control::EditableHistory::default(),
        TextCursorStyle {
            color: Color::srgb(0.62, 0.17, 0.86),
            selection_color: Color::srgba(0.72, 0.48, 0.92, 0.4),
            ..Default::default()
        },
        TextLayout::linebreak(if multiline {
            LineBreak::WordOrCharacter
        } else {
            LineBreak::NoWrap
        }),
        TextNodeFlags::default(),
        ContentSize::default(),
        TextScroll::default(),
    ));
    if input_type != InputType::Password
        && input_type != InputType::File
        && static_value(attributes, "text-animation").as_deref() != Some("none")
    {
        world
            .entity_mut(owner)
            .insert(crate::control::AnimateInputText);
    }
    let value_part = if matches!(input_type, InputType::Password | InputType::File) {
        spawn_text_part(
            world,
            owner,
            ControlPartKind::Value,
            if input_type == InputType::Password {
                super::super::state::masked_password(&value)
            } else {
                value.clone()
            },
        )
    } else {
        spawn_part(world, owner, ControlPartKind::Value)
    };
    world.entity_mut(value_part).insert(
        if matches!(input_type, InputType::Password | InputType::File) {
            Visibility::Visible
        } else {
            Visibility::Hidden
        },
    );
    let placeholder = spawn_text_part(
        world,
        owner,
        ControlPartKind::Placeholder,
        static_value(attributes, "placeholder").unwrap_or_default(),
    );
    let selection = spawn_part(world, owner, ControlPartKind::Selection);
    let cursor = spawn_part(world, owner, ControlPartKind::Cursor);
    world.entity_mut(placeholder).insert(if value.is_empty() {
        Visibility::Visible
    } else {
        Visibility::Hidden
    });
    world.entity_mut(selection).insert(Visibility::Hidden);
    world.entity_mut(cursor).insert(Visibility::Hidden);
    for part in [value_part, placeholder, selection, cursor] {
        world.entity_mut(owner).add_child(part);
    }
    world.entity_mut(owner).insert(EditableTextParts {
        value: value_part,
        placeholder,
        selection,
        cursor,
    });
    world.entity_mut(owner).insert(InputFieldOptions {
        clear_on_blur: [
            "clear-on-blur",
            "clear-on-focus-loss",
            "clear_on_focus_loss",
        ]
        .iter()
        .any(|name| crate::component::has_boolean_static_attribute(attributes, name)),
        cap_text_at: cap_text_at.as_deref().and_then(parse_cap),
        cap_to_width: cap_text_at.as_deref() == Some("width"),
        ..Default::default()
    });
    if let Some(label) = static_value(attributes, "label") {
        update_input_option(world, owner, "label", &label);
    }
    if let Some(icon) = static_value(attributes, "icon") {
        update_input_option(world, owner, "icon", &icon);
    }
    if input_type == InputType::Number
        && crate::component::has_boolean_static_attribute(attributes, "show-fields")
    {
        let decrement = number_step_part(world, owner, ControlPartKind::Decrement, "−", -1.0);
        let increment = number_step_part(world, owner, ControlPartKind::Increment, "+", 1.0);
        world
            .entity_mut(owner)
            .add_children(&[decrement, increment]);
    }
}

fn static_value(attributes: &[TemplateAttribute], expected: &str) -> Option<String> {
    attributes.iter().find_map(|attribute| match attribute {
        TemplateAttribute::Static { name, value } if name == expected => Some(value.clone()),
        _ => None,
    })
}

fn input_type(value: Option<&str>) -> InputType {
    match value.unwrap_or("text") {
        "email" => InputType::Email,
        "date" => InputType::Date,
        "range" => InputType::Range,
        "password" => InputType::Password,
        "number" => InputType::Number,
        "file" => InputType::File,
        _ => InputType::Text,
    }
}

fn parse_file_size(value: &str) -> Option<u64> {
    let compact = value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>()
        .to_ascii_uppercase();
    let (number, multiplier) = if let Some(number) = compact.strip_suffix("GB") {
        (number, 1024.0_f64.powi(3))
    } else if let Some(number) = compact.strip_suffix("MB") {
        (number, 1024.0_f64.powi(2))
    } else if let Some(number) = compact.strip_suffix("KB") {
        (number, 1024.0)
    } else if let Some(number) = compact.strip_suffix('B') {
        (number, 1.0)
    } else {
        (compact.as_str(), 1.0)
    };
    let bytes = number.parse::<f64>().ok()? * multiplier;
    (bytes.is_finite() && bytes >= 0.0 && bytes <= u64::MAX as f64).then_some(bytes.round() as u64)
}

#[cfg(test)]
mod tests {
    use super::{number_characters_allowed, number_expression, parse_file_size};

    #[test]
    fn file_size_accepts_legacy_units_and_plain_bytes() {
        assert_eq!(parse_file_size("1 KB"), Some(1024));
        assert_eq!(parse_file_size("1.5mb"), Some(1_572_864));
        assert_eq!(parse_file_size("0.5GB"), Some(536_870_912));
        assert_eq!(parse_file_size("1024"), Some(1024));
        assert_eq!(parse_file_size("-1MB"), None);
        assert_eq!(parse_file_size("NaNMB"), None);
        assert_eq!(parse_file_size("5TB"), None);
    }

    #[test]
    fn number_input_accepts_arithmetic_but_rejects_other_characters() {
        assert_eq!(number_expression("2+3*4"), Some(14.0));
        assert_eq!(number_expression("10%3-2"), Some(-1.0));
        assert_eq!(number_expression("-1.5*2"), Some(-3.0));
        assert_eq!(number_expression("1/0"), None);
        assert_eq!(number_expression("1+"), None);
        assert!(!number_characters_allowed("2a"));
        assert!(!number_characters_allowed("2  +3"));
    }

    #[test]
    fn width_cap_uses_laid_out_text_and_keeps_the_native_editor_in_sync() {
        use bevy::{
            ecs::world::World,
            math::Vec2,
            text::TextLayoutInfo,
            ui::{ComputedNode, Node},
        };
        use tilt_ui_core::TemplateAttribute;
        let mut world = World::new();
        world.init_resource::<bevy::ecs::message::Messages<crate::EditableTextChanged>>();
        let input = world
            .spawn((Node::default(), crate::ElementState::default()))
            .id();
        super::materialize_parts(
            &mut world,
            input,
            &[
                TemplateAttribute::Static {
                    name: "value".into(),
                    value: "abcdefghij".into(),
                },
                TemplateAttribute::Static {
                    name: "cap_text_at".into(),
                    value: "width".into(),
                },
            ],
            false,
        );
        world.entity_mut(input).insert((
            ComputedNode {
                size: Vec2::new(100.0, 30.0),
                ..Default::default()
            },
            TextLayoutInfo {
                size: Vec2::new(200.0, 20.0),
                ..Default::default()
            },
        ));
        super::cap_input_width(&mut world);
        assert_eq!(
            world.get::<crate::EditableText>(input).unwrap().value,
            "abcde"
        );
        assert_eq!(
            world
                .get::<bevy::text::EditableText>(input)
                .unwrap()
                .value(),
            "abcde"
        );
    }

    #[test]
    fn show_fields_creates_two_persistent_number_actions() {
        use bevy::{ecs::world::World, ui::Node};
        use tilt_ui_core::TemplateAttribute;
        let mut world = World::new();
        let input = world
            .spawn((Node::default(), crate::ElementState::default()))
            .id();
        let attrs = [
            TemplateAttribute::Static {
                name: "type".into(),
                value: "number".into(),
            },
            TemplateAttribute::Static {
                name: "show-fields".into(),
                value: "true".into(),
            },
        ];
        super::materialize_parts(&mut world, input, &attrs, false);
        let actions = world
            .get::<bevy::ecs::hierarchy::Children>(input)
            .unwrap()
            .iter()
            .copied()
            .filter(|child| world.get::<super::NumberStepAction>(*child).is_some())
            .collect::<Vec<_>>();
        assert_eq!(actions.len(), 2);
        for action in actions {
            assert!(world.get::<bevy::ui::widget::Text>(action).is_none());
            let label = world.get::<bevy::ecs::hierarchy::Children>(action).unwrap()[0];
            assert!(world.get::<bevy::ui::widget::Text>(label).is_some());
        }
    }

    #[test]
    fn number_actions_apply_step_and_bounds() {
        use bevy::{
            app::{App, Update},
            ecs::{hierarchy::Children, message::Messages},
            ui::Node,
        };
        use tilt_ui_core::TemplateAttribute;

        let mut app = App::new();
        app.add_message::<crate::ControlActivated>()
            .add_message::<crate::EditableTextChanged>()
            .add_systems(Update, super::process_number_activation);
        let input = app
            .world_mut()
            .spawn((Node::default(), crate::ElementState::default()))
            .id();
        let attrs = [
            ("type", "number"),
            ("value", "2"),
            ("min", "0"),
            ("max", "3"),
            ("step", "1"),
            ("show-fields", "true"),
        ]
        .map(|(name, value)| TemplateAttribute::Static {
            name: name.into(),
            value: value.into(),
        });
        super::materialize_parts(app.world_mut(), input, &attrs, false);
        let actions = app
            .world()
            .get::<Children>(input)
            .unwrap()
            .iter()
            .filter_map(|child| {
                app.world()
                    .get::<super::NumberStepAction>(*child)
                    .map(|action| (*child, action.direction))
            })
            .collect::<Vec<_>>();
        let increment = actions
            .iter()
            .find(|(_, direction)| *direction > 0.0)
            .unwrap()
            .0;
        let decrement = actions
            .iter()
            .find(|(_, direction)| *direction < 0.0)
            .unwrap()
            .0;

        for (action, expected) in [(increment, "3"), (increment, "3"), (decrement, "2")] {
            app.world_mut()
                .resource_mut::<Messages<crate::ControlActivated>>()
                .write(crate::ControlActivated { entity: action });
            app.update();
            assert_eq!(
                app.world().get::<crate::EditableText>(input).unwrap().value,
                expected
            );
        }
    }

    #[test]
    fn decrement_crosses_zero_without_an_explicit_minimum() {
        use bevy::{
            app::{App, Update},
            ecs::{hierarchy::Children, message::Messages},
            ui::Node,
        };
        use tilt_ui_core::TemplateAttribute;

        let mut app = App::new();
        app.add_message::<crate::ControlActivated>()
            .add_message::<crate::EditableTextChanged>()
            .add_systems(Update, super::process_number_activation);
        let input = app
            .world_mut()
            .spawn((Node::default(), crate::ElementState::default()))
            .id();
        let attrs =
            [("type", "number"), ("value", "0"), ("show-fields", "true")].map(|(name, value)| {
                TemplateAttribute::Static {
                    name: name.into(),
                    value: value.into(),
                }
            });
        super::materialize_parts(app.world_mut(), input, &attrs, false);
        let decrement = app
            .world()
            .get::<Children>(input)
            .unwrap()
            .iter()
            .find(|child| {
                app.world()
                    .get::<super::NumberStepAction>(**child)
                    .is_some_and(|action| action.direction < 0.0)
            })
            .copied()
            .unwrap();

        for expected in ["-1", "-2", "-3"] {
            app.world_mut()
                .resource_mut::<Messages<crate::ControlActivated>>()
                .write(crate::ControlActivated { entity: decrement });
            app.update();
            assert_eq!(
                app.world().get::<crate::EditableText>(input).unwrap().value,
                expected
            );
        }
    }

    #[test]
    fn number_bounds_and_pattern_are_validated() {
        use crate::widgets::state::{EditableTextOptions, editable_invalid};
        use tilt_ui_core::InputType;
        let options = EditableTextOptions {
            min: Some("2".into()),
            max: Some("8".into()),
            step: Some("2".into()),
            ..Default::default()
        };
        assert!(!editable_invalid("2+4", InputType::Number, &options));
        assert!(editable_invalid("3", InputType::Number, &options));
        assert!(editable_invalid("10", InputType::Number, &options));
        let pattern = EditableTextOptions {
            pattern: Some("[A-Z]{2}[0-9]{2}".into()),
            ..Default::default()
        };
        assert!(!editable_invalid("AB12", InputType::Text, &pattern));
        assert!(editable_invalid("AB12x", InputType::Text, &pattern));
    }
}
