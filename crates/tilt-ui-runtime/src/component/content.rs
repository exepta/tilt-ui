//! Runtime replacement of content in ordinary template containers.

use super::*;
use bevy::prelude::*;
use tilt_ui_core::{ElementKind, NodeId, Template, TemplateNode, TemplateNodeKind};

/// A failed content update leaves the existing subtree intact.
#[derive(Debug, thiserror::Error)]
pub enum InnerContentError {
    /// Only ordinary content containers can have their children replaced.
    #[error("entity is not a supported content container")]
    UnsupportedTarget,
    /// The fragment is not valid TiltUI markup.
    #[error(transparent)]
    Parse(#[from] tilt_ui_html::TemplateParseError),
    /// A referenced component or provider could not be instantiated.
    #[error(transparent)]
    Instantiate(#[from] ComponentInstantiationError),
}

// Preserve template ownership even when a widget moves into an overlay layer.
#[derive(Component)]
#[relationship(relationship_target = TemplateChildren)]
pub(super) struct TemplateParent(pub Entity);

#[derive(Component, Default)]
#[relationship_target(relationship = TemplateParent, linked_spawn)]
pub(super) struct TemplateChildren(Vec<Entity>);

#[derive(Component, PartialEq, Eq)]
pub(crate) struct LiteralInnerText;

#[derive(Component, PartialEq, Eq)]
struct AppliedContent {
    source: String,
    mode: ContentMode,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ContentMode {
    Text,
    Html,
    Bindings,
}

/// Replaces all children with literal text, including literal `{{ ... }}` sequences.
/// Supports body, div, form, field-set, p, label, headline, table-cell and button.
pub fn set_inner_text(
    world: &mut World,
    entity: Entity,
    text: impl Into<String>,
) -> Result<(), InnerContentError> {
    replace(world, entity, text.into(), ContentMode::Text)
}

/// Replaces all children with a TiltUI template fragment (not browser HTML).
/// The fragment shares the target's component CSS/ID scope and supports bindings,
/// events, registered components and providers. Invalid fragments preserve old content.
pub fn set_inner_html(
    world: &mut World,
    entity: Entity,
    html: impl Into<String>,
) -> Result<(), InnerContentError> {
    replace(world, entity, html.into(), ContentMode::Html)
}

/// Replaces all children with reactive text such as `Hello {{ user.name }}`.
pub fn set_inner_bindings(
    world: &mut World,
    entity: Entity,
    text: impl Into<String>,
) -> Result<(), InnerContentError> {
    replace(world, entity, text.into(), ContentMode::Bindings)
}

fn replace(
    world: &mut World,
    entity: Entity,
    source: String,
    mode: ContentMode,
) -> Result<(), InnerContentError> {
    let owner = world
        .get::<ComponentStyleOwner>(entity)
        .map(|owner| owner.0)
        .filter(|owner| world.get_entity(*owner).is_ok())
        .ok_or(InnerContentError::UnsupportedTarget)?;
    if !world.get::<TiltElement>(entity).is_some_and(|element| {
        matches!(
            element.kind,
            ElementKind::Body
                | ElementKind::Div
                | ElementKind::Form
                | ElementKind::FieldSet
                | ElementKind::Paragraph
                | ElementKind::Label
                | ElementKind::Headline
                | ElementKind::TableCell
                | ElementKind::Button
        )
    }) {
        return Err(InnerContentError::UnsupportedTarget);
    }
    if world
        .get::<AppliedContent>(entity)
        .is_some_and(|old| old.source == source && old.mode == mode)
    {
        return Ok(());
    }
    let template = if mode == ContentMode::Html {
        tilt_ui_html::parse_template(&source)?
    } else if source.is_empty() {
        Template {
            roots: vec![],
            nodes: vec![],
            uses: vec![],
        }
    } else {
        Template {
            roots: vec![NodeId(0)],
            uses: vec![],
            nodes: vec![TemplateNode {
                kind: TemplateNodeKind::Text(source.clone()),
                parent: None,
                children: vec![],
                attributes: vec![],
            }],
        }
    };
    let catalog = world
        .get_resource::<ComponentCatalog>()
        .copied()
        .unwrap_or_default();
    let assets = world
        .get_resource::<ComponentAssetStore>()
        .cloned()
        .unwrap_or_default();
    for root in template.roots() {
        super::spawn::validate_template_node(
            world,
            &template,
            *root,
            catalog,
            &assets,
            &mut vec![],
        )?;
    }
    if mode == ContentMode::Html && !template.uses.is_empty() {
        world
            .entity_mut(entity)
            .insert(super::TemplateImports(template.uses.clone()));
    } else {
        world.entity_mut(entity).remove::<super::TemplateImports>();
    }
    let mut children = world
        .get::<Children>(entity)
        .map(|children| children.to_vec())
        .unwrap_or_default();
    if let Some(authored) = world.get::<TemplateChildren>(entity) {
        children.extend(authored.0.iter().copied());
    }
    children.sort_unstable();
    children.dedup();
    for child in children {
        if world.get_entity(child).is_ok() && world.get::<crate::ControlPart>(child).is_none() {
            world.despawn(child);
        }
    }
    rebuild_ids(world, owner);
    let entities = super::spawn::instantiate_template_nodes(
        world,
        &template,
        catalog,
        &assets,
        entity,
        owner,
        &mut vec![],
    )?;
    if mode == ContentMode::Text {
        for child in entities.into_iter().flatten() {
            world.entity_mut(child).insert(LiteralInnerText);
        }
    }
    world
        .entity_mut(entity)
        .insert(AppliedContent { source, mode });
    world.entity_mut(owner).insert(StyleDirty);
    Ok(())
}

fn rebuild_ids(world: &mut World, owner: Entity) {
    let mut query = world.query::<(Entity, &ElementId, &ComponentStyleOwner)>();
    let mut entries = query
        .iter(world)
        .filter(|(_, _, scope)| scope.0 == owner)
        .map(|(entity, id, _)| (entity, id.0.clone()))
        .collect::<Vec<_>>();
    entries.sort_by_key(|(entity, _)| *entity);
    let mut ids = ComponentElementIds::default();
    for (entity, id) in entries {
        ids.insert(id, entity);
    }
    world.entity_mut(owner).insert(ids);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::binding_runtime::apply_bindings;

    #[derive(serde::Serialize)]
    struct State {
        value: String,
    }
    impl BeuStore for State {
        const STORE_KEY: &'static str = "State";
        const STORE_PATH: &'static str = "State";
    }

    fn setup() -> (World, Entity, Entity) {
        let mut world = World::new();
        world.init_resource::<UiBindingStore>();
        world.init_resource::<UiSharedValues>();
        let owner = world.spawn(ComponentElementIds::default()).id();
        let target = world
            .spawn((
                TiltElement {
                    kind: ElementKind::Div,
                },
                ComponentStyleOwner(owner),
            ))
            .id();
        world.entity_mut(owner).add_child(target);
        (world, owner, target)
    }

    fn set_value(world: &mut World, value: &str) {
        world.resource_mut::<UiBindingStore>().set_store(State {
            value: value.into(),
        });
        apply_bindings(world);
    }

    fn text(world: &World, target: Entity) -> &str {
        &world
            .get::<Text>(world.get::<Children>(target).unwrap()[0])
            .unwrap()
            .0
    }

    #[test]
    fn literal_text_and_reactive_text_have_distinct_semantics() {
        let (mut world, _, target) = setup();
        set_inner_text(&mut world, target, "<p>{{ state.value }}</p>").unwrap();
        set_value(&mut world, "first");
        assert_eq!(text(&world, target), "<p>{{ state.value }}</p>");
        set_inner_bindings(&mut world, target, "Value: {{ state.value }}").unwrap();
        apply_bindings(&mut world);
        assert_eq!(text(&world, target), "Value: first");
        set_value(&mut world, "second");
        assert_eq!(text(&world, target), "Value: second");
        set_inner_text(&mut world, target, "").unwrap();
        assert!(
            world
                .get::<Children>(target)
                .is_none_or(|children| children.is_empty())
        );
    }

    #[test]
    fn fragments_replace_ids_preserve_scope_and_activate_bindings() {
        let (mut world, owner, target) = setup();
        let fragment = "<p id=\"message\">{{ state.value }}</p><button id=\"action\" (click)=\"handle\">Go</button>";
        set_inner_html(&mut world, target, fragment).unwrap();
        let message = world
            .get::<ComponentElementIds>(owner)
            .unwrap()
            .get("message")
            .unwrap();
        let action = world
            .get::<ComponentElementIds>(owner)
            .unwrap()
            .get("action")
            .unwrap();
        assert_eq!(world.get::<ComponentStyleOwner>(message).unwrap().0, owner);
        assert_eq!(
            world.get::<EventBindings>(action).unwrap().bindings[0].expression,
            "handle"
        );
        assert!(world.get::<StyleDirty>(owner).is_some());
        set_value(&mut world, "updated");
        assert_eq!(text(&world, message), "updated");
        set_inner_html(&mut world, target, fragment).unwrap();
        assert!(
            world.get_entity(message).is_ok(),
            "unchanged HTML must retain control state"
        );
        set_inner_html(&mut world, target, "<p id=\"next\">Next</p>").unwrap();
        assert!(world.get_entity(message).is_err());
        assert!(world.get_entity(action).is_err());
        assert!(
            world
                .get::<ComponentElementIds>(owner)
                .unwrap()
                .get("message")
                .is_none()
        );
        assert!(
            world
                .get::<ComponentElementIds>(owner)
                .unwrap()
                .get("next")
                .is_some()
        );
    }

    #[test]
    fn invalid_fragments_and_unsupported_controls_preserve_children() {
        let (mut world, _, target) = setup();
        set_inner_text(&mut world, target, "Keep").unwrap();
        assert!(set_inner_html(&mut world, target, "<p>broken</div>").is_err());
        assert!(set_inner_html(&mut world, target, "<unknown-component />").is_err());
        assert_eq!(text(&world, target), "Keep");
        world.entity_mut(target).insert(TiltElement {
            kind: ElementKind::Input,
        });
        assert!(matches!(
            set_inner_text(&mut world, target, "Replace"),
            Err(InnerContentError::UnsupportedTarget)
        ));
        assert_eq!(text(&world, target), "Keep");
    }

    #[test]
    fn property_binding_can_populate_an_empty_container_and_replace_html() {
        let (mut world, _, target) = setup();
        world.entity_mut(target).insert(PropertyBindings {
            bindings: vec![PropertyBinding {
                name: "innerHtml".into(),
                expression: "state.value".into(),
            }],
        });
        set_value(&mut world, "<p>First</p>");
        let first = world.get::<Children>(target).unwrap()[0];
        assert_eq!(text(&world, first), "First");
        set_value(&mut world, "<p>Second</p>");
        assert!(world.get_entity(first).is_err());
        let second = world.get::<Children>(target).unwrap()[0];
        assert_eq!(text(&world, second), "Second");
        world.entity_mut(target).insert(PropertyBindings {
            bindings: vec![PropertyBinding {
                name: "innerText".into(),
                expression: "state.value".into(),
            }],
        });
        set_value(&mut world, "{{ literal }}");
        assert_eq!(text(&world, target), "{{ literal }}");
    }

    #[test]
    fn replacing_content_removes_detached_dialogs_but_keeps_control_parts() {
        let (mut world, owner, target) = setup();
        world.entity_mut(owner).insert(ComponentRoot {
            component: tilt_ui_core::ComponentId(0),
        });
        let part = world
            .spawn(crate::ControlPart {
                owner: target,
                kind: crate::ControlPartKind::ScrollbarYTrack,
            })
            .id();
        world.entity_mut(target).add_child(part);
        set_inner_html(
            &mut world,
            target,
            "<dialog id=\"overlay\"><p>Content</p></dialog>",
        )
        .unwrap();
        let dialog = world
            .get::<ComponentElementIds>(owner)
            .unwrap()
            .get("overlay")
            .unwrap();
        assert_eq!(world.get::<ChildOf>(dialog).unwrap().parent(), owner);
        set_inner_text(&mut world, target, "Replaced").unwrap();
        assert!(world.get_entity(dialog).is_err());
        assert!(world.get_entity(part).is_ok());
        assert!(
            world
                .get::<ComponentElementIds>(owner)
                .unwrap()
                .get("overlay")
                .is_none()
        );
    }
}
