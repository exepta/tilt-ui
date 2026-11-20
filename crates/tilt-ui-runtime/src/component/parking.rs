//! Explicitly parks reactivity for one retained, hidden UI subtree.

use std::collections::{HashMap, HashSet};

use bevy::ecs::query::{Or, With};
use bevy::ecs::{
    change_detection::DetectChangesMut, component::Component, entity::Entity, hierarchy::ChildOf,
    world::World,
};

use super::{PropertyBindings, TiltText, content::TemplateParent, flow::FlowNode};

/// A retained subtree whose bindings are parked. It must also be hidden by its owner.
#[derive(Component)]
pub struct ParkedUi;

struct Entry {
    entity: Entity,
    flow: Option<FlowNode>,
    properties: Option<PropertyBindings>,
}

/// Owns the removed reactive components of a single retained subtree.
///
/// This is opt-in: it neither changes visibility nor installs a per-frame system.
/// Restore before showing the subtree, or discard after its root is despawned.
#[derive(Default)]
pub struct ParkedUiBindings {
    entries: Vec<Entry>,
}

impl ParkedUiBindings {
    /// Stops flows, property expressions and interpolation, including portalled widgets.
    pub fn take(world: &mut World, root: Entity) -> Self {
        let mut children: HashMap<Entity, Vec<Entity>> = HashMap::new();
        for (entity, template, physical) in world
            .query_filtered::<(Entity, Option<&TemplateParent>, Option<&ChildOf>), Or<(With<TemplateParent>, With<bevy::ui::Node>)>>()
            .iter(world)
        {
            let parent = template
                .map(|parent| parent.0)
                .or_else(|| physical.map(ChildOf::parent));
            if let Some(parent) = parent {
                children.entry(parent).or_default().push(entity);
            }
        }
        let mut pending = vec![root];
        let mut seen = HashSet::new();
        let mut entries = Vec::new();
        while let Some(entity) = pending.pop() {
            if !seen.insert(entity) {
                continue;
            }
            if let Some(children) = children.remove(&entity) {
                pending.extend(children);
            }
            let Ok(mut node) = world.get_entity_mut(entity) else {
                continue;
            };
            let flow = node.take::<FlowNode>();
            let properties = node.take::<PropertyBindings>();
            node.insert(ParkedUi);
            entries.push(Entry {
                entity,
                flow,
                properties,
            });
        }
        Self { entries }
    }

    /// Restores the same components/entities and catches up to current state atomically.
    pub fn restore(self, world: &mut World) {
        for entry in self.entries {
            let Ok(mut node) = world.get_entity_mut(entry.entity) else {
                continue;
            };
            node.remove::<ParkedUi>();
            if let Some(mut flow) = entry.flow {
                flow.invalidate();
                node.insert(flow);
            }
            if let Some(properties) = entry.properties {
                node.insert(properties);
            }
            if let Some(mut text) = node.get_mut::<TiltText>() {
                text.set_changed();
            }
        }
    }
}
