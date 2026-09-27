use bevy::{
    app::{App, Plugin, Startup, Update},
    asset::{AssetServer, Assets},
    ecs::{
        entity::Entity,
        schedule::SystemSet,
        system::{Commands, Res},
    },
    log::error,
    prelude::IntoScheduleConfigs,
};
use tilt_ui_core::ComponentId;

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
            .init_resource::<ComponentAssetStore>()
            .init_resource::<crate::UiThemes>()
            .init_resource::<crate::UiProviderRegistry>()
            .add_systems(Startup, prepare_component_assets)
            .add_systems(
                Update,
                instantiate_pending_components.in_set(TiltUiComponentRuntimeSet::Instantiate),
            )
            .add_plugins((
                TiltUiStyleRuntimePlugin::default(),
                TiltUiControlRuntimePlugin,
                TiltUiScrollRuntimePlugin,
            ));
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
    asset_server: Res<'_, AssetServer>,
    mut assets: bevy::ecs::system::ResMut<'_, ComponentAssetStore>,
    mut stylesheets: bevy::ecs::system::ResMut<'_, bevy::asset::Assets<UiStyleSheetAsset>>,
) {
    for metadata in catalog.components() {
        assets.insert(
            metadata.id,
            LoadedComponentAssets {
                template: asset_server.load(metadata.template_asset_path),
                stylesheet: if metadata.stylesheet_asset_paths.is_empty() {
                    stylesheets.add(UiStyleSheetAsset::new(Default::default()))
                } else {
                    asset_server.load(metadata.stylesheet_asset_path)
                },
                additional_stylesheets: metadata
                    .stylesheet_asset_paths
                    .iter()
                    .skip(1)
                    .map(|path| asset_server.load(*path))
                    .collect(),
            },
        );
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
}
