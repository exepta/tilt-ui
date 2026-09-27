//! Native directory discovery for themes and Fluent catalogs.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use bevy::{
    ecs::{resource::Resource, world::World},
    log::warn,
    prelude::DetectChanges,
};
#[cfg(feature = "fluent")]
use tilt_ui_runtime::UiLocalization;
use tilt_ui_runtime::{UiRuntimeConfiguration, UiThemes, register_ui_theme, remove_ui_theme};

#[derive(Resource, Clone)]
pub(crate) struct UiDirectoryRoot(pub PathBuf);

#[derive(Resource, Default)]
pub(crate) struct DiscoveredUiFiles {
    themes: BTreeMap<String, String>,
    #[cfg(feature = "fluent")]
    languages: BTreeMap<String, String>,
}

pub(crate) fn refresh_changed_directories(world: &mut World) {
    if !world.resource_ref::<UiRuntimeConfiguration>().is_changed() {
        return;
    }
    if let Err(error) = refresh_ui_directories(world) {
        warn!("TiltUI directory discovery failed: {error}");
    }
}

/// Re-reads configured directories after files are added, edited, or removed.
/// Changing `UiRuntimeConfiguration` also triggers a scan on the next update.
/// Only files previously discovered by this function are removed from the registries.
pub fn refresh_ui_directories(world: &mut World) -> Result<(), String> {
    let root = world.resource::<UiDirectoryRoot>().0.clone();
    let config = world.resource::<UiRuntimeConfiguration>().clone();
    config.validate()?;
    let mut themes = match &config.themes_path {
        Some(path) => discover(&root.join(path), "css")?,
        None => BTreeMap::new(),
    };
    if let Some(names) = &config.theme_names {
        themes.retain(|name, _| names.contains(name));
        for name in names {
            if !themes.contains_key(name) {
                return Err(format!(
                    "theme {name:?} was not found in the configured directory"
                ));
            }
        }
    }
    let mut validated_themes = world.resource::<UiThemes>().clone();
    for (name, source) in &themes {
        validated_themes.register_css(name, source)?;
    }

    #[cfg(feature = "fluent")]
    let languages = match &config.language_path {
        Some(path) => discover(&root.join(path), "ftl")?,
        None => BTreeMap::new(),
    };
    #[cfg(feature = "fluent")]
    {
        let mut probe = UiLocalization::new("en-US").map_err(|error| error.to_string())?;
        for (locale, source) in &languages {
            probe
                .insert_ftl(locale, source)
                .map_err(|error| error.to_string())?;
        }
    }

    let previous_themes = world.resource::<DiscoveredUiFiles>().themes.clone();
    for name in previous_themes.keys() {
        if !themes.contains_key(name) {
            remove_ui_theme(world, name);
        }
    }
    for (name, source) in &themes {
        if previous_themes.get(name) != Some(source) {
            register_ui_theme(world, name, source)?;
        }
    }
    world.resource_mut::<DiscoveredUiFiles>().themes = themes;

    #[cfg(feature = "fluent")]
    {
        let previous = world.resource::<DiscoveredUiFiles>().languages.clone();
        for locale in previous.keys() {
            if !languages.contains_key(locale) {
                world
                    .resource_mut::<UiLocalization>()
                    .remove_ftl(locale)
                    .map_err(|error| error.to_string())?;
            }
        }
        for (locale, source) in &languages {
            if previous.get(locale) != Some(source) {
                world
                    .resource_mut::<UiLocalization>()
                    .insert_ftl(locale, source)
                    .map_err(|error| error.to_string())?;
            }
        }
        world.resource_mut::<DiscoveredUiFiles>().languages = languages;
    }
    Ok(())
}

fn discover(directory: &Path, extension: &str) -> Result<BTreeMap<String, String>, String> {
    let mut entries = BTreeMap::new();
    visit(directory, extension, &mut entries)?;
    Ok(entries)
}

fn visit(
    directory: &Path,
    extension: &str,
    entries: &mut BTreeMap<String, String>,
) -> Result<(), String> {
    let children =
        fs::read_dir(directory).map_err(|error| format!("{}: {error}", directory.display()))?;
    for child in children {
        let child = child.map_err(|error| error.to_string())?;
        let kind = child.file_type().map_err(|error| error.to_string())?;
        if kind.is_dir() {
            visit(&child.path(), extension, entries)?;
        } else if kind.is_file()
            && child
                .path()
                .extension()
                .is_some_and(|found| found.eq_ignore_ascii_case(extension))
        {
            let path = child.path();
            let name = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or_else(|| format!("invalid file name: {}", path.display()))?
                .to_owned();
            let source = fs::read_to_string(&path)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            if entries.insert(name.clone(), source).is_some() {
                return Err(format!(
                    "duplicate {extension} file stem {name:?} in {}",
                    directory.display()
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recursive_discovery_finds_files_and_rejects_duplicate_names() {
        let root = std::env::temp_dir().join(format!("tilt-ui-discovery-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::write(root.join("light.css"), ".a { color: red; }").unwrap();
        fs::write(root.join("nested/dark.css"), ".a { color: blue; }").unwrap();
        assert_eq!(discover(&root, "css").unwrap().len(), 2);
        fs::write(root.join("nested/light.css"), "").unwrap();
        assert!(discover(&root, "css").is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn changing_directories_replaces_discovered_themes_and_languages() {
        let root =
            std::env::temp_dir().join(format!("tilt-ui-directory-switch-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("first/themes")).unwrap();
        fs::create_dir_all(root.join("second/themes")).unwrap();
        fs::write(
            root.join("first/themes/light.css"),
            "body { color: #ff0000; }",
        )
        .unwrap();
        fs::write(
            root.join("second/themes/dark.css"),
            "body { color: #0000ff; }",
        )
        .unwrap();
        #[cfg(feature = "fluent")]
        {
            fs::create_dir_all(root.join("first/locales")).unwrap();
            fs::create_dir_all(root.join("second/locales")).unwrap();
            fs::write(root.join("first/locales/en-US.ftl"), "title = Hello").unwrap();
            fs::write(root.join("second/locales/de-DE.ftl"), "title = Hallo").unwrap();
        }

        let mut world = World::new();
        world.insert_resource(UiDirectoryRoot(root.clone()));
        world.init_resource::<UiThemes>();
        world.init_resource::<DiscoveredUiFiles>();
        #[cfg(feature = "fluent")]
        world.insert_resource(UiLocalization::new("en-US").unwrap());
        let first = UiRuntimeConfiguration::default()
            .with_themes_path("first/themes")
            .unwrap();
        #[cfg(feature = "fluent")]
        let first = first.with_language_path("first/locales").unwrap();
        world.insert_resource(first);
        refresh_ui_directories(&mut world).unwrap();
        assert!(world.resource::<UiThemes>().get("light").is_some());
        #[cfg(feature = "fluent")]
        assert_eq!(
            world
                .resource::<UiLocalization>()
                .translate("title", None)
                .as_deref(),
            Some("Hello")
        );

        let second = UiRuntimeConfiguration::default()
            .with_themes_path("second/themes")
            .unwrap();
        #[cfg(feature = "fluent")]
        let second = second.with_language_path("second/locales").unwrap();
        world.insert_resource(second);
        refresh_ui_directories(&mut world).unwrap();
        assert!(world.resource::<UiThemes>().get("light").is_none());
        assert!(world.resource::<UiThemes>().get("dark").is_some());
        #[cfg(feature = "fluent")]
        {
            assert!(
                world
                    .resource::<UiLocalization>()
                    .translate("title", None)
                    .is_none()
            );
            world
                .resource_mut::<UiLocalization>()
                .set_locale("de-DE")
                .unwrap();
            assert_eq!(
                world
                    .resource::<UiLocalization>()
                    .translate("title", None)
                    .as_deref(),
                Some("Hallo")
            );
        }
        fs::remove_dir_all(root).unwrap();
    }
}
