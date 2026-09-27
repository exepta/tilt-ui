use bevy::prelude::*;
use tilt_ui::{HtmlEvent, Router, component_init, component_update, html_fn, html_shared};

#[html_shared]
#[derive(Resource, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteView {
    path: String,
    title: String,
    subtitle: String,
    home_class: String,
    settings_class: String,
    details_class: String,
    fallback_class: String,
}

#[component_init]
fn initialize_route_view(mut commands: Commands) {
    commands.insert_resource(RouteView::default());
}

#[component_update]
fn update_route_view(router: Res<Router>, mut view: ResMut<RouteView>) {
    let path = router.path();
    if view.path == path {
        return;
    }
    view.path = path.to_owned();
    view.home_class.clear();
    view.settings_class.clear();
    view.details_class.clear();
    view.fallback_class.clear();
    match path {
        "/" => {
            view.title = "Home".into();
            view.subtitle = "Transient route · rebuilt on every visit".into();
            view.home_class = "selected".into();
        }
        "/settings" | "/preferences" => {
            view.title = "Settings".into();
            view.subtitle = if path == "/preferences" {
                "Redirect /preferences → /settings".into()
            } else {
                "Eager route · instance stays alive".into()
            };
            view.settings_class = "selected".into();
        }
        "/details" => {
            view.title = "Details".into();
            view.subtitle = "Lazy route · created on first visit".into();
            view.details_class = "selected".into();
        }
        _ => {
            view.title = "Fallback".into();
            view.subtitle = "No matching path · fallback component".into();
            view.fallback_class = "selected".into();
        }
    }
}

#[html_fn("go_home")]
fn go_home(In(_event): In<HtmlEvent>, mut router: ResMut<Router>) {
    router.navigate("/");
}

#[html_fn("go_settings")]
fn go_settings(In(_event): In<HtmlEvent>, mut router: ResMut<Router>) {
    router.navigate("/settings");
}

#[html_fn("go_redirect")]
fn go_redirect(In(_event): In<HtmlEvent>, mut router: ResMut<Router>) {
    router.navigate("/preferences/");
}

#[html_fn("go_details")]
fn go_details(In(_event): In<HtmlEvent>, mut router: ResMut<Router>) {
    router.navigate("//details/");
}

#[html_fn("go_missing")]
fn go_missing(In(_event): In<HtmlEvent>, mut router: ResMut<Router>) {
    router.navigate("/unknown");
}
