use std::collections::{BTreeMap, btree_map::Entry};

use bevy::ecs::{component::Component, entity::Entity};
use tilt_ui_core::{ElementKind, NodeId, TemplateAttribute, TemplateUse};

/// Represents a built-in TiltUI element instantiated from a template node.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TiltElement {
    /// Semantic built-in element kind declared by the source template.
    pub kind: ElementKind,
}

/// Associates a runtime entity with the template node that produced it.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TemplateNodeRef {
    /// Template-local node identifier.
    pub node: NodeId,
}

/// Stores literal text content instantiated from a template text node.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct TiltText {
    /// Literal text value from the template.
    pub value: String,
}

/// Imports declared by the template owning this component boundary.
#[derive(Component, Debug, Clone)]
pub(crate) struct TemplateImports(pub Vec<TemplateUse>);

/// Associates a materialized node with the component instance that owns its style scope.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentStyleOwner(pub bevy::ecs::entity::Entity);

/// Stores selector pseudo-class state for a materialized element.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ElementState {
    /// Indicates whether the element matches `:hover`.
    pub hovered: bool,
    /// Indicates whether the element matches `:active`.
    pub active: bool,
    /// Indicates whether the element matches `:focus`.
    pub focused: bool,
    /// Indicates whether the element matches `:disabled`.
    pub disabled: bool,
    /// Indicates whether the element matches `:loading`.
    pub loading: bool,
    /// Indicates whether the element matches `:checked`.
    pub checked: bool,
    /// Indicates whether the element matches `:readonly`.
    pub readonly: bool,
    /// Indicates whether the element matches `:invalid`.
    pub invalid: bool,
    /// Indicates whether a popup control matches `:open`.
    pub open: bool,
    /// Indicates whether a dialog has animated presentation enabled.
    pub animated: bool,
    /// Indicates whether a dialog is playing its exit animation.
    pub closing: bool,
}

/// Stores a static `id` attribute on an instantiated TiltUI element.
#[derive(Component, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ElementId(pub String);

/// Indexes static element IDs within one component instance.
///
/// Nested component internals maintain a distinct index and are never visible through this map.
#[derive(Component, Debug, Default)]
pub struct ComponentElementIds {
    entries: BTreeMap<String, Entity>,
    duplicates: Vec<String>,
}

impl ComponentElementIds {
    /// Resolves one static ID in the owning component scope.
    pub fn get(&self, id: &str) -> Option<Entity> {
        self.entries.get(id).copied()
    }

    /// Returns duplicate IDs detected while the component template was materialized.
    pub fn duplicates(&self) -> &[String] {
        &self.duplicates
    }

    pub(crate) fn insert(&mut self, id: String, entity: Entity) {
        match self.entries.entry(id) {
            Entry::Occupied(entry) => self.duplicates.push(entry.key().clone()),
            Entry::Vacant(entry) => {
                entry.insert(entity);
            }
        }
    }
}

/// Stores static CSS class names declared on an instantiated TiltUI element.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct ElementClasses {
    /// Class names in source order.
    pub classes: Vec<String>,
}

/// Static classes and reactive class bindings, combined on the same entity.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct BoundClasses {
    pub base: Vec<String>,
    pub dynamic: Vec<String>,
    pub toggles: BTreeMap<String, bool>,
}

impl BoundClasses {
    pub fn combined(&self) -> ElementClasses {
        let mut classes = Vec::new();
        for class in self.base.iter().chain(&self.dynamic) {
            if !classes.contains(class) {
                classes.push(class.clone());
            }
        }
        for (class, enabled) in &self.toggles {
            if *enabled && !classes.contains(class) {
                classes.push(class.clone());
            }
            if !enabled {
                classes.retain(|existing| existing != class);
            }
        }
        ElementClasses { classes }
    }
}

/// Represents one static template attribute not interpreted as an ID or class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticAttribute {
    /// Attribute name declared in the template.
    pub name: String,
    /// Literal attribute value declared in the template.
    pub value: String,
}

/// Preserves static template attributes for later runtime behavior and selector work.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct StaticAttributes {
    /// Static attributes excluding values represented by dedicated runtime components.
    pub attributes: Vec<StaticAttribute>,
}

/// Represents one unevaluated property binding declared by a template node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyBinding {
    /// Bound property name.
    pub name: String,
    /// Unevaluated expression source.
    pub expression: String,
}

/// Stores property bindings that have not yet been evaluated by the runtime.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct PropertyBindings {
    /// Property bindings in source order.
    pub bindings: Vec<PropertyBinding>,
}

/// Represents one unevaluated event binding declared by a template node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventBinding {
    /// Event name declared by the template.
    pub name: String,
    /// Unevaluated handler expression source.
    pub expression: String,
}

/// Stores event bindings that have not yet been connected to runtime handlers.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct EventBindings {
    /// Event bindings in source order.
    pub bindings: Vec<EventBinding>,
}

/// Returns whether a static attribute has enabled boolean semantics.
pub(crate) fn has_boolean_static_attribute(
    attributes: &[TemplateAttribute],
    expected_name: &str,
) -> bool {
    attributes.iter().any(|attribute| {
        matches!(attribute, TemplateAttribute::Static { name, value } if boolean_attribute_value(name, value, expected_name))
    })
}

/// Returns the value of a static template attribute.
pub(crate) fn static_attribute_value<'a>(
    attributes: &'a [TemplateAttribute],
    expected_name: &str,
) -> Option<&'a str> {
    attributes.iter().find_map(|attribute| match attribute {
        TemplateAttribute::Static { name, value } if name == expected_name => Some(value.as_str()),
        _ => None,
    })
}

/// Returns whether one static attribute has enabled boolean semantics.
pub(crate) fn boolean_attribute_value(name: &str, value: &str, expected_name: &str) -> bool {
    name == expected_name
        && (value.is_empty()
            || value.eq_ignore_ascii_case("true")
            || value.eq_ignore_ascii_case(expected_name))
}
