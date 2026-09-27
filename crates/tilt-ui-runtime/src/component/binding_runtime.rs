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

use crate::{ElementClasses, ElementState, PropertyBindings, TiltText};

use super::binding::{UiBindingStore, UiSharedValues};

#[derive(Component, Clone, Copy, PartialEq, Eq)]
struct AppliedPropertyRevision(u64, u64, u64, u64);

#[derive(Component, Clone, Copy, PartialEq, Eq)]
struct AppliedTextRevision(u64, u64, u64, u64);

#[derive(Resource, Default)]
struct LastBindingRevision(Option<(u64, u64, u64, u64)>);

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
    for (entity, bindings) in properties {
        if world.get_entity(entity).is_err() {
            continue;
        }
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

fn resolve(world: &World, entity: Entity, expression: &str) -> Option<Value> {
    let expression = expression.trim();
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
    if let Some((root, path)) = simple_path(expression) {
        let mut value = lookup_root(world, entity, root)?;
        for segment in path {
            value = match value {
                Value::Object(map) => map.get(segment)?.clone(),
                Value::Array(items) => items.get(segment.parse::<usize>().ok()?)?.clone(),
                _ => return None,
            };
        }
        return Some(value);
    }
    super::expression::evaluate(expression, |root| lookup_root(world, entity, root))
}

fn simple_path(expression: &str) -> Option<(&str, impl Iterator<Item = &str>)> {
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
        "disabled" => {
            let disabled = value.as_bool().unwrap_or(false);
            if disabled {
                world.entity_mut(entity).insert(InteractionDisabled);
            } else {
                world.entity_mut(entity).remove::<InteractionDisabled>();
            }
            if let Some(mut state) = world.get_mut::<ElementState>(entity) {
                state.disabled = disabled;
            }
        }
        "checked" | "selected" => {
            crate::set_control_checked(world, entity, value.as_bool().unwrap_or(false));
        }
        "value" => {
            if world.get::<crate::EditableText>(entity).is_some() {
                crate::set_editable_text(world, entity, value_text(&value));
            } else if let Some(number) = value.as_f64() {
                if world
                    .get::<crate::widgets::content::badge::BadgeValue>(entity)
                    .is_some()
                {
                    crate::set_badge_value(world, entity, number.max(0.0) as u32);
                } else {
                    crate::set_numeric_value(world, entity, number as f32);
                }
            }
        }
        "src" => {
            crate::set_image_source(
                world,
                entity,
                (!value.is_null()).then(|| value_text(&value)),
            );
        }
        "alt" => {
            if let Some(mut metadata) = world.get_mut::<crate::ImageMetadata>(entity) {
                metadata.alt = (!value.is_null()).then(|| value_text(&value));
            }
        }
        "placeholder" => {
            if let Some(parts) = world
                .get::<crate::widgets::state::EditableTextParts>(entity)
                .copied()
                && let Some(mut text) = world.get_mut::<Text>(parts.placeholder)
            {
                text.0 = value_text(&value);
            }
        }
        "title" => {
            crate::widgets::advanced::dialog::set_dialog_title(world, entity, value_text(&value));
        }
        "href" => {
            crate::set_link_href(world, entity, value_text(&value));
        }
        "class" => {
            let classes = value_text(&value)
                .split_ascii_whitespace()
                .map(str::to_owned)
                .collect();
            world.entity_mut(entity).insert(ElementClasses { classes });
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

#[cfg(test)]
mod tests {
    use bevy::{
        ecs::world::World,
        ui::{InteractionDisabled, widget::Text},
    };
    use serde::Serialize;

    use super::*;
    use crate::component::BeuStore;
    use crate::{PropertyBinding, PropertyBindings};

    #[derive(Serialize)]
    struct State {
        title: String,
        enabled: bool,
    }
    impl BeuStore for State {
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
