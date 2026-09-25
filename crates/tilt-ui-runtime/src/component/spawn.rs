use bevy::{
    asset::Assets,
    ecs::{entity::Entity, hierarchy::ChildOf, world::World},
};
use bevy_input_focus::tab_navigation::TabGroup;
use tilt_ui_core::{ComponentId, NodeId, Template, TemplateAttribute, TemplateNodeKind};

use crate::{
    UiTemplateAsset,
    render::{materialize_element, materialize_text},
};

use super::{
    ComponentAssetHandles, ComponentAssetStore, ComponentBoundary, ComponentCatalog,
    ComponentElementIds, ComponentInstantiationError, ComponentStyleOwner, ElementClasses,
    ElementId, ElementState, EventBinding, EventBindings, PropertyBinding, PropertyBindings,
    StaticAttribute, StaticAttributes, StyleDirty, TemplateNodeRef, TiltElement, TiltText,
};

/// Instantiates one loaded component and its nested component templates into ECS entities.
pub fn instantiate_component(
    world: &mut World,
    catalog: ComponentCatalog,
    assets: &ComponentAssetStore,
    component: ComponentId,
    parent: Option<Entity>,
) -> Result<Entity, ComponentInstantiationError> {
    validate_component_tree(world, catalog, assets, component, &mut Vec::new())?;
    instantiate_component_tree(
        world,
        catalog,
        assets,
        component,
        parent,
        None,
        &mut Vec::new(),
    )
}

pub(crate) fn instantiate_component_into_boundary(
    world: &mut World,
    catalog: ComponentCatalog,
    assets: &ComponentAssetStore,
    component: ComponentId,
    boundary: Entity,
) -> Result<(), ComponentInstantiationError> {
    validate_component_tree(world, catalog, assets, component, &mut Vec::new())?;
    let loaded = assets
        .get(component)
        .ok_or(ComponentInstantiationError::ComponentAssetsUnavailable { component })?
        .clone();
    world
        .entity_mut(boundary)
        .insert(ComponentAssetHandles {
            template: loaded.template,
            stylesheet: loaded.stylesheet,
        })
        .insert(ComponentElementIds::default())
        .insert(StyleDirty);
    if world.get::<ChildOf>(boundary).is_none() {
        world.entity_mut(boundary).insert(TabGroup::default());
    }
    let template = template_for_component(world, assets, component)?;
    let mut stack = vec![component];
    instantiate_template_nodes(
        world, &template, catalog, assets, boundary, boundary, &mut stack,
    )?;
    Ok(())
}

fn validate_component_tree(
    world: &World,
    catalog: ComponentCatalog,
    assets: &ComponentAssetStore,
    component: ComponentId,
    stack: &mut Vec<ComponentId>,
) -> Result<(), ComponentInstantiationError> {
    if let Some(cycle_start) = stack.iter().position(|active| *active == component) {
        let mut cycle = stack[cycle_start..].to_vec();
        cycle.push(component);
        return Err(ComponentInstantiationError::ComponentCycle { cycle });
    }
    catalog
        .component_metadata(component)
        .ok_or(ComponentInstantiationError::ComponentMetadataMissing { component })?;
    let template = template_for_component(world, assets, component)?;

    stack.push(component);
    for root in template.roots() {
        validate_template_node(world, &template, *root, catalog, assets, stack)?;
    }
    stack.pop();
    Ok(())
}

fn validate_template_node(
    world: &World,
    template: &Template,
    node_id: NodeId,
    catalog: ComponentCatalog,
    assets: &ComponentAssetStore,
    stack: &mut Vec<ComponentId>,
) -> Result<(), ComponentInstantiationError> {
    let node = template
        .get(node_id)
        .ok_or(ComponentInstantiationError::InvalidTemplateNode { node: node_id })?;

    if let TemplateNodeKind::Component(name) = &node.kind {
        let component = catalog.component_id(name.as_str()).ok_or_else(|| {
            ComponentInstantiationError::UnknownComponent {
                name: name.as_str().to_owned(),
            }
        })?;
        validate_component_tree(world, catalog, assets, component, stack)?;
    }
    for child in &node.children {
        validate_template_node(world, template, *child, catalog, assets, stack)?;
    }
    Ok(())
}

fn instantiate_component_tree(
    world: &mut World,
    catalog: ComponentCatalog,
    assets: &ComponentAssetStore,
    component: ComponentId,
    parent: Option<Entity>,
    source_node: Option<(NodeId, &[TemplateAttribute])>,
    stack: &mut Vec<ComponentId>,
) -> Result<Entity, ComponentInstantiationError> {
    let loaded = assets
        .get(component)
        .ok_or(ComponentInstantiationError::ComponentAssetsUnavailable { component })?
        .clone();
    let boundary = world
        .spawn((
            ComponentBoundary::new(component),
            StyleDirty,
            ComponentAssetHandles {
                template: loaded.template,
                stylesheet: loaded.stylesheet,
            },
            ComponentElementIds::default(),
        ))
        .id();
    if let Some(parent) = parent {
        world.entity_mut(parent).add_child(boundary);
    } else {
        world.entity_mut(boundary).insert(TabGroup::default());
    }
    if let Some((node, attributes)) = source_node {
        world.entity_mut(boundary).insert(TemplateNodeRef { node });
        insert_attributes(world, boundary, attributes);
    }

    let template = template_for_component(world, assets, component)?;
    stack.push(component);
    instantiate_template_nodes(world, &template, catalog, assets, boundary, boundary, stack)?;
    stack.pop();
    Ok(boundary)
}

fn instantiate_template_nodes(
    world: &mut World,
    template: &Template,
    catalog: ComponentCatalog,
    assets: &ComponentAssetStore,
    parent: Entity,
    owner: Entity,
    stack: &mut Vec<ComponentId>,
) -> Result<Vec<Option<Entity>>, ComponentInstantiationError> {
    let mut context = TemplateInstantiationContext {
        world,
        template,
        catalog,
        assets,
        owner,
        stack,
        entities: vec![None; template.nodes().len()],
    };
    for root in template.roots() {
        context.instantiate_node(*root, parent)?;
    }
    crate::widgets::advanced::tooltip::resolve_targets(context.world, owner);
    crate::widgets::advanced::date_picker::resolve_targets(context.world, owner);
    crate::widgets::advanced::dialog::resolve_targets(context.world, owner);
    crate::widgets::content::badge::resolve_targets(context.world, owner);
    crate::control::context_menu::resolve_targets(context.world, owner);
    crate::widgets::advanced::hyperlink::finalize_icons(context.world);
    Ok(context.entities)
}

struct TemplateInstantiationContext<'a> {
    world: &'a mut World,
    template: &'a Template,
    catalog: ComponentCatalog,
    assets: &'a ComponentAssetStore,
    owner: Entity,
    stack: &'a mut Vec<ComponentId>,
    entities: Vec<Option<Entity>>,
}

impl TemplateInstantiationContext<'_> {
    fn instantiate_node(
        &mut self,
        node_id: NodeId,
        parent: Entity,
    ) -> Result<Entity, ComponentInstantiationError> {
        let node = self
            .template
            .get(node_id)
            .ok_or(ComponentInstantiationError::InvalidTemplateNode { node: node_id })?;
        let entity = match &node.kind {
            TemplateNodeKind::Element(kind) => {
                let entity = self
                    .world
                    .spawn((
                        TiltElement { kind: *kind },
                        ComponentStyleOwner(self.owner),
                        TemplateNodeRef { node: node_id },
                    ))
                    .id();
                materialize_element(self.world, entity, *kind, &node.attributes);
                insert_attributes(self.world, entity, &node.attributes);
                if let Some(id) = self.world.get::<ElementId>(entity).map(|id| id.0.clone()) {
                    if let Some(mut ids) = self.world.get_mut::<ComponentElementIds>(self.owner) {
                        ids.insert(id, entity);
                    }
                }
                self.world.entity_mut(parent).add_child(entity);
                let child_parent = if *kind == tilt_ui_core::ElementKind::Dialog {
                    crate::widgets::advanced::dialog::panel(self.world, entity)
                } else {
                    entity
                };
                for child in &node.children {
                    self.instantiate_node(*child, child_parent)?;
                }
                match kind {
                    tilt_ui_core::ElementKind::Option => {
                        crate::widgets::controls::option::finish(self.world, entity);
                    }
                    tilt_ui_core::ElementKind::ChoiceBox => {
                        crate::widgets::controls::choice_box::finish(self.world, entity);
                    }
                    tilt_ui_core::ElementKind::ListBox => {
                        crate::widgets::controls::list_box::finish(self.world, entity);
                    }
                    tilt_ui_core::ElementKind::Table => {
                        crate::widgets::structure::table::finish(self.world, entity);
                    }
                    _ => {}
                }
                entity
            }
            TemplateNodeKind::Text(value) => {
                let selectable = self
                    .world
                    .get::<TiltElement>(parent)
                    .is_some_and(|element| {
                        matches!(
                            element.kind,
                            tilt_ui_core::ElementKind::Body
                                | tilt_ui_core::ElementKind::Div
                                | tilt_ui_core::ElementKind::Form
                                | tilt_ui_core::ElementKind::FieldSet
                                | tilt_ui_core::ElementKind::Label
                                | tilt_ui_core::ElementKind::Paragraph
                                | tilt_ui_core::ElementKind::Headline
                                | tilt_ui_core::ElementKind::TableCell
                        )
                    });
                let entity = self
                    .world
                    .spawn((
                        TiltText {
                            value: value.clone(),
                        },
                        ComponentStyleOwner(self.owner),
                        TemplateNodeRef { node: node_id },
                    ))
                    .id();
                materialize_text(self.world, entity, value);
                if selectable {
                    crate::control::text_selection::mark_selectable(self.world, entity);
                }
                self.world.entity_mut(parent).add_child(entity);
                entity
            }
            TemplateNodeKind::Component(name) => {
                let component = self.catalog.component_id(name.as_str()).ok_or_else(|| {
                    ComponentInstantiationError::UnknownComponent {
                        name: name.as_str().to_owned(),
                    }
                })?;
                instantiate_component_tree(
                    self.world,
                    self.catalog,
                    self.assets,
                    component,
                    Some(parent),
                    Some((node_id, &node.attributes)),
                    self.stack,
                )?
            }
        };
        let slot = self
            .entities
            .get_mut(node_id.0 as usize)
            .ok_or(ComponentInstantiationError::InvalidTemplateNode { node: node_id })?;
        *slot = Some(entity);
        Ok(entity)
    }
}

fn template_for_component(
    world: &World,
    assets: &ComponentAssetStore,
    component: ComponentId,
) -> Result<Template, ComponentInstantiationError> {
    let loaded = assets
        .get(component)
        .ok_or(ComponentInstantiationError::ComponentAssetsUnavailable { component })?;
    world
        .resource::<Assets<UiTemplateAsset>>()
        .get(&loaded.template)
        .map(|asset| asset.template().clone())
        .ok_or(ComponentInstantiationError::TemplateAssetUnavailable { component })
}

fn insert_attributes(world: &mut World, entity: Entity, attributes: &[TemplateAttribute]) {
    let mut id = None;
    let mut classes = Vec::new();
    let mut static_attributes = Vec::new();
    let mut property_bindings = Vec::new();
    let mut event_bindings = Vec::new();
    let mut state = ElementState::default();

    for attribute in attributes {
        match attribute {
            TemplateAttribute::Static { name, value } if name == "id" => {
                id = Some(ElementId(value.clone()));
            }
            TemplateAttribute::Static { name, value } if name == "class" => {
                classes.extend(value.split_ascii_whitespace().map(str::to_owned));
            }
            TemplateAttribute::Static { name, value } => {
                if super::boolean_attribute_value(name, value, "disabled") {
                    state.disabled = true;
                }
                if super::boolean_attribute_value(name, value, "checked") {
                    state.checked = true;
                }
                static_attributes.push(StaticAttribute {
                    name: name.clone(),
                    value: value.clone(),
                });
            }
            TemplateAttribute::PropertyBinding { name, expression } => {
                property_bindings.push(PropertyBinding {
                    name: name.clone(),
                    expression: expression.clone(),
                });
            }
            TemplateAttribute::EventBinding { name, expression } => {
                event_bindings.push(EventBinding {
                    name: name.clone(),
                    expression: expression.clone(),
                });
            }
        }
    }

    let mut entity = world.entity_mut(entity);
    if let Some(id) = id {
        entity.insert(id);
    }
    if !classes.is_empty() {
        entity.insert(ElementClasses { classes });
    }
    if !static_attributes.is_empty() {
        entity.insert(StaticAttributes {
            attributes: static_attributes,
        });
    }
    if !property_bindings.is_empty() {
        entity.insert(PropertyBindings {
            bindings: property_bindings,
        });
    }
    if !event_bindings.is_empty() {
        entity.insert(EventBindings {
            bindings: event_bindings,
        });
    }
    if state != ElementState::default() {
        entity.insert(state);
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        asset::Assets,
        camera::Camera2d,
        ecs::{hierarchy::Children, world::World},
        ui::{
            IsDefaultUiCamera, Node,
            experimental::GhostNode,
            widget::{ImageNode, Text},
        },
    };
    use tilt_ui_core::{ComponentId, ComponentKind, ComponentMetadata};
    use tilt_ui_css::parse_stylesheet;
    use tilt_ui_html::parse_template;

    use super::instantiate_component;
    use crate::{
        ComponentAssetStore, ComponentCatalog, ElementClasses, ElementId, EventBindings,
        LoadedComponentAssets, PropertyBindings, StaticAttributes, TemplateNodeRef, TiltControl,
        TiltElement, TiltText, UiStyleSheetAsset, UiTemplateAsset,
    };

    static METADATA: [ComponentMetadata; 5] = [
        ComponentMetadata {
            id: ComponentId(0),
            name: "main",
            kind: ComponentKind::Page,
            template_asset_path: "tilt-ui://pages/main.component.html",
            stylesheet_asset_path: "tilt-ui://pages/main.component.css",
        },
        ComponentMetadata {
            id: ComponentId(1),
            name: "app-header",
            kind: ComponentKind::Component,
            template_asset_path: "tilt-ui://components/app-header.component.html",
            stylesheet_asset_path: "tilt-ui://components/app-header.component.css",
        },
        ComponentMetadata {
            id: ComponentId(2),
            name: "component-a",
            kind: ComponentKind::Component,
            template_asset_path: "tilt-ui://components/component-a.component.html",
            stylesheet_asset_path: "tilt-ui://components/component-a.component.css",
        },
        ComponentMetadata {
            id: ComponentId(3),
            name: "component-b",
            kind: ComponentKind::Component,
            template_asset_path: "tilt-ui://components/component-b.component.html",
            stylesheet_asset_path: "tilt-ui://components/component-b.component.css",
        },
        ComponentMetadata {
            id: ComponentId(4),
            name: "component-c",
            kind: ComponentKind::Component,
            template_asset_path: "tilt-ui://components/component-c.component.html",
            stylesheet_asset_path: "tilt-ui://components/component-c.component.css",
        },
    ];

    fn component_id(name: &str) -> Option<ComponentId> {
        METADATA
            .iter()
            .find(|metadata| metadata.name == name)
            .map(|metadata| metadata.id)
    }

    fn component_metadata(component: ComponentId) -> Option<&'static ComponentMetadata> {
        METADATA.iter().find(|metadata| metadata.id == component)
    }

    fn catalog() -> ComponentCatalog {
        ComponentCatalog::new(&METADATA, component_id, component_metadata)
    }

    fn world_with_assets() -> World {
        let mut world = World::new();
        world.insert_resource(Assets::<UiTemplateAsset>::default());
        world.insert_resource(Assets::<UiStyleSheetAsset>::default());
        world
    }

    fn add_component_template(
        world: &mut World,
        store: &mut ComponentAssetStore,
        component: ComponentId,
        source: &str,
    ) {
        let template = parse_template(source).expect("valid template fixture");
        let template = world
            .resource_mut::<Assets<UiTemplateAsset>>()
            .add(UiTemplateAsset::new(template));
        let stylesheet =
            world
                .resource_mut::<Assets<UiStyleSheetAsset>>()
                .add(UiStyleSheetAsset::new(
                    parse_stylesheet("div { width: auto; }").expect("valid stylesheet fixture"),
                ));
        store.insert(
            component,
            LoadedComponentAssets {
                template,
                stylesheet,
            },
        );
    }

    fn only_child(world: &World, entity: bevy::ecs::entity::Entity) -> bevy::ecs::entity::Entity {
        let children = world.get::<Children>(entity).expect("children");
        assert_eq!(children.len(), 1);
        children[0]
    }

    #[test]
    fn instantiates_semantic_hierarchy_and_static_attributes() {
        let mut world = world_with_assets();
        let mut store = ComponentAssetStore::default();
        add_component_template(
            &mut world,
            &mut store,
            ComponentId(0),
            "<div class=\"root\" role=\"dialog\"><div id=\"content\">Hello TiltUI</div></div>",
        );

        let boundary = instantiate_component(&mut world, catalog(), &store, ComponentId(0), None)
            .expect("instantiated template");
        let outer = only_child(&world, boundary);
        let inner = only_child(&world, outer);
        let text = only_child(&world, inner);

        assert!(world.get::<GhostNode>(boundary).is_some());
        assert!(world.get::<Node>(boundary).is_none());
        assert_eq!(
            world
                .get::<crate::ComponentRoot>(boundary)
                .expect("component root")
                .component,
            ComponentId(0)
        );

        assert!(
            matches!(world.get::<TiltElement>(outer), Some(element) if element.kind == tilt_ui_core::ElementKind::Div)
        );
        assert!(
            matches!(world.get::<TiltElement>(inner), Some(element) if element.kind == tilt_ui_core::ElementKind::Div)
        );
        assert_eq!(
            world.get::<ElementClasses>(outer).expect("classes").classes,
            ["root"]
        );
        assert_eq!(
            world
                .get::<StaticAttributes>(outer)
                .expect("static attributes")
                .attributes[0]
                .name,
            "role"
        );
        assert_eq!(
            world
                .get::<StaticAttributes>(outer)
                .expect("static attributes")
                .attributes[0]
                .value,
            "dialog"
        );
        assert_eq!(
            world.get::<ElementId>(inner),
            Some(&ElementId("content".into()))
        );
        assert_eq!(
            world.get::<TiltText>(text),
            Some(&TiltText {
                value: "Hello TiltUI".into()
            })
        );
        assert!(world.get::<Node>(outer).is_some());
        assert!(world.get::<Node>(inner).is_some());
        assert_eq!(
            world.get::<Text>(text).expect("Bevy text").0,
            "Hello TiltUI"
        );
        assert_eq!(world.get::<TemplateNodeRef>(outer).expect("node").node.0, 0);
        assert_eq!(world.get::<TemplateNodeRef>(inner).expect("node").node.0, 1);
        assert_ne!(outer, inner);
        assert_eq!(world.query::<&TiltElement>().iter(&world).count(), 2);
        assert_eq!(world.query::<&TiltText>().iter(&world).count(), 1);
    }

    #[test]
    fn supports_multiple_template_roots() {
        let mut world = world_with_assets();
        let mut store = ComponentAssetStore::default();
        add_component_template(
            &mut world,
            &mut store,
            ComponentId(0),
            "<h1>Title</h1><p>Description</p>",
        );

        let boundary = instantiate_component(&mut world, catalog(), &store, ComponentId(0), None)
            .expect("instantiated template");
        let children = world.get::<Children>(boundary).expect("root children");

        assert!(world.get::<GhostNode>(boundary).is_some());
        assert!(world.get::<Node>(boundary).is_none());
        assert_eq!(children.len(), 2);
        assert!(
            matches!(world.get::<TiltElement>(children[0]), Some(element) if element.kind == tilt_ui_core::ElementKind::Headline)
        );
        assert!(
            matches!(world.get::<TiltElement>(children[1]), Some(element) if element.kind == tilt_ui_core::ElementKind::Paragraph)
        );
    }

    #[test]
    fn preserves_property_and_event_bindings() {
        let mut world = world_with_assets();
        let mut store = ComponentAssetStore::default();
        add_component_template(
            &mut world,
            &mut store,
            ComponentId(0),
            "<button [disabled]=\"loading\" (click)=\"start_game()\">Play</button>",
        );

        let boundary = instantiate_component(&mut world, catalog(), &store, ComponentId(0), None)
            .expect("instantiated template");
        let button = only_child(&world, boundary);

        assert_eq!(
            world
                .get::<PropertyBindings>(button)
                .expect("property bindings")
                .bindings[0]
                .name,
            "disabled"
        );
        assert_eq!(
            world
                .get::<PropertyBindings>(button)
                .expect("property bindings")
                .bindings[0]
                .expression,
            "loading"
        );
        assert_eq!(
            world
                .get::<EventBindings>(button)
                .expect("event bindings")
                .bindings[0]
                .name,
            "click"
        );
        assert_eq!(
            world
                .get::<EventBindings>(button)
                .expect("event bindings")
                .bindings[0]
                .expression,
            "start_game()"
        );
        assert!(world.get::<Node>(button).is_some());
        assert!(world.get::<TiltControl>(button).is_some());
        let text = only_child(&world, button);
        assert_eq!(world.get::<Text>(text).expect("Bevy text").0, "Play");
    }

    #[test]
    fn body_and_div_share_creation_time_node_materialization() {
        let mut world = world_with_assets();
        let mut store = ComponentAssetStore::default();
        add_component_template(
            &mut world,
            &mut store,
            ComponentId(0),
            "<body><div /></body>",
        );

        let boundary = instantiate_component(&mut world, catalog(), &store, ComponentId(0), None)
            .expect("instantiated template");
        let body = only_child(&world, boundary);
        let div = only_child(&world, body);

        assert!(
            matches!(world.get::<TiltElement>(body), Some(element) if element.kind == tilt_ui_core::ElementKind::Body)
        );
        assert!(
            matches!(world.get::<TiltElement>(div), Some(element) if element.kind == tilt_ui_core::ElementKind::Div)
        );
        assert!(world.get::<Node>(body).is_some());
        assert!(world.get::<Node>(div).is_some());
    }

    #[test]
    fn image_nodes_materialize_on_the_source_entity() {
        let mut world = world_with_assets();
        let mut store = ComponentAssetStore::default();
        add_component_template(
            &mut world,
            &mut store,
            ComponentId(0),
            "<img src=\"ui/test.png\" />",
        );

        let boundary = instantiate_component(&mut world, catalog(), &store, ComponentId(0), None)
            .expect("instantiated template");
        let image = only_child(&world, boundary);

        assert!(
            matches!(world.get::<TiltElement>(image), Some(element) if element.kind == tilt_ui_core::ElementKind::Image)
        );
        assert!(world.get::<ImageNode>(image).is_some());
        assert!(world.get::<Node>(image).is_some());
    }

    #[test]
    fn materializes_content_choices_and_advanced_widgets_from_one_template() {
        use tilt_ui_core::ElementKind;

        let mut world = world_with_assets();
        let mut store = ComponentAssetStore::default();
        add_component_template(
            &mut world,
            &mut store,
            ComponentId(0),
            "<div><p>Body</p><h3>Heading</h3><img alt=\"Logo\" /><avatar alt=\"Alex Morgan\" /><badge value=\"120\" max=\"99\" /><divider /><choice-box><option value=\"a\" selected=\"true\">Alpha</option><option value=\"b\">Beta</option></choice-box><list-box><option>One</option><option>Two</option></list-box><color-picker value=\"#A833EA\" /><input id=\"date-target\" type=\"date\" /><date-picker for=\"date-target\" value=\"2026-09-24\" /><button id=\"tip-target\">Help</button><tooltip for=\"tip-target\">Info</tooltip><hyperlink href=\"https://bevyengine.org\">Bevy</hyperlink></div>",
        );
        let boundary = instantiate_component(&mut world, catalog(), &store, ComponentId(0), None)
            .expect("widgets materialize");
        let expected = [
            ElementKind::Paragraph,
            ElementKind::Headline,
            ElementKind::Image,
            ElementKind::Avatar,
            ElementKind::Badge,
            ElementKind::Divider,
            ElementKind::ChoiceBox,
            ElementKind::Option,
            ElementKind::ListBox,
            ElementKind::ColorPicker,
            ElementKind::DatePicker,
            ElementKind::ToolTip,
            ElementKind::HyperLink,
        ];
        let mut entities = world.query::<(bevy::ecs::entity::Entity, &TiltElement)>();
        let items = entities
            .iter(&world)
            .map(|(entity, element)| (entity, element.kind))
            .collect::<Vec<_>>();
        for kind in expected {
            assert!(
                items.iter().any(|(_, found)| *found == kind),
                "missing {kind:?}"
            );
        }
        let choice = items
            .iter()
            .find(|(_, kind)| *kind == ElementKind::ChoiceBox)
            .unwrap()
            .0;
        let option = items
            .iter()
            .find(|(entity, kind)| {
                *kind == ElementKind::Option
                    && world
                        .get::<crate::widgets::controls::option::OptionData>(*entity)
                        .is_some_and(|data| data.value == "a")
            })
            .unwrap()
            .0;
        let parts = world
            .get::<crate::widgets::controls::choice_box::ChoiceBoxParts>(choice)
            .unwrap();
        assert_eq!(
            world
                .get::<bevy::ecs::hierarchy::ChildOf>(option)
                .unwrap()
                .parent(),
            parts.popup
        );
        assert!(world.get::<crate::ControlChecked>(option).unwrap().0);
        let tooltip = items
            .iter()
            .find(|(_, kind)| *kind == ElementKind::ToolTip)
            .unwrap()
            .0;
        let button = items
            .iter()
            .find(|(_, kind)| *kind == ElementKind::Button)
            .unwrap()
            .0;
        assert_eq!(
            world
                .get::<crate::widgets::advanced::tooltip::TooltipSettings>(tooltip)
                .unwrap()
                .target,
            Some(button)
        );
        let picker = items
            .iter()
            .find(|(_, kind)| *kind == ElementKind::DatePicker)
            .unwrap()
            .0;
        let date_input = items
            .iter()
            .find(|(_, kind)| *kind == ElementKind::Input)
            .unwrap()
            .0;
        assert_eq!(
            world
                .get::<crate::widgets::advanced::date_picker::DatePickerTrigger>(date_input)
                .unwrap()
                .0,
            picker
        );
        let heading = items
            .iter()
            .find(|(_, kind)| *kind == ElementKind::Headline)
            .unwrap()
            .0;
        assert!(
            world
                .get::<crate::widgets::content::headline::HeadlineLevel>(heading)
                .is_some()
        );
        assert!(
            world
                .get::<bevy::ui::experimental::GhostNode>(boundary)
                .is_some()
        );
    }

    #[test]
    fn instantiates_nested_component_boundaries() {
        let mut world = world_with_assets();
        let mut store = ComponentAssetStore::default();
        add_component_template(
            &mut world,
            &mut store,
            ComponentId(0),
            "<div><app-header /></div>",
        );
        add_component_template(
            &mut world,
            &mut store,
            ComponentId(1),
            "<div class=\"header\">Header</div>",
        );

        let main = instantiate_component(&mut world, catalog(), &store, ComponentId(0), None)
            .expect("main component");
        let main_div = only_child(&world, main);
        let header_boundary = only_child(&world, main_div);
        let header_div = only_child(&world, header_boundary);

        assert_eq!(
            world
                .get::<crate::ComponentInstance>(header_boundary)
                .expect("header boundary")
                .component,
            ComponentId(1)
        );
        assert!(
            world
                .get::<crate::ComponentAssetHandles>(header_boundary)
                .is_some()
        );
        assert!(world.get::<GhostNode>(header_boundary).is_some());
        assert!(world.get::<Node>(header_boundary).is_none());
        assert!(
            matches!(world.get::<TiltElement>(header_div), Some(element) if element.kind == tilt_ui_core::ElementKind::Div)
        );
    }

    #[test]
    fn render_preconditions_keep_component_boundaries_layout_neutral() {
        let mut world = world_with_assets();
        let camera = world.spawn((Camera2d, IsDefaultUiCamera)).id();
        let mut store = ComponentAssetStore::default();
        add_component_template(
            &mut world,
            &mut store,
            ComponentId(0),
            "<div>Hello TiltUI</div>",
        );

        let boundary = instantiate_component(&mut world, catalog(), &store, ComponentId(0), None)
            .expect("instantiated template");
        let div = only_child(&world, boundary);
        let text = only_child(&world, div);

        assert!(world.get::<Camera2d>(camera).is_some());
        assert!(world.get::<IsDefaultUiCamera>(camera).is_some());
        assert!(world.get::<GhostNode>(boundary).is_some());
        assert!(world.get::<Node>(boundary).is_none());
        assert!(world.get::<Node>(div).is_some());
        assert!(world.get::<Text>(text).is_some());
    }

    #[test]
    fn rejects_unknown_component_references() {
        let mut world = world_with_assets();
        let mut store = ComponentAssetStore::default();
        add_component_template(&mut world, &mut store, ComponentId(0), "<does-not-exist />");

        assert!(matches!(
            instantiate_component(&mut world, catalog(), &store, ComponentId(0), None),
            Err(crate::ComponentInstantiationError::UnknownComponent { .. })
        ));
    }

    #[test]
    fn detects_component_cycles_without_recursing_indefinitely() {
        let mut world = world_with_assets();
        let mut store = ComponentAssetStore::default();
        add_component_template(&mut world, &mut store, ComponentId(2), "<component-b />");
        add_component_template(&mut world, &mut store, ComponentId(3), "<component-c />");
        add_component_template(&mut world, &mut store, ComponentId(4), "<component-a />");

        let error = instantiate_component(&mut world, catalog(), &store, ComponentId(2), None)
            .expect_err("component cycle");

        assert!(matches!(
            error,
            crate::ComponentInstantiationError::ComponentCycle { cycle }
                if cycle == vec![ComponentId(2), ComponentId(3), ComponentId(4), ComponentId(2)]
        ));
    }

    #[test]
    fn component_asset_store_retains_template_and_stylesheet_handles() {
        let mut world = world_with_assets();
        let mut store = ComponentAssetStore::default();
        add_component_template(&mut world, &mut store, ComponentId(0), "<div />");
        let handles = store.get(ComponentId(0)).expect("component handles");

        assert!(
            world
                .resource::<Assets<UiTemplateAsset>>()
                .get(&handles.template)
                .is_some()
        );
        assert!(
            world
                .resource::<Assets<UiStyleSheetAsset>>()
                .get(&handles.stylesheet)
                .is_some()
        );
    }
}
