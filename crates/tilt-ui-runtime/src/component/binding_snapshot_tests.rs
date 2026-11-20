use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::json;

#[derive(Resource)]
struct Cold {
    dirty: bool,
    value: Value,
}

#[derive(Resource)]
struct Hot {
    dirty: bool,
    value: Value,
}

#[derive(Resource, Default)]
struct Captures {
    cold: AtomicUsize,
    hot: AtomicUsize,
}

fn cold(world: &World) -> Option<Value> {
    world
        .resource::<Captures>()
        .cold
        .fetch_add(1, Ordering::Relaxed);
    world
        .get_resource::<Cold>()
        .map(|state| state.value.clone())
}

fn hot(world: &World) -> Option<Value> {
    world
        .resource::<Captures>()
        .hot
        .fetch_add(1, Ordering::Relaxed);
    world.get_resource::<Hot>().map(|state| state.value.clone())
}

fn registrations() -> [SharedValueRegistration; 2] {
    [
        SharedValueRegistration {
            key: "Cold",
            alias: "shared",
            snapshot: cold,
            changed: |world| {
                world
                    .get_resource::<Cold>()
                    .is_some_and(|state| state.dirty)
            },
            present: World::contains_resource::<Cold>,
        },
        SharedValueRegistration {
            key: "shared",
            alias: "hot",
            snapshot: hot,
            changed: |world| world.get_resource::<Hot>().is_some_and(|state| state.dirty),
            present: World::contains_resource::<Hot>,
        },
    ]
}

fn fixture() -> World {
    let mut world = World::new();
    world.init_resource::<UiSharedValues>();
    world.init_resource::<Captures>();
    world.insert_resource(Cold {
        dirty: false,
        value: json!({"items": (0..512).collect::<Vec<_>>() }),
    });
    world.insert_resource(Hot {
        dirty: false,
        value: Value::from(0),
    });
    world
}

fn refresh(world: &mut World) {
    let registrations = registrations();
    refresh_registered_shared_values(world, &[&registrations[0], &registrations[1]]);
}

#[test]
fn changing_small_resource_does_not_serialize_large_unchanged_inventory_again() {
    let mut world = fixture();
    refresh(&mut world);
    for index in 1..=120 {
        let mut state = world.resource_mut::<Hot>();
        state.value = Value::from(index);
        state.dirty = true;
        refresh(&mut world);
    }
    let captures = world.resource::<Captures>();
    assert_eq!(captures.cold.load(Ordering::Relaxed), 1);
    assert_eq!(captures.hot.load(Ordering::Relaxed), 121);
    assert_eq!(
        world.resource::<UiSharedValues>().get("hot"),
        Some(&json!(120))
    );
}

#[test]
fn unchanged_serialized_snapshot_keeps_revision_and_shared_alias_allocation() {
    let mut world = fixture();
    refresh(&mut world);
    let (revision, snapshot) = {
        let shared = world.resource::<UiSharedValues>();
        (shared.revision(), shared.values["Cold"].clone())
    };
    world.resource_mut::<Cold>().dirty = true;
    refresh(&mut world);
    let shared = world.resource::<UiSharedValues>();
    assert_eq!(shared.revision(), revision);
    assert!(Arc::ptr_eq(&shared.values["Cold"], &snapshot));
    world.remove_resource::<Hot>();
    refresh(&mut world);
    let shared = world.resource::<UiSharedValues>();
    assert!(Arc::ptr_eq(
        &shared.values["Cold"],
        &shared.values["shared"]
    ));
}

#[test]
fn removing_and_readding_resources_preserves_alias_collision_precedence() {
    let mut world = fixture();
    refresh(&mut world);
    assert_eq!(
        world.resource::<UiSharedValues>().get("shared"),
        Some(&json!(0))
    );
    world.remove_resource::<Hot>();
    refresh(&mut world);
    let shared = world.resource::<UiSharedValues>();
    assert_eq!(shared.get("shared"), shared.get("Cold"));
    assert!(shared.get("hot").is_none());
    let revision = shared.revision();
    world.insert_resource(Hot {
        dirty: false,
        value: json!(42),
    });
    refresh(&mut world);
    let shared = world.resource::<UiSharedValues>();
    assert_eq!(shared.get("shared"), Some(&json!(42)));
    assert_eq!(shared.get("hot"), Some(&json!(42)));
    assert_eq!(shared.revision(), revision.wrapping_add(1));
}

#[test]
fn present_json_null_is_not_confused_with_a_missing_resource() {
    let mut world = fixture();
    world.resource_mut::<Cold>().value = Value::Null;
    refresh(&mut world);
    refresh(&mut world);
    assert_eq!(
        world.resource::<UiSharedValues>().get("Cold"),
        Some(&Value::Null)
    );
    assert_eq!(world.resource::<Captures>().cold.load(Ordering::Relaxed), 1);
    world.remove_resource::<Cold>();
    refresh(&mut world);
    assert!(world.resource::<UiSharedValues>().get("Cold").is_none());
}
