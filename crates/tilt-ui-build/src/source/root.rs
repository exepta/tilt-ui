use std::{
    env, fs,
    path::{Path, PathBuf},
};

use thiserror::Error;

/// Names the default TiltUI source directory relative to a consumer project root.
pub const DEFAULT_UI_SOURCE_DIR: &str = "src-ui";

/// Represents the filesystem root for TiltUI component source files.
///
/// The default root is resolved from a consumer project's Cargo manifest
/// directory. The type does not discover component files or parse templates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiSourceRoot {
    path: PathBuf,
}

impl UiSourceRoot {
    /// Creates a source-root representation for an explicit filesystem path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Resolves the default `src-ui` directory under a consumer project root.
    pub fn discover(project_root: impl AsRef<Path>) -> Self {
        Self::new(project_root.as_ref().join(DEFAULT_UI_SOURCE_DIR))
    }

    /// Resolves the default source root from Cargo's consumer manifest directory.
    pub fn from_cargo_manifest_dir() -> Result<Self, UiSourceRootError> {
        let manifest_dir = env::var_os("CARGO_MANIFEST_DIR")
            .ok_or(UiSourceRootError::MissingCargoManifestDirectory)?;
        Ok(Self::discover(PathBuf::from(manifest_dir)))
    }

    /// Returns the filesystem path represented by this source root.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns whether the source-root directory already exists.
    pub fn exists(&self) -> bool {
        self.path.is_dir()
    }

    /// Creates the source root without imposing a directory layout beneath it.
    ///
    /// Existing directories and files are preserved, so repeated calls are
    /// idempotent.
    pub fn ensure(&self) -> Result<(), UiSourceRootError> {
        fs::create_dir_all(&self.path)?;
        Ok(())
    }
}

/// Describes a failure while resolving or creating a UI source root.
#[derive(Debug, Error)]
pub enum UiSourceRootError {
    /// Cargo did not provide the consumer package manifest directory.
    #[error("CARGO_MANIFEST_DIR is not set")]
    MissingCargoManifestDirectory,

    /// A source-root filesystem operation failed.
    #[error("UI source-root filesystem operation failed: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        process,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{DEFAULT_UI_SOURCE_DIR, UiSourceRoot};

    struct TemporaryDirectory {
        path: PathBuf,
    }

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    impl TemporaryDirectory {
        fn new() -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "tilt-ui-build-test-{}-{timestamp}-{}",
                process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TemporaryDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.path).unwrap();
        }
    }

    #[test]
    fn discovers_the_default_source_root() {
        let project = TemporaryDirectory::new();
        let root = UiSourceRoot::discover(project.path());

        assert_eq!(root.path(), project.path().join(DEFAULT_UI_SOURCE_DIR));
        assert!(!root.exists());
    }

    #[test]
    fn ensure_creates_only_the_source_root() {
        let project = TemporaryDirectory::new();
        let root = UiSourceRoot::discover(project.path());

        root.ensure().unwrap();

        assert!(root.exists());
        assert!(!root.path().join("pages").exists());
        assert!(!root.path().join("components").exists());
    }

    #[test]
    fn ensure_preserves_existing_files_and_is_idempotent() {
        let project = TemporaryDirectory::new();
        let root = UiSourceRoot::discover(project.path());
        root.ensure().unwrap();
        fs::create_dir(root.path().join("components")).unwrap();
        let existing_file = root.path().join("components").join("header.component.html");
        fs::write(&existing_file, "<div>Header</div>").unwrap();

        root.ensure().unwrap();
        root.ensure().unwrap();

        assert_eq!(
            fs::read_to_string(existing_file).unwrap(),
            "<div>Header</div>"
        );
    }

    #[test]
    fn supports_custom_source_root_paths() {
        let project = TemporaryDirectory::new();
        let root = UiSourceRoot::new(project.path().join("interface"));

        root.ensure().unwrap();

        assert_eq!(root.path(), project.path().join("interface"));
        assert!(!root.path().join("pages").exists());
    }
}
