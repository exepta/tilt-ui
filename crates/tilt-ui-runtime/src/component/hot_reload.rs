//! Rebuilds only component instances whose template or stylesheet asset changed.

use std::collections::HashSet;

use bevy::{
    asset::AssetEvent,
    ecs::{
        entity::Entity,
        hierarchy::Children,
        message::{MessageCursor, Messages},
        resource::Resource,
        world::World,
    },
};

use crate::{UiStyleSheetAsset, UiTemplateAsset};

use super::{ComponentAssetHandles, ComponentInstance, PendingComponent, StyleDirty};

#[derive(Resource, Default)]
struct TemplateEvents(MessageCursor<AssetEvent<UiTemplateAsset>>);

#[derive(Resource, Default)]
struct StylesheetEvents(MessageCursor<AssetEvent<UiStyleSheetAsset>>);

pub(crate) fn install(app: &mut bevy::app::App) {
    use bevy::prelude::IntoScheduleConfigs;

    app.init_resource::<TemplateEvents>()
        .init_resource::<StylesheetEvents>()
        .add_systems(
            bevy::app::Update,
            rebuild_changed_components.before(super::TiltUiComponentRuntimeSet::Instantiate),
        );
}

fn rebuild_changed_components(world: &mut World) {
    let templates = world.resource_scope(
        |world, mut cursor: bevy::ecs::change_detection::Mut<TemplateEvents>| {
            cursor
                .0
                .read(world.resource::<Messages<AssetEvent<UiTemplateAsset>>>())
                .filter_map(|event| match event {
                    AssetEvent::Modified { id } => Some(*id),
                    _ => None,
                })
                .collect::<HashSet<_>>()
        },
    );
    let stylesheets = world.resource_scope(
        |world, mut cursor: bevy::ecs::change_detection::Mut<StylesheetEvents>| {
            cursor
                .0
                .read(world.resource::<Messages<AssetEvent<UiStyleSheetAsset>>>())
                .filter_map(|event| match event {
                    AssetEvent::Modified { id } => Some(*id),
                    _ => None,
                })
                .collect::<HashSet<_>>()
        },
    );
    if templates.is_empty() && stylesheets.is_empty() {
        return;
    }
    let affected = {
        let mut query = world.query::<(Entity, &ComponentInstance, &ComponentAssetHandles)>();
        query
            .iter(world)
            .filter_map(|(entity, instance, handles)| {
                (templates.contains(&handles.template.id())
                    || stylesheets.contains(&handles.stylesheet.id()))
                .then_some((
                    entity,
                    instance.component,
                    templates.contains(&handles.template.id()),
                ))
            })
            .collect::<Vec<_>>()
    };
    for (entity, component, template_changed) in affected {
        if world.get_entity(entity).is_err() {
            continue;
        }
        if template_changed {
            let children = world
                .get::<Children>(entity)
                .map(|children| children.to_vec())
                .unwrap_or_default();
            for child in children {
                world.despawn(child);
            }
            world
                .entity_mut(entity)
                .insert(PendingComponent { component });
        }
        world.entity_mut(entity).insert(StyleDirty);
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        asset::Handle,
        ecs::{message::Messages, world::World},
    };
    use tilt_ui_core::ComponentId;

    use super::*;

    #[test]
    fn modified_template_requeues_only_affected_instance() {
        let mut world = World::new();
        world.init_resource::<Messages<AssetEvent<UiTemplateAsset>>>();
        world.init_resource::<Messages<AssetEvent<UiStyleSheetAsset>>>();
        world.init_resource::<TemplateEvents>();
        world.init_resource::<StylesheetEvents>();
        let template = Handle::<UiTemplateAsset>::default();
        let stylesheet = Handle::<UiStyleSheetAsset>::default();
        let boundary = world
            .spawn((
                ComponentInstance {
                    component: ComponentId(7),
                },
                ComponentAssetHandles {
                    template: template.clone(),
                    stylesheet,
                },
            ))
            .id();
        let child = world.spawn_empty().id();
        world.entity_mut(boundary).add_child(child);
        world
            .resource_mut::<Messages<AssetEvent<UiTemplateAsset>>>()
            .write(AssetEvent::Modified { id: template.id() });

        rebuild_changed_components(&mut world);

        assert!(world.get_entity(child).is_err());
        assert_eq!(
            world.get::<PendingComponent>(boundary).unwrap().component,
            ComponentId(7)
        );
        assert!(world.get::<StyleDirty>(boundary).is_some());
    }
}
