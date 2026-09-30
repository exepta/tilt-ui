//! Applies reactive property expressions and text interpolation to materialized UI.

use bevy::{
    ecs::{
        component::Component,
        entity::Entity,
        hierarchy::{ChildOf, Children},
        query::{Added, Changed, Or, Without},
        resource::Resource,
        world::World,
    },
    ui::{InteractionDisabled, widget::Text},
};
use serde_json::Value;
use std::collections::BTreeMap;

use crate::{ElementState, PropertyBindings, TiltText};

use super::binding::{UiBindingStore, UiSharedValues};
use super::element::BoundClasses;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
struct AppliedPropertyRevision(u64, u64, u64, u64);

#[derive(Component, Clone, Copy, PartialEq, Eq)]
struct AppliedTextRevision(u64, u64, u64, u64);

#[derive(Resource, Default)]
struct LastBindingRevision(Option<(u64, u64, u64, u64)>);

/// Lexical values introduced by @let and @for on a layout-neutral ancestor.
#[derive(Component, Clone, Default)]
pub(crate) struct LocalValues(pub BTreeMap<String, Value>);

pub(crate) fn apply_bindings(world: &mut World) {
    #[cfg(feature = "fluent")]
    let fluent_revision = (
        world
            .get_resource::<crate::UiLocalization>()
            .map_or(0, |locale| locale.revision()),
        world
            .get_resource::<crate::UiFluentArgs>()
            .map_or(0, |args| args.revision()),
    );
    #[cfg(not(feature = "fluent"))]
    let fluent_revision = (0, 0);
    let revision = (
        world.resource::<UiBindingStore>().revision(),
        world.resource::<UiSharedValues>().revision(),
        fluent_revision.0,
        fluent_revision.1,
    );
    let revision_changed = world
        .get_resource::<LastBindingRevision>()
        .is_none_or(|last| last.0 != Some(revision));
    let properties = if revision_changed {
        let mut query =
            world.query::<(Entity, &PropertyBindings, Option<&AppliedPropertyRevision>)>();
        query
            .iter(world)
            .filter(|(_, _, applied)| {
                applied.map(|applied| (applied.0, applied.1, applied.2, applied.3))
                    != Some(revision)
            })
            .map(|(entity, bindings, _)| (entity, bindings.bindings.clone()))
            .collect::<Vec<_>>()
    } else {
        let mut query =
            world.query_filtered::<(Entity, &PropertyBindings), Changed<PropertyBindings>>();
        query
            .iter(world)
            .map(|(entity, bindings)| (entity, bindings.bindings.clone()))
            .collect::<Vec<_>>()
    };
    for (entity, mut bindings) in properties {
        if world.get_entity(entity).is_err() {
            continue;
        }
        // Anatomy and the first range endpoint must exist before dependent values.
        bindings.sort_by_key(|binding| match binding.name.as_str() {
            "type" => 0,
            "range-start" => 1,
            "range-end" => 2,
            _ => 3,
        });
        for binding in bindings {
            let value = resolve(world, entity, &binding.expression);
            if let Some(value) = value {
                apply_property(world, entity, &binding.name, value);
            }
        }
        world.entity_mut(entity).insert(AppliedPropertyRevision(
            revision.0, revision.1, revision.2, revision.3,
        ));
    }
    let texts = if revision_changed {
        let mut query = world.query_filtered::<(Entity, &TiltText, Option<&AppliedTextRevision>), Without<super::content::LiteralInnerText>>();
        query
            .iter(world)
            .filter(|(_, text, applied)| {
                text.value.contains("{{")
                    && applied.map(|applied| (applied.0, applied.1, applied.2, applied.3))
                        != Some(revision)
            })
            .map(|(entity, text, _)| (entity, text.value.clone()))
            .collect::<Vec<_>>()
    } else {
        let mut query = world.query_filtered::<(Entity, &TiltText), (
            Or<(Changed<TiltText>, Added<Text>)>,
            Without<super::content::LiteralInnerText>,
        )>();
        query
            .iter(world)
            .filter(|(_, text)| text.value.contains("{{"))
            .map(|(entity, text)| (entity, text.value.clone()))
            .collect::<Vec<_>>()
    };
    let mut translated_options = Vec::new();
    for (entity, source) in texts {
        let value = interpolate(world, entity, &source);
        if let Some(mut text) = world.get_mut::<Text>(entity) {
            if text.0 != value {
                text.0 = value;
            }
        }
        world.entity_mut(entity).insert(AppliedTextRevision(
            revision.0, revision.1, revision.2, revision.3,
        ));
        let mut parent = world.get::<ChildOf>(entity).map(ChildOf::parent);
        while let Some(ancestor) = parent {
            if world.get::<crate::OptionData>(ancestor).is_some() {
                if !translated_options.contains(&ancestor) {
                    translated_options.push(ancestor);
                }
                break;
            }
            parent = world.get::<ChildOf>(ancestor).map(ChildOf::parent);
        }
    }
    for option in translated_options {
        let label = world
            .get::<Children>(option)
            .map(|children| {
                children
                    .iter()
                    .filter_map(|child| world.get::<Text>(*child))
                    .map(|text| text.0.trim())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();
        if let Some(mut data) = world.get_mut::<crate::OptionData>(option) {
            data.label = label;
        }
        let popup = world.get::<ChildOf>(option).map(ChildOf::parent);
        let choice = popup.and_then(|popup| world.get::<ChildOf>(popup).map(ChildOf::parent));
        if let Some(choice) = choice
            && world.get::<crate::ChoiceBoxParts>(choice).is_some()
        {
            crate::widgets::controls::choice_box::refresh_value(world, choice);
        }
    }
    if revision_changed {
        world.insert_resource(LastBindingRevision(Some(revision)));
    }
}

pub(crate) fn resolve(world: &World, entity: Entity, expression: &str) -> Option<Value> {
    resolve_with_locals(world, entity, expression, &BTreeMap::new())
}

pub(crate) fn resolve_with_locals(
    world: &World,
    entity: Entity,
    expression: &str,
    locals: &BTreeMap<String, Value>,
) -> Option<Value> {
    resolve_with_context(world, entity, expression, locals, None)
}

pub(crate) fn resolve_with_event(
    world: &World,
    entity: Entity,
    expression: &str,
    event: &Value,
) -> Option<Value> {
    resolve_with_context(world, entity, expression, &BTreeMap::new(), Some(event))
}

fn resolve_with_context(
    world: &World,
    entity: Entity,
    expression: &str,
    locals: &BTreeMap<String, Value>,
    event: Option<&Value>,
) -> Option<Value> {
    let expression = expression.trim();
    if expression.len() > super::expression::MAX_SOURCE_BYTES {
        return None;
    }
    #[cfg(feature = "fluent")]
    if let Some(key) = expression.strip_prefix("i18n.").filter(|key| {
        key.chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    }) {
        let localization = world.get_resource::<crate::UiLocalization>()?;
        let args = world
            .get_resource::<crate::UiFluentArgs>()
            .and_then(|values| values.for_message(key));
        return Some(Value::String(
            localization
                .translate(key, args.as_ref())
                .unwrap_or_else(|| key.to_owned()),
        ));
    }
    // Most showcase bindings are simple paths and update every frame. Resolve
    // them without tokenizing an expression on each shared-state revision.
    if let Some((root, path)) = simple_path(expression)
        && !matches!(root, "true" | "false" | "null")
    {
        let mut value = locals
            .get(root)
            .cloned()
            .or_else(|| lookup_root(world, entity, root))?;
        for segment in path {
            value = match value {
                Value::Object(map) => map.get(segment)?.clone(),
                Value::Array(items) if segment == "length" => Value::from(items.len()),
                Value::Array(items) => items.get(segment.parse::<usize>().ok()?)?.clone(),
                Value::String(text) if segment == "length" => Value::from(text.chars().count()),
                _ => return None,
            };
        }
        return Some(value);
    }
    let component = world
        .get::<crate::ComponentStyleOwner>(entity)
        .and_then(|owner| world.get::<super::ComponentInstance>(owner.0))
        .and_then(|instance| {
            world
                .get_resource::<super::ComponentCatalog>()?
                .component_metadata(instance.component)
        })
        .map(|metadata| metadata.name);
    super::expression::evaluate_with_methods(
        expression,
        |root| {
            if root == "$event" {
                return event.cloned();
            }
            locals
                .get(root)
                .cloned()
                .or_else(|| lookup_root(world, entity, root))
        },
        |path, receiver, arguments| {
            world
                .get_resource::<super::UiExpressionMethods>()?
                .evaluate(component?, path, receiver, arguments)
        },
    )
}

fn simple_path(expression: &str) -> Option<(&str, impl Iterator<Item = &str>)> {
    if expression.bytes().filter(|ch| *ch == b'.').count() * 2 + 1 > super::expression::MAX_TOKENS {
        return None;
    }
    let mut segments = expression.split('.');
    let root = segments.next()?;
    if !root.starts_with(|ch: char| ch.is_ascii_alphabetic() || ch == '_')
        || !root
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        || !segments.clone().all(|segment| {
            !segment.is_empty()
                && segment
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        })
    {
        return None;
    }
    Some((root, segments))
}

fn lookup_root(world: &World, entity: Entity, root: &str) -> Option<Value> {
    let mut current = Some(entity);
    while let Some(node) = current {
        if let Some(value) = world
            .get::<LocalValues>(node)
            .and_then(|values| values.0.get(root))
        {
            return Some(value.clone());
        }
        current = world
            .get::<super::content::TemplateParent>(node)
            .map(|parent| parent.0)
            .or_else(|| world.get::<ChildOf>(node).map(ChildOf::parent));
    }
    if let Some(value) = world
        .resource::<UiBindingStore>()
        .json(root)
        .or_else(|| world.resource::<UiSharedValues>().get(root))
    {
        return Some(value.clone());
    }
    let owner = world.get::<crate::ComponentStyleOwner>(entity)?.0;
    let shared = world.resource::<UiSharedValues>();
    let mut current = Some(entity);
    while let Some(node) = current {
        if let Some(imports) = world.get::<super::TemplateImports>(node) {
            for import in &imports.0 {
                let target = import.target.rsplit("::").next().unwrap_or(&import.target);
                let Some(value) = shared.get(&import.target).or_else(|| shared.get(target)) else {
                    continue;
                };
                if import.wildcard {
                    if let Value::Object(fields) = value {
                        if let Some(field) = fields.get(root) {
                            return Some(field.clone());
                        }
                    }
                } else if import.alias == root {
                    return Some(value.clone());
                }
            }
        }
        if node == owner {
            break;
        }
        current = world
            .get::<super::content::TemplateParent>(node)
            .map(|parent| parent.0)
            .or_else(|| world.get::<ChildOf>(node).map(ChildOf::parent));
    }
    None
}

fn interpolate(world: &World, entity: Entity, source: &str) -> String {
    let mut output = String::new();
    let mut rest = source;
    while let Some(start) = rest.find("{{") {
        output.push_str(&rest[..start]);
        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find("}}") else {
            output.push_str(&rest[start..]);
            return output;
        };
        let expression = &after_start[..end];
        if let Some(value) = resolve(world, entity, expression) {
            output.push_str(&value_text(&value));
        }
        rest = &after_start[end + 2..];
    }
    output.push_str(rest);
    output
}

fn value_text(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Null => String::new(),
        _ => value.to_string(),
    }
}

fn apply_property(world: &mut World, entity: Entity, name: &str, value: Value) {
    match name {
        "loading" => {
            crate::widgets::controls::button::set_button_loading(
                world,
                entity,
                value.as_bool().unwrap_or(false),
            );
        }
        "disabled" => {
            let disabled = value.as_bool().unwrap_or(false);
            if disabled && world.get::<InteractionDisabled>(entity).is_none() {
                world.entity_mut(entity).insert(InteractionDisabled);
            } else if !disabled && world.get::<InteractionDisabled>(entity).is_some() {
                world.entity_mut(entity).remove::<InteractionDisabled>();
            }
            if world
                .get::<ElementState>(entity)
                .is_some_and(|state| state.disabled != disabled)
            {
                world.get_mut::<ElementState>(entity).unwrap().disabled = disabled;
            }
        }
        "checked" | "selected" => {
            let checked = value.as_bool().unwrap_or(false);
            if world.get::<crate::OptionData>(entity).is_some() {
                crate::set_option_selected(world, entity, checked);
            } else {
                crate::set_control_checked(world, entity, checked);
            }
        }
        "value" => {
            if let Some(state) = world.get::<crate::DatePickerState>(entity) {
                if state.range {
                    let pair = if value.is_null() || value_text(&value).is_empty() {
                        Some((None, None))
                    } else if let Some(object) = value.as_object() {
                        object
                            .get("start")
                            .and_then(Value::as_str)
                            .and_then(crate::IsoDate::parse)
                            .map(|start| {
                                (
                                    Some(start),
                                    object
                                        .get("end")
                                        .and_then(Value::as_str)
                                        .and_then(crate::IsoDate::parse),
                                )
                            })
                    } else {
                        crate::widgets::advanced::date_picker::parse_range(&value_text(&value))
                            .map(|(start, end)| (Some(start), end))
                    };
                    if let Some((start, end)) = pair {
                        crate::set_date_range(world, entity, start, end);
                    }
                } else {
                    let date = if value.is_null() || value_text(&value).is_empty() {
                        Some(None)
                    } else {
                        crate::IsoDate::parse(&value_text(&value)).map(Some)
                    };
                    if let Some(date) = date {
                        crate::set_date_value(world, entity, date);
                    }
                }
            } else if world
                .get::<crate::widgets::advanced::color_picker::ColorPickerState>(entity)
                .is_some()
            {
                if let Ok(color) = tilt_ui_css::parse_color_value(&value_text(&value)) {
                    crate::set_color_value(world, entity, color);
                }
            } else if world.get::<crate::OptionData>(entity).is_some() {
                let next = value_text(&value);
                if world
                    .get::<crate::OptionData>(entity)
                    .is_some_and(|option| option.value != next)
                {
                    world.get_mut::<crate::OptionData>(entity).unwrap().value = next;
                }
            } else if world.get::<crate::EditableText>(entity).is_some() {
                crate::set_editable_text(world, entity, value_text(&value));
            } else if let Some(number) = numeric_value(&value) {
                if world
                    .get::<crate::widgets::content::badge::BadgeValue>(entity)
                    .is_some()
                {
                    crate::set_badge_value(world, entity, number.max(0.0) as u32);
                } else if (number as f32).is_finite() {
                    crate::set_numeric_value(world, entity, number as f32);
                }
            }
        }
        "src" => {
            let source = (!value.is_null())
                .then(|| value_text(&value))
                .filter(|source| !source.trim().is_empty());
            if world.get::<crate::AvatarFallback>(entity).is_some() {
                crate::set_avatar_source(world, entity, source);
            } else {
                crate::set_image_source(world, entity, source);
            }
        }
        "alt" => {
            let next = (!value.is_null()).then(|| value_text(&value));
            if world
                .get::<crate::ImageMetadata>(entity)
                .is_some_and(|metadata| metadata.alt != next)
            {
                world.get_mut::<crate::ImageMetadata>(entity).unwrap().alt = next;
            }
            if let Some(fallback) = world.get::<crate::AvatarFallback>(entity).copied() {
                let initials = value_text(&value)
                    .split_whitespace()
                    .filter_map(|word| word.chars().next())
                    .take(2)
                    .collect::<String>()
                    .to_uppercase();
                if world
                    .get::<Text>(fallback.0)
                    .is_some_and(|text| text.0 != initials)
                {
                    world.get_mut::<Text>(fallback.0).unwrap().0 = initials;
                }
            }
        }
        "placeholder" => {
            if let Some(parts) = world
                .get::<crate::widgets::state::EditableTextParts>(entity)
                .copied()
            {
                let next = value_text(&value);
                if world
                    .get::<Text>(parts.placeholder)
                    .is_some_and(|text| text.0 != next)
                {
                    world.get_mut::<Text>(parts.placeholder).unwrap().0 = next;
                }
            }
        }
        "title" => {
            crate::widgets::advanced::dialog::set_dialog_title(world, entity, value_text(&value));
        }
        "href" => {
            crate::set_link_href(world, entity, value_text(&value));
        }
        "class" => {
            let mut classes = world
                .get::<BoundClasses>(entity)
                .cloned()
                .unwrap_or_default();
            classes.dynamic = value_text(&value)
                .split_ascii_whitespace()
                .map(str::to_owned)
                .collect();
            set_bound_classes(world, entity, classes);
        }
        name if name.starts_with("class.") => {
            let class = &name[6..];
            if class.is_empty() || class.chars().any(char::is_whitespace) {
                return;
            }
            let mut classes = world
                .get::<BoundClasses>(entity)
                .cloned()
                .unwrap_or_default();
            classes
                .toggles
                .insert(class.to_owned(), value.as_bool().unwrap_or(false));
            set_bound_classes(world, entity, classes);
        }
        "readonly" => {
            crate::set_editable_readonly(world, entity, value.as_bool().unwrap_or(false));
        }
        "required" | "minlength" | "maxlength" | "max-lines" | "name" => {
            apply_editable_option(world, entity, name, &value);
        }
        "type"
        | "orientation"
        | "alignment"
        | "dots"
        | "show-tip"
        | "show-labels"
        | "dot-anchor"
        | "show-fields"
        | "folder"
        | "extensions"
        | "show-size"
        | "max-size"
        | "for"
        | "trigger"
        | "triggger"
        | "variant"
        | "prio"
        | "priority"
        | "action"
        | "validate"
        | "renderer"
        | "animated"
        | "layout"
        | "label"
        | "icon"
        | "clear-on-blur"
        | "clear-on-focus-loss"
        | "clear_on_focus_loss"
        | "cap-text-at"
        | "cap_text_at" => {
            let next = value_text(&value);
            if world
                .get::<crate::StaticAttributes>(entity)
                .is_some_and(|attrs| {
                    attrs
                        .attributes
                        .iter()
                        .any(|attr| attr.name == name && attr.value == next)
                })
            {
                return;
            }
            if world.get::<crate::SliderSettings>(entity).is_some() {
                crate::widgets::controls::slider::reconfigure(world, entity, name, &next);
            } else if world.get::<crate::EditableText>(entity).is_some() {
                crate::widgets::controls::input::reconfigure_input(world, entity, name, &next);
                crate::widgets::controls::input::update_input_option(world, entity, name, &next);
            } else if world.get::<crate::TooltipSettings>(entity).is_some() {
                crate::widgets::advanced::tooltip::reconfigure(world, entity, name, &next);
            } else if world.get::<crate::DialogState>(entity).is_some() {
                crate::widgets::advanced::dialog::reconfigure(world, entity, name, &next);
            } else if let Some(mut settings) = world.get_mut::<crate::FormSettings>(entity) {
                match name {
                    "action" => settings.action = (!next.is_empty()).then_some(next.clone()),
                    "validate" => {
                        settings.validation = match next.as_str() {
                            "always" | "allways" | "all" => {
                                tilt_ui_core::FormValidationMode::Always
                            }
                            "interact" => tilt_ui_core::FormValidationMode::Interact,
                            _ => tilt_ui_core::FormValidationMode::Send,
                        }
                    }
                    _ => return,
                }
            } else if name == "type" {
                if let Some(mut button) = world.get_mut::<crate::FormButton>(entity) {
                    button.0 = match next.as_str() {
                        "button" => tilt_ui_core::ButtonType::Button,
                        "submit" => tilt_ui_core::ButtonType::Submit,
                        "reset" => tilt_ui_core::ButtonType::Reset,
                        _ => tilt_ui_core::ButtonType::Auto,
                    };
                } else {
                    return;
                }
            } else {
                return;
            }
            set_bound_attribute(world, entity, name, &next);
        }
        "min" | "max" | "step" => {
            if let Some(number) = finite_f32(&value) {
                crate::widgets::state::set_numeric_bound(world, entity, name, number);
            }
        }
        "range-start" | "range-end" => {
            if let Some(state) = world
                .get::<crate::DatePickerState>(entity)
                .filter(|state| state.range)
            {
                let date = if value.is_null() || value_text(&value).is_empty() {
                    Some(None)
                } else {
                    crate::IsoDate::parse(&value_text(&value)).map(Some)
                };
                if let Some(date) = date {
                    let (start, end) = if name == "range-start" {
                        (date, state.range_end)
                    } else {
                        (state.range_start, date)
                    };
                    crate::set_date_range(world, entity, start, end);
                }
            } else if let (Some(number), Some(settings)) = (
                finite_f32(&value),
                world.get::<crate::SliderSettings>(entity).copied(),
            ) {
                if name == "range-start" {
                    crate::set_slider_values(world, entity, number, settings.upper);
                } else {
                    crate::set_slider_values(world, entity, settings.lower, number);
                }
            }
        }
        "open" => {
            let open = value.as_bool().unwrap_or(false);
            if world.get::<crate::ChoiceBoxParts>(entity).is_some() {
                crate::set_choice_open(world, entity, open);
            } else if world.get::<crate::DatePickerState>(entity).is_some() {
                crate::set_date_picker_open(world, entity, open);
            } else if world.get::<crate::ColorPickerState>(entity).is_some() {
                crate::set_color_picker_open(world, entity, open);
            } else if world.get::<crate::DialogState>(entity).is_some() {
                if open {
                    crate::open_dialog(world, entity);
                } else {
                    crate::close_dialog(world, entity, crate::DialogResult::Closed);
                }
            } else if world.get::<crate::TooltipSettings>(entity).is_some() {
                crate::widgets::advanced::tooltip::set_bound_open(world, entity, open);
            }
        }
        "style" => {
            let dynamic_source = value_text(&value);
            if world
                .get::<crate::style::InlineStyle>(entity)
                .is_some_and(|old| old.dynamic_source == dynamic_source)
            {
                return;
            }
            let static_source = world
                .get::<crate::style::InlineStyle>(entity)
                .map_or("", |old| old.static_source.as_str());
            match crate::style::InlineStyle::from_parts(static_source, &dynamic_source) {
                Ok(style) => {
                    world.entity_mut(entity).insert(style);
                }
                Err(error) => bevy::log::warn!("Invalid bound inline style: {error}"),
            }
        }
        "innerHtml" | "innerHTML" => {
            if let Err(error) = super::set_inner_html(world, entity, value_text(&value)) {
                bevy::log::warn!("Could not apply {name}: {error}");
            }
        }
        "text" | "innerText" | "textContent" => {
            if let Some(mut text) = world.get_mut::<Text>(entity) {
                text.0 = value_text(&value);
            } else if super::set_inner_text(world, entity, value_text(&value)).is_err() {
                // Retain the legacy label binding for specialized controls whose
                // generated anatomy cannot be replaced by the content setters.
                let target = world.get::<Children>(entity).and_then(|children| {
                    children
                        .iter()
                        .copied()
                        .find(|child| world.get::<Text>(*child).is_some())
                });
                if let Some(target) = target {
                    world.get_mut::<Text>(target).unwrap().0 = value_text(&value);
                }
            }
        }
        _ => {}
    }
}

fn apply_editable_option(world: &mut World, entity: Entity, name: &str, value: &Value) {
    let Some(mut options) = world.get::<crate::EditableTextOptions>(entity).cloned() else {
        return;
    };
    let old = options.clone();
    match name {
        "required" => options.required = value.as_bool().unwrap_or(false),
        "minlength" => options.min_length = usize_value(value),
        "maxlength" => options.max_characters = usize_value(value),
        "max-lines" => options.max_lines = usize_value(value),
        "name" => options.name = (!value.is_null()).then(|| value_text(value)),
        _ => return,
    }
    if options == old {
        return;
    }
    world.entity_mut(entity).insert(options);
    if name == "maxlength" {
        let cap = world
            .get::<crate::widgets::controls::input::InputFieldOptions>(entity)
            .and_then(|options| options.cap_text_at);
        let max_characters = cap.or_else(|| {
            world
                .get::<crate::EditableTextOptions>(entity)
                .and_then(|options| options.max_characters)
        });
        if let Some(mut native) = world.get_mut::<bevy::text::EditableText>(entity) {
            native.max_characters = max_characters;
        }
    }
    let Some(editable) = world.get::<crate::EditableText>(entity) else {
        return;
    };
    let invalid = crate::widgets::state::editable_invalid(
        &editable.value,
        editable.input_type,
        world.get::<crate::EditableTextOptions>(entity).unwrap(),
    );
    if let Some(mut state) = world.get_mut::<ElementState>(entity) {
        state.invalid = invalid;
    }
}

fn set_bound_attribute(world: &mut World, entity: Entity, name: &str, value: &str) {
    let mut attrs = world
        .get::<crate::StaticAttributes>(entity)
        .cloned()
        .unwrap_or_default();
    if let Some(attr) = attrs.attributes.iter_mut().find(|attr| attr.name == name) {
        if attr.value == value {
            return;
        }
        attr.value = value.to_owned();
    } else {
        attrs.attributes.push(crate::StaticAttribute {
            name: name.to_owned(),
            value: value.to_owned(),
        });
    }
    world.entity_mut(entity).insert(attrs);
}

fn set_bound_classes(world: &mut World, entity: Entity, classes: BoundClasses) {
    if world.get::<BoundClasses>(entity) == Some(&classes) {
        return;
    }
    let combined = classes.combined();
    world.entity_mut(entity).insert(classes);
    if world.get::<crate::ElementClasses>(entity) != Some(&combined) {
        world.entity_mut(entity).insert(combined);
    }
}

fn numeric_value(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
    .filter(|number: &f64| number.is_finite())
}

fn finite_f32(value: &Value) -> Option<f32> {
    let converted = numeric_value(value)? as f32;
    converted.is_finite().then_some(converted)
}

fn usize_value(value: &Value) -> Option<usize> {
    match value {
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
    .and_then(|number| usize::try_from(number).ok())
}

#[cfg(test)]
mod tests {
    use bevy::{
        ecs::world::World,
        ui::{InteractionDisabled, widget::Text},
    };
    use serde::Serialize;
    use tilt_ui_core::{ComponentId, ComponentKind, ComponentMetadata};

    use super::*;
    use crate::component::UiStore;
    use crate::{PropertyBinding, PropertyBindings};

    static PROFILE_COMPONENT: [ComponentMetadata; 1] = [ComponentMetadata {
        id: ComponentId(0),
        name: "profile",
        kind: ComponentKind::Page,
        template_asset_path: "",
        stylesheet_asset_path: "",
        stylesheet_asset_paths: &[],
    }];

    fn profile_id(name: &str) -> Option<ComponentId> {
        (name == "profile").then_some(ComponentId(0))
    }

    fn profile_metadata(id: ComponentId) -> Option<&'static ComponentMetadata> {
        (id == ComponentId(0)).then_some(&PROFILE_COMPONENT[0])
    }

    fn upper_title(receiver: &Value, arguments: &[Value]) -> Option<Value> {
        if !arguments.is_empty() {
            return None;
        }
        Some(Value::String(
            receiver.get("title")?.as_str()?.to_uppercase(),
        ))
    }

    #[derive(Serialize)]
    struct State {
        title: String,
        enabled: bool,
    }
    impl UiStore for State {
        const STORE_KEY: &'static str = "State";
        const STORE_PATH: &'static str = "tests::State";
    }

    #[test]
    fn property_paths_and_text_interpolation_follow_store_updates() {
        let mut world = World::new();
        let mut store = UiBindingStore::default();
        store.set_store(State {
            title: "First".into(),
            enabled: true,
        });
        world.insert_resource(store);
        world.init_resource::<UiSharedValues>();
        let text = world
            .spawn((
                TiltText {
                    value: "Title: {{ state.title }}".into(),
                },
                Text::new("Title: {{ state.title }}"),
            ))
            .id();
        apply_bindings(&mut world);
        assert_eq!(world.get::<Text>(text).unwrap().0, "Title: First");
        world.resource_mut::<UiBindingStore>().set_store(State {
            title: "Second".into(),
            enabled: false,
        });
        apply_bindings(&mut world);
        assert_eq!(world.get::<Text>(text).unwrap().0, "Title: Second");
        assert_eq!(
            resolve(&world, text, "state.title.length"),
            Some(Value::from(6))
        );
    }

    #[test]
    fn component_method_binding_tracks_store_changes_and_scope() {
        let mut world = World::new();
        let mut store = UiBindingStore::default();
        store.set_store(State {
            title: "First".into(),
            enabled: true,
        });
        world.insert_resource(store);
        world.init_resource::<UiSharedValues>();
        world.insert_resource(super::super::ComponentCatalog::new(
            &PROFILE_COMPONENT,
            profile_id,
            profile_metadata,
        ));
        let mut methods = super::super::UiExpressionMethods::default();
        assert!(methods.register("profile", "state.upper_title", upper_title));
        assert!(methods.register("other", "state.other_title", upper_title));
        assert!(!methods.register("profile", "state.upper_title", upper_title));
        world.insert_resource(methods);
        let owner = world
            .spawn(super::super::ComponentInstance {
                component: ComponentId(0),
            })
            .id();
        let text = world
            .spawn((
                crate::ComponentStyleOwner(owner),
                TiltText {
                    value: "{{ state.upper_title() }}".into(),
                },
                Text::new(""),
            ))
            .id();

        apply_bindings(&mut world);
        assert_eq!(world.get::<Text>(text).unwrap().0, "FIRST");
        assert_eq!(resolve(&world, text, "state.missing()"), None);
        assert_eq!(resolve(&world, text, "state.other_title()"), None);
        world.resource_mut::<UiBindingStore>().set_store(State {
            title: "Second".into(),
            enabled: true,
        });
        apply_bindings(&mut world);
        assert_eq!(world.get::<Text>(text).unwrap().0, "SECOND");
    }

    #[test]
    fn disabled_property_tracks_a_boolean_store_field() {
        let mut world = World::new();
        let mut store = UiBindingStore::default();
        store.set_store(State {
            title: String::new(),
            enabled: true,
        });
        world.insert_resource(store);
        world.init_resource::<UiSharedValues>();
        let control = world
            .spawn(PropertyBindings {
                bindings: vec![PropertyBinding {
                    name: "disabled".into(),
                    expression: "state.enabled".into(),
                }],
            })
            .id();
        apply_bindings(&mut world);
        assert!(world.get::<InteractionDisabled>(control).is_some());
        world.resource_mut::<UiBindingStore>().set_store(State {
            title: String::new(),
            enabled: false,
        });
        apply_bindings(&mut world);
        assert!(world.get::<InteractionDisabled>(control).is_none());
    }

    #[test]
    fn loading_property_tracks_a_boolean_store_field() {
        let mut world = World::new();
        let mut store = UiBindingStore::default();
        store.set_store(State {
            title: String::new(),
            enabled: true,
        });
        world.insert_resource(store);
        world.init_resource::<UiSharedValues>();
        let spinner = world.spawn_empty().id();
        let button = world
            .spawn((
                crate::LoadingButton {
                    active: false,
                    spinner,
                },
                ElementState::default(),
                PropertyBindings {
                    bindings: vec![PropertyBinding {
                        name: "loading".into(),
                        expression: "state.enabled".into(),
                    }],
                },
            ))
            .id();

        apply_bindings(&mut world);
        assert!(world.get::<crate::LoadingButton>(button).unwrap().active);
        assert!(world.get::<ElementState>(button).unwrap().loading);
        world.resource_mut::<UiBindingStore>().set_store(State {
            title: String::new(),
            enabled: false,
        });
        apply_bindings(&mut world);
        assert!(!world.get::<crate::LoadingButton>(button).unwrap().active);
        assert!(!world.get::<ElementState>(button).unwrap().loading);
    }

    #[test]
    fn dynamic_classes_preserve_static_classes_and_toggle_individual_names() {
        let mut world = World::new();
        let mut store = UiBindingStore::default();
        store.set_store(State {
            title: "accent".into(),
            enabled: true,
        });
        world.insert_resource(store);
        world.init_resource::<UiSharedValues>();
        let entity = world
            .spawn((
                crate::ElementClasses {
                    classes: vec!["base".into()],
                },
                BoundClasses {
                    base: vec!["base".into()],
                    ..Default::default()
                },
                PropertyBindings {
                    bindings: vec![
                        PropertyBinding {
                            name: "class".into(),
                            expression: "state.title".into(),
                        },
                        PropertyBinding {
                            name: "class.active".into(),
                            expression: "state.enabled".into(),
                        },
                    ],
                },
            ))
            .id();
        apply_bindings(&mut world);
        assert_eq!(
            world.get::<crate::ElementClasses>(entity).unwrap().classes,
            ["base", "accent", "active"]
        );
        world.resource_mut::<UiBindingStore>().set_store(State {
            title: "muted".into(),
            enabled: false,
        });
        apply_bindings(&mut world);
        assert_eq!(
            world.get::<crate::ElementClasses>(entity).unwrap().classes,
            ["base", "muted"]
        );
    }

    #[test]
    fn editable_and_numeric_widget_attributes_update_existing_state() {
        let mut world = World::new();
        world.init_resource::<UiBindingStore>();
        world.init_resource::<UiSharedValues>();
        let input = world
            .spawn((
                crate::EditableText::new(
                    String::new(),
                    tilt_ui_core::InputType::Text,
                    false,
                    false,
                ),
                crate::EditableTextOptions::default(),
                ElementState::default(),
                PropertyBindings {
                    bindings: vec![
                        PropertyBinding {
                            name: "readonly".into(),
                            expression: "true".into(),
                        },
                        PropertyBinding {
                            name: "required".into(),
                            expression: "true".into(),
                        },
                        PropertyBinding {
                            name: "maxlength".into(),
                            expression: "5".into(),
                        },
                    ],
                },
            ))
            .id();
        let slider = world
            .spawn((
                crate::NumericRange::new(0.0, 100.0, 90.0, None),
                PropertyBindings {
                    bindings: vec![
                        PropertyBinding {
                            name: "min".into(),
                            expression: "20".into(),
                        },
                        PropertyBinding {
                            name: "max".into(),
                            expression: "80".into(),
                        },
                        PropertyBinding {
                            name: "step".into(),
                            expression: "5".into(),
                        },
                    ],
                },
            ))
            .id();
        apply_bindings(&mut world);
        assert!(world.get::<crate::EditableText>(input).unwrap().readonly);
        assert!(world.get::<ElementState>(input).unwrap().invalid);
        assert_eq!(
            world
                .get::<crate::EditableTextOptions>(input)
                .unwrap()
                .max_characters,
            Some(5)
        );
        let range = world.get::<crate::NumericRange>(slider).unwrap();
        assert_eq!(
            (range.min, range.max, range.value, range.step),
            (20.0, 80.0, 80.0, Some(5.0))
        );
    }

    #[test]
    fn selected_binding_keeps_single_select_options_exclusive() {
        let mut world = World::new();
        world.init_resource::<UiBindingStore>();
        world.init_resource::<UiSharedValues>();
        let choice = world
            .spawn(crate::TiltElement {
                kind: tilt_ui_core::ElementKind::ChoiceBox,
            })
            .id();
        let first = world
            .spawn((
                crate::OptionData {
                    value: "one".into(),
                    label: "One".into(),
                },
                crate::ControlChecked(true),
                ElementState {
                    checked: true,
                    ..Default::default()
                },
            ))
            .id();
        let second = world
            .spawn((
                crate::OptionData {
                    value: "two".into(),
                    label: "Two".into(),
                },
                crate::ControlChecked(false),
                ElementState::default(),
                PropertyBindings {
                    bindings: vec![PropertyBinding {
                        name: "selected".into(),
                        expression: "true".into(),
                    }],
                },
            ))
            .id();
        world.entity_mut(choice).add_children(&[first, second]);
        apply_bindings(&mut world);
        assert!(!world.get::<crate::ControlChecked>(first).unwrap().0);
        assert!(world.get::<crate::ControlChecked>(second).unwrap().0);
    }

    #[test]
    fn property_binding_does_not_skip_text_on_the_same_entity() {
        let mut world = World::new();
        let mut store = UiBindingStore::default();
        store.set_store(State {
            title: "Visible".into(),
            enabled: true,
        });
        world.insert_resource(store);
        world.init_resource::<UiSharedValues>();
        let entity = world
            .spawn((
                TiltText {
                    value: "{{ state.title }}".into(),
                },
                Text::new(""),
                PropertyBindings {
                    bindings: vec![PropertyBinding {
                        name: "disabled".into(),
                        expression: "state.enabled".into(),
                    }],
                },
            ))
            .id();

        apply_bindings(&mut world);

        assert!(world.get::<InteractionDisabled>(entity).is_some());
        assert_eq!(world.get::<Text>(entity).unwrap().0, "Visible");
    }

    #[test]
    fn input_anatomy_and_legacy_options_update_without_replacing_owner() {
        use bevy::{ecs::hierarchy::Children, ui::Node};
        use tilt_ui_core::InputType;

        let mut world = World::new();
        let input = world.spawn((Node::default(), ElementState::default())).id();
        crate::widgets::controls::input::materialize_parts(&mut world, input, &[], false);
        apply_property(&mut world, input, "type", Value::from("number"));
        apply_property(&mut world, input, "show-fields", Value::Bool(true));
        apply_property(&mut world, input, "label", Value::from("Quantity"));
        apply_property(&mut world, input, "cap-text-at", Value::from(4));
        let children = world.get::<Children>(input).unwrap();
        assert_eq!(
            world.get::<crate::EditableText>(input).unwrap().input_type,
            InputType::Number
        );
        assert_eq!(
            children
                .iter()
                .filter(
                    |child| world
                        .get::<crate::ControlPart>(**child)
                        .is_some_and(|part| matches!(
                            part.kind,
                            crate::ControlPartKind::Increment | crate::ControlPartKind::Decrement
                        ))
                )
                .count(),
            2
        );
        assert!(
            world
                .get::<crate::widgets::controls::input::InputFieldOptions>(input)
                .unwrap()
                .label
                .is_some()
        );
        assert_eq!(
            world
                .get::<bevy::text::EditableText>(input)
                .unwrap()
                .max_characters,
            Some(4)
        );
        apply_property(&mut world, input, "type", Value::from("text"));
        assert!(world.get::<Children>(input).unwrap().iter().all(|child| {
            world.get::<crate::ControlPart>(*child).is_none_or(|part| {
                !matches!(
                    part.kind,
                    crate::ControlPartKind::Increment | crate::ControlPartKind::Decrement
                )
            })
        }));
    }

    #[test]
    fn slider_parts_reconfigure_in_place() {
        use bevy::{ecs::hierarchy::Children, ui::Node};
        let mut world = World::new();
        let slider = world.spawn(Node::default()).id();
        crate::widgets::controls::slider::materialize_parts(&mut world, slider, &[]);
        apply_property(&mut world, slider, "dots", Value::from(4));
        apply_property(&mut world, slider, "orientation", Value::from("vertical"));
        apply_property(&mut world, slider, "show-tip", Value::Bool(true));
        apply_property(&mut world, slider, "type", Value::from("range"));
        let parts = world.get::<crate::NumericParts>(slider).unwrap();
        assert_eq!(parts.orientation, crate::RangeOrientation::Vertical);
        assert!(parts.second_thumb.is_some() && parts.second_tip.is_some());
        assert_eq!(
            world
                .get::<Children>(parts.track)
                .unwrap()
                .iter()
                .filter(|child| world
                    .get::<crate::ControlPart>(**child)
                    .is_some_and(|part| part.kind == crate::ControlPartKind::Dot))
                .count(),
            5
        );
        apply_property(&mut world, slider, "show-tip", Value::Bool(false));
        assert!(
            world
                .get::<crate::NumericParts>(slider)
                .unwrap()
                .tip
                .is_none()
        );
    }

    #[test]
    fn date_range_bindings_apply_both_endpoints_even_when_declared_in_reverse_order() {
        use bevy::ui::Node;
        let mut world = World::new();
        world.init_resource::<UiBindingStore>();
        world.init_resource::<UiSharedValues>();
        let picker = world
            .spawn((
                Node::default(),
                ElementState::default(),
                PropertyBindings {
                    bindings: vec![
                        PropertyBinding {
                            name: "range-end".into(),
                            expression: "'2026-09-22'".into(),
                        },
                        PropertyBinding {
                            name: "range-start".into(),
                            expression: "'2026-09-10'".into(),
                        },
                    ],
                },
            ))
            .id();
        crate::widgets::advanced::date_picker::materialize(
            &mut world,
            picker,
            &[tilt_ui_core::TemplateAttribute::Static {
                name: "type".into(),
                value: "range".into(),
            }],
        );
        apply_bindings(&mut world);
        let state = world.get::<crate::DatePickerState>(picker).unwrap();
        assert_eq!(state.range_start, crate::IsoDate::parse("2026-09-10"));
        assert_eq!(state.range_end, crate::IsoDate::parse("2026-09-22"));
        apply_property(
            &mut world,
            picker,
            "value",
            serde_json::json!({"start": "2026-10-01", "end": "2026-10-07"}),
        );
        let state = world.get::<crate::DatePickerState>(picker).unwrap();
        assert_eq!(state.range_start, crate::IsoDate::parse("2026-10-01"));
        assert_eq!(state.range_end, crate::IsoDate::parse("2026-10-07"));
    }

    #[test]
    fn newly_added_text_is_interpolated_without_a_store_change() {
        let mut world = World::new();
        let mut store = UiBindingStore::default();
        store.set_store(State {
            title: "Current".into(),
            enabled: false,
        });
        world.insert_resource(store);
        world.init_resource::<UiSharedValues>();
        apply_bindings(&mut world);

        let entity = world
            .spawn((
                TiltText {
                    value: "{{ state.title }}".into(),
                },
                Text::new(""),
            ))
            .id();
        apply_bindings(&mut world);
        assert_eq!(world.get::<Text>(entity).unwrap().0, "Current");
    }

    #[cfg(feature = "fluent")]
    #[test]
    fn fluent_bindings_follow_language_and_argument_changes() {
        let mut world = World::new();
        world.init_resource::<UiBindingStore>();
        world.init_resource::<UiSharedValues>();
        let mut localization = crate::UiLocalization::new("en-US").unwrap();
        localization
            .insert_ftl("en-US", "welcome = Hello, { $name }!")
            .unwrap();
        localization
            .insert_ftl("de-DE", "welcome = Hallo, { $name }!")
            .unwrap();
        world.insert_resource(localization);
        let mut args = crate::UiFluentArgs::default();
        args.set("welcome", "name", "Ada");
        world.insert_resource(args);
        let entity = world
            .spawn((
                TiltText {
                    value: "{{ i18n.welcome }}".into(),
                },
                Text::new(""),
            ))
            .id();

        apply_bindings(&mut world);
        assert_eq!(world.get::<Text>(entity).unwrap().0, "Hello, Ada!");
        world
            .resource_mut::<crate::UiLocalization>()
            .set_locale("de-DE")
            .unwrap();
        apply_bindings(&mut world);
        assert_eq!(world.get::<Text>(entity).unwrap().0, "Hallo, Ada!");
        world
            .resource_mut::<crate::UiFluentArgs>()
            .set("welcome", "name", "Bea");
        apply_bindings(&mut world);
        assert_eq!(world.get::<Text>(entity).unwrap().0, "Hallo, Bea!");
    }

    #[cfg(feature = "fluent")]
    #[test]
    fn translated_option_and_placeholder_follow_the_locale() {
        use crate::widgets::state::EditableTextParts;

        let mut world = World::new();
        world.init_resource::<UiBindingStore>();
        world.init_resource::<UiSharedValues>();
        let mut localization = crate::UiLocalization::new("en-US").unwrap();
        localization
            .insert_ftl("en-US", "fruit = Apple\nhint = Search")
            .unwrap();
        localization
            .insert_ftl("de-DE", "fruit = Apfel\nhint = Suchen")
            .unwrap();
        world.insert_resource(localization);
        world.init_resource::<crate::UiFluentArgs>();

        let selected_text = world.spawn(Text::new("")).id();
        let popup = world.spawn_empty().id();
        let choice = world
            .spawn(crate::ChoiceBoxParts {
                value: selected_text,
                popup,
                open: false,
            })
            .id();
        world.entity_mut(choice).add_child(popup);
        let option = world
            .spawn((
                crate::OptionData {
                    value: "apple".into(),
                    label: String::new(),
                },
                crate::ControlChecked(true),
            ))
            .id();
        world.entity_mut(popup).add_child(option);
        let option_text = world
            .spawn((
                TiltText {
                    value: "{{ i18n.fruit }}".into(),
                },
                Text::new(""),
            ))
            .id();
        world.entity_mut(option).add_child(option_text);

        let hint = world.spawn(Text::new("")).id();
        world.spawn((
            EditableTextParts {
                value: hint,
                placeholder: hint,
                selection: hint,
                cursor: hint,
            },
            PropertyBindings {
                bindings: vec![PropertyBinding {
                    name: "placeholder".into(),
                    expression: "i18n.hint".into(),
                }],
            },
        ));
        apply_bindings(&mut world);
        assert_eq!(world.get::<Text>(selected_text).unwrap().0, "Apple");
        assert_eq!(world.get::<Text>(hint).unwrap().0, "Search");

        world
            .resource_mut::<crate::UiLocalization>()
            .set_locale("de-DE")
            .unwrap();
        apply_bindings(&mut world);
        assert_eq!(world.get::<Text>(selected_text).unwrap().0, "Apfel");
        assert_eq!(world.get::<Text>(hint).unwrap().0, "Suchen");
    }
}
