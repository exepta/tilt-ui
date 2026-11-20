use super::*;
use bevy::{
    ecs::{entity::Entity, hierarchy::ChildOf},
    ui::{InteractionDisabled, experimental::GhostNode, widget::Text},
};
use serde_json::json;

use crate::component::{
    ComponentAssetStore, ComponentCatalog, ComponentElementIds, PropertyBinding, PropertyBindings,
    TiltText,
    binding_runtime::{LocalValues, apply_bindings},
    dependencies::Evaluations,
    flow::{FlowNode, update_flow},
    spawn::instantiate_template_nodes,
};

#[derive(Resource)]
struct Inputs {
    inventory: Option<Value>,
    menu: Value,
    map: Value,
}

fn refresh(world: &mut World) {
    let registrations = [
        SharedValueRegistration {
            key: "InventoryView",
            alias: "inventoryView",
            snapshot: |world| world.resource::<Inputs>().inventory.clone(),
            changed: |_| true,
            present: |world| world.resource::<Inputs>().inventory.is_some(),
        },
        SharedValueRegistration {
            key: "MenuView",
            alias: "menuView",
            snapshot: |world| Some(world.resource::<Inputs>().menu.clone()),
            changed: |_| true,
            present: |_| true,
        },
        SharedValueRegistration {
            key: "MiniMapView",
            alias: "miniMapView",
            snapshot: |world| Some(world.resource::<Inputs>().map.clone()),
            changed: |_| true,
            present: |_| true,
        },
    ];
    refresh_registered_shared_values(world, &registrations.iter().collect::<Vec<_>>());
}

fn fixture() -> World {
    let mut world = World::new();
    world.init_resource::<UiBindingStore>();
    world.init_resource::<UiSharedValues>();
    world.init_resource::<Evaluations>();
    world.init_resource::<ComponentCatalog>();
    world.init_resource::<ComponentAssetStore>();
    world.insert_resource(Inputs {
        inventory: Some(
            json!({"title":"Inventory", "enabled":false, "items":[{"id":1,"name":"A"}]}),
        ),
        menu: json!({"title":"Menu", "enabled":false}),
        map: json!({"title":"Map", "enabled":false, "index":0}),
    });
    refresh(&mut world);
    world
}

fn property(world: &mut World, expression: &str) -> Entity {
    world
        .spawn(PropertyBindings {
            bindings: vec![PropertyBinding {
                name: "disabled".into(),
                expression: expression.into(),
            }],
        })
        .id()
}

fn text(world: &mut World, source: &str) -> Entity {
    world
        .spawn((
            TiltText {
                value: source.into(),
            },
            Text::new(""),
        ))
        .id()
}

fn count(world: &World, entity: Entity) -> usize {
    world
        .resource::<Evaluations>()
        .0
        .get(&entity)
        .copied()
        .unwrap_or(0)
}

#[test]
fn incremental_shared_minimap_does_not_reevaluate_inventory_menu_or_text() {
    let mut world = fixture();
    let inventory = property(&mut world, "inventoryView.enabled");
    let menu = property(&mut world, "menuView.enabled");
    let map = property(&mut world, "miniMapView.enabled");
    let inventory_text = text(&mut world, "{{ inventoryView.title }}");
    let mixed_text = text(
        &mut world,
        "{{ InventoryView.title }} / {{ miniMapView.title }}",
    );
    apply_bindings(&mut world);
    world.clear_trackers();
    for frame in 1..=120 {
        world.resource_mut::<Inputs>().map =
            json!({"title":format!("Map {frame}"), "enabled":true, "index":frame});
        refresh(&mut world);
        apply_bindings(&mut world);
        world.clear_trackers();
    }
    for entity in [inventory, menu, inventory_text] {
        assert_eq!(count(&world, entity), 1);
    }
    assert_eq!(count(&world, map), 121);
    assert_eq!(count(&world, mixed_text), 121);
    assert_eq!(
        world.get::<Text>(mixed_text).unwrap().0,
        "Inventory / Map 120"
    );
    world.resource_mut::<Inputs>().inventory.as_mut().unwrap()["enabled"] = json!(true);
    world.resource_mut::<Inputs>().inventory.as_mut().unwrap()["title"] = json!("Changed");
    refresh(&mut world);
    apply_bindings(&mut world);
    assert!(world.get::<InteractionDisabled>(inventory).is_some());
    assert_eq!(count(&world, inventory), 2);
    assert_eq!(world.get::<Text>(inventory_text).unwrap().0, "Changed");
    assert_eq!(count(&world, menu), 1);
}

#[test]
fn incremental_shared_flows_skip_unrelated_roots_but_preserve_loop_locals() {
    let mut world = fixture();
    let owner = world
        .spawn((GhostNode, ComponentElementIds::default()))
        .id();
    let template = tilt_ui_html::parse_template(
        "<div>@if (inventoryView.enabled) {<p>Inventory</p>} @if (menuView.enabled) {<p>Menu</p>} @if (miniMapView.enabled) {<p>Map</p>} @for (item in inventoryView.items; track item.id) {<p>{{ item.name }}</p>}</div>",
    ).unwrap();
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
    apply_bindings(&mut world);
    let flows = world
        .query::<(Entity, &FlowNode)>()
        .iter(&world)
        .map(|(entity, _)| entity)
        .collect::<Vec<_>>();
    assert_eq!(flows.len(), 4);
    world.clear_trackers();
    for frame in 1..=120 {
        world.resource_mut::<Inputs>().map = json!({"enabled":true,"index":frame});
        refresh(&mut world);
        update_flow(&mut world);
        apply_bindings(&mut world);
        world.clear_trackers();
    }
    let mut counts = flows
        .iter()
        .map(|entity| count(&world, *entity))
        .collect::<Vec<_>>();
    counts.sort_unstable();
    assert_eq!(counts, [1, 1, 1, 121]);
    world.resource_mut::<Inputs>().inventory.as_mut().unwrap()["items"][0]["name"] =
        json!("Updated");
    refresh(&mut world);
    update_flow(&mut world);
    apply_bindings(&mut world);
    assert!(
        world
            .query::<&Text>()
            .iter(&world)
            .any(|text| text.0 == "Updated")
    );
}

#[test]
fn incremental_shared_removal_reinsertion_and_new_binding_invalidate_aliases() {
    let mut world = fixture();
    let label = text(
        &mut world,
        "{{ inventoryView.title }} {{ InventoryView.title }}",
    );
    apply_bindings(&mut world);
    world.clear_trackers();
    world.resource_mut::<Inputs>().inventory = None;
    refresh(&mut world);
    apply_bindings(&mut world);
    assert_eq!(world.get::<Text>(label).unwrap().0, " ");
    world.clear_trackers();
    world.resource_mut::<Inputs>().inventory = Some(json!({"title":"Restored","enabled":true}));
    refresh(&mut world);
    apply_bindings(&mut world);
    assert_eq!(world.get::<Text>(label).unwrap().0, "Restored Restored");
    world.clear_trackers();
    let new = property(&mut world, "inventoryView.enabled");
    apply_bindings(&mut world);
    assert!(world.get::<InteractionDisabled>(new).is_some());
    assert_eq!(count(&world, label), 3);
}

#[test]
fn incremental_shared_local_scopes_and_method_calls_remain_conservative() {
    let mut world = fixture();
    let local = world
        .spawn(LocalValues(BTreeMap::from([(
            "local".into(),
            json!({"title":"Local"}),
        )])))
        .id();
    let local_text = text(&mut world, "{{ local.title }}");
    world.entity_mut(local_text).insert(ChildOf(local));
    let call = property(&mut world, "inventoryView.title.equals('Inventory')");
    apply_bindings(&mut world);
    world.clear_trackers();
    world.resource_mut::<Inputs>().map = json!({"index":1});
    refresh(&mut world);
    apply_bindings(&mut world);
    assert_eq!(count(&world, local_text), 2);
    assert_eq!(count(&world, call), 2);
}

#[test]
fn incremental_shared_new_local_or_import_scope_invalidates_existing_dependencies() {
    use crate::component::{ComponentStyleOwner, TemplateImports};
    use tilt_ui_core::TemplateUse;

    let mut world = fixture();
    let owner = world.spawn_empty().id();
    let imported = property(&mut world, "model.enabled");
    world
        .entity_mut(imported)
        .insert((ChildOf(owner), ComponentStyleOwner(owner)));
    let local = text(&mut world, "{{ scope.title }}");
    apply_bindings(&mut world);
    world.clear_trackers();

    world
        .entity_mut(owner)
        .insert(TemplateImports(vec![TemplateUse {
            target: "MenuView".into(),
            alias: "model".into(),
            wildcard: false,
        }]));
    let new_parent = world
        .spawn(LocalValues(BTreeMap::from([(
            "scope".into(),
            json!({"title":"New scope"}),
        )])))
        .id();
    world.entity_mut(local).insert(ChildOf(new_parent));
    // Only an unrelated root changes. Cached direct-root dependencies must not
    // hide the newly introduced lookup context.
    world.resource_mut::<Inputs>().map = json!({"index":1});
    refresh(&mut world);
    apply_bindings(&mut world);
    assert_eq!(count(&world, imported), 2);
    assert_eq!(count(&world, local), 2);
    assert_eq!(world.get::<Text>(local).unwrap().0, "New scope");

    world.clear_trackers();
    world.resource_mut::<Inputs>().menu["enabled"] = json!(true);
    refresh(&mut world);
    apply_bindings(&mut world);
    assert!(world.get::<InteractionDisabled>(imported).is_some());
}
