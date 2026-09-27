//! Reactive control blocks. Structural work happens only when binding revisions change.

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use bevy::{
    ecs::{
        component::Component,
        entity::Entity,
        hierarchy::{ChildOf, Children},
        query::Added,
        resource::Resource,
        world::World,
    },
    log::error,
    ui::experimental::GhostNode,
};
use serde_json::Value;
use tilt_ui_core::{NodeId, Template, TemplateAttribute};

use super::{
    ComponentAssetStore, ComponentCatalog, ComponentElementIds, ComponentStyleOwner, ElementId,
    StyleDirty,
    binding::{UiBindingStore, UiSharedValues},
    binding_runtime::{LocalValues, resolve, resolve_with_locals},
    content::TemplateParent,
    spawn::{instantiate_template_children, resolve_template_targets},
};

const PREFIX: &str = "tilt-flow-";

pub(crate) fn is_flow_tag(name: &str) -> bool {
    name.starts_with(PREFIX)
}

#[derive(Clone)]
enum FlowKind {
    If {
        expression: String,
    },
    Match {
        expression: String,
    },
    For {
        variable: String,
        index: String,
        expression: String,
        track: String,
    },
    Let {
        name: String,
        expression: String,
    },
}

#[derive(Clone)]
struct LoopEntry {
    key: String,
    entity: Entity,
}

#[derive(Component, Clone)]
pub(crate) struct FlowNode {
    kind: FlowKind,
    template: Arc<Template>,
    node: NodeId,
    owner: Entity,
    revision: Option<(u64, u64)>,
    selected: Option<NodeId>,
    entries: Vec<LoopEntry>,
    initialized: bool,
}

impl FlowNode {
    pub(crate) fn from_tag(
        name: &str,
        attributes: &[TemplateAttribute],
        template: Arc<Template>,
        node: NodeId,
        owner: Entity,
    ) -> Option<Self> {
        let attr = |key: &str| {
            attributes
                .iter()
                .find_map(|attribute| match attribute {
                    TemplateAttribute::Static { name, value } if name == key => Some(value.clone()),
                    _ => None,
                })
                .unwrap_or_default()
        };
        let kind = match name {
            "tilt-flow-if" => FlowKind::If {
                expression: attr("expression"),
            },
            "tilt-flow-match" => FlowKind::Match {
                expression: attr("expression"),
            },
            "tilt-flow-for" => FlowKind::For {
                variable: attr("variable"),
                index: attr("index"),
                expression: attr("expression"),
                track: attr("track"),
            },
            "tilt-flow-let" => FlowKind::Let {
                name: attr("name"),
                expression: attr("expression"),
            },
            _ => return None,
        };
        Some(Self {
            kind,
            template,
            node,
            owner,
            revision: None,
            selected: None,
            entries: Vec::new(),
            initialized: false,
        })
    }
}

#[derive(Resource, Default)]
struct LastFlowRevision(Option<(u64, u64)>);

pub(crate) fn update_flow(world: &mut World) {
    let revision = (
        world.resource::<UiBindingStore>().revision(),
        world.resource::<UiSharedValues>().revision(),
    );
    let changed = world
        .get_resource::<LastFlowRevision>()
        .is_none_or(|last| last.0 != Some(revision));
    let added = world
        .query_filtered::<Entity, Added<FlowNode>>()
        .iter(world)
        .next()
        .is_some();
    if !changed && !added {
        return;
    }
    let (Some(catalog), Some(assets)) = (
        world.get_resource::<ComponentCatalog>().copied(),
        world.get_resource::<ComponentAssetStore>().cloned(),
    ) else {
        return;
    };
    let mut dirty_owners = Vec::new();
    // A newly selected branch can contain another control. Resolve it in this update.
    for _ in 0..64 {
        let mut pending = world
            .query::<(Entity, &FlowNode)>()
            .iter(world)
            .filter(|(_, flow)| flow.revision != Some(revision))
            .map(|(entity, flow)| (entity, flow.clone()))
            .collect::<Vec<_>>();
        if pending.is_empty() {
            break;
        }
        pending.sort_by_key(|(entity, _)| depth(world, *entity));
        for (entity, mut flow) in pending {
            if world.get_entity(entity).is_err() {
                continue;
            }
            let changed_tree = evaluate_flow(world, entity, &mut flow, catalog, &assets);
            flow.revision = Some(revision);
            world.entity_mut(entity).insert(flow.clone());
            if changed_tree && !dirty_owners.contains(&flow.owner) {
                dirty_owners.push(flow.owner);
            }
        }
    }
    for owner in dirty_owners {
        if world.get_entity(owner).is_ok() {
            rebuild_ids(world, owner);
            world.entity_mut(owner).insert(StyleDirty);
        }
    }
    world.insert_resource(LastFlowRevision(Some(revision)));
}

fn depth(world: &World, entity: Entity) -> usize {
    let mut depth = 0;
    let mut current = Some(entity);
    while let Some(node) = current {
        current = world
            .get::<TemplateParent>(node)
            .map(|parent| parent.0)
            .or_else(|| world.get::<ChildOf>(node).map(ChildOf::parent));
        depth += 1;
    }
    depth
}

fn evaluate_flow(
    world: &mut World,
    entity: Entity,
    flow: &mut FlowNode,
    catalog: ComponentCatalog,
    assets: &ComponentAssetStore,
) -> bool {
    match &flow.kind {
        FlowKind::Let { name, expression } => {
            let scope = world
                .get::<TemplateParent>(entity)
                .map(|parent| parent.0)
                .or_else(|| world.get::<ChildOf>(entity).map(ChildOf::parent))
                .unwrap_or(entity);
            let value = resolve(world, scope, expression).unwrap_or(Value::Null);
            let mut locals = BTreeMap::new();
            locals.insert(name.clone(), value);
            if world
                .get::<LocalValues>(entity)
                .is_none_or(|old| old.0 != locals)
            {
                world.entity_mut(entity).insert(LocalValues(locals));
            }
            if flow.initialized {
                return false;
            }
            flow.initialized = true;
            let children = flow
                .template
                .get(flow.node)
                .map(|node| node.children.clone())
                .unwrap_or_default();
            instantiate(world, flow, entity, &children, catalog, assets, true);
            true
        }
        FlowKind::If { expression } => {
            let condition = resolve(world, entity, expression).is_some_and(|value| truthy(&value));
            let branch_name = if condition {
                "tilt-flow-then"
            } else {
                "tilt-flow-else"
            };
            let selected = flow.template.get(flow.node).and_then(|node| node.children.iter().copied().find(|child| {
                flow.template.get(*child).is_some_and(|node| matches!(&node.kind, tilt_ui_core::TemplateNodeKind::Component(name) if name.as_str() == branch_name))
            }));
            select_branch(world, entity, flow, selected, catalog, assets)
        }
        FlowKind::Match { expression } => {
            let value = resolve(world, entity, expression).unwrap_or(Value::Null);
            let mut fallback = None;
            let mut selected = None;
            if let Some(node) = flow.template.get(flow.node) {
                for arm in &node.children {
                    let Some(case) = flow.template.get(*arm) else {
                        continue;
                    };
                    let pattern = case
                        .attributes
                        .iter()
                        .find_map(|attribute| match attribute {
                            TemplateAttribute::Static { name, value } if name == "pattern" => {
                                Some(value.as_str())
                            }
                            _ => None,
                        })
                        .unwrap_or("");
                    if pattern.trim() == "_" {
                        fallback = Some(*arm);
                        continue;
                    }
                    if pattern.split('|').any(|part| {
                        resolve(world, entity, part.trim())
                            .is_some_and(|candidate| candidate == value)
                    }) {
                        selected = Some(*arm);
                        break;
                    }
                }
            }
            select_branch(world, entity, flow, selected.or(fallback), catalog, assets)
        }
        FlowKind::For {
            variable,
            index,
            expression,
            track,
        } => {
            let values = iterable(world, entity, expression);
            let mut old = flow
                .entries
                .drain(..)
                .map(|entry| (entry.key, entry.entity))
                .collect::<HashMap<_, _>>();
            let mut occurrences = HashMap::<String, usize>::new();
            let mut next = Vec::with_capacity(values.len());
            let mut created = false;
            for (position, value) in values.into_iter().enumerate() {
                let mut locals = BTreeMap::new();
                locals.insert(variable.clone(), value.clone());
                if !index.is_empty() {
                    locals.insert(index.clone(), Value::from(position));
                }
                let key_value = if track.is_empty() {
                    value.clone()
                } else {
                    resolve_with_locals(world, entity, track, &locals).unwrap_or(Value::Null)
                };
                let key = key_value.to_string();
                let count = occurrences.entry(key.clone()).or_default();
                let key = format!("{key}#{count}");
                *count += 1;
                let item = if let Some(existing) = old.remove(&key) {
                    if world
                        .get::<LocalValues>(existing)
                        .is_none_or(|old| old.0 != locals)
                    {
                        world.entity_mut(existing).insert(LocalValues(locals));
                    }
                    existing
                } else {
                    let item = world
                        .spawn((
                            GhostNode,
                            ComponentStyleOwner(flow.owner),
                            LocalValues(locals),
                            TemplateParent(entity),
                        ))
                        .id();
                    world.entity_mut(entity).add_child(item);
                    let children = flow
                        .template
                        .get(flow.node)
                        .map(|node| node.children.clone())
                        .unwrap_or_default();
                    instantiate(world, flow, item, &children, catalog, assets, false);
                    created = true;
                    item
                };
                next.push(LoopEntry { key, entity: item });
            }
            let removed = !old.is_empty();
            for (_, entity) in old {
                world.despawn(entity);
            }
            let order = next.iter().map(|entry| entry.entity).collect::<Vec<_>>();
            let prior = world
                .get::<Children>(entity)
                .map(|children| children.iter().copied().collect::<Vec<_>>())
                .unwrap_or_default();
            let reordered = prior != order;
            if reordered {
                world.entity_mut(entity).replace_children(&order);
            }
            flow.entries = next;
            flow.initialized = true;
            if created || removed {
                resolve_template_targets(world, flow.owner);
            }
            removed || reordered
        }
    }
}

fn select_branch(
    world: &mut World,
    entity: Entity,
    flow: &mut FlowNode,
    selected: Option<NodeId>,
    catalog: ComponentCatalog,
    assets: &ComponentAssetStore,
) -> bool {
    if flow.initialized && flow.selected == selected {
        return false;
    }
    let children = world
        .get::<Children>(entity)
        .map(|children| children.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default();
    for child in children {
        world.despawn(child);
    }
    flow.selected = selected;
    flow.initialized = true;
    if let Some(branch) = selected {
        let children = flow
            .template
            .get(branch)
            .map(|node| node.children.clone())
            .unwrap_or_default();
        instantiate(world, flow, entity, &children, catalog, assets, true);
    } else {
        resolve_template_targets(world, flow.owner);
    }
    true
}

fn instantiate(
    world: &mut World,
    flow: &FlowNode,
    parent: Entity,
    children: &[NodeId],
    catalog: ComponentCatalog,
    assets: &ComponentAssetStore,
    finalize: bool,
) {
    if let Err(reason) = instantiate_template_children(
        world,
        &flow.template,
        children,
        catalog,
        assets,
        parent,
        flow.owner,
        &mut Vec::new(),
        Some(flow.template.clone()),
        false,
        finalize,
    ) {
        error!("cannot instantiate template control block: {reason}");
    }
}

fn iterable(world: &World, entity: Entity, expression: &str) -> Vec<Value> {
    if let Some((start, end)) = expression
        .split_once("..=")
        .or_else(|| expression.split_once(".."))
    {
        let inclusive = expression.contains("..=");
        let start = resolve(world, entity, start.trim()).and_then(|value| value.as_i64());
        let end = resolve(world, entity, end.trim()).and_then(|value| value.as_i64());
        if let (Some(start), Some(end)) = (start, end) {
            let end = if inclusive {
                end.saturating_add(1)
            } else {
                end
            };
            return (start..end).take(10_000).map(Value::from).collect();
        }
    }
    match resolve(world, entity, expression) {
        Some(Value::Array(items)) => items,
        _ => Vec::new(),
    }
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
    }
}

fn rebuild_ids(world: &mut World, owner: Entity) {
    let ids = world
        .query::<(Entity, &ElementId, &ComponentStyleOwner)>()
        .iter(world)
        .filter(|(_, _, scope)| scope.0 == owner)
        .map(|(entity, id, _)| (id.0.clone(), entity))
        .collect::<Vec<_>>();
    let mut map = ComponentElementIds::default();
    for (id, entity) in ids {
        map.insert(id, entity);
    }
    world.entity_mut(owner).insert(map);
}

#[cfg(test)]
mod tests {
    use bevy::{
        asset::Assets,
        ecs::world::World,
        ui::{experimental::GhostNode, widget::Text},
    };
    use serde::Serialize;
    use serde_json::json;
    use tilt_ui_core::{ComponentId, ComponentKind, ComponentMetadata};
    use tilt_ui_html::parse_template;

    use super::*;
    use crate::component::{binding::UiStore, spawn::instantiate_template_nodes};
    use crate::{LoadedComponentAssets, UiStyleSheetAsset, UiTemplateAsset};

    #[derive(Serialize)]
    struct State {
        enabled: bool,
        word: String,
        items: Vec<Value>,
    }

    impl UiStore for State {
        const STORE_KEY: &'static str = "state";
        const STORE_PATH: &'static str = "test::State";
    }

    #[derive(Serialize)]
    #[serde(transparent)]
    struct DemoFixture(Value);

    impl UiStore for DemoFixture {
        const STORE_KEY: &'static str = "demoState";
        const STORE_PATH: &'static str = "test::DemoFixture";
    }

    fn state(enabled: bool, word: &str, items: &[(&str, &str)]) -> State {
        State {
            enabled,
            word: word.into(),
            items: items
                .iter()
                .map(|(id, name)| json!({"id": id, "name": name}))
                .collect(),
        }
    }

    #[test]
    fn reactive_controls_and_tracked_items_preserve_identity() {
        let mut world = World::new();
        let mut stores = UiBindingStore::default();
        stores.set_store(state(true, "Rust", &[("a", "Alpha"), ("b", "Beta")]));
        world.insert_resource(stores);
        world.insert_resource(UiSharedValues::default());
        world.insert_resource(ComponentCatalog::default());
        world.insert_resource(ComponentAssetStore::default());
        let owner = world
            .spawn((GhostNode, ComponentElementIds::default()))
            .id();
        let template = parse_template("<div>@let word = state.word; @if (state.enabled && word.equalsIgnoreCase('rust')) {<p>YES</p>} @else {<p>NO</p>} @match (word) { 'Rust' => {<p>MATCH</p>}, _ => {<p>OTHER</p>} } @for ((item, index) in state.items; track item.id) {<p>{{ index }} {{ item.name }}</p>}</div>").unwrap();
        instantiate_template_nodes(
            &mut world,
            &template,
            ComponentCatalog::default(),
            &ComponentAssetStore::default(),
            owner,
            owner,
            &mut Vec::new(),
        )
        .unwrap();
        update_flow(&mut world);
        super::super::binding_runtime::apply_bindings(&mut world);
        let texts = |world: &mut World| {
            world
                .query::<&Text>()
                .iter(world)
                .map(|text| text.0.clone())
                .collect::<Vec<_>>()
        };
        assert!(texts(&mut world).iter().any(|text| text == "YES"));
        assert!(texts(&mut world).iter().any(|text| text == "MATCH"));
        assert!(texts(&mut world).iter().any(|text| text.contains("Alpha")));
        let identity = |world: &mut World, id: &str| {
            world
                .query::<(Entity, &LocalValues)>()
                .iter(world)
                .find(|(_, locals)| {
                    locals
                        .0
                        .get("item")
                        .and_then(|item| item.get("id"))
                        .and_then(Value::as_str)
                        == Some(id)
                })
                .map(|(entity, _)| entity)
                .unwrap()
        };
        let a = identity(&mut world, "a");
        let b = identity(&mut world, "b");
        world.resource_mut::<UiBindingStore>().set_store(state(
            false,
            "Other",
            &[("b", "Beta"), ("a", "Alpha")],
        ));
        update_flow(&mut world);
        super::super::binding_runtime::apply_bindings(&mut world);
        assert_eq!(identity(&mut world, "a"), a);
        assert_eq!(identity(&mut world, "b"), b);
        let list = world
            .query::<(Entity, &FlowNode)>()
            .iter(&world)
            .find(|(_, flow)| matches!(&flow.kind, FlowKind::For { .. }))
            .map(|(entity, _)| entity)
            .unwrap();
        assert_eq!(
            world
                .get::<Children>(list)
                .unwrap()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![b, a]
        );
        let updated = texts(&mut world);
        assert!(updated.iter().any(|text| text == "NO"));
        assert!(updated.iter().any(|text| text == "OTHER"), "{updated:?}");
        assert!(!updated.iter().any(|text| text == "YES"));
    }

    #[test]
    fn basic_showcase_stylesheet_parses() {
        tilt_ui_css::parse_stylesheet(include_str!(
            "../../../../examples/basic/src-ui/pages/main.component.css"
        ))
        .unwrap();
    }

    #[test]
    fn string_truthiness_and_inclusive_ranges_match_template_rules() {
        assert!(truthy(&json!("false")));
        assert!(!truthy(&json!("")));
        assert!(!truthy(&json!(0)));
        let mut world = World::new();
        world.insert_resource(UiBindingStore::default());
        world.insert_resource(UiSharedValues::default());
        let anchor = world.spawn_empty().id();
        assert_eq!(
            iterable(&world, anchor, "1..=3"),
            vec![json!(1), json!(2), json!(3)]
        );
    }

    #[test]
    fn basic_showcase_instantiates_all_control_sections() {
        static PROBE: [ComponentMetadata; 1] = [ComponentMetadata {
            id: ComponentId(0),
            name: "state-probe",
            kind: ComponentKind::Component,
            template_asset_path: "tilt-ui://components/state-probe.component.html",
            stylesheet_asset_path: "tilt-ui://components/state-probe.component.css",
            stylesheet_asset_paths: &["tilt-ui://components/state-probe.component.css"],
        }];
        fn probe_id(name: &str) -> Option<ComponentId> {
            (name == "state-probe").then_some(ComponentId(0))
        }
        fn probe_metadata(id: ComponentId) -> Option<&'static ComponentMetadata> {
            (id == ComponentId(0)).then_some(&PROBE[0])
        }
        let catalog = ComponentCatalog::new(&PROBE, probe_id, probe_metadata);
        let mut world = World::new();
        let mut stores = UiBindingStore::default();
        stores.set_store(DemoFixture(json!({
            "enabled": true, "query": "Rust", "count": 3, "mode": "ready",
            "items": [{"id": 1, "name": "Alpha"}, {"id": 2, "name": "Beta"}],
            "next_id": 3
        })));
        world.insert_resource(stores);
        world.insert_resource(UiSharedValues::default());
        world.insert_resource(catalog);
        world.init_resource::<Assets<UiTemplateAsset>>();
        world.init_resource::<Assets<UiStyleSheetAsset>>();
        let probe_template =
            world
                .resource_mut::<Assets<UiTemplateAsset>>()
                .add(UiTemplateAsset::new(
                    parse_template(include_str!(
                        "../../../../examples/basic/src-ui/components/state-probe.component.html"
                    ))
                    .unwrap(),
                ));
        let probe_stylesheet =
            world
                .resource_mut::<Assets<UiStyleSheetAsset>>()
                .add(UiStyleSheetAsset::new(
                    tilt_ui_css::parse_stylesheet(include_str!(
                        "../../../../examples/basic/src-ui/components/state-probe.component.css"
                    ))
                    .unwrap(),
                ));
        let mut assets = ComponentAssetStore::default();
        assets.insert(
            ComponentId(0),
            LoadedComponentAssets {
                template: probe_template,
                stylesheet: probe_stylesheet,
                additional_stylesheets: Vec::new(),
            },
        );
        world.insert_resource(assets.clone());
        let owner = world
            .spawn((GhostNode, ComponentElementIds::default()))
            .id();
        let template = parse_template(include_str!(
            "../../../../examples/basic/src-ui/pages/main.component.html"
        ))
        .unwrap();
        instantiate_template_nodes(
            &mut world,
            &template,
            catalog,
            &assets,
            owner,
            owner,
            &mut Vec::new(),
        )
        .unwrap();
        update_flow(&mut world);
        super::super::binding_runtime::apply_bindings(&mut world);
        let rendered = world
            .query::<&Text>()
            .iter(&world)
            .map(|text| text.0.as_str())
            .collect::<Vec<_>>();
        for snippet in [
            "Ready:",
            "Alpha",
            "Beta",
            "equals('Rust')",
            "3",
            "I am visible",
        ] {
            assert!(
                rendered.iter().any(|text| text.contains(snippet)),
                "missing {snippet}"
            );
        }
    }

    #[test]
    fn nested_let_shadowing_reads_outer_value_on_every_revision() {
        let mut world = World::new();
        let mut stores = UiBindingStore::default();
        stores.set_store(state(true, "A", &[]));
        world.insert_resource(stores);
        world.insert_resource(UiSharedValues::default());
        world.insert_resource(ComponentCatalog::default());
        world.insert_resource(ComponentAssetStore::default());
        let owner = world
            .spawn((GhostNode, ComponentElementIds::default()))
            .id();
        let template = parse_template(
            "<div>@let word = state.word; @let word = word + '!'; <p>{{ word }}</p></div>",
        )
        .unwrap();
        instantiate_template_nodes(
            &mut world,
            &template,
            ComponentCatalog::default(),
            &ComponentAssetStore::default(),
            owner,
            owner,
            &mut Vec::new(),
        )
        .unwrap();
        update_flow(&mut world);
        super::super::binding_runtime::apply_bindings(&mut world);
        world
            .resource_mut::<UiBindingStore>()
            .set_store(state(true, "B", &[]));
        update_flow(&mut world);
        super::super::binding_runtime::apply_bindings(&mut world);
        assert!(
            world
                .query::<&Text>()
                .iter(&world)
                .any(|text| text.0 == "B!")
        );
    }
}
