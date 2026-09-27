//! Layout-neutral template providers and named CSS themes.

use std::{collections::HashMap, sync::Arc};

use bevy::{
    app::App,
    ecs::{component::Component, resource::Resource, world::World},
};
use tilt_ui_css::{StyleParseError, StyleSheet, parse_stylesheet};

use crate::{ComponentAssetHandles, StyleDirty};

/// Attributes and current theme visible to a provider while it resolves its effect.
pub struct ProviderContext<'a> {
    /// Static attributes on the provider tag.
    pub attributes: &'a [(String, String)],
    /// Theme selected globally, if any.
    pub active_theme: Option<&'a str>,
    /// Available named themes.
    pub themes: &'a UiThemes,
}

impl ProviderContext<'_> {
    /// Returns a static attribute by name.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// Returns whether a named theme is registered.
    pub fn has_theme(&self, name: &str) -> bool {
        self.themes.get(name).is_some()
    }
}

/// Styles applied to the descendants of a provider, below component-authored CSS.
#[derive(Default, Clone)]
pub struct ProviderEffect {
    /// Named theme to use within this subtree.
    pub theme: Option<String>,
    /// Additional stylesheets, ordered from lower to higher priority.
    pub stylesheets: Vec<Arc<StyleSheet>>,
}

/// Restricts which direct element tags a provider accepts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ProviderChildPolicy {
    /// Accept any direct child.
    #[default]
    Any,
    /// Accept only these direct element or component tags.
    Only(Vec<&'static str>),
}

/// Structural validation for a provider tag.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProviderRules {
    /// Require a direct `<body>` child.
    pub requires_body_child: bool,
    /// Restrict direct child tags.
    pub child_policy: ProviderChildPolicy,
}

/// Resolves an extensible, layout-neutral template tag.
pub trait UiProvider: Send + Sync + 'static {
    /// The custom HTML tag handled by this provider.
    fn tag(&self) -> &'static str;
    /// Optional structural constraints checked before a component is instantiated.
    fn rules(&self) -> ProviderRules {
        ProviderRules::default()
    }
    /// Called when styles are recomputed, so a provider can react to theme changes.
    fn resolve(&self, context: ProviderContext<'_>) -> Result<ProviderEffect, String>;
}

/// Registry of providers available during template instantiation and styling.
#[derive(Resource, Default, Clone)]
pub struct UiProviderRegistry(HashMap<String, Arc<dyn UiProvider>>);

impl UiProviderRegistry {
    /// Registers or replaces a provider with the same tag.
    pub fn register<P: UiProvider>(&mut self, provider: P) {
        self.0
            .insert(provider.tag().to_ascii_lowercase(), Arc::new(provider));
    }

    /// Finds a registered provider by template tag.
    pub fn get(&self, tag: &str) -> Option<&Arc<dyn UiProvider>> {
        self.0.get(tag)
    }
}

/// Associates a layout-neutral provider node with its static attributes.
#[derive(Component, Debug, Clone)]
pub struct ProviderScope {
    /// Provider tag name.
    pub tag: String,
    /// Static template attributes passed to the provider.
    pub attributes: Vec<(String, String)>,
}

/// Registry of parsed, named themes. A selected theme applies globally unless
/// a nested provider selects a different one.
#[derive(Resource, Default, Clone)]
pub struct UiThemes {
    themes: HashMap<String, Arc<StyleSheet>>,
    active: Option<String>,
}

impl UiThemes {
    /// Names available for selection, sorted for stable UI lists.
    pub fn names(&self) -> Vec<&str> {
        let mut names = self.themes.keys().map(String::as_str).collect::<Vec<_>>();
        names.sort_unstable();
        names
    }

    /// Currently selected global theme.
    pub fn active(&self) -> Option<&str> {
        self.active.as_deref()
    }

    /// Returns a parsed named theme.
    pub fn get(&self, name: &str) -> Option<&StyleSheet> {
        self.themes.get(name).map(Arc::as_ref)
    }

    pub(crate) fn has_media_rules(&self) -> bool {
        self.themes.values().any(|sheet| sheet.has_media_rules())
    }

    /// Registers or replaces a named theme. Invalid CSS leaves the old theme intact.
    pub fn register_css(&mut self, name: &str, css: &str) -> Result<(), String> {
        if !valid_theme_name(name) {
            return Err(format!(
                "invalid theme name {name:?}; use ASCII letters, digits, '-' or '_'"
            ));
        }
        let sheet = parse_stylesheet(css).map_err(|error: StyleParseError| error.to_string())?;
        self.themes.insert(name.to_owned(), Arc::new(sheet));
        Ok(())
    }

    /// Selects a registered theme by name.
    pub fn select(&mut self, name: &str) -> Result<(), String> {
        if !self.themes.contains_key(name) {
            return Err(format!("unknown UI theme {name:?}"));
        }
        self.active = Some(name.to_owned());
        Ok(())
    }

    /// Removes a theme and clears the global selection if it was active.
    pub fn remove(&mut self, name: &str) -> bool {
        let removed = self.themes.remove(name).is_some();
        if self.active.as_deref() == Some(name) {
            self.active = None;
        }
        removed
    }
}

fn valid_theme_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

/// Registers CSS for a named theme and restyles existing components.
pub fn register_ui_theme(world: &mut World, name: &str, css: &str) -> Result<(), String> {
    world.init_resource::<UiThemes>();
    world.resource_mut::<UiThemes>().register_css(name, css)?;
    mark_components_dirty(world);
    Ok(())
}

/// Selects a named theme globally and restyles existing components.
pub fn switch_ui_theme(world: &mut World, name: &str) -> Result<(), String> {
    world.init_resource::<UiThemes>();
    if world.resource::<UiThemes>().active() == Some(name) {
        return Ok(());
    }
    world.resource_mut::<UiThemes>().select(name)?;
    mark_components_dirty(world);
    Ok(())
}

/// Removes a named theme and restyles existing components.
pub fn remove_ui_theme(world: &mut World, name: &str) -> bool {
    let removed = world.resource_mut::<UiThemes>().remove(name);
    if removed {
        mark_components_dirty(world);
    }
    removed
}

fn mark_components_dirty(world: &mut World) {
    let owners = world
        .query_filtered::<bevy::ecs::entity::Entity, bevy::ecs::query::With<ComponentAssetHandles>>(
        )
        .iter(world)
        .collect::<Vec<_>>();
    for owner in owners {
        world.entity_mut(owner).insert(StyleDirty);
    }
}

/// Registers template providers on a Bevy app.
pub trait UiProviderAppExt {
    /// Registers or replaces a custom provider.
    fn register_ui_provider<P: UiProvider>(&mut self, provider: P) -> &mut Self;
}

impl UiProviderAppExt for App {
    fn register_ui_provider<P: UiProvider>(&mut self, provider: P) -> &mut Self {
        self.init_resource::<UiProviderRegistry>();
        self.world_mut()
            .resource_mut::<UiProviderRegistry>()
            .register(provider);
        self
    }
}

/// Registers named CSS themes on a Bevy app before it starts.
pub trait UiThemeAppExt {
    /// Registers or replaces a theme, returning a parse error for invalid CSS.
    fn register_ui_theme_css(&mut self, name: &str, css: &str) -> Result<&mut Self, String>;
}

impl UiThemeAppExt for App {
    fn register_ui_theme_css(&mut self, name: &str, css: &str) -> Result<&mut Self, String> {
        register_ui_theme(self.world_mut(), name, css)?;
        Ok(self)
    }
}

/// Built-in provider that selects a theme for its subtree.
#[derive(Debug, Default, Clone, Copy)]
pub struct ThemeProvider;

impl UiProvider for ThemeProvider {
    fn tag(&self) -> &'static str {
        "theme-provider"
    }

    fn resolve(&self, context: ProviderContext<'_>) -> Result<ProviderEffect, String> {
        let theme = context
            .attr("theme")
            .or(context.active_theme)
            .or_else(|| context.attr("default"));
        if theme.is_some_and(|name| !valid_theme_name(name)) {
            return Err("theme name may only contain ASCII letters, digits, '-' or '_'".into());
        }
        Ok(ProviderEffect {
            theme: theme.map(str::to_owned),
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::world::World;

    use super::{
        ProviderContext, ThemeProvider, UiProvider, UiThemes, register_ui_theme, switch_ui_theme,
    };

    #[test]
    fn invalid_theme_updates_preserve_active_theme_and_css() {
        let mut world = World::new();
        register_ui_theme(&mut world, "light", "button { color: #ffffff; }").unwrap();
        switch_ui_theme(&mut world, "light").unwrap();
        assert!(register_ui_theme(&mut world, "light", "button { unknown-property: 1; }").is_err());
        assert!(register_ui_theme(&mut world, "../bad", "button { color: #000000; }").is_err());
        assert!(switch_ui_theme(&mut world, "unknown").is_err());
        let themes = world.resource::<UiThemes>();
        assert_eq!(themes.active(), Some("light"));
        assert_eq!(themes.names(), vec!["light"]);
        assert_eq!(themes.get("light").unwrap().rules().len(), 1);
    }

    #[test]
    fn theme_provider_prefers_pinned_theme_then_global_then_default() {
        let provider = ThemeProvider;
        let themes = UiThemes::default();
        let attributes = vec![("default".into(), "light".into())];
        let context = ProviderContext {
            attributes: &attributes,
            active_theme: Some("dark"),
            themes: &themes,
        };
        assert_eq!(
            provider.resolve(context).unwrap().theme.as_deref(),
            Some("dark")
        );
        let attributes = vec![("theme".into(), "light".into())];
        let context = ProviderContext {
            attributes: &attributes,
            active_theme: Some("dark"),
            themes: &themes,
        };
        assert_eq!(
            provider.resolve(context).unwrap().theme.as_deref(),
            Some("light")
        );
    }
}
