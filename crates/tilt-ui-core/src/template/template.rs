use super::{NodeId, TemplateNode};

/// Owns the nodes that define one component template.
///
/// Nodes are centrally stored and connected by template-local [`NodeId`] values.
/// The structure describes template data only and has no dependency on runtime
/// ECS entities.
#[derive(Debug, Clone)]
pub struct Template {
    pub roots: Vec<NodeId>,
    pub nodes: Vec<TemplateNode>,
    /// Shared value imports declared with `@use` in this template.
    pub uses: Vec<TemplateUse>,
}

/// Imports one registered shared resource under an alias or as direct fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateUse {
    pub target: String,
    pub alias: String,
    pub wildcard: bool,
}

impl Template {
    /// Returns the identifiers of the template's top-level nodes.
    ///
    /// Templates may contain no roots, a single root, or multiple roots.
    pub fn roots(&self) -> &[NodeId] {
        &self.roots
    }

    /// Returns all nodes owned by this template in storage order.
    pub fn nodes(&self) -> &[TemplateNode] {
        &self.nodes
    }

    /// Returns the node referenced by a template-local identifier.
    pub fn get(&self, id: NodeId) -> Option<&TemplateNode> {
        self.nodes.get(id.0 as usize)
    }

    /// Returns mutable access to the node referenced by a template-local identifier.
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut TemplateNode> {
        self.nodes.get_mut(id.0 as usize)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        ComponentName, ElementKind, NodeId, Template, TemplateAttribute, TemplateNode,
        TemplateNodeKind,
    };

    fn menu_template() -> Template {
        let root = NodeId(0);
        let header = NodeId(1);
        let button = NodeId(2);
        let text = NodeId(3);

        Template {
            roots: vec![root],
            uses: vec![],
            nodes: vec![
                TemplateNode {
                    kind: TemplateNodeKind::Element(ElementKind::Div),
                    parent: None,
                    children: vec![header, button],
                    attributes: vec![TemplateAttribute::Static {
                        name: "class".into(),
                        value: "menu".into(),
                    }],
                },
                TemplateNode {
                    kind: TemplateNodeKind::Component(ComponentName::new("app-header")),
                    parent: Some(root),
                    children: Vec::new(),
                    attributes: Vec::new(),
                },
                TemplateNode {
                    kind: TemplateNodeKind::Element(ElementKind::Button),
                    parent: Some(root),
                    children: vec![text],
                    attributes: vec![
                        TemplateAttribute::PropertyBinding {
                            name: "disabled".into(),
                            expression: "loading".into(),
                        },
                        TemplateAttribute::EventBinding {
                            name: "click".into(),
                            expression: "start_game()".into(),
                        },
                    ],
                },
                TemplateNode {
                    kind: TemplateNodeKind::Text("Start".into()),
                    parent: Some(button),
                    children: Vec::new(),
                    attributes: Vec::new(),
                },
            ],
        }
    }

    #[test]
    fn stores_and_looks_up_template_nodes() {
        let template = menu_template();

        assert_eq!(template.roots(), [NodeId(0)]);
        assert_eq!(template.nodes().len(), 4);
        assert!(matches!(
            template.get(NodeId(0)).map(|node| &node.kind),
            Some(TemplateNodeKind::Element(ElementKind::Div))
        ));
        assert!(template.get(NodeId(4)).is_none());
    }

    #[test]
    fn distinguishes_elements_from_components() {
        let template = menu_template();

        assert!(matches!(
            &template.get(NodeId(0)).unwrap().kind,
            TemplateNodeKind::Element(ElementKind::Div)
        ));
        assert!(matches!(
            &template.get(NodeId(1)).unwrap().kind,
            TemplateNodeKind::Component(name) if name.as_str() == "app-header"
        ));
    }

    #[test]
    fn preserves_static_property_and_event_attributes() {
        let template = menu_template();
        let root = template.get(NodeId(0)).unwrap();
        let button = template.get(NodeId(2)).unwrap();

        assert!(matches!(
            root.attributes.as_slice(),
            [TemplateAttribute::Static { name, value }]
                if name == "class" && value == "menu"
        ));
        assert!(matches!(
            button.attributes.as_slice(),
            [
                TemplateAttribute::PropertyBinding { name: property_name, expression: property_expression },
                TemplateAttribute::EventBinding { name: event_name, expression: event_expression },
            ] if property_name == "disabled"
                && property_expression == "loading"
                && event_name == "click"
                && event_expression == "start_game()"
        ));
    }

    #[test]
    fn supports_empty_and_multiple_root_nodes() {
        let empty = Template {
            roots: Vec::new(),
            nodes: Vec::new(),
            uses: vec![],
        };
        let multiple = Template {
            roots: vec![NodeId(0), NodeId(1)],
            nodes: Vec::new(),
            uses: vec![],
        };

        assert!(empty.roots().is_empty());
        assert_eq!(multiple.roots(), [NodeId(0), NodeId(1)]);
    }
}
