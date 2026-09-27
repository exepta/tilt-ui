use std::path::PathBuf;

use tilt_ui_core::{ComponentId, ComponentKind};

/// A fully validated component discovered from a TiltUI source tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredComponent {
    /// Deterministic identifier assigned by sorted discovery output.
    pub id: ComponentId,
    /// Convention-derived component or page name.
    pub name: String,
    /// `pages/` remains an optional convention; other locations are components.
    pub kind: ComponentKind,
    /// Directory relative to the `src-ui` source root.
    pub relative_directory: PathBuf,
    /// Absolute path to the original Rust component logic file.
    pub logic_path: PathBuf,
    /// Absolute path to the component HTML template source file.
    pub template_path: PathBuf,
    /// Absolute path to the component CSS stylesheet source file.
    pub stylesheet_path: PathBuf,
    /// CSS paths in the order authored by metadata (or the conventional CSS file).
    pub stylesheet_paths: Vec<PathBuf>,
    /// Asset paths for every component stylesheet in cascade order.
    pub stylesheet_asset_paths: Vec<String>,
    /// Valid Rust module identifier generated for the logic module.
    pub module_identifier: String,
    /// TiltUI asset-source path for the HTML template.
    pub template_asset_path: String,
    /// TiltUI asset-source path for the CSS stylesheet.
    pub stylesheet_asset_path: String,
}

/// Result of writing a generated component manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentManifest {
    /// Components included in deterministic generated order.
    pub components: Vec<DiscoveredComponent>,
    /// Path to the generated Rust manifest in Cargo's output directory.
    pub generated_path: PathBuf,
}
