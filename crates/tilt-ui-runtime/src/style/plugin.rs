use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use bevy::{
    app::{App, Plugin, Update},
    asset::{AssetEvent, Assets, Handle},
    ecs::{
        change_detection::DetectChanges,
        component::Component,
        entity::Entity,
        hierarchy::ChildOf,
        message::MessageReader,
        query::Changed,
        resource::Resource,
        schedule::SystemSet,
        system::{Commands, Query, Res, ResMut},
        world::World,
    },
    prelude::IntoScheduleConfigs,
    ui::ComputedNode,
    window::{PrimaryWindow, Window},
};

use crate::component::document::DocumentStylesheets;
use crate::scroll::ensure_scrollbar_parts;
use crate::theme::DefaultThemeStyleSheet;
use crate::{
    ComponentAssetHandles, ComponentStyleOwner, ElementClasses, ElementId, ElementState,
    StaticAttributes, StyleDirty, TiltControlSystems, TiltUiComponentRuntimeSet, UiStyleSheetAsset,
};
use crate::{ProviderContext, ProviderEffect, ProviderScope, UiProviderRegistry, UiThemes};

use super::state::StyleVariables;
use super::{
    apply::{resolve_and_apply_tree, sync_text_transform},
    cascade::{Cascade, StyleOrigin, key},
    inline::InlineStyle,
    matcher::{SelectorView, matching_specificity},
    motion::{reconcile_scope, tick_animations},
};

#[derive(Component, Clone, Copy)]
struct DeferredValueContext(bevy::math::Vec2);

type SelectorMetadataChanged = bevy::ecs::query::Or<(
    Changed<ElementState>,
    Changed<ElementClasses>,
    Changed<ElementId>,
    Changed<StaticAttributes>,
    Changed<InlineStyle>,
)>;

/// Stores the viewport used to evaluate typed CSS media conditions.
#[derive(bevy::ecs::resource::Resource, Debug, Clone, Copy, Default)]
pub struct TiltUiMediaEnvironment(pub Option<tilt_ui_css::MediaEnvironment>);

/// Records a scope that has received available defaults while its author stylesheet loads.
#[derive(Component)]
pub(crate) struct AuthorStylePending;

/// Ordering point after initial and updated styles have been applied.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TiltUiStyleRuntimeSet {
    Apply,
}

pub(crate) fn author_style_pending(world: &World, entity: Entity) -> bool {
    world.get::<AuthorStylePending>(entity).is_some()
}

#[derive(Component, Default)]
struct MediaMatchState(Vec<bool>);

#[derive(Resource, Default)]
struct StyleInvalidations(HashMap<Entity, HashSet<Entity>>);

#[derive(Component)]
struct StyleSelectorCache(SelectorView);

#[derive(Resource)]
struct DocumentStyleSheet(Arc<tilt_ui_css::StyleSheet>);

/// Registers component-scoped CSS matching, cascade, and Bevy UI application.
#[derive(Debug, Clone, Copy)]
pub struct TiltUiStyleRuntimePlugin {
    default_theme: bool,
}

impl Default for TiltUiStyleRuntimePlugin {
    fn default() -> Self {
        Self {
            default_theme: true,
        }
    }
}

impl TiltUiStyleRuntimePlugin {
    /// Configures whether the built-in default theme participates in the cascade.
    pub const fn with_default_theme(mut self, enabled: bool) -> Self {
        self.default_theme = enabled;
        self
    }
}

impl Plugin for TiltUiStyleRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(super::background::BackgroundRuntimePlugin);
        app.add_plugins(super::border::BorderRuntimePlugin);
        app.add_plugins(super::backdrop::BackdropRuntimePlugin);
        app.add_plugins(super::animated::AnimatedRuntimePlugin);
        app.add_message::<AssetEvent<UiStyleSheetAsset>>();
        if self.default_theme && !app.world().contains_resource::<DefaultThemeStyleSheet>() {
            app.insert_resource(DefaultThemeStyleSheet::parse());
        }
        app.init_resource::<TiltUiMediaEnvironment>()
            .init_resource::<StyleInvalidations>();
        app.add_systems(
            Update,
            (
                mark_theme_changes,
                refresh_document_styles,
                invalidate_responsive_styles,
                invalidate_deferred_values,
                mark_ready_author_styles,
                mark_changed_style_owners,
                apply_dirty_styles.in_set(TiltUiStyleRuntimeSet::Apply),
                sync_text_transform,
                tick_animations,
            )
                .chain()
                .after(TiltUiComponentRuntimeSet::Instantiate)
                .after(TiltControlSystems::Selection)
                .after(crate::component::binding_runtime::apply_bindings),
        );
    }
}

fn refresh_document_styles(
    mut commands: Commands,
    links: Option<Res<DocumentStylesheets>>,
    mut events: MessageReader<AssetEvent<UiStyleSheetAsset>>,
    assets: Res<Assets<UiStyleSheetAsset>>,
    current: Option<Res<DocumentStyleSheet>>,
    scopes: Query<Entity, bevy::ecs::query::With<ComponentAssetHandles>>,
) {
    let changed = events.read().any(|event| {
        let id = match event {
            AssetEvent::Added { id }
            | AssetEvent::Modified { id }
            | AssetEvent::Removed { id }
            | AssetEvent::Unused { id }
            | AssetEvent::LoadedWithDependencies { id } => *id,
        };
        links
            .as_ref()
            .is_some_and(|links| links.0.iter().any(|handle| handle.id() == id))
    });
    let Some(links) = links else { return };
    if !links.is_changed() && !changed && current.is_some() {
        return;
    }
    let Some(stylesheet) = merge_stylesheets(&assets, links.0.iter()) else {
        return;
    };
    commands.insert_resource(DocumentStyleSheet(Arc::new(stylesheet)));
    for scope in &scopes {
        commands.entity(scope).insert(StyleDirty);
    }
}

fn mark_theme_changes(
    mut commands: Commands<'_, '_>,
    themes: Option<Res<'_, UiThemes>>,
    providers: Option<Res<'_, UiProviderRegistry>>,
    scopes: Query<'_, '_, Entity, bevy::ecs::query::With<ComponentAssetHandles>>,
) {
    if !themes.as_ref().is_some_and(|themes| themes.is_changed())
        && !providers
            .as_ref()
            .is_some_and(|providers| providers.is_changed())
    {
        return;
    }
    for owner in &scopes {
        commands.entity(owner).insert(StyleDirty);
    }
}

fn invalidate_responsive_styles(
    mut commands: Commands<'_, '_>,
    mut environment: ResMut<'_, TiltUiMediaEnvironment>,
    windows: Query<'_, '_, &Window, bevy::ecs::query::With<PrimaryWindow>>,
    stylesheets: bevy::ecs::system::Res<'_, Assets<UiStyleSheetAsset>>,
    default_theme: Option<bevy::ecs::system::Res<'_, DefaultThemeStyleSheet>>,
    themes: Option<bevy::ecs::system::Res<'_, UiThemes>>,
    providers: Query<'_, '_, &ProviderScope>,
    scopes: Query<'_, '_, (Entity, &ComponentAssetHandles, Option<&MediaMatchState>)>,
    deferred: Query<'_, '_, &ComponentStyleOwner, bevy::ecs::query::With<DeferredValueContext>>,
) {
    let Some(window) = windows.iter().next() else {
        return;
    };
    let next = media_environment_for_window(window);
    if environment.0 == Some(next) {
        return;
    }
    environment.0 = Some(next);
    for owner in &deferred {
        commands.entity(owner.0).insert(StyleDirty);
    }
    let provider_media_possible = !providers.is_empty();
    let named_theme_has_media = themes
        .as_ref()
        .is_some_and(|themes| themes.has_media_rules());
    for (owner, handles, old_state) in &scopes {
        if provider_media_possible || named_theme_has_media {
            commands.entity(owner).insert(StyleDirty);
            continue;
        }
        let Some(stylesheet) = merge_author_stylesheets(&stylesheets, handles) else {
            continue;
        };
        let theme_has_media = default_theme
            .as_ref()
            .is_some_and(|theme| theme.0.has_media_rules());
        if !theme_has_media && !stylesheet.has_media_rules() {
            continue;
        }
        let mut matches = default_theme
            .as_ref()
            .map(|theme| {
                theme
                    .0
                    .media_rules()
                    .iter()
                    .map(|rule| rule.matches(next))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        matches.extend(
            stylesheet
                .media_rules()
                .iter()
                .map(|rule| rule.matches(next)),
        );
        if old_state.is_none_or(|old| old.0 != matches) {
            commands
                .entity(owner)
                .insert((StyleDirty, MediaMatchState(matches)));
        }
    }
}

fn media_environment_for_window(window: &Window) -> tilt_ui_css::MediaEnvironment {
    let canvas = tilt_ui_css::MediaEnvironment {
        width: window.resolution.width(),
        height: window.resolution.height(),
    };
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(browser) = web_sys::window() {
            let width = browser.inner_width().ok().and_then(|value| value.as_f64());
            let height = browser.inner_height().ok().and_then(|value| value.as_f64());
            if let (Some(width), Some(height)) = (width, height)
                && width.is_finite()
                && height.is_finite()
                && width > 0.0
                && height > 0.0
            {
                return tilt_ui_css::MediaEnvironment {
                    width: width as f32,
                    height: height as f32,
                };
            }
        }
    }
    canvas
}

fn invalidate_deferred_values(
    mut commands: Commands,
    deferred: Query<(Entity, &ComponentStyleOwner, &DeferredValueContext)>,
    parents: Query<&ChildOf>,
    detached: Query<&super::backdrop::DetachedBackdrop>,
    nodes: Query<&ComputedNode>,
    environment: Res<TiltUiMediaEnvironment>,
) {
    let viewport = environment.0.unwrap_or(tilt_ui_css::MediaEnvironment {
        width: 0.0,
        height: 0.0,
    });
    for (entity, owner, previous) in &deferred {
        if containing_size_from_queries(entity, &parents, &detached, &nodes, viewport) != previous.0
        {
            commands.entity(owner.0).insert(StyleDirty);
        }
    }
}

fn mark_ready_author_styles(
    mut commands: Commands<'_, '_>,
    stylesheets: bevy::ecs::system::Res<'_, Assets<UiStyleSheetAsset>>,
    pending: Query<
        '_,
        '_,
        (Entity, &ComponentAssetHandles),
        bevy::ecs::query::With<AuthorStylePending>,
    >,
) {
    for (owner, handles) in &pending {
        if merge_author_stylesheets(&stylesheets, handles).is_some() {
            commands
                .entity(owner)
                .remove::<AuthorStylePending>()
                .insert(StyleDirty);
        }
    }
}

fn mark_changed_style_owners(
    mut commands: Commands<'_, '_>,
    mut invalidations: ResMut<'_, StyleInvalidations>,
    changed: Query<'_, '_, (Entity, &ComponentStyleOwner), SelectorMetadataChanged>,
    changed_inline: Query<'_, '_, &ComponentStyleOwner, Changed<InlineStyle>>,
    hierarchy_changed: Query<'_, '_, &ComponentStyleOwner, Changed<ChildOf>>,
) {
    for (entity, owner) in &changed {
        // The selector view includes descendants and following siblings in a
        // partial invalidation. That also propagates changed CSS variables;
        // marking the whole scope dirty here would restyle every widget on
        // each hover transition.
        invalidations.0.entry(owner.0).or_default().insert(entity);
    }
    for owner in &changed_inline {
        commands.entity(owner.0).insert(StyleDirty);
    }
    for owner in &hierarchy_changed {
        commands.entity(owner.0).insert(StyleDirty);
    }
}

fn apply_dirty_styles(world: &mut World) {
    crate::theme::ensure_default_fonts(world);
    let dirty = {
        let mut query = world.query::<(Entity, &ComponentAssetHandles, &StyleDirty)>();
        query
            .iter(world)
            .map(|(entity, handles, _)| (entity, handles.clone()))
            .collect::<Vec<_>>()
    };
    let full_owners = dirty
        .iter()
        .map(|(owner, _)| *owner)
        .collect::<HashSet<_>>();
    let invalidations = std::mem::take(&mut world.resource_mut::<StyleInvalidations>().0);
    if dirty.is_empty() && invalidations.is_empty() {
        return;
    }
    let default_theme = world
        .get_resource::<DefaultThemeStyleSheet>()
        .map(|theme| theme.0.clone());
    let themes = world
        .get_resource::<UiThemes>()
        .cloned()
        .unwrap_or_default();
    let providers = world
        .get_resource::<UiProviderRegistry>()
        .cloned()
        .unwrap_or_default();
    let environment = world
        .get_resource::<TiltUiMediaEnvironment>()
        .and_then(|environment| environment.0)
        .unwrap_or(tilt_ui_css::MediaEnvironment {
            width: 0.0,
            height: 0.0,
        });
    for (owner, handles) in dirty {
        let stylesheet = restyle_scope(
            world,
            owner,
            &handles,
            default_theme.as_deref(),
            &themes,
            &providers,
            environment,
            None,
        );
        let mut owner_entity = world.entity_mut(owner);
        owner_entity.remove::<StyleDirty>();
        if stylesheet.is_some() {
            owner_entity.remove::<AuthorStylePending>();
        } else {
            owner_entity.insert(AuthorStylePending);
        }
    }
    for (owner, changed) in invalidations {
        if full_owners.contains(&owner) {
            continue;
        }
        let Some(handles) = world.get::<ComponentAssetHandles>(owner).cloned() else {
            continue;
        };
        restyle_scope(
            world,
            owner,
            &handles,
            default_theme.as_deref(),
            &themes,
            &providers,
            environment,
            Some(&changed),
        );
    }
}

fn merge_author_stylesheets(
    assets: &Assets<UiStyleSheetAsset>,
    handles: &ComponentAssetHandles,
) -> Option<tilt_ui_css::StyleSheet> {
    merge_stylesheets(
        assets,
        std::iter::once(&handles.stylesheet).chain(&handles.additional_stylesheets),
    )
}

fn merge_stylesheets<'a>(
    assets: &Assets<UiStyleSheetAsset>,
    handles: impl IntoIterator<Item = &'a Handle<UiStyleSheetAsset>>,
) -> Option<tilt_ui_css::StyleSheet> {
    let mut merged = tilt_ui_css::StyleSheet::default();
    for handle in handles {
        let sheet = assets.get(handle)?.stylesheet();
        let offset = merged
            .rules
            .iter()
            .map(|rule| rule.source_order)
            .chain(
                merged
                    .media_rules
                    .iter()
                    .flat_map(|media| media.rules.iter().map(|rule| rule.source_order)),
            )
            .max()
            .map_or(0, |last| last + 1);
        merged
            .rules
            .extend(sheet.rules.iter().cloned().map(|mut rule| {
                rule.source_order += offset;
                rule
            }));
        merged
            .media_rules
            .extend(sheet.media_rules.iter().cloned().map(|mut media| {
                for rule in &mut media.rules {
                    rule.source_order += offset;
                }
                media
            }));
        // A later author file replaces an earlier keyframe definition of the same name.
        for keyframes in &sheet.keyframes {
            merged
                .keyframes
                .retain(|earlier| earlier.name != keyframes.name);
            merged.keyframes.push(keyframes.clone());
        }
    }
    Some(merged)
}

fn restyle_scope(
    world: &mut World,
    owner: Entity,
    handles: &ComponentAssetHandles,
    default_theme: Option<&tilt_ui_css::StyleSheet>,
    themes: &UiThemes,
    providers: &UiProviderRegistry,
    environment: tilt_ui_css::MediaEnvironment,
    changed: Option<&HashSet<Entity>>,
) -> Option<tilt_ui_css::StyleSheet> {
    let stylesheet =
        merge_author_stylesheets(world.resource::<Assets<UiStyleSheetAsset>>(), handles);
    let document_styles = world
        .get_resource::<DocumentStyleSheet>()
        .map(|styles| styles.0.clone());
    let has_provider_scopes = world.query::<&ProviderScope>().iter(world).next().is_some();
    if default_theme.is_none()
        && stylesheet.is_none()
        && document_styles.is_none()
        && themes.active().is_none()
        && !has_provider_scopes
    {
        return stylesheet;
    }
    let mut view = if changed.is_some() {
        world
            .entity_mut(owner)
            .take::<StyleSelectorCache>()
            .map_or_else(|| SelectorView::build(world, owner), |cache| cache.0)
    } else {
        SelectorView::build(world, owner)
    };
    if changed.is_some_and(|changed| !view.refresh(world, changed)) {
        view = SelectorView::build(world, owner);
    }
    let affected = changed.map_or_else(
        || view.entities().collect::<Vec<_>>(),
        |changed| view.affected_by(changed),
    );
    let mut overflow_changed = false;
    let mut provider_cache = HashMap::new();
    let mut affected = affected;
    affected.sort_by_key(|entity| hierarchy_depth(world, *entity));
    for entity in affected {
        let mut cascade = Cascade::with_variables(inherited_variables(world, entity));
        let (selected_theme, extra_styles) = if has_provider_scopes {
            provider_layers(world, entity, themes, providers, &mut provider_cache)
        } else {
            (themes.active().map(str::to_owned), Vec::new())
        };
        apply_stylesheet(
            &mut cascade,
            &view,
            entity,
            default_theme,
            StyleOrigin::DefaultTheme,
            environment,
        );
        apply_stylesheet(
            &mut cascade,
            &view,
            entity,
            selected_theme.as_deref().and_then(|name| themes.get(name)),
            StyleOrigin::NamedTheme,
            environment,
        );
        apply_stylesheet(
            &mut cascade,
            &view,
            entity,
            document_styles.as_deref(),
            StyleOrigin::Document,
            environment,
        );
        for (index, sheet) in &extra_styles {
            apply_stylesheet(
                &mut cascade,
                &view,
                entity,
                Some(sheet),
                StyleOrigin::Provider(*index),
                environment,
            );
        }
        apply_stylesheet(
            &mut cascade,
            &view,
            entity,
            stylesheet.as_ref(),
            StyleOrigin::Author,
            environment,
        );
        apply_inline_style(&mut cascade, world.get::<InlineStyle>(entity));
        let next = finish_cascade(world, entity, cascade, environment);
        let previous = world.get::<super::state::CascadedStyle>(entity);
        if previous != Some(&next) {
            overflow_changed |= previous.is_none_or(|previous| {
                previous.0.overflow_x != next.0.overflow_x
                    || previous.0.overflow_y != next.0.overflow_y
            });
            world.entity_mut(entity).insert(next);
        }
    }
    if (changed.is_none() || overflow_changed) && ensure_scrollbar_parts(world, owner) {
        view = SelectorView::build(world, owner);
        for entity in view.entities() {
            if world.get::<super::state::CascadedStyle>(entity).is_some() {
                continue;
            }
            let mut cascade = Cascade::with_variables(inherited_variables(world, entity));
            let (selected_theme, extra_styles) = if has_provider_scopes {
                provider_layers(world, entity, themes, providers, &mut provider_cache)
            } else {
                (themes.active().map(str::to_owned), Vec::new())
            };
            apply_stylesheet(
                &mut cascade,
                &view,
                entity,
                default_theme,
                StyleOrigin::DefaultTheme,
                environment,
            );
            apply_stylesheet(
                &mut cascade,
                &view,
                entity,
                selected_theme.as_deref().and_then(|name| themes.get(name)),
                StyleOrigin::NamedTheme,
                environment,
            );
            apply_stylesheet(
                &mut cascade,
                &view,
                entity,
                document_styles.as_deref(),
                StyleOrigin::Document,
                environment,
            );
            for (index, sheet) in &extra_styles {
                apply_stylesheet(
                    &mut cascade,
                    &view,
                    entity,
                    Some(sheet),
                    StyleOrigin::Provider(*index),
                    environment,
                );
            }
            apply_stylesheet(
                &mut cascade,
                &view,
                entity,
                stylesheet.as_ref(),
                StyleOrigin::Author,
                environment,
            );
            apply_inline_style(&mut cascade, world.get::<InlineStyle>(entity));
            let next = finish_cascade(world, entity, cascade, environment);
            world.entity_mut(entity).insert(next);
        }
    }
    resolve_and_apply_tree(world, owner);
    reconcile_scope(world, owner, stylesheet.as_ref(), default_theme);
    world.entity_mut(owner).insert(StyleSelectorCache(view));
    stylesheet
}

fn hierarchy_depth(world: &World, entity: Entity) -> usize {
    let mut current = entity;
    let mut depth = 0;
    while let Some(parent) = super::backdrop::logical_parent(world, current) {
        depth += 1;
        current = parent;
    }
    depth
}

fn inherited_variables(
    world: &World,
    entity: Entity,
) -> std::collections::BTreeMap<String, String> {
    let mut current = super::backdrop::logical_parent(world, entity);
    while let Some(parent) = current {
        if let Some(variables) = world.get::<StyleVariables>(parent) {
            return variables.0.clone();
        }
        current = super::backdrop::logical_parent(world, parent);
    }
    Default::default()
}

fn containing_size(
    world: &World,
    entity: Entity,
    environment: tilt_ui_css::MediaEnvironment,
) -> bevy::math::Vec2 {
    let mut current = super::backdrop::logical_parent(world, entity);
    while let Some(parent) = current {
        if let Some(node) = world.get::<ComputedNode>(parent) {
            let size = (node.content_box().size() * node.inverse_scale_factor())
                .max(bevy::math::Vec2::ZERO);
            return size;
        }
        current = super::backdrop::logical_parent(world, parent);
    }
    bevy::math::Vec2::new(environment.width, environment.height)
}

fn containing_size_from_queries(
    entity: Entity,
    parents: &Query<&ChildOf>,
    detached: &Query<&super::backdrop::DetachedBackdrop>,
    nodes: &Query<&ComputedNode>,
    environment: tilt_ui_css::MediaEnvironment,
) -> bevy::math::Vec2 {
    let parent_of = |entity| {
        parents
            .get(entity)
            .ok()
            .map(|parent| parent.0)
            .or_else(|| detached.get(entity).ok().map(|marker| marker.parent))
    };
    let mut current = parent_of(entity);
    while let Some(parent) = current {
        if let Ok(node) = nodes.get(parent) {
            let size = (node.content_box().size() * node.inverse_scale_factor())
                .max(bevy::math::Vec2::ZERO);
            return size;
        }
        current = parent_of(parent);
    }
    bevy::math::Vec2::new(environment.width, environment.height)
}

fn finish_cascade(
    world: &mut World,
    entity: Entity,
    mut cascade: Cascade,
    environment: tilt_ui_css::MediaEnvironment,
) -> super::state::CascadedStyle {
    let size = containing_size(world, entity, environment);
    let font_size = inherited_font_size(world, entity, environment);
    let has_deferred = cascade.has_deferred();
    let variables = StyleVariables(cascade.variables());
    cascade.resolve_deferred(|property| {
        let vertical = matches!(
            property,
            "height" | "min-height" | "max-height" | "top" | "bottom" | "row-gap"
        );
        tilt_ui_css::ValueContext {
            percentage_base: if property == "font-size" {
                font_size
            } else if vertical {
                size.y
            } else {
                size.x
            },
            viewport_width: environment.width,
            viewport_height: environment.height,
        }
    });
    if world.get::<StyleVariables>(entity) != Some(&variables) {
        world.entity_mut(entity).insert(variables);
    }
    if has_deferred {
        world.entity_mut(entity).insert(DeferredValueContext(size));
    } else {
        world.entity_mut(entity).remove::<DeferredValueContext>();
    }
    cascade.finish()
}

fn inherited_font_size(
    world: &World,
    entity: Entity,
    environment: tilt_ui_css::MediaEnvironment,
) -> f32 {
    let mut ancestors = Vec::new();
    let mut current = super::backdrop::logical_parent(world, entity);
    while let Some(parent) = current {
        ancestors.push(parent);
        current = super::backdrop::logical_parent(world, parent);
    }
    let mut size = 16.0;
    for ancestor in ancestors.into_iter().rev() {
        let Some(value) = world
            .get::<super::state::CascadedStyle>(ancestor)
            .and_then(|style| style.0.font_size)
        else {
            continue;
        };
        size = match value {
            tilt_ui_css::Length::Auto => size,
            tilt_ui_css::Length::Px(value) => value,
            tilt_ui_css::Length::Percent(value) => size * value / 100.0,
            tilt_ui_css::Length::Vw(value) => environment.width * value / 100.0,
            tilt_ui_css::Length::Vh(value) => environment.height * value / 100.0,
        };
    }
    size
}

fn provider_layers(
    world: &World,
    entity: Entity,
    themes: &UiThemes,
    providers: &UiProviderRegistry,
    cache: &mut HashMap<Entity, ProviderEffect>,
) -> (
    Option<String>,
    Vec<(usize, std::sync::Arc<tilt_ui_css::StyleSheet>)>,
) {
    let mut scopes = Vec::new();
    let mut current = Some(entity);
    while let Some(node) = current {
        if world.get::<ProviderScope>(node).is_some() {
            scopes.push(node);
        }
        current = super::backdrop::logical_parent(world, node);
    }
    scopes.reverse();
    let mut theme = themes.active().map(str::to_owned);
    let mut sheets = Vec::new();
    for scope in scopes {
        let effect = cache.entry(scope).or_insert_with(|| {
            let Some(node) = world.get::<ProviderScope>(scope) else {
                return ProviderEffect::default();
            };
            let Some(provider) = providers.get(&node.tag) else {
                return ProviderEffect::default();
            };
            match provider.resolve(ProviderContext {
                attributes: &node.attributes,
                active_theme: themes.active(),
                themes,
            }) {
                Ok(mut effect) => {
                    if let Some(name) = &effect.theme
                        && themes.get(name).is_none()
                    {
                        bevy::log::warn!(
                            "provider <{}> requested unknown UI theme {name:?}",
                            node.tag
                        );
                        effect.theme = None;
                    }
                    effect
                }
                Err(error) => {
                    bevy::log::warn!("provider <{}> failed: {error}", node.tag);
                    ProviderEffect::default()
                }
            }
        });
        if let Some(name) = &effect.theme {
            theme = Some(name.clone());
        }
        for sheet in &effect.stylesheets {
            sheets.push((sheets.len(), sheet.clone()));
        }
    }
    (theme, sheets)
}

fn apply_stylesheet(
    cascade: &mut Cascade,
    view: &SelectorView,
    entity: Entity,
    stylesheet: Option<&tilt_ui_css::StyleSheet>,
    origin: StyleOrigin,
    environment: tilt_ui_css::MediaEnvironment,
) {
    let Some(stylesheet) = stylesheet else {
        return;
    };
    for rule in stylesheet.rules() {
        apply_rule(cascade, view, entity, rule, origin);
    }
    for media_rule in stylesheet
        .media_rules()
        .iter()
        .filter(|rule| rule.matches(environment))
    {
        for rule in &media_rule.rules {
            apply_rule(cascade, view, entity, rule, origin);
        }
    }
}

fn apply_rule(
    cascade: &mut Cascade,
    view: &SelectorView,
    entity: Entity,
    rule: &tilt_ui_css::StyleRule,
    origin: StyleOrigin,
) {
    if let Some(specificity) = matching_specificity(view, entity, &rule.selectors) {
        for (declaration_order, declaration) in rule.declarations.iter().enumerate() {
            cascade.apply(
                declaration,
                key(origin, specificity, rule.source_order, declaration_order),
            );
        }
    }
}

fn apply_inline_style(cascade: &mut Cascade, inline: Option<&InlineStyle>) {
    let Some(inline) = inline else { return };
    for (index, declaration) in inline.declarations.iter().enumerate() {
        cascade.apply(
            declaration,
            key(StyleOrigin::Inline, tilt_ui_css::Specificity(0), 0, index),
        );
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        app::App,
        asset::{Assets, Handle},
        color::Color,
        ecs::world::World,
        text::{TextColor, TextFont},
        time::Time,
        ui::{BackgroundColor, BorderColor, Node, Val},
        window::{PrimaryWindow, Window},
    };
    use std::time::Duration;
    use tilt_ui_core::ElementKind;
    use tilt_ui_css::parse_stylesheet;

    use super::{TiltUiMediaEnvironment, TiltUiStyleRuntimePlugin, merge_author_stylesheets};

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn media_environment_uses_logical_window_size_on_native() {
        let mut window = Window::default();
        window.resolution.set(640.0, 480.0);
        assert_eq!(
            super::media_environment_for_window(&window),
            tilt_ui_css::MediaEnvironment {
                width: 640.0,
                height: 480.0
            }
        );
    }
    use crate::style::InlineStyle;
    use crate::{
        ComponentAssetHandles, ComponentStyleOwner, ControlChecked, ElementClasses, ElementId,
        ElementState, PropertyBinding, PropertyBindings, ProviderContext, ProviderEffect,
        ProviderScope, StyleDirty, ThemeProvider, TiltElement, TiltText, UiProvider,
        UiProviderRegistry, UiStyleSheetAsset, UiTemplateAsset, UiThemes, register_ui_theme,
        set_control_checked, switch_ui_theme,
    };

    #[derive(Clone, Copy)]
    struct AccentProvider;

    impl UiProvider for AccentProvider {
        fn tag(&self) -> &'static str {
            "accent-provider"
        }

        fn resolve(&self, context: ProviderContext<'_>) -> Result<ProviderEffect, String> {
            let color = context.attr("color").unwrap_or("#000000");
            Ok(ProviderEffect {
                stylesheets: vec![std::sync::Arc::new(
                    parse_stylesheet(&format!("button {{ border-color: {color}; }}"))
                        .map_err(|error| error.to_string())?,
                )],
                ..Default::default()
            })
        }
    }

    fn stylesheet(world: &mut World, source: &str) -> Handle<UiStyleSheetAsset> {
        world
            .resource_mut::<Assets<UiStyleSheetAsset>>()
            .add(UiStyleSheetAsset::new(parse_stylesheet(source).unwrap()))
    }

    #[test]
    fn multiple_author_stylesheets_keep_css_order_and_media_rules() {
        let mut world = World::new();
        world.init_resource::<Assets<UiStyleSheetAsset>>();
        let first = stylesheet(
            &mut world,
            "button { color: #ff0000; } @media (min-width: 300px) { button { width: 10px; } }",
        );
        let second = stylesheet(
            &mut world,
            "button { color: #0000ff; } @media (min-width: 300px) { button { width: 20px; } }",
        );
        let handles = ComponentAssetHandles {
            template: Handle::<UiTemplateAsset>::default(),
            stylesheet: first,
            additional_stylesheets: vec![second],
        };
        let merged =
            merge_author_stylesheets(world.resource::<Assets<UiStyleSheetAsset>>(), &handles)
                .unwrap();
        assert!(merged.rules[1].source_order > merged.rules[0].source_order);
        assert!(
            merged.media_rules[1].rules[0].source_order
                > merged.media_rules[0].rules[0].source_order
        );
    }

    fn owner(
        world: &mut World,
        stylesheet: Handle<UiStyleSheetAsset>,
    ) -> bevy::ecs::entity::Entity {
        world
            .spawn((
                ComponentAssetHandles {
                    template: Handle::<UiTemplateAsset>::default(),
                    stylesheet,
                    additional_stylesheets: Vec::new(),
                },
                StyleDirty,
            ))
            .id()
    }

    fn app() -> App {
        let mut app = App::new();
        app.insert_resource(Assets::<UiStyleSheetAsset>::default());
        app.add_message::<bevy::asset::AssetEvent<UiStyleSheetAsset>>();
        app.add_plugins(TiltUiStyleRuntimePlugin::default().with_default_theme(false));
        app
    }

    #[test]
    fn document_css_reaches_nested_component_scopes_before_author_css() {
        let mut app = app();
        let linked = stylesheet(
            app.world_mut(),
            "button { background-color: #123456; width: 50px; }",
        );
        app.world_mut()
            .insert_resource(crate::component::document::DocumentStylesheets(vec![
                linked,
            ]));
        let empty = stylesheet(app.world_mut(), "");
        let document = owner(app.world_mut(), empty);
        let nested_author = stylesheet(app.world_mut(), "button { width: 80px; }");
        let nested = owner(app.world_mut(), nested_author);
        app.world_mut().entity_mut(document).add_child(nested);
        let button = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Button,
                },
                ComponentStyleOwner(nested),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(nested).add_child(button);
        app.update();
        assert_eq!(
            app.world().get::<BackgroundColor>(button).unwrap().0,
            Color::srgb(18.0 / 255.0, 52.0 / 255.0, 86.0 / 255.0)
        );
        assert_eq!(
            app.world().get::<Node>(button).unwrap().width,
            Val::Px(80.0)
        );
    }

    #[test]
    fn bound_inline_style_overrides_author_and_restores_static_style_when_cleared() {
        let mut app = app();
        app.world_mut().init_resource::<crate::UiBindingStore>();
        app.world_mut().init_resource::<crate::UiSharedValues>();
        let author = stylesheet(app.world_mut(), "button { width: 24px; }");
        let boundary = owner(app.world_mut(), author);
        let button = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Button,
                },
                ComponentStyleOwner(boundary),
                Node::default(),
                InlineStyle::parse("width: 50px;").unwrap(),
                PropertyBindings {
                    bindings: vec![PropertyBinding {
                        name: "style".into(),
                        expression: "'width: 80px;'".into(),
                    }],
                },
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(button);
        crate::component::binding_runtime::apply_bindings(app.world_mut());
        app.update();
        assert_eq!(
            app.world().get::<Node>(button).unwrap().width,
            Val::Px(80.0)
        );

        app.world_mut()
            .get_mut::<PropertyBindings>(button)
            .unwrap()
            .bindings[0]
            .expression = "''".into();
        crate::component::binding_runtime::apply_bindings(app.world_mut());
        app.update();
        assert_eq!(
            app.world().get::<Node>(button).unwrap().width,
            Val::Px(50.0)
        );
        assert_eq!(
            app.world().get::<ComponentStyleOwner>(button).unwrap().0,
            boundary
        );
    }

    #[test]
    fn bound_inline_important_overrides_important_stylesheet() {
        let mut app = app();
        app.world_mut().init_resource::<crate::UiBindingStore>();
        app.world_mut().init_resource::<crate::UiSharedValues>();
        let author = stylesheet(app.world_mut(), "button { width: 24px !important; }");
        let boundary = owner(app.world_mut(), author);
        let button = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Button,
                },
                ComponentStyleOwner(boundary),
                Node::default(),
                InlineStyle::parse("width: 50px;").unwrap(),
                PropertyBindings {
                    bindings: vec![PropertyBinding {
                        name: "style".into(),
                        expression: "'width: 80px !important;'".into(),
                    }],
                },
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(button);
        crate::component::binding_runtime::apply_bindings(app.world_mut());
        app.update();
        assert_eq!(
            app.world().get::<Node>(button).unwrap().width,
            Val::Px(80.0)
        );
        app.world_mut()
            .get_mut::<PropertyBindings>(button)
            .unwrap()
            .bindings[0]
            .expression = "''".into();
        crate::component::binding_runtime::apply_bindings(app.world_mut());
        app.update();
        assert_eq!(
            app.world().get::<Node>(button).unwrap().width,
            Val::Px(24.0)
        );
    }

    #[test]
    fn root_variables_math_and_important_restyle_descendants() {
        let mut app = app();
        let author = stylesheet(
            app.world_mut(),
            ":root { --space: calc(50% + 5px); font-size: 20px; } button { width: var(--space); font-size: calc(100% + 2px); color: #111111 !important; }",
        );
        let boundary = owner(app.world_mut(), author);
        let root = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Div,
                },
                ComponentStyleOwner(boundary),
                Node::default(),
                bevy::ui::ComputedNode {
                    size: bevy::math::Vec2::new(200.0, 100.0),
                    ..Default::default()
                },
            ))
            .id();
        let button = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Button,
                },
                ComponentStyleOwner(boundary),
                Node::default(),
                InlineStyle::parse("color: #ffffff;").unwrap(),
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(root);
        app.world_mut().entity_mut(root).add_child(button);
        app.update();
        assert_eq!(
            app.world().get::<Node>(button).unwrap().width,
            Val::Px(105.0)
        );
        assert_eq!(
            app.world()
                .get::<crate::RuntimeComputedStyle>(button)
                .unwrap()
                .0
                .font_size,
            Some(tilt_ui_css::Length::Px(22.0))
        );
        assert_eq!(
            app.world()
                .get::<crate::RuntimeComputedStyle>(button)
                .unwrap()
                .0
                .color,
            Some(tilt_ui_css::CssColor::rgba(
                17.0 / 255.0,
                17.0 / 255.0,
                17.0 / 255.0,
                1.0
            ))
        );

        app.world_mut()
            .entity_mut(root)
            .insert(InlineStyle::parse("--space: 40px;").unwrap());
        app.update();
        assert_eq!(
            app.world().get::<Node>(button).unwrap().width,
            Val::Px(40.0)
        );

        app.world_mut()
            .entity_mut(root)
            .insert(InlineStyle::parse("").unwrap());
        app.world_mut()
            .get_mut::<bevy::ui::ComputedNode>(root)
            .unwrap()
            .size
            .x = 300.0;
        app.update();
        assert_eq!(
            app.world().get::<Node>(button).unwrap().width,
            Val::Px(155.0)
        );
    }

    #[test]
    fn mixed_viewport_lengths_recompute_after_resize() {
        let mut app = app();
        let mut window = Window::default();
        window.resolution.set(800.0, 600.0);
        let window = app.world_mut().spawn((window, PrimaryWindow)).id();
        let author = stylesheet(
            app.world_mut(),
            "button { width: max(calc(10vw + 5px), 40px); }",
        );
        let boundary = owner(app.world_mut(), author);
        let button = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Button,
                },
                ComponentStyleOwner(boundary),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(button);
        app.update();
        assert_eq!(
            app.world().get::<Node>(button).unwrap().width,
            Val::Px(85.0)
        );
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .resolution
            .set(1000.0, 600.0);
        app.update();
        assert_eq!(
            app.world().get::<Node>(button).unwrap().width,
            Val::Px(105.0)
        );
    }

    #[test]
    fn theme_variables_recompute_existing_elements() {
        let mut app = app();
        register_ui_theme(app.world_mut(), "light", ":root { --ink: #222222; }").unwrap();
        register_ui_theme(app.world_mut(), "dark", ":root { --ink: #eeeeee; }").unwrap();
        switch_ui_theme(app.world_mut(), "light").unwrap();
        let author = stylesheet(app.world_mut(), "button { color: var(--ink); }");
        let boundary = owner(app.world_mut(), author);
        let root = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Div,
                },
                ComponentStyleOwner(boundary),
                Node::default(),
            ))
            .id();
        let button = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Button,
                },
                ComponentStyleOwner(boundary),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(root);
        app.world_mut().entity_mut(root).add_child(button);
        app.update();
        let light = app
            .world()
            .get::<crate::RuntimeComputedStyle>(button)
            .unwrap()
            .0
            .color;
        switch_ui_theme(app.world_mut(), "dark").unwrap();
        app.update();
        let dark = app
            .world()
            .get::<crate::RuntimeComputedStyle>(button)
            .unwrap()
            .0
            .color;
        assert_eq!(
            light,
            Some(tilt_ui_css::CssColor::rgba(
                34.0 / 255.0,
                34.0 / 255.0,
                34.0 / 255.0,
                1.0
            ))
        );
        assert_eq!(
            dark,
            Some(tilt_ui_css::CssColor::rgba(
                238.0 / 255.0,
                238.0 / 255.0,
                238.0 / 255.0,
                1.0
            ))
        );
    }

    fn themed_app() -> App {
        let mut app = App::new();
        app.insert_resource(Assets::<UiStyleSheetAsset>::default());
        app.add_plugins(TiltUiStyleRuntimePlugin::default());
        app
    }

    #[test]
    fn direct_runtime_text_inherits_named_theme_color() {
        let mut app = app();
        register_ui_theme(app.world_mut(), "light", "#message { color: #252149; }").unwrap();
        register_ui_theme(app.world_mut(), "dark", "#message { color: #e6edf3; }").unwrap();
        switch_ui_theme(app.world_mut(), "light").unwrap();
        let handle = stylesheet(app.world_mut(), "");
        let boundary = owner(app.world_mut(), handle);
        let target = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Div,
                },
                ElementId("message".into()),
                ComponentStyleOwner(boundary),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(target);
        crate::set_inner_text(app.world_mut(), target, "Plain text").unwrap();
        app.update();
        let text = app
            .world()
            .get::<bevy::ecs::hierarchy::Children>(target)
            .unwrap()[0];
        let light = *app
            .world()
            .get::<TextColor>(text)
            .expect("visible light-theme text");
        switch_ui_theme(app.world_mut(), "dark").unwrap();
        app.update();
        let dark = *app
            .world()
            .get::<TextColor>(text)
            .expect("visible dark-theme text");
        assert_ne!(light, dark);

        app.world_mut().init_resource::<crate::UiBindingStore>();
        app.world_mut().init_resource::<crate::UiSharedValues>();
        crate::set_inner_bindings(app.world_mut(), target, "Value: {{ 2 + 3 }}").unwrap();
        crate::component::binding_runtime::apply_bindings(app.world_mut());
        app.update();
        let text = app
            .world()
            .get::<bevy::ecs::hierarchy::Children>(target)
            .unwrap()[0];
        assert_eq!(
            app.world().get::<bevy::ui::widget::Text>(text).unwrap().0,
            "Value: 5"
        );
        assert_eq!(*app.world().get::<TextColor>(text).unwrap(), dark);
    }

    #[test]
    fn named_themes_and_nested_providers_restyle_without_rebuilding() {
        let mut app = app();
        app.init_resource::<UiProviderRegistry>();
        app.world_mut()
            .resource_mut::<UiProviderRegistry>()
            .register(ThemeProvider);
        app.world_mut()
            .resource_mut::<UiProviderRegistry>()
            .register(AccentProvider);
        register_ui_theme(
            app.world_mut(),
            "light",
            "button { background-color: #ffffff; }",
        )
        .unwrap();
        register_ui_theme(
            app.world_mut(),
            "dark",
            "button { background-color: #111111; }",
        )
        .unwrap();
        switch_ui_theme(app.world_mut(), "light").unwrap();
        let handle = stylesheet(app.world_mut(), "");
        let boundary = owner(app.world_mut(), handle);
        let outside = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Button,
                },
                ComponentStyleOwner(boundary),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(outside);
        let scoped = app
            .world_mut()
            .spawn(ProviderScope {
                tag: "theme-provider".into(),
                attributes: vec![("theme".into(), "dark".into())],
            })
            .id();
        app.world_mut().entity_mut(boundary).add_child(scoped);
        let accent = app
            .world_mut()
            .spawn(ProviderScope {
                tag: "accent-provider".into(),
                attributes: vec![("color".into(), "#ff0000".into())],
            })
            .id();
        app.world_mut().entity_mut(scoped).add_child(accent);
        let inside = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Button,
                },
                ComponentStyleOwner(boundary),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(accent).add_child(inside);
        let nested_handle = stylesheet(app.world_mut(), "");
        let nested_boundary = owner(app.world_mut(), nested_handle);
        app.world_mut()
            .entity_mut(scoped)
            .add_child(nested_boundary);
        let nested_button = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Button,
                },
                ComponentStyleOwner(nested_boundary),
                Node::default(),
            ))
            .id();
        app.world_mut()
            .entity_mut(nested_boundary)
            .add_child(nested_button);
        app.update();
        assert_eq!(
            app.world().get::<BackgroundColor>(outside).unwrap().0,
            Color::srgb(1.0, 1.0, 1.0)
        );
        assert_eq!(
            app.world().get::<BackgroundColor>(inside).unwrap().0,
            Color::srgb(17.0 / 255.0, 17.0 / 255.0, 17.0 / 255.0)
        );
        assert_eq!(
            app.world().get::<BackgroundColor>(nested_button).unwrap().0,
            Color::srgb(17.0 / 255.0, 17.0 / 255.0, 17.0 / 255.0)
        );
        switch_ui_theme(app.world_mut(), "dark").unwrap();
        app.update();
        assert_eq!(
            app.world().get::<BackgroundColor>(outside).unwrap().0,
            Color::srgb(17.0 / 255.0, 17.0 / 255.0, 17.0 / 255.0)
        );
        assert_eq!(
            app.world().get::<BorderColor>(inside).unwrap().left,
            Color::srgb(1.0, 0.0, 0.0)
        );
        assert!(switch_ui_theme(app.world_mut(), "missing").is_err());
        assert_eq!(
            app.world().get::<BackgroundColor>(inside).unwrap().0,
            Color::srgb(17.0 / 255.0, 17.0 / 255.0, 17.0 / 255.0)
        );
        switch_ui_theme(app.world_mut(), "light").unwrap();
        app.update();
        assert_eq!(
            app.world().get::<BackgroundColor>(outside).unwrap().0,
            Color::srgb(1.0, 1.0, 1.0)
        );
        assert_eq!(
            app.world().get::<BackgroundColor>(nested_button).unwrap().0,
            Color::srgb(17.0 / 255.0, 17.0 / 255.0, 17.0 / 255.0)
        );
        app.world_mut()
            .resource_mut::<UiThemes>()
            .select("dark")
            .unwrap();
        app.update();
        assert_eq!(
            app.world().get::<BackgroundColor>(outside).unwrap().0,
            Color::srgb(17.0 / 255.0, 17.0 / 255.0, 17.0 / 255.0)
        );
    }

    #[test]
    fn named_theme_applies_while_author_css_is_unavailable() {
        let mut app = app();
        register_ui_theme(
            app.world_mut(),
            "plain",
            "button { background-color: #123456; }",
        )
        .unwrap();
        switch_ui_theme(app.world_mut(), "plain").unwrap();
        let boundary = owner(app.world_mut(), Handle::<UiStyleSheetAsset>::default());
        let button = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Button,
                },
                ComponentStyleOwner(boundary),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(button);
        app.update();
        assert_eq!(
            app.world().get::<BackgroundColor>(button).unwrap().0,
            Color::srgb(18.0 / 255.0, 52.0 / 255.0, 86.0 / 255.0)
        );
    }

    #[test]
    fn named_theme_media_rules_restyle_on_window_resize() {
        let mut app = app();
        register_ui_theme(
            app.world_mut(),
            "responsive",
            "button { width: 100px; } @media (max-width: 800px) { button { width: 200px; } }",
        )
        .unwrap();
        switch_ui_theme(app.world_mut(), "responsive").unwrap();
        let mut window = Window::default();
        window.resolution.set(900.0, 600.0);
        let window = app.world_mut().spawn((window, PrimaryWindow)).id();
        let handle = stylesheet(app.world_mut(), "");
        let boundary = owner(app.world_mut(), handle);
        let button = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Button,
                },
                ComponentStyleOwner(boundary),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(button);
        app.update();
        assert_eq!(
            app.world().get::<Node>(button).unwrap().width,
            Val::Px(100.0)
        );
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .resolution
            .set(700.0, 600.0);
        app.update();
        assert_eq!(
            app.world().get::<Node>(button).unwrap().width,
            Val::Px(200.0)
        );
    }

    #[test]
    fn cascade_uses_specificity_source_order_and_the_matching_selector() {
        let mut app = app();
        let handle = stylesheet(
            app.world_mut(),
            "button { width: 100px; } .primary { width: 120px; } #play { width: 140px; } button, #play { height: 48px; } .primary { width: 130px; }",
        );
        let boundary = owner(app.world_mut(), handle);
        let button = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Button,
                },
                ComponentStyleOwner(boundary),
                ElementClasses {
                    classes: vec!["primary".into()],
                },
                ElementId("play".into()),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(button);
        app.update();
        let node = app.world().get::<Node>(button).unwrap();
        assert_eq!(node.width, Val::Px(140.0));
        assert_eq!(node.height, Val::Px(48.0));
    }

    #[test]
    fn gradient_background_creates_cached_image_node() {
        let mut app = app();
        let handle = stylesheet(
            app.world_mut(),
            "button { background-image: linear-gradient(to right, #D34CED, #8424F5); }",
        );
        let boundary = owner(app.world_mut(), handle);
        let first = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Button,
                },
                ComponentStyleOwner(boundary),
                Node::default(),
            ))
            .id();
        let second = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Button,
                },
                ComponentStyleOwner(boundary),
                Node::default(),
            ))
            .id();
        app.world_mut()
            .entity_mut(boundary)
            .add_children(&[first, second]);
        app.update();
        let image = app
            .world()
            .get::<bevy::ui::widget::ImageNode>(first)
            .unwrap();
        assert_eq!(
            image.image,
            app.world()
                .get::<bevy::ui::widget::ImageNode>(second)
                .unwrap()
                .image
        );
        assert_eq!(
            app.world().resource::<Assets<bevy::image::Image>>().len(),
            1
        );
    }

    #[test]
    fn descendant_matching_stops_at_nested_component_boundaries() {
        let mut app = app();
        let parent_handle = stylesheet(
            app.world_mut(),
            ".menu button { width: 500px; } .title { background-color: #ff0000; }",
        );
        let child_handle = stylesheet(app.world_mut(), ".title { background-color: #0000ff; }");
        let parent = owner(app.world_mut(), parent_handle);
        let menu = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Div,
                },
                ComponentStyleOwner(parent),
                ElementClasses {
                    classes: vec!["menu".into()],
                },
                Node::default(),
            ))
            .id();
        let child = owner(app.world_mut(), child_handle);
        let button = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Button,
                },
                ComponentStyleOwner(child),
                ElementClasses {
                    classes: vec!["title".into()],
                },
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(parent).add_child(menu);
        app.world_mut().entity_mut(menu).add_child(child);
        app.world_mut().entity_mut(child).add_child(button);
        app.update();
        assert_eq!(app.world().get::<Node>(button).unwrap().width, Val::Auto);
        assert_eq!(
            app.world().get::<BackgroundColor>(button).unwrap().0,
            Color::srgb(0.0, 0.0, 1.0)
        );
    }

    #[test]
    fn inherited_text_values_flow_through_a_component_boundary() {
        let mut app = app();
        let parent_handle = stylesheet(
            app.world_mut(),
            ".parent { color: #ffffff; font-size: 24px; }",
        );
        let child_handle = stylesheet(app.world_mut(), "");
        let parent = owner(app.world_mut(), parent_handle);
        let element = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Div,
                },
                ComponentStyleOwner(parent),
                ElementClasses {
                    classes: vec!["parent".into()],
                },
                Node::default(),
            ))
            .id();
        let child = owner(app.world_mut(), child_handle);
        let text = app
            .world_mut()
            .spawn((
                TiltText {
                    value: "Hello".into(),
                },
                ComponentStyleOwner(child),
            ))
            .id();
        app.world_mut().entity_mut(parent).add_child(element);
        app.world_mut().entity_mut(element).add_child(child);
        app.world_mut().entity_mut(child).add_child(text);
        app.update();
        assert_eq!(
            app.world().get::<TextColor>(text).unwrap().0,
            Color::srgba(1.0, 1.0, 1.0, 1.0)
        );
        assert_eq!(
            app.world().get::<TextFont>(text).unwrap().font_size,
            bevy::text::FontSize::Px(24.0)
        );
    }

    #[test]
    fn pseudo_state_recomputes_and_reverts_complete_node_style() {
        let mut app = app();
        let handle = stylesheet(
            app.world_mut(),
            "button { width: 100px; } button:hover { width: 120px; background-color: #4285f4; }",
        );
        let boundary = owner(app.world_mut(), handle);
        let button = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Button,
                },
                ComponentStyleOwner(boundary),
                ElementState::default(),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(button);
        app.update();
        assert_eq!(
            app.world().get::<Node>(button).unwrap().width,
            Val::Px(100.0)
        );
        assert_eq!(
            app.world().get::<BackgroundColor>(button).unwrap().0,
            Color::NONE
        );
        app.world_mut()
            .get_mut::<ElementState>(button)
            .unwrap()
            .hovered = true;
        app.update();
        assert_eq!(
            app.world().get::<Node>(button).unwrap().width,
            Val::Px(120.0)
        );
        assert!(app.world().get::<BackgroundColor>(button).is_some());
        app.world_mut()
            .get_mut::<ElementState>(button)
            .unwrap()
            .hovered = false;
        app.update();
        assert_eq!(
            app.world().get::<Node>(button).unwrap().width,
            Val::Px(100.0)
        );
        assert_eq!(
            app.world().get::<BackgroundColor>(button).unwrap().0,
            Color::NONE
        );
    }

    #[test]
    fn hover_variable_restyles_its_descendants() {
        let mut app = app();
        let author = stylesheet(
            app.world_mut(),
            ":root { --space: 10px; } .card:hover { --space: 24px; } button { width: var(--space); }",
        );
        let boundary = owner(app.world_mut(), author);
        let root = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Div,
                },
                ComponentStyleOwner(boundary),
                Node::default(),
            ))
            .id();
        let card = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Div,
                },
                ComponentStyleOwner(boundary),
                ElementClasses {
                    classes: vec!["card".into()],
                },
                ElementState::default(),
                Node::default(),
            ))
            .id();
        let button = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Button,
                },
                ComponentStyleOwner(boundary),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(root);
        app.world_mut().entity_mut(root).add_child(card);
        app.world_mut().entity_mut(card).add_child(button);

        app.update();
        assert_eq!(
            app.world().get::<Node>(button).unwrap().width,
            Val::Px(10.0)
        );
        app.world_mut()
            .get_mut::<ElementState>(card)
            .unwrap()
            .hovered = true;
        app.update();
        assert_eq!(
            app.world().get::<Node>(button).unwrap().width,
            Val::Px(24.0)
        );
        app.world_mut()
            .get_mut::<ElementState>(card)
            .unwrap()
            .hovered = false;
        app.update();
        assert_eq!(
            app.world().get::<Node>(button).unwrap().width,
            Val::Px(10.0)
        );
    }

    #[test]
    fn pseudo_state_restyles_descendants_and_following_siblings_only() {
        let mut app = app();
        let handle = stylesheet(
            app.world_mut(),
            "button { width: 100px; } .trigger:hover button { width: 120px; } .trigger:hover ~ .following button { width: 140px; }",
        );
        let boundary = owner(app.world_mut(), handle);
        let before = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Div,
                },
                ComponentStyleOwner(boundary),
                Node::default(),
            ))
            .id();
        let trigger = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Div,
                },
                ComponentStyleOwner(boundary),
                ElementClasses {
                    classes: vec!["trigger".into()],
                },
                ElementState::default(),
                Node::default(),
            ))
            .id();
        let following = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Div,
                },
                ComponentStyleOwner(boundary),
                ElementClasses {
                    classes: vec!["following".into()],
                },
                Node::default(),
            ))
            .id();
        let mut buttons = Vec::new();
        for parent in [before, trigger, following] {
            let button = app
                .world_mut()
                .spawn((
                    TiltElement {
                        kind: ElementKind::Button,
                    },
                    ComponentStyleOwner(boundary),
                    Node::default(),
                ))
                .id();
            app.world_mut().entity_mut(parent).add_child(button);
            app.world_mut().entity_mut(boundary).add_child(parent);
            buttons.push(button);
        }
        app.update();
        for button in &buttons {
            assert_eq!(
                app.world().get::<Node>(*button).unwrap().width,
                Val::Px(100.0)
            );
        }

        app.world_mut()
            .get_mut::<ElementState>(trigger)
            .unwrap()
            .hovered = true;
        app.update();

        assert_eq!(
            app.world().get::<Node>(buttons[0]).unwrap().width,
            Val::Px(100.0)
        );
        assert_eq!(
            app.world().get::<Node>(buttons[1]).unwrap().width,
            Val::Px(120.0)
        );
        assert_eq!(
            app.world().get::<Node>(buttons[2]).unwrap().width,
            Val::Px(140.0)
        );
        assert!(
            app.world()
                .get::<super::super::apply::PreviousComputedStyle>(buttons[0])
                .is_none()
        );
    }

    #[test]
    fn editable_readonly_and_invalid_states_follow_the_normal_css_pipeline() {
        let mut app = app();
        let handle = stylesheet(
            app.world_mut(),
            "input { width: 100px; } input:readonly { width: 110px; } input:invalid { width: 120px; }",
        );
        let boundary = owner(app.world_mut(), handle);
        let input = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Input,
                },
                ComponentStyleOwner(boundary),
                ElementState::default(),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(input);
        app.update();
        assert_eq!(
            app.world().get::<Node>(input).unwrap().width,
            Val::Px(100.0)
        );
        app.world_mut()
            .get_mut::<ElementState>(input)
            .unwrap()
            .readonly = true;
        app.update();
        assert_eq!(
            app.world().get::<Node>(input).unwrap().width,
            Val::Px(110.0)
        );
        app.world_mut()
            .get_mut::<ElementState>(input)
            .unwrap()
            .invalid = true;
        app.update();
        assert_eq!(
            app.world().get::<Node>(input).unwrap().width,
            Val::Px(120.0)
        );
        app.world_mut()
            .get_mut::<ElementState>(input)
            .unwrap()
            .invalid = false;
        app.update();
        assert_eq!(
            app.world().get::<Node>(input).unwrap().width,
            Val::Px(110.0)
        );
    }

    #[test]
    fn checked_state_recomputes_through_the_shared_css_pipeline() {
        let mut app = app();
        let handle = stylesheet(
            app.world_mut(),
            "checkbox { width: 20px; } checkbox:checked { width: 24px; }",
        );
        let boundary = owner(app.world_mut(), handle);
        let checkbox = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Checkbox,
                },
                ComponentStyleOwner(boundary),
                ControlChecked(false),
                ElementState::default(),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(checkbox);
        app.update();
        assert_eq!(
            app.world().get::<Node>(checkbox).unwrap().width,
            Val::Px(20.0)
        );

        assert!(set_control_checked(app.world_mut(), checkbox, true));
        app.update();
        assert_eq!(
            app.world().get::<Node>(checkbox).unwrap().width,
            Val::Px(24.0)
        );

        assert!(set_control_checked(app.world_mut(), checkbox, false));
        app.update();
        assert_eq!(
            app.world().get::<Node>(checkbox).unwrap().width,
            Val::Px(20.0)
        );
    }

    #[test]
    fn checkbox_mark_is_hidden_until_checked_in_the_default_theme() {
        let mut app = themed_app();
        let handle = stylesheet(app.world_mut(), "");
        let boundary = owner(app.world_mut(), handle);
        let checkbox = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Checkbox,
                },
                ComponentStyleOwner(boundary),
            ))
            .id();
        crate::render::materialize_element(app.world_mut(), checkbox, ElementKind::Checkbox, &[]);
        app.world_mut().entity_mut(boundary).add_child(checkbox);
        let mark = app
            .world_mut()
            .query::<(bevy::ecs::entity::Entity, &crate::ControlPart)>()
            .iter(app.world())
            .find(|(_, part)| part.owner == checkbox && part.kind == crate::ControlPartKind::Mark)
            .map(|(entity, _)| entity)
            .unwrap();

        app.update();
        assert_eq!(
            app.world().get::<Node>(mark).unwrap().display,
            bevy::ui::Display::None
        );
        assert!(set_control_checked(app.world_mut(), checkbox, true));
        app.update();
        assert_eq!(
            app.world().get::<Node>(mark).unwrap().display,
            bevy::ui::Display::Flex
        );
        assert!(set_control_checked(app.world_mut(), checkbox, false));
        app.update();
        assert_eq!(
            app.world().get::<Node>(mark).unwrap().display,
            bevy::ui::Display::None
        );
    }

    #[test]
    fn author_styles_override_the_default_theme_origin() {
        let mut app = themed_app();
        let handle = stylesheet(app.world_mut(), "button { background-color: #ff0000; }");
        let boundary = owner(app.world_mut(), handle);
        let button = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Button,
                },
                ComponentStyleOwner(boundary),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(button);
        app.update();

        assert_eq!(
            app.world().get::<BackgroundColor>(button).unwrap().0,
            Color::srgb(1.0, 0.0, 0.0)
        );
    }

    #[test]
    fn built_in_theme_styles_controls_without_author_control_rules() {
        let mut app = themed_app();
        let handle = stylesheet(app.world_mut(), ".layout { gap: 12px; }");
        let boundary = owner(app.world_mut(), handle);
        let button = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Button,
                },
                ComponentStyleOwner(boundary),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(button);
        app.update();

        assert_eq!(
            app.world().get::<Node>(button).unwrap().height,
            Val::Px(40.0)
        );
        assert_eq!(
            app.world().get::<BackgroundColor>(button).unwrap().0,
            Color::srgb(168.0 / 255.0, 51.0 / 255.0, 234.0 / 255.0)
        );
    }

    #[test]
    fn default_theme_centers_slider_thumb_and_input_text_inset() {
        let mut app = themed_app();
        let handle = stylesheet(app.world_mut(), "");
        let boundary = owner(app.world_mut(), handle);
        let input = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Input,
                },
                ComponentStyleOwner(boundary),
                Node::default(),
            ))
            .id();
        let slider = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Slider,
                },
                ComponentStyleOwner(boundary),
                Node::default(),
            ))
            .id();
        let track = app
            .world_mut()
            .spawn((
                crate::ControlPart {
                    owner: slider,
                    kind: crate::ControlPartKind::Track,
                },
                ComponentStyleOwner(boundary),
                Node::default(),
            ))
            .id();
        let thumb = app
            .world_mut()
            .spawn((
                crate::ControlPart {
                    owner: slider,
                    kind: crate::ControlPartKind::Thumb,
                },
                ComponentStyleOwner(boundary),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(input);
        app.world_mut().entity_mut(boundary).add_child(slider);
        app.world_mut().entity_mut(slider).add_child(track);
        app.world_mut().entity_mut(track).add_child(thumb);
        app.update();

        let input_node = app.world().get::<Node>(input).unwrap();
        assert_eq!(input_node.padding.top, Val::Px(8.0));
        assert_eq!(input_node.padding.bottom, Val::Px(8.0));
        let track_node = app.world().get::<Node>(track).unwrap();
        let thumb_node = app.world().get::<Node>(thumb).unwrap();
        assert_eq!(track_node.height, Val::Px(6.0));
        assert_eq!(thumb_node.height, Val::Px(18.0));
        assert_eq!(thumb_node.top, Val::Px(-6.0));
    }

    #[test]
    fn password_mask_and_native_caret_use_the_same_monospace_face() {
        let mut app = themed_app();
        let handle = stylesheet(app.world_mut(), "");
        let boundary = owner(app.world_mut(), handle);
        let input = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Input,
                },
                ComponentStyleOwner(boundary),
                crate::StaticAttributes {
                    attributes: vec![crate::StaticAttribute {
                        name: "type".into(),
                        value: "password".into(),
                    }],
                },
                crate::EditableText::new(
                    "secret".into(),
                    tilt_ui_core::InputType::Password,
                    false,
                    false,
                ),
                bevy::text::EditableText::new("secret"),
                Node::default(),
            ))
            .id();
        let mask = app
            .world_mut()
            .spawn((
                crate::ControlPart {
                    owner: input,
                    kind: crate::ControlPartKind::Value,
                },
                ComponentStyleOwner(boundary),
                bevy::ui::widget::Text::new("******"),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(input);
        app.world_mut().entity_mut(input).add_child(mask);
        app.update();
        assert_eq!(
            app.world().get::<TextFont>(input).unwrap().font,
            bevy::text::FontSource::default()
        );
        assert_eq!(
            app.world().get::<TextFont>(mask).unwrap().font,
            bevy::text::FontSource::default()
        );
    }

    #[test]
    fn overflow_css_creates_one_persistent_styled_scrollbar() {
        let mut app = themed_app();
        let handle = stylesheet(app.world_mut(), "div { height: 80px; overflow-y: auto; }");
        let boundary = owner(app.world_mut(), handle);
        let element = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Div,
                },
                ComponentStyleOwner(boundary),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(element);
        app.update();
        assert_eq!(
            app.world().get::<Node>(element).unwrap().overflow.y,
            bevy::ui::OverflowAxis::Scroll
        );
        let tracks = |world: &mut World| {
            let mut query = world.query::<(bevy::ecs::entity::Entity, &crate::ControlPart)>();
            query
                .iter(world)
                .filter(|(_, part)| {
                    part.owner == element && part.kind == crate::ControlPartKind::ScrollbarYTrack
                })
                .map(|(entity, _)| entity)
                .collect::<Vec<_>>()
        };
        let first = tracks(app.world_mut());
        assert_eq!(first.len(), 1);
        assert!(app.world().get::<BackgroundColor>(first[0]).is_some());
        app.world_mut().entity_mut(boundary).insert(StyleDirty);
        app.update();
        assert_eq!(tracks(app.world_mut()), first);
    }

    #[test]
    fn hovered_overflow_creates_scrollbar_after_partial_restyle() {
        let mut app = themed_app();
        let handle = stylesheet(
            app.world_mut(),
            "div { height: 80px; overflow-y: hidden; } div:hover { overflow-y: auto; }",
        );
        let boundary = owner(app.world_mut(), handle);
        let element = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Div,
                },
                ComponentStyleOwner(boundary),
                ElementState::default(),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(element);
        app.update();
        assert_eq!(
            app.world().get::<Node>(element).unwrap().overflow.y,
            bevy::ui::OverflowAxis::Hidden
        );

        app.world_mut()
            .get_mut::<ElementState>(element)
            .unwrap()
            .hovered = true;
        app.update();
        assert_eq!(
            app.world().get::<Node>(element).unwrap().overflow.y,
            bevy::ui::OverflowAxis::Scroll
        );
        let mut query = app.world_mut().query::<&crate::ControlPart>();
        assert!(query.iter(app.world()).any(|part| {
            part.owner == element && part.kind == crate::ControlPartKind::ScrollbarYTrack
        }));
    }

    #[test]
    fn author_font_family_overrides_the_bundled_theme_face() {
        let mut app = themed_app();
        app.insert_resource(Assets::<bevy::text::Font>::default());
        let handle = stylesheet(app.world_mut(), ".mono { font-family: monospace; }");
        let boundary = owner(app.world_mut(), handle);
        let themed = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Paragraph,
                },
                ComponentStyleOwner(boundary),
                Node::default(),
            ))
            .id();
        let themed_text = app
            .world_mut()
            .spawn(TiltText {
                value: "Theme".into(),
            })
            .id();
        app.world_mut().entity_mut(themed).add_child(themed_text);
        let mono = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Paragraph,
                },
                ElementClasses {
                    classes: vec!["mono".into()],
                },
                ComponentStyleOwner(boundary),
                Node::default(),
            ))
            .id();
        let mono_text = app
            .world_mut()
            .spawn(TiltText {
                value: "Author".into(),
            })
            .id();
        app.world_mut().entity_mut(mono).add_child(mono_text);
        app.world_mut().entity_mut(boundary).add_child(themed);
        app.world_mut().entity_mut(boundary).add_child(mono);
        app.update();

        let fonts = app.world().resource::<crate::theme::DefaultThemeFonts>();
        assert_eq!(
            app.world().get::<TextFont>(themed_text).unwrap().font,
            bevy::text::FontSource::Handle(fonts.regular.clone())
        );
        assert_eq!(
            app.world().get::<TextFont>(mono_text).unwrap().font,
            bevy::text::FontSource::default()
        );
    }

    #[test]
    fn default_theme_can_be_disabled_without_disabling_author_css() {
        let mut app = app();
        let handle = stylesheet(app.world_mut(), "button { width: 150px; }");
        let boundary = owner(app.world_mut(), handle);
        let button = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Button,
                },
                ComponentStyleOwner(boundary),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(button);
        app.update();

        assert_eq!(
            app.world().get::<Node>(button).unwrap().width,
            Val::Px(150.0)
        );
        assert_eq!(
            app.world().get::<BackgroundColor>(button).unwrap().0,
            Color::NONE
        );
    }

    #[test]
    fn author_pseudo_element_rules_override_default_part_rules() {
        let mut app = themed_app();
        let handle = stylesheet(
            app.world_mut(),
            "switch-button::track { background-color: #ff0000; }",
        );
        let boundary = owner(app.world_mut(), handle);
        let switch = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::SwitchButton,
                },
                ComponentStyleOwner(boundary),
                ElementState::default(),
                Node::default(),
            ))
            .id();
        let track = app
            .world_mut()
            .spawn((
                crate::ControlPart {
                    owner: switch,
                    kind: crate::ControlPartKind::Track,
                },
                ComponentStyleOwner(boundary),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(switch);
        app.world_mut().entity_mut(switch).add_child(track);
        app.update();

        assert_eq!(
            app.world().get::<BackgroundColor>(track).unwrap().0,
            Color::srgb(1.0, 0.0, 0.0)
        );
    }

    #[test]
    fn cached_selector_view_updates_control_part_hover_state() {
        let mut app = app();
        let handle = stylesheet(
            app.world_mut(),
            "checkbox::indicator { background-color: #000000; } checkbox:hover::indicator { background-color: #ffffff; }",
        );
        let boundary = owner(app.world_mut(), handle);
        let checkbox = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Checkbox,
                },
                ComponentStyleOwner(boundary),
                ElementState::default(),
                Node::default(),
            ))
            .id();
        let indicator = app
            .world_mut()
            .spawn((
                crate::ControlPart {
                    owner: checkbox,
                    kind: crate::ControlPartKind::Indicator,
                },
                ComponentStyleOwner(boundary),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(checkbox).add_child(indicator);
        app.world_mut().entity_mut(boundary).add_child(checkbox);
        app.update();
        assert_eq!(
            app.world().get::<BackgroundColor>(indicator).unwrap().0,
            Color::srgb(0.0, 0.0, 0.0)
        );

        app.world_mut()
            .get_mut::<ElementState>(checkbox)
            .unwrap()
            .hovered = true;
        app.update();

        assert_eq!(
            app.world().get::<BackgroundColor>(indicator).unwrap().0,
            Color::srgb(1.0, 1.0, 1.0)
        );
    }

    #[test]
    fn matching_media_rules_keep_normal_cascade_source_order() {
        let mut app = app();
        app.world_mut().resource_mut::<TiltUiMediaEnvironment>().0 =
            Some(tilt_ui_css::MediaEnvironment {
                width: 700.0,
                height: 800.0,
            });
        let handle = stylesheet(
            app.world_mut(),
            ".card { width: 100px; } @media (max-width: 800px) { .card { width: 200px; } } .card { width: 300px; }",
        );
        let boundary = owner(app.world_mut(), handle);
        let card = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Div,
                },
                ComponentStyleOwner(boundary),
                ElementClasses {
                    classes: vec!["card".into()],
                },
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(card);
        app.update();
        assert_eq!(app.world().get::<Node>(card).unwrap().width, Val::Px(300.0));
    }

    #[test]
    fn keyframe_and_hover_transition_use_the_motion_overlay() {
        let mut app = app();
        app.world_mut().insert_resource(Time::<()>::default());
        let handle = stylesheet(
            app.world_mut(),
            "@keyframes pulse { from { background-color: #000000; } to { background-color: #ffffff; } } .card { width: 100px; animation: pulse 1s linear infinite; transition: width 1s linear; } .card:hover { width: 200px; }",
        );
        let boundary = owner(app.world_mut(), handle);
        let card = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Div,
                },
                ComponentStyleOwner(boundary),
                ElementClasses {
                    classes: vec!["card".into()],
                },
                ElementState::default(),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(card);
        app.update();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(500));
        app.update();
        assert_eq!(
            app.world().get::<BackgroundColor>(card).unwrap().0,
            Color::srgb(0.5, 0.5, 0.5)
        );

        app.world_mut()
            .get_mut::<ElementState>(card)
            .unwrap()
            .hovered = true;
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(500));
        app.update();
        assert_eq!(app.world().get::<Node>(card).unwrap().width, Val::Px(150.0));

        app.world_mut()
            .get_mut::<ElementState>(card)
            .unwrap()
            .hovered = false;
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(250));
        app.update();
        assert_eq!(app.world().get::<Node>(card).unwrap().width, Val::Px(137.5));
    }

    #[test]
    fn unchanged_or_transition_none_style_creates_no_transition_state() {
        let mut app = app();
        app.world_mut().insert_resource(Time::<()>::default());
        let handle = stylesheet(
            app.world_mut(),
            ".card { width: 100px; transition: width 1s linear; } .card:hover { width: 200px; transition: none; }",
        );
        let boundary = owner(app.world_mut(), handle);
        let card = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Div,
                },
                ComponentStyleOwner(boundary),
                ElementClasses {
                    classes: vec!["card".into()],
                },
                ElementState::default(),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(card);
        app.update();
        assert!(
            app.world()
                .get::<super::super::motion::ActiveTransitions>(card)
                .is_none()
        );
        app.world_mut()
            .get_mut::<ElementState>(card)
            .unwrap()
            .hovered = true;
        app.update();
        assert_eq!(app.world().get::<Node>(card).unwrap().width, Val::Px(200.0));
        assert!(
            app.world()
                .get::<super::super::motion::ActiveTransitions>(card)
                .is_none()
        );
    }

    #[test]
    fn checked_state_starts_the_normal_css_transition_path() {
        let mut app = app();
        app.world_mut().insert_resource(Time::<()>::default());
        let handle = stylesheet(
            app.world_mut(),
            "toggle-button { width: 100px; transition: width 1s linear; } toggle-button:checked { width: 200px; }",
        );
        let boundary = owner(app.world_mut(), handle);
        let toggle = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::ToggleButton,
                },
                ComponentStyleOwner(boundary),
                ControlChecked(false),
                ElementState::default(),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(toggle);
        app.update();
        assert!(set_control_checked(app.world_mut(), toggle, true));
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(500));
        app.update();
        assert_eq!(
            app.world().get::<Node>(toggle).unwrap().width,
            Val::Px(150.0)
        );
    }

    #[test]
    fn transition_none_removes_an_active_transition_immediately() {
        let mut app = app();
        app.world_mut().insert_resource(Time::<()>::default());
        let handle = stylesheet(
            app.world_mut(),
            ".card { width: 100px; transition: width 1s linear; } .card:hover { width: 200px; } .card:hover:active { width: 300px; transition: none; }",
        );
        let boundary = owner(app.world_mut(), handle);
        let card = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Div,
                },
                ComponentStyleOwner(boundary),
                ElementClasses {
                    classes: vec!["card".into()],
                },
                ElementState::default(),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(card);
        app.update();
        app.world_mut()
            .get_mut::<ElementState>(card)
            .unwrap()
            .hovered = true;
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(400));
        app.update();
        app.world_mut()
            .get_mut::<ElementState>(card)
            .unwrap()
            .active = true;
        app.update();
        assert_eq!(app.world().get::<Node>(card).unwrap().width, Val::Px(300.0));
        assert!(
            app.world()
                .get::<super::super::motion::ActiveTransitions>(card)
                .is_none()
        );
    }

    #[test]
    fn unrelated_scope_recalculation_does_not_restart_an_animation() {
        let mut app = app();
        app.world_mut().insert_resource(Time::<()>::default());
        let handle = stylesheet(
            app.world_mut(),
            "@keyframes fade { from { background-color: #000000; } to { background-color: #ffffff; } } .pulse { animation: fade 1s linear infinite; }",
        );
        let boundary = owner(app.world_mut(), handle);
        let pulse = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Div,
                },
                ComponentStyleOwner(boundary),
                ElementClasses {
                    classes: vec!["pulse".into()],
                },
                Node::default(),
            ))
            .id();
        let unrelated = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Div,
                },
                ComponentStyleOwner(boundary),
                ElementState::default(),
                Node::default(),
            ))
            .id();
        app.world_mut().entity_mut(boundary).add_child(pulse);
        app.world_mut().entity_mut(boundary).add_child(unrelated);
        app.update();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(400));
        app.update();
        app.world_mut()
            .get_mut::<ElementState>(unrelated)
            .unwrap()
            .hovered = true;
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(100));
        app.update();
        assert_eq!(
            app.world().get::<BackgroundColor>(pulse).unwrap().0,
            Color::srgb(0.5, 0.5, 0.5)
        );
    }
}
