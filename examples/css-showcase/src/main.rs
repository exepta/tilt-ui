use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use tilt_ui::prelude::*;

tilt_ui::include_components!();

fn main() {
    let resolution = std::env::var("TILT_UI_CSS_SIZE")
        .ok()
        .and_then(|size| {
            let (width, height) = size.split_once('x')?;
            Some((width.parse::<u32>().ok()?, height.parse::<u32>().ok()?))
        })
        .unwrap_or((1440, 1060));
    App::new()
        .add_plugins(
            TiltUiPlugin::new(tilt_ui_component_catalog())
                .with_source_root(concat!(env!("CARGO_MANIFEST_DIR"), "/src-ui")),
        )
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "TiltUI CSS showcase".into(),
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
    if let Ok(path) = std::env::var("TILT_UI_CSS_SCREENSHOT") {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
    *captured = true;
}

#[cfg(test)]
mod tests {
    #[test]
    fn css_showcase_stylesheet_parses() {
        tilt_ui::tilt_ui_css::parse_stylesheet(include_str!(
            "../src-ui/pages/css-gallery.component.css"
        ))
        .expect("valid CSS showcase stylesheet");
    }
}
