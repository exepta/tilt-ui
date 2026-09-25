use std::path::PathBuf;

use thiserror::Error;

use crate::UiSourceRootError;

/// Describes a failure while discovering or generating TiltUI components.
#[derive(Debug, Error)]
pub enum ComponentBuildError {
    /// The configured TiltUI source root does not exist.
    #[error("TiltUI source root does not exist: {}", path.display())]
    SourceRootMissing {
        /// Missing source-root path.
        path: PathBuf,
    },

    /// A component directory could not be read.
    #[error("failed to read component directory `{}`: {source}", path.display())]
    ReadDirectory {
        /// Directory that could not be read.
        path: PathBuf,
        /// Underlying filesystem error.
        #[source]
        source: std::io::Error,
    },

    /// A source entry could not be inspected.
    #[error("failed to inspect component source `{}`: {source}", path.display())]
    InspectPath {
        /// Source entry that could not be inspected.
        path: PathBuf,
        /// Underlying filesystem error.
        #[source]
        source: std::io::Error,
    },

    /// A component source path could not be canonicalized.
    #[error("failed to canonicalize component source `{}`: {source}", path.display())]
    CanonicalizePath {
        /// Source path that could not be canonicalized.
        path: PathBuf,
        /// Underlying filesystem error.
        #[source]
        source: std::io::Error,
    },

    /// A grouped component does not contain its Rust logic file.
    #[error("component `{}` is missing `.component.rs`", path.display())]
    MissingLogic {
        /// Convention-derived component base path.
        path: PathBuf,
    },

    /// A grouped component does not contain its HTML template file.
    #[error("component `{}` is missing `.component.html`", path.display())]
    MissingTemplate {
        /// Convention-derived component base path.
        path: PathBuf,
    },

    /// A grouped component does not contain its CSS stylesheet file.
    #[error("component `{}` is missing `.component.css`", path.display())]
    MissingStylesheet {
        /// Convention-derived component base path.
        path: PathBuf,
    },

    /// A single component source file has no matching triplet companions.
    #[error("orphan component source file: {}", path.display())]
    OrphanComponentFile {
        /// Source file without a matching component triplet.
        path: PathBuf,
    },

    /// A reusable component name does not follow TiltUI's tag convention.
    #[error("invalid component name `{name}` at {}", path.display())]
    InvalidComponentName {
        /// Invalid convention-derived name.
        name: String,
        /// Source path containing the invalid name.
        path: PathBuf,
    },

    /// A reusable component name collides with a built-in TiltUI element tag.
    #[error("component name `{name}` at {} collides with a built-in element", path.display())]
    BuiltInElementCollision {
        /// Conflicting component name.
        name: String,
        /// Logic path for the conflicting component.
        path: PathBuf,
    },

    /// Two discovered entries would create an ambiguous component name.
    #[error(
        "duplicate component name `{name}` at {} and {}",
        first.display(),
        second.display()
    )]
    DuplicateComponentName {
        /// Ambiguous convention-derived name.
        name: String,
        /// First discovered logic path.
        first: PathBuf,
        /// Conflicting logic path.
        second: PathBuf,
    },

    /// Two discovered entries would create the same Rust module identifier.
    #[error(
        "module identifier `{identifier}` collides for {} and {}",
        first.display(),
        second.display()
    )]
    ModuleIdentifierCollision {
        /// Generated identifier shared by both entries.
        identifier: String,
        /// First discovered logic path.
        first: PathBuf,
        /// Conflicting logic path.
        second: PathBuf,
    },

    /// More components were discovered than fit in `ComponentId`.
    #[error("too many discovered components")]
    TooManyComponents,

    /// Cargo did not provide its build output directory.
    #[error("OUT_DIR is not set")]
    MissingOutDirectory,

    /// The generated manifest could not be written.
    #[error("failed to write generated component manifest `{}`: {source}", path.display())]
    WriteGeneratedManifest {
        /// Generated manifest output path.
        path: PathBuf,
        /// Underlying filesystem error.
        #[source]
        source: std::io::Error,
    },

    /// Source-root resolution or scaffolding failed.
    #[error(transparent)]
    SourceRoot(#[from] UiSourceRootError),
}
