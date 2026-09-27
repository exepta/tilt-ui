//! Paths relative to the fixed `tilt-ui://` asset source.

use std::path::{Component, Path, PathBuf};

use bevy::ecs::resource::Resource;

/// Mutable locations for UI assets and directory-based theme/locale discovery.
///
/// The physical asset source is registered before Bevy's `AssetPlugin` and cannot
/// be changed at runtime. Every path here is relative to that source. Generated
/// Rust component IDs remain fixed; `components_path` relocates their template
/// and stylesheet assets, but does not discover new Rust components.
#[derive(Resource, Debug, Clone, PartialEq, Eq, Default)]
pub struct UiRuntimeConfiguration {
    /// Prefix applied to generated template and stylesheet asset paths.
    pub components_path: PathBuf,
    /// Directory used for relative image sources such as `icon.png`.
    pub assets_path: PathBuf,
    /// Directory recursively scanned for Fluent `.ftl` files on native targets.
    pub language_path: Option<PathBuf>,
    /// Directory recursively scanned for theme `.css` files on native targets.
    pub themes_path: Option<PathBuf>,
    /// Optional allowlist of theme file stems.
    pub theme_names: Option<Vec<String>>,
}

impl UiRuntimeConfiguration {
    /// Rejects absolute paths and parent traversal before the configuration is applied.
    pub fn validate(&self) -> Result<(), String> {
        checked_path(&self.components_path)?;
        checked_path(&self.assets_path)?;
        if let Some(path) = &self.language_path {
            checked_path(path)?;
        }
        if let Some(path) = &self.themes_path {
            checked_path(path)?;
        }
        Ok(())
    }

    /// Relocates generated component template and stylesheet asset paths.
    pub fn with_components_path(mut self, path: impl AsRef<Path>) -> Result<Self, String> {
        self.components_path = checked_path(path.as_ref())?;
        Ok(self)
    }

    /// Sets the directory for relative image sources.
    pub fn with_assets_path(mut self, path: impl AsRef<Path>) -> Result<Self, String> {
        self.assets_path = checked_path(path.as_ref())?;
        Ok(self)
    }

    /// Discovers Fluent catalogs below this directory on native targets.
    pub fn with_language_path(mut self, path: impl AsRef<Path>) -> Result<Self, String> {
        self.language_path = Some(checked_path(path.as_ref())?);
        Ok(self)
    }

    /// Discovers CSS themes below this directory on native targets.
    pub fn with_themes_path(mut self, path: impl AsRef<Path>) -> Result<Self, String> {
        self.themes_path = Some(checked_path(path.as_ref())?);
        Ok(self)
    }

    /// Restricts discovery to these CSS file stems; `None` discovers all themes.
    pub fn with_theme_names(mut self, names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.theme_names = Some(names.into_iter().map(Into::into).collect());
        self
    }

    /// Resolves an authored image/asset path, preserving explicit Bevy paths.
    pub fn asset_path(&self, path: &str) -> String {
        if path.contains("://")
            || Path::new(path).is_absolute()
            || self.assets_path.as_os_str().is_empty()
            || checked_path(&self.assets_path).is_err()
        {
            return path.to_owned();
        }
        format!("tilt-ui://{}/{}", self.assets_path.display(), path)
    }

    pub(crate) fn component_path(&self, original: &str) -> String {
        if self.components_path.as_os_str().is_empty()
            || checked_path(&self.components_path).is_err()
        {
            return original.to_owned();
        }
        let relative = original.strip_prefix("tilt-ui://").unwrap_or(original);
        format!("tilt-ui://{}/{}", self.components_path.display(), relative)
    }
}

fn checked_path(path: &Path) -> Result<PathBuf, String> {
    if path
        .components()
        .all(|part| matches!(part, Component::Normal(_) | Component::CurDir))
    {
        Ok(path.to_path_buf())
    } else {
        Err(format!(
            "UI path must stay inside the asset source: {}",
            path.display()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_stay_inside_source_and_explicit_assets_are_preserved() {
        assert!(
            UiRuntimeConfiguration::default()
                .with_themes_path("../outside")
                .is_err()
        );
        let config = UiRuntimeConfiguration::default()
            .with_components_path("alternate")
            .unwrap()
            .with_assets_path("media")
            .unwrap();
        assert_eq!(
            config.component_path("tilt-ui://pages/main.component.html"),
            "tilt-ui://alternate/pages/main.component.html"
        );
        assert_eq!(
            config.asset_path("picture.png"),
            "tilt-ui://media/picture.png"
        );
        assert_eq!(
            config.asset_path("tilt-ui://icons/icon.png"),
            "tilt-ui://icons/icon.png"
        );
    }
}
