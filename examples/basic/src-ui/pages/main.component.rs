use bevy::prelude::*;
use tilt_ui::serde_json::Value;
use tilt_ui::{HtmlChange, HtmlEvent, component_init, html_fn, html_method, html_shared};

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
