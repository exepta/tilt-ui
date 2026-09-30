use bevy::{
    app::App,
    ecs::{
        message::Messages,
        resource::Resource,
        system::{In, ResMut},
    },
    ui::widget::Text,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use tilt_ui::{
    ControlActivated, HtmlClick, HtmlEvent, HtmlSubmit, TiltUiCodePlugin, component_init,
    component_update, html_fn, html_shared, html_use, ui_routes,
};

mod imported {
    #[derive(bevy::ecs::resource::Resource, tilt_ui::serde::Serialize)]
    pub struct ImportedState {
        pub label: String,
    }
}

#[html_use]
use imported::ImportedState;

#[derive(Resource, Default)]
struct Hits {
    startup: usize,
    updates: usize,
    clicked: usize,
    typed_clicked: usize,
    submitted: usize,
}

#[component_init]
fn initialize(mut hits: ResMut<Hits>) {
    hits.startup += 1;
}

#[component_update]
fn update_hits(mut hits: ResMut<Hits>) {
    hits.updates += 1;
}

#[html_fn("handle_click")]
fn clicked(In(event): In<HtmlEvent>, mut hits: ResMut<Hits>) {
    assert_eq!(event.kind, "click");
    hits.clicked += 1;
}

#[html_fn("typed_click")]
fn typed_clicked(In(_event): In<HtmlClick>, mut hits: ResMut<Hits>) {
    hits.typed_clicked += 1;
}

#[html_fn("save_form")]
fn saved(In(event): In<HtmlSubmit>, mut hits: ResMut<Hits>) {
    assert_eq!(event.data.get("title").map(String::as_str), Some("Draft"));
    assert_eq!(
        event.form_data.get("title"),
        Some(&vec![
            tilt_ui::FormValue::Text("Draft".into()),
            tilt_ui::FormValue::Text("Revision".into()),
        ])
    );
    assert_eq!(
        event.form_data.get("upload"),
        Some(&vec![tilt_ui::FormValue::File(tilt_ui::FormFile {
            name: "notes.txt".into(),
            size_bytes: Some(5),
            native_path: None,
        })])
    );
    hits.submitted += 1;
}

#[derive(Default, tilt_ui::UiStore, tilt_ui::serde::Serialize)]
struct Profile {
    name: String,
}

#[derive(Default, tilt_ui::UiStore, tilt_ui::serde::Serialize, tilt_ui::serde::Deserialize)]
#[ui_store(mutable)]
struct InlineState {
    count: i32,
    enabled: bool,
}

#[html_shared]
#[derive(Resource, tilt_ui::serde::Serialize)]
struct SharedState {
    label: String,
}

static SHARED_SERIALIZATIONS: AtomicUsize = AtomicUsize::new(0);

#[html_shared]
#[derive(Resource)]
struct CountedState(&'static str);

impl tilt_ui::serde::Serialize for CountedState {
    fn serialize<S: tilt_ui::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        SHARED_SERIALIZATIONS.fetch_add(1, Ordering::Relaxed);
        serializer.serialize_str(self.0)
    }
}

#[ui_routes]
fn application_routes() -> tilt_ui::Routes {
    tilt_ui::Routes::new()
        .route("/", tilt_ui::ComponentId(1))
        .redirect("/home", "/")
}

#[test]
fn macros_register_and_dispatch_rust_systems() {
    let mut app = App::new();
    app.init_resource::<Hits>()
        .add_message::<ControlActivated>()
        .add_message::<tilt_ui::ControlCheckedChanged>()
        .add_message::<tilt_ui::EditableTextChanged>()
        .add_plugins(TiltUiCodePlugin);
    let entity = app
        .world_mut()
        .spawn(tilt_ui::StaticAttributes {
            attributes: vec![tilt_ui::StaticAttribute {
                name: "onclick".into(),
                value: "handle_click".into(),
            }],
        })
        .id();
    app.update();
    assert_eq!(app.world().resource::<Hits>().startup, 1);
    assert_eq!(app.world().resource::<Hits>().updates, 1);
    app.world_mut()
        .resource_mut::<Messages<ControlActivated>>()
        .write(ControlActivated { entity });
    app.update();
    assert_eq!(app.world().resource::<Hits>().clicked, 1);
    app.world_mut()
        .entity_mut(entity)
        .insert(tilt_ui::StaticAttributes {
            attributes: vec![tilt_ui::StaticAttribute {
                name: "onclick".into(),
                value: "typed_click()".into(),
            }],
        });
    app.world_mut()
        .resource_mut::<Messages<ControlActivated>>()
        .write(ControlActivated { entity });
    app.update();
    assert_eq!(app.world().resource::<Hits>().typed_clicked, 1);
}

#[test]
fn inline_actions_dispatch_with_rust_handlers_and_update_bindings() {
    let mut app = App::new();
    app.init_resource::<Hits>().add_plugins(TiltUiCodePlugin);
    let button = app
        .world_mut()
        .spawn(tilt_ui::StaticAttributes {
            attributes: vec![tilt_ui::StaticAttribute {
                name: "onclick".into(),
                value: "$add(inlineState.count, 2); typed_click(); $toggle(inlineState.enabled)"
                    .into(),
            }],
        })
        .id();
    let checkbox = app
        .world_mut()
        .spawn(tilt_ui::StaticAttributes {
            attributes: vec![tilt_ui::StaticAttribute {
                name: "onchange".into(),
                value: "$set(inlineState.enabled, $event.checked)".into(),
            }],
        })
        .id();
    let text = app
        .world_mut()
        .spawn((
            tilt_ui::TiltText {
                value: "{{ inlineState.count }}".into(),
            },
            Text::new(""),
        ))
        .id();
    app.update();
    app.world_mut()
        .resource_mut::<Messages<ControlActivated>>()
        .write(ControlActivated { entity: button });
    app.update();
    let state = app.world().resource::<tilt_ui::UiBindingStore>();
    assert_eq!(state.get_store::<InlineState>().unwrap().count, 2);
    assert!(state.get_store::<InlineState>().unwrap().enabled);
    assert_eq!(app.world().resource::<Hits>().typed_clicked, 1);

    app.world_mut()
        .resource_mut::<Messages<tilt_ui::ControlCheckedChanged>>()
        .write(tilt_ui::ControlCheckedChanged {
            entity: checkbox,
            checked: false,
        });
    app.update();
    assert!(
        !app.world()
            .resource::<tilt_ui::UiBindingStore>()
            .get_store::<InlineState>()
            .unwrap()
            .enabled
    );
    assert_eq!(app.world().get::<Text>(text).unwrap().0, "2");
}

#[test]
fn derived_store_and_shared_resource_drive_template_text() {
    let mut app = App::new();
    let mut store = tilt_ui::UiBindingStore::default();
    store.set_store(Profile {
        name: "Preloaded".into(),
    });
    app.insert_resource(store);
    app.insert_resource(SharedState {
        label: "Ready".into(),
    })
    .add_message::<ControlActivated>()
    .add_message::<tilt_ui::ControlCheckedChanged>()
    .add_message::<tilt_ui::EditableTextChanged>()
    .init_resource::<Hits>()
    .add_plugins(TiltUiCodePlugin);
    assert!(
        app.world()
            .resource::<tilt_ui::UiBindingStore>()
            .get_store::<Profile>()
            .is_some()
    );
    assert_eq!(
        app.world()
            .resource::<tilt_ui::UiBindingStore>()
            .get_store::<Profile>()
            .unwrap()
            .name,
        "Preloaded"
    );
    let text = app
        .world_mut()
        .spawn((
            tilt_ui::TiltText {
                value: "{{ profile.name }} / {{ sharedState.label }}".into(),
            },
            Text::new(""),
        ))
        .id();
    app.world_mut()
        .resource_mut::<tilt_ui::UiBindingStore>()
        .set_store(Profile { name: "Ada".into() });
    app.update();
    assert_eq!(app.world().get::<Text>(text).unwrap().0, "Ada / Ready");
    app.world_mut().resource_mut::<SharedState>().label = "Updated".into();
    app.update();
    assert_eq!(app.world().get::<Text>(text).unwrap().0, "Ada / Updated");
}

#[test]
fn imported_resource_is_available_to_template_bindings() {
    let mut app = App::new();
    app.init_resource::<Hits>()
        .insert_resource(ImportedState {
            label: "Imported".into(),
        })
        .add_message::<ControlActivated>()
        .add_message::<tilt_ui::ControlCheckedChanged>()
        .add_message::<tilt_ui::EditableTextChanged>()
        .add_plugins(TiltUiCodePlugin);
    let text = app
        .world_mut()
        .spawn((
            tilt_ui::TiltText {
                value: "{{ importedState.label }}".into(),
            },
            Text::new(""),
        ))
        .id();
    app.update();
    assert_eq!(app.world().get::<Text>(text).unwrap().0, "Imported");
}

#[test]
fn unchanged_shared_resource_is_not_serialized_each_frame() {
    let mut app = App::new();
    app.init_resource::<Hits>()
        .insert_resource(CountedState("First"))
        .add_message::<ControlActivated>()
        .add_message::<tilt_ui::ControlCheckedChanged>()
        .add_message::<tilt_ui::EditableTextChanged>()
        .add_plugins(TiltUiCodePlugin);
    let initial = SHARED_SERIALIZATIONS.load(Ordering::Relaxed);
    app.update();
    let after_first_update = SHARED_SERIALIZATIONS.load(Ordering::Relaxed);
    assert_eq!(after_first_update, initial + 1);
    app.update();
    assert_eq!(
        SHARED_SERIALIZATIONS.load(Ordering::Relaxed),
        after_first_update
    );
    app.world_mut().resource_mut::<CountedState>().0 = "Second";
    app.update();
    assert_eq!(
        SHARED_SERIALIZATIONS.load(Ordering::Relaxed),
        after_first_update + 1
    );
}

#[test]
fn route_macro_registers_route_table() {
    let mut app = App::new();
    app.add_plugins(tilt_ui::TiltUiRouterPlugin);
    assert_eq!(
        app.world().resource::<tilt_ui::Router>().target(),
        Some(tilt_ui::ComponentId(1))
    );
    app.world_mut()
        .resource_mut::<tilt_ui::Router>()
        .navigate("/home");
    assert_eq!(
        app.world().resource::<tilt_ui::Router>().target(),
        Some(tilt_ui::ComponentId(1))
    );
}

#[test]
fn form_action_reaches_typed_submit_handler() {
    let mut app = App::new();
    app.init_resource::<Hits>()
        .add_message::<ControlActivated>()
        .add_message::<tilt_ui::ControlCheckedChanged>()
        .add_message::<tilt_ui::EditableTextChanged>()
        .add_plugins(TiltUiCodePlugin);
    let form = app.world_mut().spawn_empty().id();
    let submitter = app.world_mut().spawn_empty().id();
    app.update();
    app.world_mut()
        .resource_mut::<Messages<tilt_ui::FormSubmitted>>()
        .write(tilt_ui::FormSubmitted {
            form,
            submitter,
            action: Some("save_form".into()),
            data: [
                (
                    "title".into(),
                    vec![
                        tilt_ui::FormValue::Text("Draft".into()),
                        tilt_ui::FormValue::Text("Revision".into()),
                    ],
                ),
                (
                    "upload".into(),
                    vec![tilt_ui::FormValue::File(tilt_ui::FormFile {
                        name: "notes.txt".into(),
                        size_bytes: Some(5),
                        native_path: None,
                    })],
                ),
            ]
            .into(),
        });
    app.update();
    assert_eq!(app.world().resource::<Hits>().submitted, 1);
}
