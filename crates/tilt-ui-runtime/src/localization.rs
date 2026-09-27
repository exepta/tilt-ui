//! Fluent catalogs loaded through Bevy's asset server and used by `i18n.*` bindings.

use std::collections::BTreeMap;

use bevy::{
    app::{App, Plugin, PostUpdate, Update},
    asset::{Asset, AssetApp, AssetLoader, AssetServer, Assets, Handle, LoadContext, io::Reader},
    ecs::{
        resource::Resource,
        system::{Res, ResMut},
        world::World,
    },
    log::warn,
    prelude::{DetectChanges, IntoScheduleConfigs},
    reflect::TypePath,
    ui::UiSystems,
};
use fluent_bundle::{FluentArgs, FluentResource, concurrent::FluentBundle};
use thiserror::Error;
use unic_langid::LanguageIdentifier;

/// Error while configuring or parsing a Fluent catalog.
#[derive(Debug, Error)]
pub enum UiFluentError {
    #[error("invalid language tag: {0}")]
    InvalidLocale(String),
    #[error("invalid UTF-8 in Fluent catalog: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error("cannot read Fluent catalog: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid Fluent catalog: {0}")]
    Parse(String),
}

fn parse_locale(tag: &str) -> Result<LanguageIdentifier, UiFluentError> {
    tag.replace('_', "-")
        .parse()
        .map_err(|_| UiFluentError::InvalidLocale(tag.to_owned()))
}

/// Paths to one `.ftl` file per locale, relative to the `tilt-ui://` asset source.
#[derive(Debug, Clone)]
pub struct UiFluentConfig {
    fallback: LanguageIdentifier,
    catalogs: BTreeMap<LanguageIdentifier, String>,
}

impl UiFluentConfig {
    /// Creates configuration with a required fallback language.
    pub fn new(fallback: &str) -> Result<Self, UiFluentError> {
        Ok(Self {
            fallback: parse_locale(fallback)?,
            catalogs: BTreeMap::new(),
        })
    }

    /// Adds a catalog path such as `locales/de-DE.ftl`.
    pub fn with_catalog(
        mut self,
        locale: &str,
        path: impl Into<String>,
    ) -> Result<Self, UiFluentError> {
        self.catalogs.insert(parse_locale(locale)?, path.into());
        Ok(self)
    }
}

/// Raw catalog asset; it is validated before entering the asset store.
#[derive(Asset, TypePath, Debug)]
pub struct UiFluentAsset {
    source: String,
}

/// Loads `.ftl` assets from the configured TiltUI source root.
#[derive(Default, TypePath)]
pub struct UiFluentAssetLoader;

impl AssetLoader for UiFluentAssetLoader {
    type Asset = UiFluentAsset;
    type Settings = ();
    type Error = UiFluentError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let source = String::from_utf8(bytes)?;
        parse_resource(&source)?;
        Ok(UiFluentAsset { source })
    }

    fn extensions(&self) -> &[&str] {
        &["ftl"]
    }
}

fn parse_resource(source: &str) -> Result<FluentResource, UiFluentError> {
    FluentResource::try_new(source.to_owned())
        .map_err(|(_, errors)| UiFluentError::Parse(format!("{errors:?}")))
}

struct Catalog {
    source: String,
    bundle: FluentBundle<FluentResource>,
}

/// Active language and cached Fluent bundles. Call `set_locale` to switch at runtime.
#[derive(Resource)]
pub struct UiLocalization {
    selected: LanguageIdentifier,
    fallback: LanguageIdentifier,
    catalogs: BTreeMap<LanguageIdentifier, Catalog>,
    revision: u64,
}

impl UiLocalization {
    /// Creates an empty localization store using the given fallback language.
    pub fn new(fallback: &str) -> Result<Self, UiFluentError> {
        let fallback = parse_locale(fallback)?;
        Ok(Self {
            selected: fallback.clone(),
            fallback,
            catalogs: BTreeMap::new(),
            revision: 0,
        })
    }

    /// Returns the selected canonical BCP-47 language tag.
    pub fn locale(&self) -> &LanguageIdentifier {
        &self.selected
    }

    /// Selects a language. Exact, base-language, then fallback catalogs are tried.
    pub fn set_locale(&mut self, locale: &str) -> Result<(), UiFluentError> {
        let locale = parse_locale(locale)?;
        if self.selected != locale {
            self.selected = locale;
            self.revision = self.revision.wrapping_add(1);
        }
        Ok(())
    }

    /// Replaces an in-memory catalog, useful for tests or generated translations.
    pub fn insert_ftl(&mut self, locale: &str, source: &str) -> Result<(), UiFluentError> {
        let locale = parse_locale(locale)?;
        if self
            .catalogs
            .get(&locale)
            .is_some_and(|old| old.source == source)
        {
            return Ok(());
        }
        let resource = parse_resource(source)?;
        let mut bundle = FluentBundle::new_concurrent(vec![locale.clone()]);
        bundle.set_use_isolating(false);
        bundle
            .add_resource(resource)
            .map_err(|errors| UiFluentError::Parse(format!("{errors:?}")))?;
        self.catalogs.insert(
            locale,
            Catalog {
                source: source.to_owned(),
                bundle,
            },
        );
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }

    /// Removes a catalog previously added from a directory or in memory.
    pub fn remove_ftl(&mut self, locale: &str) -> Result<bool, UiFluentError> {
        let removed = self.catalogs.remove(&parse_locale(locale)?).is_some();
        if removed {
            self.revision = self.revision.wrapping_add(1);
        }
        Ok(removed)
    }

    /// Formats a Fluent message, falling back when the selected catalog lacks it.
    pub fn translate(&self, key: &str, args: Option<&FluentArgs<'_>>) -> Option<String> {
        let (message_id, attribute) = key
            .split_once('.')
            .map_or((key, None), |(message, attribute)| {
                (message, Some(attribute))
            });
        let mut candidates = Vec::with_capacity(4);
        for locale in [&self.selected, &self.fallback] {
            if !candidates.contains(locale) {
                candidates.push(locale.clone());
            }
            if let Ok(base) = locale.language.to_string().parse::<LanguageIdentifier>()
                && !candidates.contains(&base)
            {
                candidates.push(base);
            }
        }
        // Check regional and base catalogs for the selection before the fallback.
        for locale in candidates {
            let Some(catalog) = self.catalogs.get(&locale) else {
                continue;
            };
            let Some(message) = catalog.bundle.get_message(message_id) else {
                continue;
            };
            let pattern = match attribute {
                Some(attribute) => message.get_attribute(attribute).map(|entry| entry.value()),
                None => message.value(),
            };
            let Some(pattern) = pattern else {
                continue;
            };
            let mut errors = Vec::new();
            let value = catalog
                .bundle
                .format_pattern(pattern, args, &mut errors)
                .into_owned();
            if errors.is_empty() {
                return Some(value);
            }
            warn!("Fluent message {key:?} for {locale} has formatting errors: {errors:?}");
        }
        None
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }
}

/// Owned argument value for a Fluent message.
#[derive(Debug, Clone, PartialEq)]
pub enum UiFluentValue {
    Text(String),
    Number(f64),
}

impl From<&str> for UiFluentValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}
impl From<String> for UiFluentValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}
impl From<i32> for UiFluentValue {
    fn from(value: i32) -> Self {
        Self::Number(value as f64)
    }
}
impl From<u32> for UiFluentValue {
    fn from(value: u32) -> Self {
        Self::Number(value as f64)
    }
}
impl From<f64> for UiFluentValue {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}

/// Dynamic arguments for `{{ i18n.message-id }}` bindings.
#[derive(Resource, Default)]
pub struct UiFluentArgs {
    values: BTreeMap<String, BTreeMap<String, UiFluentValue>>,
    revision: u64,
}

impl UiFluentArgs {
    /// Sets `$name` for the specified Fluent message.
    pub fn set(&mut self, message: &str, name: &str, value: impl Into<UiFluentValue>) {
        let value = value.into();
        let entry = self.values.entry(message.to_owned()).or_default();
        if entry.get(name) != Some(&value) {
            entry.insert(name.to_owned(), value);
            self.revision = self.revision.wrapping_add(1);
        }
    }

    /// Removes all dynamic arguments for a message.
    pub fn clear(&mut self, message: &str) {
        if self.values.remove(message).is_some() {
            self.revision = self.revision.wrapping_add(1);
        }
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn for_message(&self, message: &str) -> Option<FluentArgs<'static>> {
        let values = self.values.get(message)?;
        let mut args = FluentArgs::with_capacity(values.len());
        for (name, value) in values {
            match value {
                UiFluentValue::Text(value) => args.set(name.clone(), value.clone()),
                UiFluentValue::Number(value) => args.set(name.clone(), *value),
            }
        }
        Some(args)
    }
}

#[derive(Resource)]
struct CatalogHandles(BTreeMap<LanguageIdentifier, Handle<UiFluentAsset>>);

#[derive(Resource, Default)]
struct LastCalendarLocaleRevision(Option<u64>);

/// Installs Fluent assets and keeps loaded catalogs in sync with asset reloads.
#[derive(Clone)]
pub struct UiFluentPlugin {
    config: UiFluentConfig,
}

impl UiFluentPlugin {
    pub fn new(config: UiFluentConfig) -> Self {
        Self { config }
    }
}

impl Plugin for UiFluentPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<UiFluentAsset>()
            .register_asset_loader(UiFluentAssetLoader)
            .insert_resource(
                UiLocalization::new(&self.config.fallback.to_string()).expect("valid fallback"),
            )
            .init_resource::<UiFluentArgs>()
            .init_resource::<LastCalendarLocaleRevision>();
        let server = app.world().resource::<AssetServer>();
        let handles = self
            .config
            .catalogs
            .iter()
            .map(|(locale, path)| (locale.clone(), server.load(format!("tilt-ui://{path}"))))
            .collect();
        app.insert_resource(CatalogHandles(handles))
            .add_systems(
                Update,
                sync_catalogs.before(crate::component::binding_runtime::apply_bindings),
            )
            .add_systems(
                PostUpdate,
                refresh_calendar_locale.before(UiSystems::Layout),
            );
    }
}

fn sync_catalogs(
    assets: Res<Assets<UiFluentAsset>>,
    handles: Res<CatalogHandles>,
    mut localization: ResMut<UiLocalization>,
) {
    if !assets.is_changed() {
        return;
    }
    for (locale, handle) in &handles.0 {
        let Some(asset) = assets.get(handle) else {
            continue;
        };
        if let Err(error) = localization.insert_ftl(&locale.to_string(), &asset.source) {
            warn!("Fluent catalog for {locale} could not be loaded: {error}");
        }
    }
}

fn refresh_calendar_locale(world: &mut World) {
    let revision = world.resource::<UiLocalization>().revision();
    if world.resource::<LastCalendarLocaleRevision>().0 == Some(revision) {
        return;
    }
    world.resource_mut::<LastCalendarLocaleRevision>().0 = Some(revision);
    crate::widgets::advanced::date_picker::refresh_localized_calendar_labels(world);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_switch_fallback_and_plural_arguments() {
        let mut localization = UiLocalization::new("en-US").unwrap();
        localization
            .insert_ftl(
                "en-US",
                "title = Hello\n    .tooltip = Greeting\nitems = { $count ->\n [one] One item\n*[other] { $count } items\n}",
            )
            .unwrap();
        localization.insert_ftl("de", "title = Hallo").unwrap();
        localization.set_locale("de-DE").unwrap();
        assert_eq!(
            localization.translate("title", None).as_deref(),
            Some("Hallo")
        );
        let mut args = FluentArgs::new();
        args.set("count", 2);
        assert_eq!(
            localization.translate("items", Some(&args)).as_deref(),
            Some("2 items")
        );
        assert_eq!(
            localization.translate("title.tooltip", None).as_deref(),
            Some("Greeting")
        );
        assert!(localization.insert_ftl("de", "broken = {").is_err());
        assert_eq!(
            localization.translate("title", None).as_deref(),
            Some("Hallo")
        );
    }

    #[test]
    fn changed_asset_replaces_the_catalog_without_restarting() {
        let mut app = App::new();
        app.add_plugins(bevy::asset::AssetPlugin::default());
        app.init_asset::<UiFluentAsset>();
        let handle = app
            .world_mut()
            .resource_mut::<Assets<UiFluentAsset>>()
            .add(UiFluentAsset {
                source: "title = First".into(),
            });
        app.insert_resource(UiLocalization::new("en-US").unwrap());
        app.insert_resource(CatalogHandles(BTreeMap::from([(
            parse_locale("en-US").unwrap(),
            handle.clone(),
        )])));
        app.add_systems(Update, sync_catalogs);
        app.update();
        assert_eq!(
            app.world()
                .resource::<UiLocalization>()
                .translate("title", None)
                .as_deref(),
            Some("First")
        );

        app.world_mut()
            .resource_mut::<Assets<UiFluentAsset>>()
            .get_mut(&handle)
            .unwrap()
            .source = "title = Second".into();
        app.update();
        assert_eq!(
            app.world()
                .resource::<UiLocalization>()
                .translate("title", None)
                .as_deref(),
            Some("Second")
        );
    }
}
