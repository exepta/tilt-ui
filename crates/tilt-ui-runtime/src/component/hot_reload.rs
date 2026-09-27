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

use super::{
    ComponentAssetHandles, ComponentInstance, FailedComponentInstantiation, PendingComponent,
    StyleDirty,
};

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
    let (templates, added_templates) = world.resource_scope(
        |world, mut cursor: bevy::ecs::change_detection::Mut<TemplateEvents>| {
            let mut modified = HashSet::new();
            let mut added = HashSet::new();
            for event in cursor
                .0
                .read(world.resource::<Messages<AssetEvent<UiTemplateAsset>>>())
            {
                match event {
                    AssetEvent::Modified { id } => {
                        modified.insert(*id);
                    }
                    AssetEvent::Added { id } => {
                        added.insert(*id);
                    }
                    _ => {}
                }
            }
            (modified, added)
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
    if templates.is_empty() && added_templates.is_empty() && stylesheets.is_empty() {
        return;
    }
    let affected = {
        let mut query = world.query::<(
            Entity,
            &ComponentInstance,
            &ComponentAssetHandles,
            Option<&FailedComponentInstantiation>,
        )>();
        query
            .iter(world)
            .filter_map(|(entity, instance, handles, failed)| {
                let template_changed = templates.contains(&handles.template.id())
                    || (failed.is_some() && added_templates.contains(&handles.template.id()));
                (template_changed
                    || stylesheets.contains(&handles.stylesheet.id())
                    || handles
                        .additional_stylesheets
                        .iter()
                        .any(|handle| stylesheets.contains(&handle.id())))
                .then_some((entity, instance.component, template_changed))
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
                super::state::hide_before_despawn(world, child);
                world.despawn(child);
            }
            world
                .entity_mut(entity)
                .remove::<FailedComponentInstantiation>()
                .insert(PendingComponent { component });
        }
        world.entity_mut(entity).insert(StyleDirty);
        super::state::reload_component(world, entity);
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
                    additional_stylesheets: Vec::new(),
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

    #[test]
    fn modifying_a_secondary_stylesheet_marks_its_component_dirty() {
        let mut world = World::new();
        world.insert_resource(bevy::asset::Assets::<UiStyleSheetAsset>::default());
        world.init_resource::<Messages<AssetEvent<UiTemplateAsset>>>();
        world.init_resource::<Messages<AssetEvent<UiStyleSheetAsset>>>();
        world.init_resource::<TemplateEvents>();
        world.init_resource::<StylesheetEvents>();
        let second = world
            .resource_mut::<bevy::asset::Assets<UiStyleSheetAsset>>()
            .add(UiStyleSheetAsset::new(Default::default()));
        let owner = world
            .spawn((
                ComponentInstance {
                    component: ComponentId(0),
                },
                ComponentAssetHandles {
                    template: Handle::default(),
                    stylesheet: Handle::default(),
                    additional_stylesheets: vec![second.clone()],
                },
            ))
            .id();
        world
            .resource_mut::<Messages<AssetEvent<UiStyleSheetAsset>>>()
            .write(AssetEvent::Modified { id: second.id() });
        rebuild_changed_components(&mut world);
        assert!(world.get::<StyleDirty>(owner).is_some());
        assert!(world.get::<PendingComponent>(owner).is_none());
    }

    #[test]
    fn first_successful_load_retries_a_failed_component() {
        let mut world = World::new();
        world.init_resource::<Messages<AssetEvent<UiTemplateAsset>>>();
        world.init_resource::<Messages<AssetEvent<UiStyleSheetAsset>>>();
        world.init_resource::<TemplateEvents>();
        world.init_resource::<StylesheetEvents>();
        let template = Handle::<UiTemplateAsset>::default();
        let owner = world
            .spawn((
                ComponentInstance {
                    component: ComponentId(9),
                },
                ComponentAssetHandles {
                    template: template.clone(),
                    stylesheet: Handle::default(),
                    additional_stylesheets: Vec::new(),
                },
                FailedComponentInstantiation {
                    error: super::super::ComponentInstantiationError::TemplateAssetFailed {
                        component: ComponentId(9),
                    },
                },
            ))
            .id();
        world
            .resource_mut::<Messages<AssetEvent<UiTemplateAsset>>>()
            .write(AssetEvent::Added { id: template.id() });

        rebuild_changed_components(&mut world);

        assert!(world.get::<FailedComponentInstantiation>(owner).is_none());
        assert!(world.get::<PendingComponent>(owner).is_some());
    }
}
