//! Built-in CSS themes for TiltUI controls.

use std::sync::Arc;

use bevy::{
    asset::{Assets, Handle},
    ecs::{resource::Resource, world::World},
    text::Font,
};
use tilt_ui_css::{StyleSheet, parse_stylesheet};

use crate::{ComponentAssetHandles, StyleDirty};

/// Provides the lowest-priority stylesheet used for built-in control appearance.
#[derive(Resource, Debug, Clone)]
pub(crate) struct DefaultThemeStyleSheet(pub(crate) Arc<StyleSheet>);

impl DefaultThemeStyleSheet {
    /// Parses the bundled default theme once during style runtime setup.
    pub(crate) fn parse() -> Self {
        Self(Arc::new(
            parse_stylesheet(include_str!("default.css"))
                .expect("bundled TiltUI default theme must be valid CSS"),
        ))
    }
}

/// Replaces the lowest-priority theme CSS and restyles existing components.
pub fn set_default_theme_css(
    world: &mut World,
    source: &str,
) -> Result<(), tilt_ui_css::StyleParseError> {
    let parsed = Arc::new(parse_stylesheet(source)?);
    world.insert_resource(DefaultThemeStyleSheet(parsed));
    mark_all_components_dirty(world);
    Ok(())
}

/// Restores the bundled default theme and restyles existing components.
pub fn reset_default_theme(world: &mut World) {
    world.insert_resource(DefaultThemeStyleSheet::parse());
    mark_all_components_dirty(world);
}

fn mark_all_components_dirty(world: &mut World) {
    let owners = {
        let mut query = world.query_filtered::<bevy::ecs::entity::Entity, bevy::ecs::query::With<ComponentAssetHandles>>();
        query.iter(world).collect::<Vec<_>>()
    };
    for owner in owners {
        world.entity_mut(owner).insert(StyleDirty);
    }
}

#[derive(Resource)]
pub(crate) struct DefaultThemeFonts {
    pub(crate) regular: Handle<Font>,
    pub(crate) bold: Handle<Font>,
    pub(crate) symbols: Handle<Font>,
}

pub(crate) fn ensure_default_fonts(world: &mut World) {
    if world.contains_resource::<DefaultThemeFonts>() || !world.contains_resource::<Assets<Font>>()
    {
        return;
    }
    let mut fonts = world.resource_mut::<Assets<Font>>();
    let regular = fonts.add(Font::from_bytes(
        include_bytes!("fonts/NotoSans-Regular.ttf").to_vec(),
    ));
    let bold = fonts.add(Font::from_bytes(
        include_bytes!("fonts/NotoSans-Bold.ttf").to_vec(),
    ));
    let symbols = fonts.add(Font::from_bytes(
        include_bytes!("fonts/NotoSansSymbols2-Regular.ttf").to_vec(),
    ));
    world.insert_resource(DefaultThemeFonts {
        regular,
        bold,
        symbols,
    });
}

#[cfg(test)]
mod tests {
    use bevy::{asset::Handle, ecs::world::World};

    use super::*;
    use crate::{UiStyleSheetAsset, UiTemplateAsset};

    #[test]
    fn switching_theme_restyles_existing_components() {
        let mut world = World::new();
        let scope = world
            .spawn(ComponentAssetHandles {
                template: Handle::<UiTemplateAsset>::default(),
                stylesheet: Handle::<UiStyleSheetAsset>::default(),
            })
            .id();
        set_default_theme_css(&mut world, "button { color: #FF0000; }").unwrap();
        assert!(world.get::<StyleDirty>(scope).is_some());
        assert!(
            !world
                .resource::<DefaultThemeStyleSheet>()
                .0
                .rules
                .is_empty()
        );
        assert!(set_default_theme_css(&mut world, "button { missing-property: 1; }").is_err());
        assert_eq!(world.resource::<DefaultThemeStyleSheet>().0.rules.len(), 1);
    }
}
