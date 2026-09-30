//! The single HTML5 entry document is loaded and instantiated once.

use bevy::{
    app::{App, Startup, Update},
    asset::{AssetServer, Assets, Handle},
    ecs::{component::Component, resource::Resource, world::World},
    log::error,
    prelude::IntoScheduleConfigs,
    ui::experimental::GhostNode,
};
#[cfg(feature = "hot-reload")]
use bevy::{
    asset::AssetEvent,
    ecs::message::{MessageCursor, Messages},
};
use bevy_input_focus::tab_navigation::TabGroup;
use tilt_ui_core::{ElementKind, Template, TemplateNodeKind};
use tilt_ui_html::DocumentHead;

use crate::{UiStyleSheetAsset, UiTemplateAsset};

use super::{
    ComponentAssetHandles, ComponentAssetStore, ComponentCatalog, ComponentElementIds, StyleDirty,
    TiltUiComponentRuntimeSet, UiErrorCode, UiStateError,
    spawn::{instantiate_template_nodes, validate_template_node},
};

/// Global stylesheets linked by the HTML5 entry document, in source order.
#[derive(Resource, Default, Clone)]
pub(crate) struct DocumentStylesheets(pub Vec<Handle<UiStyleSheetAsset>>);

/// Metadata retained on the root entity created from `src-ui/index.html`.
#[derive(Component, Debug, Clone)]
pub struct UiDocumentInfo(pub DocumentHead);

#[derive(Resource)]
pub(super) struct PendingDocument {
    pub(super) handle: Handle<UiTemplateAsset>,
    pub(super) parsed: Option<(Template, DocumentHead)>,
    pub(super) finished: bool,
    pub(super) failure: Option<UiStateError>,
}

pub(super) fn install(app: &mut App) {
    app.add_systems(Startup, request_document).add_systems(
        Update,
        instantiate_document
            .after(TiltUiComponentRuntimeSet::Instantiate)
            .before(super::router::sync_outlets)
            .before(super::flow::update_flow),
    );
    #[cfg(feature = "hot-reload")]
    app.init_resource::<DocumentTemplateEvents>()
        .init_resource::<DocumentStylesheetEvents>()
        .add_systems(
            Update,
            rebuild_changed_document.before(TiltUiComponentRuntimeSet::Instantiate),
        );
}

#[cfg(feature = "hot-reload")]
#[derive(Resource, Default)]
struct DocumentTemplateEvents(MessageCursor<AssetEvent<UiTemplateAsset>>);

#[cfg(feature = "hot-reload")]
#[derive(Resource, Default)]
struct DocumentStylesheetEvents(MessageCursor<AssetEvent<UiStyleSheetAsset>>);

#[cfg(feature = "hot-reload")]
fn rebuild_changed_document(world: &mut World) {
    let Some(handle) = world
        .get_resource::<PendingDocument>()
        .map(|pending| pending.handle.clone())
    else {
        return;
    };
    let retry_failed_document = world.resource::<PendingDocument>().failure.is_some();
    let template_changed = world.resource_scope(
        |world, mut cursor: bevy::ecs::change_detection::Mut<DocumentTemplateEvents>| {
            cursor
                .0
                .read(world.resource::<Messages<AssetEvent<UiTemplateAsset>>>())
                .any(|event| match event {
                    AssetEvent::Modified { id } => *id == handle.id(),
                    AssetEvent::Added { id } => retry_failed_document && *id == handle.id(),
                    _ => false,
                })
        },
    );
    let stylesheet_changed = world.resource_scope(
        |world, mut cursor: bevy::ecs::change_detection::Mut<DocumentStylesheetEvents>| {
            cursor
                .0
                .read(world.resource::<Messages<AssetEvent<UiStyleSheetAsset>>>())
                .any(|event| {
                    matches!(event, AssetEvent::Modified { id }
                if world.get_resource::<DocumentStylesheets>()
                    .is_some_and(|styles| styles.0.iter().any(|handle| handle.id() == *id)))
                })
        },
    );
    if !template_changed && !stylesheet_changed {
        return;
    }
    let mut state = std::mem::take(&mut world.resource_mut::<super::UiDocumentState>().0);
    if template_changed {
        let roots = {
            let mut query = world.query_filtered::<bevy::ecs::entity::Entity, bevy::ecs::query::With<UiDocumentInfo>>();
            query.iter(world).collect::<Vec<_>>()
        };
        for root in roots {
            super::state::hide_before_despawn(world, root);
            world.despawn(root);
        }
        if state.visible == Some(true) {
            super::state::transition(
                world,
                super::UiStateTarget::Document,
                &mut state,
                super::UiStateChange::Hidden,
            );
        }
        world.insert_resource(DocumentStylesheets::default());
        let mut pending = world.resource_mut::<PendingDocument>();
        pending.parsed = None;
        pending.finished = false;
        pending.failure = None;
    }
    super::state::transition(
        world,
        super::UiStateTarget::Document,
        &mut state,
        super::UiStateChange::Loading,
    );
    world.resource_mut::<super::UiDocumentState>().0 = state;
}

fn request_document(
    mut commands: bevy::ecs::system::Commands,
    server: bevy::ecs::system::Res<AssetServer>,
) {
    commands.insert_resource(PendingDocument {
        handle: server.load("tilt-ui://index.html"),
        parsed: None,
        finished: false,
        failure: None,
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
                let mut pending = world.resource_mut::<PendingDocument>();
                pending.finished = true;
                pending.failure = Some(UiStateError {
                    code: UiErrorCode::InvalidDocument,
                    message: "tilt-ui://index.html did not contain a parsed HTML5 document".into(),
                });
                return;
            }
            None => {
                if let Some(bevy::asset::LoadState::Failed(error)) =
                    world.resource::<AssetServer>().get_load_state(handle.id())
                {
                    error!("required tilt-ui://index.html could not be loaded");
                    let mut pending = world.resource_mut::<PendingDocument>();
                    pending.finished = true;
                    pending.failure = Some(UiStateError {
                        code: UiErrorCode::DocumentLoad,
                        message: error.to_string(),
                    });
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
            if let super::ComponentInstantiationError::TemplateAssetUnavailable { component } =
                error
            {
                if let Some(loaded) = components.get(component)
                    && let Some(bevy::asset::LoadState::Failed(load_error)) = world
                        .resource::<AssetServer>()
                        .get_load_state(loaded.template.id())
                {
                    let mut pending = world.resource_mut::<PendingDocument>();
                    pending.finished = true;
                    pending.failure = Some(UiStateError {
                        code: UiErrorCode::TemplateLoad,
                        message: load_error.to_string(),
                    });
                }
                return;
            }
            error!("cannot instantiate tilt-ui://index.html: {error}");
            let mut pending = world.resource_mut::<PendingDocument>();
            pending.finished = true;
            pending.failure = Some(UiStateError {
                code: UiErrorCode::Instantiation,
                message: error.to_string(),
            });
            return;
        }
    }
    #[cfg(feature = "fluent")]
    if let Some(lang) = head.lang.as_deref()
        && let Some(mut localization) = world.get_resource_mut::<crate::UiLocalization>()
        && let Err(error) = localization.set_locale(lang)
    {
        bevy::log::warn!("Ignoring invalid document language {lang:?}: {error}");
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
            let mut pending = world.resource_mut::<PendingDocument>();
            pending.finished = true;
            pending.failure = Some(UiStateError {
                code: UiErrorCode::Instantiation,
                message: error.to_string(),
            });
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
            failure: None,
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
            failure: None,
        });
        instantiate_document(app.world_mut());
        let world = app.world_mut();
        assert_eq!(world.query::<&UiDocumentInfo>().iter(world).count(), 1);
        assert_eq!(world.query::<&TiltElement>().iter(world).count(), 0);
        assert!(world.resource::<PendingDocument>().finished);
    }

    #[cfg(feature = "fluent")]
    #[test]
    fn document_language_overrides_the_preferred_startup_language() {
        let mut app = App::new();
        app.add_plugins(bevy::app::TaskPoolPlugin::default())
            .add_plugins(AssetPlugin::default())
            .add_plugins(TiltUiAssetsPlugin);
        app.world_mut()
            .insert_resource(ComponentCatalog::new(&[], component_id, metadata));
        app.world_mut()
            .insert_resource(ComponentAssetStore::default());
        let mut localization = crate::UiLocalization::new("en-US").unwrap();
        localization.set_locale("fr-FR").unwrap();
        app.world_mut().insert_resource(localization);
        let document =
            parse_document("<!doctype html><html lang='de-DE'><head></head><body></body></html>")
                .unwrap();
        let handle = app
            .world_mut()
            .resource_mut::<Assets<UiTemplateAsset>>()
            .add(UiTemplateAsset::from_document(document));
        app.world_mut().insert_resource(PendingDocument {
            handle,
            parsed: None,
            finished: false,
            failure: None,
        });
        instantiate_document(app.world_mut());
        assert_eq!(
            app.world()
                .resource::<crate::UiLocalization>()
                .locale()
                .to_string(),
            "de-DE"
        );
    }

    #[cfg(feature = "hot-reload")]
    #[test]
    fn modified_document_reports_hidden_then_loading_and_requeues() {
        use crate::{UiDocumentState, UiLoadState, UiState, UiStateChange, UiStateEvent};

        let mut world = World::new();
        world.init_resource::<Messages<AssetEvent<UiTemplateAsset>>>();
        world.init_resource::<Messages<AssetEvent<UiStyleSheetAsset>>>();
        world.init_resource::<Messages<UiStateEvent>>();
        world.init_resource::<DocumentTemplateEvents>();
        world.init_resource::<DocumentStylesheetEvents>();
        let document = UiTemplateAsset::from_document(
            parse_document("<!doctype html><html><body>Before</body></html>").unwrap(),
        );
        let root = world
            .spawn((
                GhostNode,
                UiDocumentInfo(document.document_head().unwrap().clone()),
            ))
            .id();
        let handle = Handle::<UiTemplateAsset>::default();
        world.insert_resource(PendingDocument {
            handle: handle.clone(),
            parsed: None,
            finished: true,
            failure: None,
        });
        world.insert_resource(UiDocumentState(UiState {
            load: UiLoadState::Ready,
            visible: Some(true),
            announced: true,
        }));
        world
            .resource_mut::<Messages<AssetEvent<UiTemplateAsset>>>()
            .write(AssetEvent::Modified { id: handle.id() });

        rebuild_changed_document(&mut world);

        assert!(world.get_entity(root).is_err());
        assert!(!world.resource::<PendingDocument>().finished);
        assert!(world.resource::<PendingDocument>().parsed.is_none());
        assert_eq!(
            world.resource::<UiDocumentState>().0.load,
            UiLoadState::Loading
        );
        assert_eq!(world.resource::<UiDocumentState>().0.visible, Some(false));
        assert_eq!(
            world
                .resource_mut::<Messages<UiStateEvent>>()
                .drain()
                .map(UiStateEvent::change)
                .collect::<Vec<_>>(),
            vec![UiStateChange::Hidden, UiStateChange::Loading]
        );
    }
}
