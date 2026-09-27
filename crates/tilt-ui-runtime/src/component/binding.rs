//! Typed Rust state exposed to component property and text bindings.

use std::{
    any::{Any, TypeId},
    collections::{BTreeMap, HashMap},
};

use bevy::ecs::{resource::Resource, world::World};
use serde::Serialize;
use serde_json::Value;

/// Implemented by `#[derive(BeuStore)]` on serializable UI state types.
pub trait BeuStore: Send + Sync + 'static {
    const STORE_KEY: &'static str;
    const STORE_PATH: &'static str;
}

/// Registration emitted by `#[derive(BeuStore)]`.
pub struct UiStoreRegistration {
    pub register: fn(&mut UiBindingStore),
}

inventory::collect!(UiStoreRegistration);

/// A typed, serializable store that can be read from Rust and templates.
#[derive(Resource, Default)]
pub struct UiBindingStore {
    typed: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    json: BTreeMap<String, Value>,
    revision: u64,
}

impl UiBindingStore {
    /// Initializes a derived store with its default value once.
    pub fn register<T: BeuStore + Default + Serialize>(&mut self) {
        if self.get_store::<T>().is_none() {
            self.set_store(T::default());
        }
    }

    /// Writes a typed value and makes its serialized fields visible to bindings.
    pub fn set_store<T: BeuStore + Serialize>(&mut self, value: T) {
        let json = serde_json::to_value(&value).unwrap_or(Value::Null);
        let alias = lower_first(T::STORE_KEY);
        if self.json.get(T::STORE_KEY) != Some(&json) {
            self.revision = self.revision.wrapping_add(1);
        }
        self.json.insert(T::STORE_KEY.to_owned(), json.clone());
        self.json.insert(alias, json);
        self.typed.insert(TypeId::of::<T>(), Box::new(value));
    }

    /// Reads a typed store value.
    pub fn get_store<T: BeuStore>(&self) -> Option<&T> {
        self.typed.get(&TypeId::of::<T>())?.downcast_ref::<T>()
    }

    /// Returns the serialized snapshot for a store key or its lowercase alias.
    pub fn json(&self, key: &str) -> Option<&Value> {
        self.json.get(key)
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }
}

/// Registration emitted by `#[html_shared]` or `#[html_use]`.
pub struct SharedValueRegistration {
    pub key: &'static str,
    pub alias: &'static str,
    pub snapshot: fn(&World) -> Option<Value>,
    pub changed: fn(&World) -> bool,
    pub present: fn(&World) -> bool,
}

inventory::collect!(SharedValueRegistration);

/// Serialized snapshots of registered Bevy resources.
#[derive(Resource, Default)]
pub struct UiSharedValues {
    values: BTreeMap<String, Value>,
    revision: u64,
}

impl UiSharedValues {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.values.get(key)
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }
}

pub(crate) fn install(app: &mut bevy::app::App) {
    app.init_resource::<UiBindingStore>();
    for registration in inventory::iter::<UiStoreRegistration> {
        (registration.register)(&mut app.world_mut().resource_mut::<UiBindingStore>());
    }
    app.init_resource::<UiSharedValues>();
}

pub(crate) fn refresh_shared_values(world: &mut World) {
    let registrations = inventory::iter::<SharedValueRegistration>
        .into_iter()
        .collect::<Vec<_>>();
    let needs_refresh = registrations.iter().any(|registration| {
        let present = (registration.present)(world);
        let was_present = world
            .resource::<UiSharedValues>()
            .values
            .contains_key(registration.key);
        present != was_present || (present && (registration.changed)(world))
    });
    if !needs_refresh {
        return;
    }
    let snapshots = registrations
        .into_iter()
        .filter_map(|registration| {
            (registration.snapshot)(world)
                .map(|value| (registration.key, registration.alias, value))
        })
        .collect::<Vec<_>>();
    let mut shared = world.resource_mut::<UiSharedValues>();
    let mut next = BTreeMap::new();
    for (key, alias, value) in snapshots {
        next.insert(key.to_owned(), value.clone());
        next.insert(alias.to_owned(), value);
    }
    if shared.values != next {
        shared.values = next;
        shared.revision = shared.revision.wrapping_add(1);
    }
}

fn lower_first(name: &str) -> String {
    let mut chars = name.chars();
    chars
        .next()
        .map(char::to_lowercase)
        .into_iter()
        .flatten()
        .collect::<String>()
        + chars.as_str()
}

#[cfg(test)]
mod tests {
    use bevy::{ecs::world::World, ui::widget::Text};
    use serde_json::json;
    use tilt_ui_core::TemplateUse;

    use super::{UiBindingStore, UiSharedValues};
    use crate::{ComponentStyleOwner, TiltText, component::TemplateImports};

    #[test]
    fn template_use_alias_and_wildcard_follow_shared_value_updates() {
        let mut world = World::new();
        world.init_resource::<UiBindingStore>();
        world.init_resource::<UiSharedValues>();
        let owner = world
            .spawn(TemplateImports(vec![
                TemplateUse {
                    target: "State".into(),
                    alias: "model".into(),
                    wildcard: false,
                },
                TemplateUse {
                    target: "State".into(),
                    alias: "*".into(),
                    wildcard: true,
                },
            ]))
            .id();
        let text = world
            .spawn((
                ComponentStyleOwner(owner),
                TiltText {
                    value: "{{ model.count + count }}".into(),
                },
                Text::new(""),
            ))
            .id();
        world.entity_mut(owner).add_child(text);
        world
            .resource_mut::<UiSharedValues>()
            .values
            .insert("State".into(), json!({"count": 3}));
        super::super::binding_runtime::apply_bindings(&mut world);
        assert_eq!(world.get::<Text>(text).unwrap().0, "6");
        let mut shared = world.resource_mut::<UiSharedValues>();
        shared.values.insert("State".into(), json!({"count": 5}));
        shared.revision += 1;
        super::super::binding_runtime::apply_bindings(&mut world);
        assert_eq!(world.get::<Text>(text).unwrap().0, "10");
    }
}
