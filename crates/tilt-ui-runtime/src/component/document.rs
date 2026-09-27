//! The single HTML5 entry document is loaded and instantiated once.

use bevy::{
    app::{App, Startup, Update},
    asset::{AssetServer, Assets, Handle},
    ecs::{component::Component, resource::Resource, world::World},
    log::error,
    prelude::IntoScheduleConfigs,
    ui::experimental::GhostNode,
};
use bevy_input_focus::tab_navigation::TabGroup;
use tilt_ui_core::{ElementKind, Template, TemplateNodeKind};
use tilt_ui_html::DocumentHead;

use crate::{UiStyleSheetAsset, UiTemplateAsset};

use super::{
    ComponentAssetHandles, ComponentAssetStore, ComponentCatalog, ComponentElementIds, StyleDirty,
    TiltUiComponentRuntimeSet,
    spawn::{instantiate_template_nodes, validate_template_node},
};

/// Global stylesheets linked by the HTML5 entry document, in source order.
#[derive(Resource, Default, Clone)]
pub(crate) struct DocumentStylesheets(pub Vec<Handle<UiStyleSheetAsset>>);

/// Metadata retained on the root entity created from `src-ui/index.html`.
#[derive(Component, Debug, Clone)]
pub struct UiDocumentInfo(pub DocumentHead);

#[derive(Resource)]
struct PendingDocument {
    handle: Handle<UiTemplateAsset>,
    parsed: Option<(Template, DocumentHead)>,
    finished: bool,
}

pub(super) fn install(app: &mut App) {
    app.add_systems(Startup, request_document).add_systems(
        Update,
        instantiate_document.after(TiltUiComponentRuntimeSet::Instantiate),
    );
}

fn request_document(
    mut commands: bevy::ecs::system::Commands,
    server: bevy::ecs::system::Res<AssetServer>,
) {
    commands.insert_resource(PendingDocument {
        handle: server.load("tilt-ui://index.html"),
        parsed: None,
        finished: false,
    });
}

fn instantiate_document(world: &mut World) {
    let Some(pending) = world.get_resource::<PendingDocument>() else {
        return;
    };
    if pending.finished {
        return;
    }
    let handle = pending.handle.clone();
    if pending.parsed.is_none() {
        let parsed = world
            .resource::<Assets<UiTemplateAsset>>()
            .get(&handle)
            .map(|asset| {
                asset
                    .document_head()
                    .cloned()
                    .map(|head| (asset.template().clone(), head))
            });
        match parsed {
            Some(Some(parsed)) => world.resource_mut::<PendingDocument>().parsed = Some(parsed),
            Some(None) => {
                error!("tilt-ui://index.html did not contain a parsed HTML5 document");
                world.resource_mut::<PendingDocument>().finished = true;
                return;
            }
            None => {
                if world
                    .resource::<AssetServer>()
                    .get_load_state(handle.id())
                    .is_some_and(|state| state.is_failed())
                {
                    error!("required tilt-ui://index.html could not be loaded");
                    world.resource_mut::<PendingDocument>().finished = true;
                }
                return;
            }
        }
    }
    let (template, head) = world
        .resource::<PendingDocument>()
        .parsed
        .as_ref()
        .unwrap()
        .clone();
    let catalog = *world.resource::<ComponentCatalog>();
    let components = world.resource::<ComponentAssetStore>().clone();
    for root in template.roots() {
        if let Err(error) = validate_template_node(
            world,
            &template,
            *root,
            catalog,
            &components,
            &mut Vec::new(),
        ) {
            if matches!(
                error,
                super::ComponentInstantiationError::TemplateAssetUnavailable { .. }
            ) {
                return;
            }
            error!("cannot instantiate tilt-ui://index.html: {error}");
            world.resource_mut::<PendingDocument>().finished = true;
            return;
        }
    }
    let styles = head
        .stylesheet_links
        .iter()
        .map(|href| {
            world
                .resource::<AssetServer>()
                .load::<UiStyleSheetAsset>(format!("tilt-ui://{}", href.trim_start_matches('/')))
        })
        .collect::<Vec<_>>();
    let empty = world
        .resource_mut::<Assets<UiStyleSheetAsset>>()
        .add(UiStyleSheetAsset::new(Default::default()));
    let root = world
        .spawn((
            GhostNode,
            TabGroup::default(),
            ComponentElementIds::default(),
            StyleDirty,
            UiDocumentInfo(head.clone()),
            ComponentAssetHandles {
                template: handle,
                stylesheet: empty,
                additional_stylesheets: Vec::new(),
            },
        ))
        .id();
    // Several examples mount their page from a Rust startup system. Their
    // required index.html only supplies document metadata; materializing its
    // empty body would add an unrelated full-window picking surface.
    let empty_body = template.roots().len() == 1
        && template.get(template.roots()[0]).is_some_and(|node| {
            matches!(node.kind, TemplateNodeKind::Element(ElementKind::Body))
                && node.children.is_empty()
                && node.attributes.is_empty()
        });
    let result = if empty_body {
        Ok(Vec::new())
    } else {
        instantiate_template_nodes(
            world,
            &template,
            catalog,
            &components,
            root,
            root,
            &mut Vec::new(),
        )
    };
    match result {
        Ok(_) => {
            world.insert_resource(DocumentStylesheets(styles));
            world.resource_mut::<PendingDocument>().finished = true;
            if let Some(title) = head.title {
                let mut windows = world.query_filtered::<&mut bevy::window::Window, bevy::ecs::query::With<bevy::window::PrimaryWindow>>();
                if let Some(mut window) = windows.iter_mut(world).next() {
                    window.title = title;
                }
            }
        }
        Err(error) => {
            error!("cannot instantiate tilt-ui://index.html: {error}");
            world.despawn(root);
            world.resource_mut::<PendingDocument>().finished = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        app::App,
        asset::{AssetPlugin, Assets},
        ecs::{entity::Entity, hierarchy::Children},
    };
    use tilt_ui_core::{ComponentId, ComponentKind, ComponentMetadata, ElementKind};
    use tilt_ui_html::{parse_document, parse_template};

    use super::*;
    use crate::{LoadedComponentAssets, TiltElement, TiltUiAssetsPlugin};

    static COMPONENTS: [ComponentMetadata; 1] = [ComponentMetadata {
        id: ComponentId(0),
        name: "app-main",
        kind: ComponentKind::Page,
        template_asset_path: "tilt-ui://pages/app-main.component.html",
        stylesheet_asset_path: "tilt-ui://pages/app-main.component.css",
        stylesheet_asset_paths: &["tilt-ui://pages/app-main.component.css"],
    }];

    fn component_id(name: &str) -> Option<ComponentId> {
        (name == "app-main").then_some(ComponentId(0))
    }

    fn metadata(id: ComponentId) -> Option<&'static ComponentMetadata> {
        (id == ComponentId(0)).then_some(&COMPONENTS[0])
    }

    #[test]
    fn mounts_document_body_and_nested_component_once() {
        let mut app = App::new();
        app.add_plugins(bevy::app::TaskPoolPlugin::default())
            .add_plugins(AssetPlugin::default())
            .add_plugins(TiltUiAssetsPlugin);
        app.world_mut()
            .insert_resource(ComponentCatalog::new(&COMPONENTS, component_id, metadata));
        let component = app
            .world_mut()
            .resource_mut::<Assets<UiTemplateAsset>>()
            .add(UiTemplateAsset::new(
                parse_template("<p>Loaded</p>").unwrap(),
            ));
        let style = app
            .world_mut()
            .resource_mut::<Assets<UiStyleSheetAsset>>()
            .add(UiStyleSheetAsset::new(Default::default()));
        let mut store = ComponentAssetStore::default();
        store.insert(
            ComponentId(0),
            LoadedComponentAssets {
                template: component,
                stylesheet: style,
                additional_stylesheets: Vec::new(),
            },
        );
        app.world_mut().insert_resource(store);
        let document = parse_document(
            "<!doctype html><html><head><meta name='test'><link rel='stylesheet' href='site.css'></head><body><app-main></app-main></body></html>",
        )
        .unwrap();
        let handle = app
            .world_mut()
            .resource_mut::<Assets<UiTemplateAsset>>()
            .add(UiTemplateAsset::from_document(document));
        app.world_mut().insert_resource(PendingDocument {
            handle,
            parsed: None,
            finished: false,
        });
        instantiate_document(app.world_mut());
        instantiate_document(app.world_mut());
        let world = app.world_mut();
        let roots = world
            .query::<(Entity, &UiDocumentInfo)>()
            .iter(world)
            .map(|(entity, _)| entity)
            .collect::<Vec<_>>();
        assert_eq!(roots.len(), 1);
        let body = world.get::<Children>(roots[0]).unwrap()[0];
        assert_eq!(
            world.get::<TiltElement>(body).unwrap().kind,
            ElementKind::Body
        );
        assert_eq!(world.resource::<DocumentStylesheets>().0.len(), 1);
        assert!(
            world
                .query::<&TiltElement>()
                .iter(world)
                .any(|element| element.kind == ElementKind::Paragraph)
        );
    }

    #[test]
    fn metadata_only_document_does_not_create_a_picking_body() {
        let mut app = App::new();
        app.add_plugins(bevy::app::TaskPoolPlugin::default())
            .add_plugins(AssetPlugin::default())
            .add_plugins(TiltUiAssetsPlugin);
        app.world_mut()
            .insert_resource(ComponentCatalog::new(&[], component_id, metadata));
        app.world_mut()
            .insert_resource(ComponentAssetStore::default());
        let document = parse_document(
            "<!doctype html><html><head><title>Showcase</title></head><body></body></html>",
        )
        .unwrap();
        let handle = app
            .world_mut()
            .resource_mut::<Assets<UiTemplateAsset>>()
            .add(UiTemplateAsset::from_document(document));
        app.world_mut().insert_resource(PendingDocument {
            handle,
            parsed: None,
            finished: false,
        });
        instantiate_document(app.world_mut());
        let world = app.world_mut();
        assert_eq!(world.query::<&UiDocumentInfo>().iter(world).count(), 1);
        assert_eq!(world.query::<&TiltElement>().iter(world).count(), 0);
        assert!(world.resource::<PendingDocument>().finished);
    }
}
