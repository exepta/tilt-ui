use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use tilt_ui::prelude::*;

tilt_ui::include_components!();

fn main() {
    let resolution = std::env::var("TILT_UI_ROUTING_SIZE")
        .ok()
        .and_then(|size| {
            let (width, height) = size.split_once('x')?;
            Some((width.parse::<u32>().ok()?, height.parse::<u32>().ok()?))
        })
        .unwrap_or((1320, 1060));
    App::new()
        .add_plugins(
            TiltUiPlugin::new(tilt_ui_component_catalog())
                .with_source_root(concat!(env!("CARGO_MANIFEST_DIR"), "/src-ui")),
        )
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "TiltUI routing showcase".into(),
                resolution: resolution.into(),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Update, capture_showcase)
        .run();
}

fn capture_showcase(mut commands: Commands, time: Res<Time>, mut captured: Local<bool>) {
    if *captured || time.elapsed_secs() < 2.0 {
        return;
    }
    let Ok(path) = std::env::var("TILT_UI_ROUTING_SCREENSHOT") else {
        *captured = true;
        return;
    };
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
    *captured = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_router_registry_loads_modules_from_different_folders() {
        let mut app = App::new();
        app.add_plugins(tilt_ui::TiltUiRouterPlugin);
        assert_eq!(
            app.world().resource::<Router>().target(),
            tilt_ui_component_id("home")
        );
        app.world_mut()
            .resource_mut::<Router>()
            .navigate("/preferences");
        assert_eq!(
            app.world().resource::<Router>().target(),
            tilt_ui_component_id("settings")
        );
        app.world_mut()
            .resource_mut::<Router>()
            .navigate("/details");
        assert_eq!(
            app.world().resource::<Router>().target(),
            tilt_ui_component_id("details")
        );
    }

    #[test]
    fn showcase_stylesheets_parse() {
        for source in [
            include_str!("../src-ui/styles/routing.css"),
            include_str!("../src-ui/components/app-shell.component.css"),
            include_str!("../src-ui/pages/home.component.css"),
            include_str!("../src-ui/pages/settings.component.css"),
            include_str!("../src-ui/pages/details.component.css"),
            include_str!("../src-ui/pages/missing.component.css"),
        ] {
            tilt_ui::tilt_ui_css::parse_stylesheet(source).expect("valid routing showcase CSS");
        }
    }

    #[test]
    fn showcase_templates_parse() {
        for source in [
            include_str!("../src-ui/components/app-shell.component.html"),
            include_str!("../src-ui/pages/home.component.html"),
            include_str!("../src-ui/pages/settings.component.html"),
            include_str!("../src-ui/pages/details.component.html"),
            include_str!("../src-ui/pages/missing.component.html"),
        ] {
            tilt_ui_html::parse_template(source).expect("valid routing showcase template");
        }
    }
}
