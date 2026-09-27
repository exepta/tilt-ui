use super::{ComponentId, ComponentKind};

/// Immutable runtime-facing metadata for a component discovered at build time.
///
/// Asset paths use the `tilt-ui` Bevy asset-source convention and remain
/// distinct from physical source paths used only during the build process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentMetadata {
    /// Stable identifier within the generated component manifest.
    pub id: ComponentId,
    /// Convention-derived component or page name.
    pub name: &'static str,
    /// Indicates whether the entry is a page or reusable component.
    pub kind: ComponentKind,
    /// Bevy asset path for the component template.
    pub template_asset_path: &'static str,
    /// Bevy asset path for the component stylesheet.
    pub stylesheet_asset_path: &'static str,
    /// All CSS asset paths in authored cascade order. Empty means no author CSS.
    pub stylesheet_asset_paths: &'static [&'static str],
}
