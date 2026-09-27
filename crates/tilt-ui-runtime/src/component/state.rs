//! Public lifecycle state for the entry document and component instances.

use bevy::{
    app::{App, Update},
    asset::{AssetServer, Assets, Handle, LoadState},
    ecs::{
        component::Component,
        entity::Entity,
        hierarchy::{ChildOf, Children},
        message::{Message, Messages},
        resource::Resource,
        schedule::SystemSet,
        world::World,
    },
    prelude::IntoScheduleConfigs,
    prelude::Visibility,
    ui::{Display, Node},
};

use crate::{UiStyleSheetAsset, UiTemplateAsset};

use super::{
    ComponentAssetHandles, ComponentInstance, FailedComponentInstantiation, PendingComponent,
    StyleDirty, UiDocumentInfo,
    document::{DocumentStylesheets, PendingDocument},
};

/// Stable category of a loading or instantiation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiErrorCode {
    DocumentLoad,
    InvalidDocument,
    TemplateLoad,
    StylesheetLoad,
    Instantiation,
}

/// Failure retained in the current UI state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiStateError {
    pub code: UiErrorCode,
    pub message: String,
}

/// The durable loading phase. Visibility is tracked separately in [`UiState`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum UiLoadState {
    #[default]
    Loading,
    Loaded,
    Ready,
    Error(UiStateError),
}

/// Current state of a component boundary, readable with `Query<&UiState>`.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct UiState {
    pub load: UiLoadState,
    /// `None` until the first readiness check; afterwards reflects effective visibility.
    pub visible: Option<bool>,
    pub(crate) announced: bool,
}

/// Current state of `src-ui/index.html`, readable with `Res<UiDocumentState>`.
#[derive(Resource, Debug, Clone, Default)]
pub struct UiDocumentState(pub UiState);

/// Identifies the document or the component boundary that changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiStateTarget {
    Document,
    Component(Entity),
}

/// A distinct transition in an instance's lifecycle, used internally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UiStateChange {
    Loading,
    Loaded,
    Ready,
    Visible,
    Hidden,
    Error { code: UiErrorCode, message: String },
}

/// Read state changes with `MessageReader<UiStateEvent>`.
/// Every variant identifies the affected document or component instance.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub enum UiStateEvent {
    Loading(UiStateTarget),
    Loaded(UiStateTarget),
    Ready(UiStateTarget),
    Visible(UiStateTarget),
    Hidden(UiStateTarget),
    Error {
        target: UiStateTarget,
        code: UiErrorCode,
        message: String,
    },
}

impl UiStateEvent {
    /// Returns the document or component boundary described by this event.
    pub fn target(&self) -> UiStateTarget {
        match self {
            Self::Loading(target)
            | Self::Loaded(target)
            | Self::Ready(target)
            | Self::Visible(target)
            | Self::Hidden(target)
            | Self::Error { target, .. } => *target,
        }
    }

    fn from_change(target: UiStateTarget, change: UiStateChange) -> Self {
        match change {
            UiStateChange::Loading => Self::Loading(target),
            UiStateChange::Loaded => Self::Loaded(target),
            UiStateChange::Ready => Self::Ready(target),
            UiStateChange::Visible => Self::Visible(target),
            UiStateChange::Hidden => Self::Hidden(target),
            UiStateChange::Error { code, message } => Self::Error {
                target,
                code,
                message,
            },
        }
    }

    #[cfg(test)]
    pub(crate) fn change(self) -> UiStateChange {
        match self {
            Self::Loading(_) => UiStateChange::Loading,
            Self::Loaded(_) => UiStateChange::Loaded,
            Self::Ready(_) => UiStateChange::Ready,
            Self::Visible(_) => UiStateChange::Visible,
            Self::Hidden(_) => UiStateChange::Hidden,
            Self::Error { code, message, .. } => UiStateChange::Error { code, message },
        }
    }
}

/// Order application systems after this set to receive transitions in the same update.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiStateRuntimeSet {
    Observe,
}

pub(super) fn install(app: &mut App) {
    app.add_message::<UiStateEvent>()
        .init_resource::<UiDocumentState>()
        .add_systems(
            Update,
            observe_ui_states
                .in_set(UiStateRuntimeSet::Observe)
                .after(super::TiltUiComponentRuntimeSet::Instantiate)
                .after(super::router::sync_outlets)
                .after(crate::style::TiltUiStyleRuntimeSet::Apply),
        );
}

pub(crate) fn transition(
    world: &mut World,
    target: UiStateTarget,
    state: &mut UiState,
    change: UiStateChange,
) {
    let changed = match &change {
        UiStateChange::Loading => {
            if state.load == UiLoadState::Loading && state.announced {
                false
            } else {
                state.load = UiLoadState::Loading;
                true
            }
        }
        UiStateChange::Loaded => {
            if matches!(state.load, UiLoadState::Loaded | UiLoadState::Ready) {
                false
            } else {
                state.load = UiLoadState::Loaded;
                true
            }
        }
        UiStateChange::Ready => {
            if state.load == UiLoadState::Ready {
                false
            } else {
                state.load = UiLoadState::Ready;
                true
            }
        }
        UiStateChange::Visible | UiStateChange::Hidden => {
            let visible = matches!(change, UiStateChange::Visible);
            if state.visible == Some(visible) {
                false
            } else {
                state.visible = Some(visible);
                true
            }
        }
        UiStateChange::Error { code, message } => {
            let error = UiStateError {
                code: *code,
                message: message.clone(),
            };
            if state.load == UiLoadState::Error(error.clone()) {
                false
            } else {
                state.load = UiLoadState::Error(error);
                true
            }
        }
    };
    state.announced = true;
    if changed && let Some(mut messages) = world.get_resource_mut::<Messages<UiStateEvent>>() {
        messages.write(UiStateEvent::from_change(target, change));
    }
}

/// Moves an existing instance back to Loading, for example on hot reload.
pub(crate) fn reload_component(world: &mut World, entity: Entity) {
    if let Some(original) = world.get::<UiState>(entity).cloned() {
        let mut state = original.clone();
        transition(
            world,
            UiStateTarget::Component(entity),
            &mut state,
            UiStateChange::Loading,
        );
        if state != original {
            *world.get_mut::<UiState>(entity).unwrap() = state;
        }
    }
}

/// Reports the loss of visibility for component instances about to be removed.
pub(crate) fn hide_before_despawn(world: &mut World, root: Entity) {
    let mut pending = vec![root];
    while let Some(entity) = pending.pop() {
        if let Some(children) = world.get::<Children>(entity) {
            pending.extend(children.iter());
        }
        if let Some(original) = world.get::<UiState>(entity).cloned() {
            let mut state = original;
            if state.visible == Some(true) {
                transition(
                    world,
                    UiStateTarget::Component(entity),
                    &mut state,
                    UiStateChange::Hidden,
                );
            }
        }
    }
}

fn observe_ui_states(world: &mut World) {
    let boundaries = {
        let mut query = world.query::<(Entity, &ComponentInstance)>();
        query
            .iter(world)
            .map(|(entity, _)| entity)
            .collect::<Vec<_>>()
    };
    let global_failure = global_stylesheet_failure(world);
    let global_available = global_styles_available(world);
    for entity in boundaries {
        let original = world.get::<UiState>(entity).cloned();
        let mut state = original.clone().unwrap_or_default();
        let target = UiStateTarget::Component(entity);
        if !state.announced {
            transition(world, target, &mut state, UiStateChange::Loading);
        }
        if let Some(failure) = world.get::<FailedComponentInstantiation>(entity) {
            let code = match failure.error {
                super::ComponentInstantiationError::TemplateAssetFailed { .. } => {
                    UiErrorCode::TemplateLoad
                }
                _ => UiErrorCode::Instantiation,
            };
            let change = world
                .get::<ComponentAssetHandles>(entity)
                .and_then(|handles| component_asset_failure(world, handles))
                .unwrap_or_else(|| UiStateChange::Error {
                    code,
                    message: failure.error.to_string(),
                });
            transition(world, target, &mut state, change);
        } else if let Some(failure) = &global_failure {
            transition(world, target, &mut state, failure.clone());
        } else if let Some((failure, available)) =
            world.get::<ComponentAssetHandles>(entity).map(|handles| {
                (
                    component_asset_failure(world, handles),
                    component_assets_available(world, handles) && global_available,
                )
            })
        {
            if let Some(failure) = failure {
                transition(world, target, &mut state, failure);
            } else if available {
                if matches!(state.load, UiLoadState::Error(_)) {
                    transition(world, target, &mut state, UiStateChange::Loading);
                }
                transition(world, target, &mut state, UiStateChange::Loaded);
                if world.get::<PendingComponent>(entity).is_none()
                    && world.get::<StyleDirty>(entity).is_none()
                    && !crate::style::author_style_pending(world, entity)
                {
                    transition(world, target, &mut state, UiStateChange::Ready);
                }
            }
        }
        if state.load == UiLoadState::Ready {
            let change = if effective_visibility(world, entity) {
                UiStateChange::Visible
            } else {
                UiStateChange::Hidden
            };
            transition(world, target, &mut state, change);
        }
        if original.as_ref() != Some(&state) {
            world.entity_mut(entity).insert(state);
        }
    }
    observe_document(world);
}

fn observe_document(world: &mut World) {
    let original = world.resource::<UiDocumentState>().0.clone();
    let mut state = original.clone();
    let target = UiStateTarget::Document;
    if !state.announced {
        transition(world, target, &mut state, UiStateChange::Loading);
    }
    if let Some(pending) = world.get_resource::<PendingDocument>() {
        if let Some(error) = pending.failure.clone() {
            transition(
                world,
                target,
                &mut state,
                UiStateChange::Error {
                    code: error.code,
                    message: error.message,
                },
            );
        } else {
            let root = {
                let mut query = world.query::<(Entity, &UiDocumentInfo)>();
                query.iter(world).map(|(entity, _)| entity).next()
            };
            if let Some(root) = root {
                let descendants = descendant_status(world, root);
                if let Some(failure) = document_asset_failure(world) {
                    transition(world, target, &mut state, failure);
                } else if let Some(error) = descendants.error {
                    transition(
                        world,
                        target,
                        &mut state,
                        UiStateChange::Error {
                            code: error.code,
                            message: error.message,
                        },
                    );
                } else {
                    if matches!(state.load, UiLoadState::Error(_))
                        || (state.load == UiLoadState::Ready && !descendants.ready)
                    {
                        transition(world, target, &mut state, UiStateChange::Loading);
                    }
                    if !document_assets_available(world, root) || !descendants.assets_available {
                        if state != original {
                            world.resource_mut::<UiDocumentState>().0 = state;
                        }
                        return;
                    }
                    transition(world, target, &mut state, UiStateChange::Loaded);
                    if world.get::<StyleDirty>(root).is_none()
                        && !crate::style::author_style_pending(world, root)
                        && descendants.ready
                    {
                        transition(world, target, &mut state, UiStateChange::Ready);
                    }
                    if state.load == UiLoadState::Ready {
                        let change = if effective_visibility(world, root) {
                            UiStateChange::Visible
                        } else {
                            UiStateChange::Hidden
                        };
                        transition(world, target, &mut state, change);
                    }
                }
            }
        }
    }
    if state != original {
        world.resource_mut::<UiDocumentState>().0 = state;
    }
}

fn component_assets_available(world: &World, handles: &ComponentAssetHandles) -> bool {
    let templates = world.resource::<Assets<UiTemplateAsset>>();
    let styles = world.resource::<Assets<UiStyleSheetAsset>>();
    templates.get(&handles.template).is_some()
        && styles.get(&handles.stylesheet).is_some()
        && handles
            .additional_stylesheets
            .iter()
            .all(|handle| styles.get(handle).is_some())
}

fn component_asset_failure(
    world: &World,
    handles: &ComponentAssetHandles,
) -> Option<UiStateChange> {
    let server = world.resource::<AssetServer>();
    failed_asset(server, &handles.template, UiErrorCode::TemplateLoad)
        .or_else(|| failed_asset(server, &handles.stylesheet, UiErrorCode::StylesheetLoad))
        .or_else(|| {
            handles
                .additional_stylesheets
                .iter()
                .find_map(|handle| failed_asset(server, handle, UiErrorCode::StylesheetLoad))
        })
}

fn failed_asset<A: bevy::asset::Asset>(
    server: &AssetServer,
    handle: &Handle<A>,
    code: UiErrorCode,
) -> Option<UiStateChange> {
    match server.get_load_state(handle.id()) {
        Some(LoadState::Failed(error)) => Some(UiStateChange::Error {
            code,
            message: error.to_string(),
        }),
        _ => None,
    }
}

fn document_asset_failure(world: &World) -> Option<UiStateChange> {
    let server = world.resource::<AssetServer>();
    if let Some(pending) = world.get_resource::<PendingDocument>()
        && let Some(failure) = failed_asset(server, &pending.handle, UiErrorCode::DocumentLoad)
    {
        return Some(failure);
    }
    global_stylesheet_failure(world)
}

fn global_stylesheet_failure(world: &World) -> Option<UiStateChange> {
    let server = world.resource::<AssetServer>();
    let styles = world.get_resource::<DocumentStylesheets>()?;
    styles
        .0
        .iter()
        .find_map(|handle| failed_asset(server, handle, UiErrorCode::StylesheetLoad))
}

fn global_styles_available(world: &World) -> bool {
    let Some(pending) = world.get_resource::<PendingDocument>() else {
        return true;
    };
    if pending.failure.is_some() {
        return true;
    }
    let Some(styles) = world.get_resource::<DocumentStylesheets>() else {
        return false;
    };
    let assets = world.resource::<Assets<UiStyleSheetAsset>>();
    styles.0.iter().all(|handle| assets.get(handle).is_some())
}

fn document_assets_available(world: &World, root: Entity) -> bool {
    let Some(styles) = world.get_resource::<DocumentStylesheets>() else {
        return false;
    };
    let assets = world.resource::<Assets<UiStyleSheetAsset>>();
    styles.0.iter().all(|handle| assets.get(handle).is_some())
        && world
            .get::<ComponentAssetHandles>(root)
            .is_some_and(|handles| component_assets_available(world, handles))
}

struct DescendantStatus {
    assets_available: bool,
    ready: bool,
    error: Option<UiStateError>,
}

fn descendant_status(world: &mut World, root: Entity) -> DescendantStatus {
    let mut status = DescendantStatus {
        assets_available: true,
        ready: true,
        error: None,
    };
    let mut query = world.query::<(
        Entity,
        &ComponentInstance,
        Option<&ComponentAssetHandles>,
        &UiState,
    )>();
    for (entity, _, handles, state) in query.iter(world) {
        if !is_descendant_of(world, entity, root) {
            continue;
        }
        status.assets_available &=
            handles.is_some_and(|handles| component_assets_available(world, handles));
        status.ready &= state.load == UiLoadState::Ready;
        if status.error.is_none()
            && let UiLoadState::Error(error) = &state.load
        {
            status.error = Some(UiStateError {
                code: error.code,
                message: format!("component {entity:?}: {}", error.message),
            });
        }
    }
    status
}

fn is_descendant_of(world: &World, mut entity: Entity, ancestor: Entity) -> bool {
    while let Some(parent) = world.get::<ChildOf>(entity).map(ChildOf::parent) {
        if parent == ancestor {
            return true;
        }
        entity = parent;
    }
    false
}

fn effective_visibility(world: &World, mut entity: Entity) -> bool {
    let mut visibility = None;
    loop {
        if world
            .get::<Node>(entity)
            .is_some_and(|node| node.display == Display::None)
        {
            return false;
        }
        match world.get::<Visibility>(entity) {
            Some(Visibility::Hidden) if visibility.is_none() => visibility = Some(false),
            Some(Visibility::Visible) if visibility.is_none() => visibility = Some(true),
            _ => {}
        }
        let Some(parent) = world.get::<ChildOf>(entity).map(ChildOf::parent) else {
            return visibility.unwrap_or(true);
        };
        entity = parent;
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        app::{App, Update},
        asset::{AssetPlugin, Assets},
        ecs::{
            message::{MessageReader, Messages},
            resource::Resource,
            system::ResMut,
        },
        prelude::{IntoScheduleConfigs, Visibility},
    };
    use tilt_ui_core::ComponentId;
    use tilt_ui_html::{parse_document, parse_template};

    use super::*;
    use crate::{TiltUiAssetsPlugin, UiStyleSheetAsset, UiTemplateAsset};

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(bevy::app::TaskPoolPlugin::default())
            .add_plugins(AssetPlugin::default())
            .add_plugins(TiltUiAssetsPlugin);
        app.world_mut().init_resource::<Messages<UiStateEvent>>();
        app.world_mut().init_resource::<UiDocumentState>();
        app
    }

    fn changes(app: &mut App) -> Vec<UiStateEvent> {
        app.world_mut()
            .resource_mut::<Messages<UiStateEvent>>()
            .drain()
            .collect()
    }

    #[derive(Resource, Default)]
    struct SeenEvents(Vec<UiStateEvent>);

    fn collect_events(mut events: MessageReader<UiStateEvent>, mut seen: ResMut<SeenEvents>) {
        seen.0.extend(events.read().cloned());
    }

    #[test]
    fn application_system_can_read_a_transition_in_the_same_update() {
        let mut app = app();
        install(&mut app);
        app.init_resource::<SeenEvents>()
            .add_systems(Update, collect_events.after(UiStateRuntimeSet::Observe));
        app.update();
        assert_eq!(
            app.world().resource::<SeenEvents>().0,
            vec![UiStateEvent::Loading(UiStateTarget::Document)]
        );
    }

    #[test]
    fn component_reports_each_loading_readiness_and_visibility_transition_once() {
        let mut app = app();
        let template = app
            .world_mut()
            .resource_mut::<Assets<UiTemplateAsset>>()
            .add(UiTemplateAsset::new(parse_template("<p>Hi</p>").unwrap()));
        let stylesheet = app
            .world_mut()
            .resource_mut::<Assets<UiStyleSheetAsset>>()
            .add(UiStyleSheetAsset::new(Default::default()));
        let parent = app.world_mut().spawn(Visibility::Inherited).id();
        let component = app
            .world_mut()
            .spawn((
                super::super::ComponentBoundary::new(ComponentId(3)),
                ComponentAssetHandles {
                    template,
                    stylesheet,
                    additional_stylesheets: Vec::new(),
                },
                StyleDirty,
            ))
            .id();
        app.world_mut().entity_mut(parent).add_child(component);

        observe_ui_states(app.world_mut());
        assert_eq!(
            changes(&mut app)
                .into_iter()
                .filter(|event| event.target() == UiStateTarget::Component(component))
                .map(UiStateEvent::change)
                .collect::<Vec<_>>(),
            vec![UiStateChange::Loading, UiStateChange::Loaded]
        );
        assert_eq!(
            app.world().get::<UiState>(component).unwrap().load,
            UiLoadState::Loaded
        );

        app.world_mut().entity_mut(component).remove::<StyleDirty>();
        observe_ui_states(app.world_mut());
        assert_eq!(
            changes(&mut app)
                .into_iter()
                .filter(|event| event.target() == UiStateTarget::Component(component))
                .map(UiStateEvent::change)
                .collect::<Vec<_>>(),
            vec![UiStateChange::Ready, UiStateChange::Visible]
        );
        assert_eq!(
            app.world().get::<UiState>(component).unwrap().visible,
            Some(true)
        );

        app.world_mut()
            .entity_mut(parent)
            .insert(Visibility::Hidden);
        observe_ui_states(app.world_mut());
        assert!(
            changes(&mut app).iter().any(|event| {
                *event == UiStateEvent::Hidden(UiStateTarget::Component(component))
            })
        );
        assert_eq!(
            app.world().get::<UiState>(component).unwrap().visible,
            Some(false)
        );
        observe_ui_states(app.world_mut());
        assert!(changes(&mut app).is_empty());
        app.world_mut().clear_trackers();
        observe_ui_states(app.world_mut());
        let world = app.world_mut();
        let mut changed = world.query_filtered::<Entity, bevy::ecs::query::Changed<UiState>>();
        assert_eq!(changed.iter(world).count(), 0);
        assert!(changes(&mut app).is_empty());

        app.world_mut()
            .entity_mut(parent)
            .insert(Visibility::Visible);
        observe_ui_states(app.world_mut());
        assert!(
            changes(&mut app).iter().any(|event| {
                *event == UiStateEvent::Visible(UiStateTarget::Component(component))
            })
        );

        reload_component(app.world_mut(), component);
        assert_eq!(
            changes(&mut app),
            vec![UiStateEvent::Loading(UiStateTarget::Component(component))]
        );
        observe_ui_states(app.world_mut());
        assert_eq!(
            changes(&mut app)
                .into_iter()
                .filter(|event| event.target() == UiStateTarget::Component(component))
                .map(UiStateEvent::change)
                .collect::<Vec<_>>(),
            vec![UiStateChange::Loaded, UiStateChange::Ready]
        );

        hide_before_despawn(app.world_mut(), component);
        assert_eq!(
            changes(&mut app),
            vec![UiStateEvent::Hidden(UiStateTarget::Component(component))]
        );
    }

    #[test]
    fn component_waits_for_linked_document_stylesheet() {
        let mut app = app();
        let template = app
            .world_mut()
            .resource_mut::<Assets<UiTemplateAsset>>()
            .add(UiTemplateAsset::new(parse_template("<p>Hi</p>").unwrap()));
        let stylesheet = app
            .world_mut()
            .resource_mut::<Assets<UiStyleSheetAsset>>()
            .add(UiStyleSheetAsset::new(Default::default()));
        let global = app
            .world()
            .resource::<Assets<UiStyleSheetAsset>>()
            .reserve_handle();
        app.world_mut()
            .insert_resource(DocumentStylesheets(vec![global.clone()]));
        app.world_mut().insert_resource(PendingDocument {
            handle: Handle::default(),
            parsed: None,
            finished: false,
            failure: None,
        });
        let component = app
            .world_mut()
            .spawn((
                super::super::ComponentBoundary::new(ComponentId(5)),
                ComponentAssetHandles {
                    template,
                    stylesheet,
                    additional_stylesheets: Vec::new(),
                },
            ))
            .id();
        observe_ui_states(app.world_mut());
        assert_eq!(
            changes(&mut app)
                .into_iter()
                .filter(|event| event.target() == UiStateTarget::Component(component))
                .collect::<Vec<_>>(),
            vec![UiStateEvent::Loading(UiStateTarget::Component(component))]
        );
        app.world_mut()
            .resource_mut::<Assets<UiStyleSheetAsset>>()
            .insert(global.id(), UiStyleSheetAsset::new(Default::default()))
            .unwrap();
        observe_ui_states(app.world_mut());
        assert_eq!(
            changes(&mut app)
                .into_iter()
                .filter(|event| event.target() == UiStateTarget::Component(component))
                .collect::<Vec<_>>(),
            vec![
                UiStateEvent::Loaded(UiStateTarget::Component(component)),
                UiStateEvent::Ready(UiStateTarget::Component(component)),
                UiStateEvent::Visible(UiStateTarget::Component(component)),
            ]
        );
    }

    #[test]
    fn failures_are_queryable_and_emit_one_structured_event() {
        let mut app = app();
        let component = app
            .world_mut()
            .spawn((
                super::super::ComponentBoundary::new(ComponentId(4)),
                FailedComponentInstantiation {
                    error: super::super::ComponentInstantiationError::TemplateAssetFailed {
                        component: ComponentId(4),
                    },
                },
            ))
            .id();
        observe_ui_states(app.world_mut());
        let events = changes(&mut app);
        assert!(events.iter().any(|event| {
            event.target() == UiStateTarget::Component(component)
                && matches!(
                    event,
                    UiStateEvent::Error {
                        code: UiErrorCode::TemplateLoad,
                        ..
                    }
                )
        }));
        assert!(matches!(
            app.world().get::<UiState>(component).unwrap().load,
            UiLoadState::Error(UiStateError {
                code: UiErrorCode::TemplateLoad,
                ..
            })
        ));
        observe_ui_states(app.world_mut());
        assert!(changes(&mut app).is_empty());
    }

    #[test]
    fn document_failure_remains_readable_without_a_mounted_root() {
        let mut app = app();
        app.world_mut().insert_resource(PendingDocument {
            handle: Handle::default(),
            parsed: None,
            finished: true,
            failure: Some(UiStateError {
                code: UiErrorCode::InvalidDocument,
                message: "missing HTML5 document".into(),
            }),
        });
        observe_ui_states(app.world_mut());
        assert_eq!(
            changes(&mut app),
            vec![
                UiStateEvent::Loading(UiStateTarget::Document),
                UiStateEvent::Error {
                    target: UiStateTarget::Document,
                    code: UiErrorCode::InvalidDocument,
                    message: "missing HTML5 document".into(),
                },
            ]
        );
        assert!(matches!(
            app.world().resource::<UiDocumentState>().0.load,
            UiLoadState::Error(_)
        ));
    }

    #[test]
    fn document_reports_ready_and_later_visibility_changes() {
        let mut app = app();
        let document = UiTemplateAsset::from_document(
            parse_document("<!doctype html><html><head></head><body></body></html>").unwrap(),
        );
        let head = document.document_head().unwrap().clone();
        let template = app
            .world_mut()
            .resource_mut::<Assets<UiTemplateAsset>>()
            .add(document);
        let stylesheet = app
            .world_mut()
            .resource_mut::<Assets<UiStyleSheetAsset>>()
            .add(UiStyleSheetAsset::new(Default::default()));
        let root = app
            .world_mut()
            .spawn((
                UiDocumentInfo(head),
                ComponentAssetHandles {
                    template: template.clone(),
                    stylesheet,
                    additional_stylesheets: Vec::new(),
                },
                Visibility::Inherited,
            ))
            .id();
        app.world_mut()
            .insert_resource(DocumentStylesheets(Vec::new()));
        app.world_mut().insert_resource(PendingDocument {
            handle: template,
            parsed: None,
            finished: true,
            failure: None,
        });

        observe_ui_states(app.world_mut());
        assert_eq!(
            changes(&mut app)
                .into_iter()
                .map(UiStateEvent::change)
                .collect::<Vec<_>>(),
            vec![
                UiStateChange::Loading,
                UiStateChange::Loaded,
                UiStateChange::Ready,
                UiStateChange::Visible,
            ]
        );
        assert_eq!(
            app.world().resource::<UiDocumentState>().0.load,
            UiLoadState::Ready
        );
        app.world_mut().entity_mut(root).insert(Visibility::Hidden);
        observe_ui_states(app.world_mut());
        assert_eq!(
            changes(&mut app)
                .into_iter()
                .map(UiStateEvent::change)
                .collect::<Vec<_>>(),
            vec![UiStateChange::Hidden]
        );
    }
}
