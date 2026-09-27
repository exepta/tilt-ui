use bevy::{
    app::{App, Plugin, Startup, Update},
    asset::{AssetServer, Assets},
    ecs::{
        entity::Entity,
        hierarchy::Children,
        resource::Resource,
        schedule::SystemSet,
        system::{Commands, Res},
        world::World,
    },
    log::error,
    prelude::IntoScheduleConfigs,
};
use tilt_ui_core::ComponentId;

use crate::UiRuntimeConfiguration;
use crate::{
    TiltUiControlRuntimePlugin, TiltUiStyleRuntimePlugin, scroll::TiltUiScrollRuntimePlugin,
};
use crate::{UiStyleSheetAsset, UiTemplateAsset};

use super::spawn::instantiate_component_into_boundary;
use super::{
    ComponentAssetStore, ComponentBoundary, ComponentCatalog, ComponentInstantiationError,
    FailedComponentInstantiation, LoadedComponentAssets, PendingComponent,
};

/// Registers component asset preparation and pending template instantiation systems.
///
/// This plugin requires `TiltUiAssetsPlugin` to have initialized the TiltUI
/// template and stylesheet asset types before it is added.
#[derive(Debug, Clone, Copy)]
pub struct TiltUiComponentRuntimePlugin {
    catalog: ComponentCatalog,
}

/// Orders systems that materialize component template entities.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TiltUiComponentRuntimeSet {
    /// Instantiates pending component templates into semantic and Bevy UI entities.
    Instantiate,
}

#[derive(Resource, Default)]
struct LastComponentPath(Option<std::path::PathBuf>);
#[derive(Resource, Default)]
struct LastAssetsPath(Option<std::path::PathBuf>);

impl TiltUiComponentRuntimePlugin {
    /// Creates a component runtime plugin backed by generated static component metadata.
    pub const fn new(catalog: ComponentCatalog) -> Self {
        Self { catalog }
    }
}

impl Default for TiltUiComponentRuntimePlugin {
    fn default() -> Self {
        Self::new(ComponentCatalog::default())
    }
}

impl Plugin for TiltUiComponentRuntimePlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<AssetServer>()
            || !app.world().contains_resource::<Assets<UiTemplateAsset>>()
            || !app.world().contains_resource::<Assets<UiStyleSheetAsset>>()
        {
            error!(
                "TiltUiComponentRuntimePlugin requires TiltUiAssetsPlugin after AssetPlugin or DefaultPlugins"
            );
            return;
        }

        app.insert_resource(self.catalog)
            .init_resource::<UiRuntimeConfiguration>()
            .init_resource::<LastComponentPath>()
            .init_resource::<LastAssetsPath>()
            .init_resource::<ComponentAssetStore>()
            .init_resource::<crate::UiThemes>()
            .init_resource::<crate::UiProviderRegistry>()
            .add_systems(Startup, prepare_component_assets)
            .add_systems(
                Update,
                (
                    refresh_component_paths.before(TiltUiComponentRuntimeSet::Instantiate),
                    refresh_asset_paths.before(TiltUiComponentRuntimeSet::Instantiate),
                    instantiate_pending_components.in_set(TiltUiComponentRuntimeSet::Instantiate),
                ),
            )
            .add_plugins((
                TiltUiStyleRuntimePlugin::default(),
                TiltUiControlRuntimePlugin,
                TiltUiScrollRuntimePlugin,
            ));
        super::document::install(app);
        super::state::install(app);
        if !app.is_plugin_added::<super::handlers::TiltUiCodePlugin>() {
            app.add_plugins(super::handlers::TiltUiCodePlugin);
        }
        if app
            .world()
            .resource::<crate::UiProviderRegistry>()
            .get("theme-provider")
            .is_none()
        {
            app.world_mut()
                .resource_mut::<crate::UiProviderRegistry>()
                .register(crate::ThemeProvider);
        }
        crate::widgets::structure::form::install(app);
        crate::widgets::advanced::dialog::install(app);
        #[cfg(feature = "hot-reload")]
        super::hot_reload::install(app);
        if !app.is_plugin_added::<super::router::TiltUiRouterPlugin>() {
            app.add_plugins(super::router::TiltUiRouterPlugin);
        }
    }
}

/// Spawns a component boundary that will instantiate when its template asset is ready.
pub fn spawn_component(commands: &mut Commands<'_, '_>, component: ComponentId) -> Entity {
    commands
        .spawn((
            ComponentBoundary::new(component),
            PendingComponent { component },
        ))
        .id()
}

fn prepare_component_assets(
    catalog: Res<'_, ComponentCatalog>,
    config: Res<'_, UiRuntimeConfiguration>,
    asset_server: Res<'_, AssetServer>,
    mut assets: bevy::ecs::system::ResMut<'_, ComponentAssetStore>,
    mut stylesheets: bevy::ecs::system::ResMut<'_, bevy::asset::Assets<UiStyleSheetAsset>>,
    mut last_path: bevy::ecs::system::ResMut<'_, LastComponentPath>,
    mut last_assets_path: bevy::ecs::system::ResMut<'_, LastAssetsPath>,
) {
    last_path.0 = Some(config.components_path.clone());
    last_assets_path.0 = Some(config.assets_path.clone());
    load_component_assets(
        *catalog,
        &config,
        &asset_server,
        &mut assets,
        &mut stylesheets,
    );
}

fn refresh_asset_paths(world: &mut World) {
    let config = world.resource::<UiRuntimeConfiguration>();
    if let Err(error) = config.validate() {
        error!("invalid TiltUI runtime configuration: {error}");
        return;
    }
    let path = config.assets_path.clone();
    if world.resource::<LastAssetsPath>().0.as_ref() == Some(&path) {
        return;
    }
    world.resource_mut::<LastAssetsPath>().0 = Some(path);
    let images = world
        .query::<(Entity, &crate::ImageMetadata)>()
        .iter(world)
        .filter_map(|(entity, metadata)| {
            metadata
                .source
                .as_ref()
                .map(|source| (entity, source.clone()))
        })
        .collect::<Vec<_>>();
    for (entity, source) in images {
        let handle = crate::widgets::content::image::load_image_handle(world, &source);
        if let Some(mut node) = world.get_mut::<bevy::ui::widget::ImageNode>(entity) {
            node.image = handle.unwrap_or_default();
        }
    }
}

fn load_component_assets(
    catalog: ComponentCatalog,
    config: &UiRuntimeConfiguration,
    asset_server: &AssetServer,
    assets: &mut ComponentAssetStore,
    stylesheets: &mut bevy::asset::Assets<UiStyleSheetAsset>,
) {
    for metadata in catalog.components() {
        assets.insert(
            metadata.id,
            LoadedComponentAssets {
                template: asset_server.load(config.component_path(metadata.template_asset_path)),
                stylesheet: if metadata.stylesheet_asset_paths.is_empty() {
                    stylesheets.add(UiStyleSheetAsset::new(Default::default()))
                } else {
                    asset_server.load(config.component_path(metadata.stylesheet_asset_path))
                },
                additional_stylesheets: metadata
                    .stylesheet_asset_paths
                    .iter()
                    .skip(1)
                    .map(|path| asset_server.load(config.component_path(path)))
                    .collect(),
            },
        );
    }
}

fn refresh_component_paths(world: &mut World) {
    let config = world.resource::<UiRuntimeConfiguration>().clone();
    if let Err(error) = config.validate() {
        error!("invalid TiltUI runtime configuration: {error}");
        return;
    }
    if world.resource::<LastComponentPath>().0.as_ref() == Some(&config.components_path) {
        return;
    }
    world.resource_mut::<LastComponentPath>().0 = Some(config.components_path.clone());
    let catalog = *world.resource::<ComponentCatalog>();
    let server = world.resource::<AssetServer>().clone();
    world.resource_scope(
        |world, mut store: bevy::ecs::change_detection::Mut<ComponentAssetStore>| {
            let mut stylesheets = world.resource_mut::<bevy::asset::Assets<UiStyleSheetAsset>>();
            load_component_assets(catalog, &config, &server, &mut store, &mut stylesheets);
        },
    );
    let instances = world
        .query::<(Entity, &super::ComponentInstance)>()
        .iter(world)
        .map(|(entity, instance)| (entity, instance.component))
        .collect::<Vec<_>>();
    for (entity, component) in instances {
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
            .remove::<super::ComponentAssetHandles>()
            .insert(PendingComponent { component });
        super::state::reload_component(world, entity);
    }
}

fn instantiate_pending_components(world: &mut bevy::ecs::world::World) {
    let pending = {
        let mut query = world.query::<(Entity, &PendingComponent)>();
        query
            .iter(world)
            .map(|(entity, pending)| (entity, pending.component))
            .collect::<Vec<_>>()
    };
    if pending.is_empty() {
        return;
    }
    let catalog = *world.resource::<ComponentCatalog>();
    let assets = world.resource::<ComponentAssetStore>().clone();

    for (boundary, component) in pending {
        match instantiate_component_into_boundary(world, catalog, &assets, component, boundary) {
            Ok(()) => {
                world.entity_mut(boundary).remove::<PendingComponent>();
            }
            Err(ComponentInstantiationError::TemplateAssetUnavailable {
                component: unavailable,
            }) => {
                let Some(loaded) = assets.get(unavailable) else {
                    record_failure(
                        world,
                        boundary,
                        ComponentInstantiationError::ComponentAssetsUnavailable {
                            component: unavailable,
                        },
                    );
                    continue;
                };
                if world
                    .resource::<AssetServer>()
                    .get_load_state(loaded.template.id())
                    .is_some_and(|state| state.is_failed())
                {
                    record_failure(
                        world,
                        boundary,
                        ComponentInstantiationError::TemplateAssetFailed {
                            component: unavailable,
                        },
                    );
                }
            }
            Err(error) => record_failure(world, boundary, error),
        }
    }
}

fn record_failure(
    world: &mut bevy::ecs::world::World,
    boundary: Entity,
    error: ComponentInstantiationError,
) {
    world
        .entity_mut(boundary)
        .remove::<PendingComponent>()
        .insert(FailedComponentInstantiation { error });
    // Retain the handles so a later asset modification can requeue this boundary.
    if let Some(component) = world
        .get::<super::ComponentInstance>(boundary)
        .map(|instance| instance.component)
        && let Some(loaded) = world
            .resource::<ComponentAssetStore>()
            .get(component)
            .cloned()
    {
        world
            .entity_mut(boundary)
            .insert(super::ComponentAssetHandles {
                template: loaded.template,
                stylesheet: loaded.stylesheet,
                additional_stylesheets: loaded.additional_stylesheets,
            });
    }
}
