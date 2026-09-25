//! Runtime semantics specific to the TiltUI input element.

use bevy::ecs::{component::Component, message::Message};
use bevy::{
    color::Color,
    ecs::{entity::Entity, world::World},
    prelude::Visibility,
    text::{EditableText as BevyEditableText, LineBreak, TextCursorStyle, TextEdit, TextLayout},
    ui::{
        ContentSize,
        widget::{TextNodeFlags, TextScroll},
    },
};
use tilt_ui_core::{InputType, TemplateAttribute};

use crate::{ControlPartKind, EditableText, widgets::state::EditableTextParts};

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

/// Stores a successfully selected native file or directory.
#[derive(Component, Debug, Clone)]
pub struct FileInputSelection {
    /// User-visible filename.
    pub name: String,
    /// File size in bytes when known.
    pub size_bytes: Option<u64>,
    /// Native filesystem path, unavailable on browser targets.
    pub native_path: Option<std::path::PathBuf>,
}

/// Notification emitted when a file Input completes user selection.
#[derive(Message, Debug, Clone)]
pub struct FileInputSelected {
    /// File Input entity.
    pub entity: Entity,
    /// Selected metadata.
    pub selection: FileInputSelection,
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
        world.entity_mut(owner).insert(FileInputOptions {
            folder: crate::component::has_boolean_static_attribute(attributes, "folder"),
            extensions: static_value(attributes, "extensions").map_or_else(Vec::new, |raw| {
                raw.trim_matches(|c| c == '[' || c == ']')
                    .split(',')
                    .map(|value| value.trim().trim_start_matches('.').to_owned())
                    .filter(|value| !value.is_empty())
                    .collect()
            }),
            show_size: crate::component::has_boolean_static_attribute(attributes, "show-size"),
            max_size_bytes: static_value(attributes, "max-size")
                .and_then(|value| parse_file_size(&value)),
        });
    }
    let readonly = crate::component::has_boolean_static_attribute(attributes, "readonly");
    let options = super::super::state::EditableTextOptions {
        name: static_value(attributes, "name"),
        required: crate::component::has_boolean_static_attribute(attributes, "required"),
        min_length: static_value(attributes, "minlength").and_then(|value| value.parse().ok()),
        max_characters: static_value(attributes, "maxlength").and_then(|value| value.parse().ok()),
        max_lines: static_value(attributes, "max-lines").and_then(|value| value.parse().ok()),
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
    native.max_characters = options.max_characters;
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
    use super::parse_file_size;

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
}
