use super::*;
use crate::component::UiStore;
use serde::Serialize;
use serde_json::json;

#[derive(Serialize)]
struct InventoryView {
    title: String,
    items: Vec<Value>,
}

impl UiStore for InventoryView {
    const STORE_KEY: &'static str = "InventoryView";
    const STORE_PATH: &'static str = "tests::InventoryView";
}

fn fixture() -> (World, Entity) {
    let mut world = World::new();
    world.init_resource::<UiBindingStore>();
    world.init_resource::<UiSharedValues>();
    world
        .resource_mut::<UiBindingStore>()
        .set_store(InventoryView {
            title: "Schnee und Eis".into(),
            items: (0..512)
                .map(|index| json!({"id": index, "name": "Kristall", "count": 128}))
                .collect(),
        });
    let entity = world.spawn_empty().id();
    (world, entity)
}

#[test]
fn root_lookup_borrows_the_original_large_store_and_local_scope() {
    let (mut world, entity) = fixture();
    assert!(std::ptr::eq(
        lookup_root_ref(&world, entity, "inventoryView").unwrap(),
        world
            .resource::<UiBindingStore>()
            .json("inventoryView")
            .unwrap(),
    ));
    let parent = world
        .spawn(LocalValues(BTreeMap::from([(
            "inventoryView".into(),
            json!({"title": "Local"}),
        )])))
        .id();
    world.entity_mut(entity).insert(ChildOf(parent));
    assert!(std::ptr::eq(
        lookup_root_ref(&world, entity, "inventoryView").unwrap(),
        &world.get::<LocalValues>(parent).unwrap().0["inventoryView"],
    ));
    assert_eq!(
        resolve(&world, entity, "inventoryView.title"),
        Some(json!("Local"))
    );
}

#[test]
fn borrowed_paths_preserve_arrays_unicode_lengths_and_invalid_suffixes() {
    let (mut world, entity) = fixture();
    world
        .entity_mut(entity)
        .insert(LocalValues(BTreeMap::from([(
            "word".into(),
            Value::String("Gr\u{00fc}\u{00df}e".into()),
        )])));
    for (path, expected) in [
        ("inventoryView.items.length", Some(json!(512))),
        ("inventoryView.items.511.id", Some(json!(511))),
        ("inventoryView.items.512.id", None),
        ("inventoryView.items.length.id", None),
        ("inventoryView.title.length.id", None),
        ("word.length", Some(json!(5))),
        ("word.length.length", None),
        ("inventoryView.title.0", None),
        ("true", Some(json!(true))),
    ] {
        assert_eq!(resolve(&world, entity, path), expected, "{path}");
    }
}

#[test]
fn explicit_locals_and_complex_owned_fallback_keep_their_lookup_precedence() {
    let (world, entity) = fixture();
    let locals = BTreeMap::from([(
        "inventoryView".into(),
        json!({"title": "Override", "items": [1, 2]}),
    )]);
    assert_eq!(
        resolve_with_locals(&world, entity, "inventoryView.title", &locals),
        Some(json!("Override")),
    );
    assert_eq!(
        resolve_with_locals(&world, entity, "inventoryView.items.length", &locals),
        Some(json!(2)),
    );
    assert_eq!(
        resolve_with_locals(
            &world,
            entity,
            "inventoryView.title == 'Override' ? 'local' : 'global'",
            &locals,
        ),
        Some(json!("local")),
    );
    assert_eq!(
        resolve(&world, entity, "inventoryView.items.length > 500"),
        Some(json!(true)),
    );
}
