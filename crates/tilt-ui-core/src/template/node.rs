use crate::enums::ElementKind;

use super::{ComponentName, NodeId, TemplateAttribute};

/// Describes the semantic kind of a template node.
///
/// Built-in TiltUI elements use `Element`, while user-defined components use
/// `Component`. Text content is represented independently of either tag kind.
#[derive(Debug, Clone)]
pub enum TemplateNodeKind {
    /// A built-in TiltUI element.
    Element(ElementKind),

    /// A user-defined TiltUI component.
    Component(ComponentName),

    /// Literal text content in the template.
    Text(String),
}

/// Represents a node stored by a [`Template`](super::Template).
///
/// Templates own all nodes in one contiguous collection. Parent and child
/// relationships use local [`NodeId`] values, keeping the structure independent
/// from ECS entities and runtime state.
#[derive(Debug, Clone)]
pub struct TemplateNode {
    pub kind: TemplateNodeKind,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub attributes: Vec<TemplateAttribute>,
}
