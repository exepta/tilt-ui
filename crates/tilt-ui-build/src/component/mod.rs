//! Build-time component discovery, validation, and manifest generation.

mod definition;
mod discovered;
mod discovery;
mod error;
mod generation;
mod validation;

pub use discovered::{ComponentManifest, DiscoveredComponent};
pub use discovery::discover_components;
pub use error::ComponentBuildError;
pub use generation::{
    build, build_from, cargo_rebuild_directives, generate_components, generate_manifest,
};
