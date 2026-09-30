use bevy::prelude::*;
use tilt_ui::serde_json::Value;
use tilt_ui::{
    ComponentCatalog, ComponentInstance, HtmlChange, HtmlEvent, UiDocumentState, UiLoadState,
    UiState, UiStateEvent, UiStateTarget, component_init, html_fn, html_method, html_shared,
};

/// The page reads the public lifecycle resource and component states into HTML bindings.
#[html_shared]
#[derive(Resource, Clone, PartialEq, Eq, serde::Serialize)]
pub struct LifecycleView {
    document: LifecycleCard,
    main: LifecycleCard,
    probe: LifecycleCard,
    events: Vec<LifecycleEntry>,
    total_events: u64,
}

#[derive(Clone, PartialEq, Eq, serde::Serialize)]
struct LifecycleCard {
    phase: String,
    visibility: String,
}

impl Default for LifecycleCard {
    fn default() -> Self {
        Self {
            phase: "Waiting".into(),
            visibility: "Unknown".into(),
        }
    }
}

impl Default for LifecycleView {
    fn default() -> Self {
        Self {
            document: LifecycleCard::default(),
            main: LifecycleCard::default(),
            probe: LifecycleCard::default(),
            events: Vec::new(),
            total_events: 0,
        }
    }
}

#[derive(Clone, PartialEq, Eq, serde::Serialize)]
struct LifecycleEntry {
    id: u64,
    target: String,
    state: String,
}

fn lifecycle_card(state: &UiState) -> LifecycleCard {
    let phase = match &state.load {
        UiLoadState::Loading => "Loading".into(),
        UiLoadState::Loaded => "Loaded".into(),
        UiLoadState::Ready => "Ready".into(),
        UiLoadState::Error(error) => format!("Error ({:?})", error.code),
    };
    let visibility = match state.visible {
        Some(true) => "Visible",
        Some(false) => "Hidden",
        None => "Unknown",
    };
    LifecycleCard {
        phase,
        visibility: visibility.into(),
    }
}

/// Runs after TiltUI's state observer, so the event list updates in the same frame.
pub fn track_ui_lifecycle(
    mut events: MessageReader<UiStateEvent>,
    document: Res<UiDocumentState>,
    components: Query<(Entity, &ComponentInstance, &UiState)>,
    changed_states: Query<(), Changed<UiState>>,
    catalog: Res<ComponentCatalog>,
    mut view: ResMut<LifecycleView>,
) {
    let received = events.read().collect::<Vec<_>>();
    if received.is_empty() && !document.is_changed() && changed_states.is_empty() {
        return;
    }
    let mut next = (*view).clone();
    next.document = lifecycle_card(&document.0);
    for (_, instance, state) in &components {
        match catalog.component_metadata(instance.component).map(|meta| meta.name) {
            Some("main") => next.main = lifecycle_card(state),
            Some("state-probe") => next.probe = lifecycle_card(state),
            _ => {}
        }
    }
    for event in received {
        let target = match event.target() {
            UiStateTarget::Document => "Document".to_owned(),
            UiStateTarget::Component(entity) => components
                .get(entity)
                .ok()
                .and_then(|(_, instance, _)| catalog.component_metadata(instance.component))
                .map_or_else(|| format!("Component {entity:?}"), |meta| meta.name.to_owned()),
        };
        let state = match event {
            UiStateEvent::Loading(_) => "Loading".to_owned(),
            UiStateEvent::Loaded(_) => "Loaded".to_owned(),
            UiStateEvent::Ready(_) => "Ready".to_owned(),
            UiStateEvent::Visible(_) => "Visible".to_owned(),
            UiStateEvent::Hidden(_) => "Hidden".to_owned(),
            UiStateEvent::Error { code, message, .. } => {
                format!("Error {code:?}: {message}")
            }
        };
        next.total_events += 1;
        next.events.insert(
            0,
            LifecycleEntry {
                id: next.total_events,
                target,
                state,
            },
        );
        next.events.truncate(10);
    }
    if *view != next {
        *view = next;
    }
}

fn set_probe_visibility(catalog: &ComponentCatalog, probes: &mut Query<(&ComponentInstance, &mut Visibility)>, visible: bool) {
    let Some(probe) = catalog.component_id("state-probe") else { return };
    for (instance, mut visibility) in probes.iter_mut() {
        if instance.component == probe {
            let next = if visible { Visibility::Inherited } else { Visibility::Hidden };
            if *visibility != next {
                *visibility = next;
            }
        }
    }
}

#[html_fn("hide_probe")]
fn hide_probe(
    In(_click): In<HtmlEvent>,
    catalog: Res<ComponentCatalog>,
    mut probes: Query<(&ComponentInstance, &mut Visibility)>,
) {
    set_probe_visibility(&catalog, &mut probes, false);
}

#[html_fn("show_probe")]
fn show_probe(
    In(_click): In<HtmlEvent>,
    catalog: Res<ComponentCatalog>,
    mut probes: Query<(&ComponentInstance, &mut Visibility)>,
) {
    set_probe_visibility(&catalog, &mut probes, true);
}

/// The opt-in keeps inline writes synchronized with the Rust store value.
#[derive(tilt_ui::UiStore, serde::Serialize, serde::Deserialize)]
#[ui_store(mutable)]
struct ActionState {
    name: String,
    count: f64,
    enabled: bool,
    tags: Vec<String>,
}

impl Default for ActionState {
    fn default() -> Self {
        Self {
            name: "Ada".into(),
            count: 2.0,
            enabled: true,
            tags: vec!["demo".into()],
        }
    }
}

/// State and template methods live beside the page, without a controller.
#[html_shared]
#[derive(Resource, serde::Serialize)]
struct User {
    first_name: String,
    last_name: String,
}

impl Default for User {
    fn default() -> Self {
        Self {
            first_name: "Ada".into(),
            last_name: "Lovelace".into(),
        }
    }
}

#[component_init]
fn initialize_user(mut commands: Commands) {
    commands.insert_resource(User::default());
}

#[html_fn("set_first_name")]
fn set_first_name(In(change): In<HtmlChange>, mut user: ResMut<User>) {
    user.first_name = change.value.unwrap_or_default();
}

#[html_fn("set_last_name")]
fn set_last_name(In(change): In<HtmlChange>, mut user: ResMut<User>) {
    user.last_name = change.value.unwrap_or_default();
}

#[html_method("main", "user.full_name")]
fn full_name(user: &Value, arguments: &[Value]) -> Option<Value> {
    if !arguments.is_empty() {
        return None;
    }
    let first = user.get("first_name")?.as_str()?;
    let last = user.get("last_name")?.as_str()?;
    Some(Value::String(format!("{first} {last}").trim().to_owned()))
}

#[html_method("main", "user.greeting")]
fn greeting(user: &Value, arguments: &[Value]) -> Option<Value> {
    let [Value::String(prefix)] = arguments else {
        return None;
    };
    let first = user.get("first_name")?.as_str()?;
    Some(Value::String(format!("{prefix}, {first}!")))
}

#[component_init]
fn initialize_demo_state(mut commands: Commands) {
    commands.insert_resource(DemoState::default());
}

#[derive(Clone, serde::Serialize)]
struct DemoItem {
    id: u32,
    name: String,
}

#[html_shared]
#[derive(Resource, serde::Serialize)]
struct DemoState {
    enabled: bool,
    query: String,
    count: i32,
    mode: String,
    items: Vec<DemoItem>,
    next_id: u32,
}

impl Default for DemoState {
    fn default() -> Self {
        Self {
            enabled: true,
            query: "Rust".into(),
            count: 3,
            mode: "ready".into(),
            items: vec![
                DemoItem {
                    id: 1,
                    name: "Alpha".into(),
                },
                DemoItem {
                    id: 2,
                    name: "Beta".into(),
                },
                DemoItem {
                    id: 3,
                    name: "Gamma".into(),
                },
            ],
            next_id: 4,
        }
    }
}

#[html_fn("set_query")]
fn set_query(In(change): In<HtmlChange>, mut state: ResMut<DemoState>) {
    state.query = change.value.unwrap_or_default();
}

#[html_fn("toggle_enabled")]
fn toggle_enabled(In(_click): In<HtmlEvent>, mut state: ResMut<DemoState>) {
    state.enabled = !state.enabled;
}

#[html_fn("increase_count")]
fn increase_count(In(_click): In<HtmlEvent>, mut state: ResMut<DemoState>) {
    state.count = (state.count + 1).min(8);
}

#[html_fn("decrease_count")]
fn decrease_count(In(_click): In<HtmlEvent>, mut state: ResMut<DemoState>) {
    state.count = (state.count - 1).max(0);
}

#[html_fn("cycle_mode")]
fn cycle_mode(In(_click): In<HtmlEvent>, mut state: ResMut<DemoState>) {
    state.mode = match state.mode.as_str() {
        "ready" => "busy",
        "busy" => "done",
        _ => "ready",
    }
    .into();
}

#[html_fn("add_item")]
fn add_item(In(_click): In<HtmlEvent>, mut state: ResMut<DemoState>) {
    let id = state.next_id;
    state.next_id += 1;
    state.items.push(DemoItem {
        id,
        name: format!("Item {id}"),
    });
}

#[html_fn("remove_item")]
fn remove_item(In(_click): In<HtmlEvent>, mut state: ResMut<DemoState>) {
    state.items.pop();
}

#[html_fn("reverse_items")]
fn reverse_items(In(_click): In<HtmlEvent>, mut state: ResMut<DemoState>) {
    state.items.reverse();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn click(target: Entity, handler: &str) -> HtmlEvent {
        HtmlEvent {
            target,
            kind: "click",
            value: None,
            submitter: None,
            data: Default::default(),
            form_data: None,
            handler: handler.into(),
        }
    }

    #[test]
    fn hide_and_show_buttons_change_the_probe_boundary_visibility() {
        let mut world = World::new();
        let catalog = crate::tilt_ui_component_catalog();
        let probe_id = catalog.component_id("state-probe").unwrap();
        world.insert_resource(catalog);
        let probe = world
            .spawn((ComponentInstance { component: probe_id }, Visibility::Inherited))
            .id();
        let hide = world.register_system(hide_probe);
        let show = world.register_system(show_probe);

        world.run_system_with(hide, click(probe, "hide_probe")).unwrap();
        assert_eq!(world.get::<Visibility>(probe), Some(&Visibility::Hidden));
        world.run_system_with(show, click(probe, "show_probe")).unwrap();
        assert_eq!(world.get::<Visibility>(probe), Some(&Visibility::Inherited));
    }

    #[test]
    fn lifecycle_view_reads_current_state_and_consumes_events_once() {
        let mut app = App::new();
        app.add_message::<UiStateEvent>()
            .init_resource::<LifecycleView>()
            .insert_resource(crate::tilt_ui_component_catalog())
            .add_systems(Update, track_ui_lifecycle);
        let mut document = UiDocumentState::default();
        document.0.load = UiLoadState::Ready;
        document.0.visible = Some(true);
        app.insert_resource(document);
        let probe_id = app
            .world()
            .resource::<ComponentCatalog>()
            .component_id("state-probe")
            .unwrap();
        let mut probe_state = UiState::default();
        probe_state.load = UiLoadState::Ready;
        probe_state.visible = Some(false);
        let probe = app
            .world_mut()
            .spawn((ComponentInstance { component: probe_id }, probe_state))
            .id();
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<UiStateEvent>>()
            .write(UiStateEvent::Hidden(UiStateTarget::Component(probe)));

        app.update();
        let view = app.world().resource::<LifecycleView>();
        assert_eq!(view.document.phase, "Ready");
        assert_eq!(view.probe.visibility, "Hidden");
        assert_eq!(view.events[0].target, "state-probe");
        assert_eq!(view.events[0].state, "Hidden");
        assert_eq!(view.total_events, 1);

        app.update();
        assert_eq!(app.world().resource::<LifecycleView>().total_events, 1);
    }
}
