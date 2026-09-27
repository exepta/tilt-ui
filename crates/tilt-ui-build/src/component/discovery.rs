use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use tilt_ui_core::ComponentKind;

use crate::UiSourceRoot;

use super::{
    ComponentBuildError, DiscoveredComponent,
    validation::{CollectedComponent, validate_components},
};

const COMPONENT_LOGIC_SUFFIX: &str = ".component.rs";
const COMPONENT_TEMPLATE_SUFFIX: &str = ".component.html";
const COMPONENT_STYLESHEET_SUFFIX: &str = ".component.css";

/// Recursively discovers and validates component triplets under a UI source root.
pub fn discover_components(
    source_root: &UiSourceRoot,
) -> Result<Vec<DiscoveredComponent>, ComponentBuildError> {
    if !source_root.exists() {
        return Err(ComponentBuildError::SourceRootMissing {
            path: source_root.path().to_path_buf(),
        });
    }

    let root_path = fs::canonicalize(source_root.path()).map_err(|source| {
        ComponentBuildError::CanonicalizePath {
            path: source_root.path().to_path_buf(),
            source,
        }
    })?;
    let mut collected = BTreeMap::new();

    collect_directory(&root_path, &root_path, &mut collected)?;

    validate_components(collected, &root_path)
}

fn collect_directory(
    root_path: &Path,
    directory: &Path,
    collected: &mut BTreeMap<PathBuf, CollectedComponent>,
) -> Result<(), ComponentBuildError> {
    if !directory.exists() {
        return Ok(());
    }

    let mut entries = fs::read_dir(directory)
        .map_err(|source| ComponentBuildError::ReadDirectory {
            path: directory.to_path_buf(),
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| ComponentBuildError::ReadDirectory {
            path: directory.to_path_buf(),
            source,
        })?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| ComponentBuildError::InspectPath {
                path: path.clone(),
                source,
            })?;
        if file_type.is_dir() {
            collect_directory(root_path, &path, collected)?;
        } else if file_type.is_file() {
            collect_component_file(root_path, &path, collected)?;
        }
    }
    Ok(())
}

fn collect_component_file(
    root_path: &Path,
    path: &Path,
    collected: &mut BTreeMap<PathBuf, CollectedComponent>,
) -> Result<(), ComponentBuildError> {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return Ok(());
    };
    let Some((name, suffix)) = component_file_name(file_name) else {
        return Ok(());
    };
    let relative_path =
        path.strip_prefix(root_path)
            .map_err(|_| ComponentBuildError::CanonicalizePath {
                path: path.to_path_buf(),
                source: std::io::Error::other("component path is outside the source root"),
            })?;
    let relative_directory = relative_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let kind = if relative_directory
        .components()
        .next()
        .is_some_and(|part| part.as_os_str() == "pages")
    {
        ComponentKind::Page
    } else {
        ComponentKind::Component
    };
    let key = relative_directory.join(name);
    let component = collected
        .entry(key)
        .or_insert_with(|| CollectedComponent::new(name.to_owned(), kind, relative_directory));

    match suffix {
        COMPONENT_LOGIC_SUFFIX => component.logic_path = Some(path.to_path_buf()),
        COMPONENT_TEMPLATE_SUFFIX => component.template_path = Some(path.to_path_buf()),
        COMPONENT_STYLESHEET_SUFFIX => component.stylesheet_path = Some(path.to_path_buf()),
        _ => unreachable!("component suffixes are filtered before collection"),
    }
    Ok(())
}

fn component_file_name(file_name: &str) -> Option<(&str, &str)> {
    [
        COMPONENT_LOGIC_SUFFIX,
        COMPONENT_TEMPLATE_SUFFIX,
        COMPONENT_STYLESHEET_SUFFIX,
    ]
    .into_iter()
    .find_map(|suffix| file_name.strip_suffix(suffix).map(|name| (name, suffix)))
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

    use tilt_ui_core::{ComponentId, ComponentKind};

    use super::discover_components;
    use crate::{
        ComponentBuildError, UiSourceRoot, cargo_rebuild_directives, generate_components,
        generate_manifest,
    };

    struct TemporaryDirectory {
        path: PathBuf,
    }

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    impl TemporaryDirectory {
        fn new() -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "tilt-ui-component-test-{}-{timestamp}-{}",
                process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("temporary directory");
            Self { path }
        }

        fn root(&self) -> UiSourceRoot {
            let root = UiSourceRoot::new(self.path.join("src-ui"));
            root.ensure().expect("source root");
            root
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TemporaryDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.path).expect("remove temporary directory");
        }
    }

    fn write_triplet(root: &UiSourceRoot, directory: &str, name: &str, logic: &str) {
        let directory = root.path().join(directory);
        fs::create_dir_all(&directory).expect("component directory");
        fs::write(directory.join(format!("{name}.component.rs")), logic).expect("logic");
        fs::write(directory.join(format!("{name}.component.html")), "<div />").expect("template");
        fs::write(
            directory.join(format!("{name}.component.css")),
            ".component { display: flex; }",
        )
        .expect("stylesheet");
    }

    #[test]
    fn discovers_page_component_and_nested_triplets() {
        let temporary = TemporaryDirectory::new();
        let root = temporary.root();
        write_triplet(&root, "pages", "main", "pub fn page_logic() {}\n");
        write_triplet(
            &root,
            "components/inventory",
            "inventory-slot",
            "pub fn component_logic() {}\n",
        );

        let components = discover_components(&root).expect("valid components");

        assert_eq!(components.len(), 2);
        assert_eq!(components[0].kind, ComponentKind::Component);
        assert_eq!(components[0].name, "inventory-slot");
        assert_eq!(components[1].kind, ComponentKind::Page);
        assert_eq!(components[1].name, "main");
        assert_eq!(
            components[0].template_asset_path,
            "tilt-ui://components/inventory/inventory-slot.component.html"
        );
        assert_eq!(
            components[0].stylesheet_asset_path,
            "tilt-ui://components/inventory/inventory-slot.component.css"
        );
    }

    #[test]
    fn discovers_components_directly_in_root_and_arbitrary_folders() {
        let temporary = TemporaryDirectory::new();
        let root = temporary.root();
        write_triplet(&root, "", "dashboard", "pub fn dashboard() {}\n");
        write_triplet(&root, "features/reports", "chart", "pub fn chart() {}\n");

        let components = discover_components(&root).unwrap();
        assert_eq!(components.len(), 2);
        assert!(
            components
                .iter()
                .all(|component| component.kind == ComponentKind::Component)
        );
        assert!(
            components
                .iter()
                .any(|component| component.template_asset_path
                    == "tilt-ui://dashboard.component.html")
        );
        assert!(
            components
                .iter()
                .any(|component| component.template_asset_path
                    == "tilt-ui://features/reports/chart.component.html")
        );
    }

    #[test]
    fn legacy_metadata_names_component_and_loads_ordered_styles() {
        let temporary = TemporaryDirectory::new();
        let root = temporary.root();
        write_triplet(
            &root,
            "components",
            "help",
            r#"
            struct Definition { template_name: &'static str, template_file: &'static str, styles: &'static [&'static str] }
            const HELP: Definition = Definition {
                template_name: "app-help",
                template_file: "help-content.component.html",
                styles: &["help.component.css", "help-extra.css"],
            };
        "#,
        );
        fs::write(
            root.path().join("components/help-extra.css"),
            "div { color: red; }",
        )
        .unwrap();
        fs::write(
            root.path().join("components/help-content.component.html"),
            "<div />",
        )
        .unwrap();
        fs::remove_file(root.path().join("components/help.component.html")).unwrap();
        let components = discover_components(&root).unwrap();
        assert_eq!(components[0].name, "app-help");
        assert_eq!(
            components[0].template_asset_path,
            "tilt-ui://components/help-content.component.html"
        );
        assert_eq!(
            components[0].stylesheet_asset_paths,
            [
                "tilt-ui://components/help.component.css",
                "tilt-ui://components/help-extra.css",
            ]
        );
        let generated = generate_manifest(&components);
        assert!(generated.contains("stylesheet_asset_paths: &[\"tilt-ui://components/help.component.css\", \"tilt-ui://components/help-extra.css\"]"));
        assert!(
            cargo_rebuild_directives(root.path(), &components)
                .iter()
                .any(|line| line.ends_with("help-extra.css"))
        );
    }

    #[test]
    fn reports_missing_triplet_files_and_orphans() {
        let temporary = TemporaryDirectory::new();
        let root = temporary.root();
        let directory = root.path().join("components");
        fs::create_dir(&directory).unwrap();

        fs::write(directory.join("missing-logic.component.html"), "<div />").unwrap();
        fs::write(directory.join("missing-logic.component.css"), ".x {}").unwrap();
        assert!(matches!(
            discover_components(&root),
            Err(ComponentBuildError::MissingLogic { .. })
        ));

        fs::remove_file(directory.join("missing-logic.component.html")).unwrap();
        fs::remove_file(directory.join("missing-logic.component.css")).unwrap();
        fs::write(
            directory.join("missing-template.component.rs"),
            "pub fn logic() {}",
        )
        .unwrap();
        fs::write(directory.join("missing-template.component.css"), ".x {}").unwrap();
        assert!(matches!(
            discover_components(&root),
            Err(ComponentBuildError::MissingTemplate { .. })
        ));

        fs::remove_file(directory.join("missing-template.component.rs")).unwrap();
        fs::remove_file(directory.join("missing-template.component.css")).unwrap();
        fs::write(
            directory.join("missing-style.component.rs"),
            "pub fn logic() {}",
        )
        .unwrap();
        fs::write(directory.join("missing-style.component.html"), "<div />").unwrap();
        assert!(matches!(
            discover_components(&root),
            Err(ComponentBuildError::MissingStylesheet { .. })
        ));

        fs::remove_file(directory.join("missing-style.component.rs")).unwrap();
        fs::remove_file(directory.join("missing-style.component.html")).unwrap();
        fs::write(directory.join("orphan.component.css"), ".x {}").unwrap();
        assert!(matches!(
            discover_components(&root),
            Err(ComponentBuildError::OrphanComponentFile { .. })
        ));
    }

    #[test]
    fn rejects_duplicate_invalid_and_builtin_component_names() {
        let temporary = TemporaryDirectory::new();
        let root = temporary.root();
        write_triplet(
            &root,
            "components/header",
            "app-header",
            "pub fn first() {}",
        );
        write_triplet(
            &root,
            "components/navigation",
            "app-header",
            "pub fn second() {}",
        );
        assert!(matches!(
            discover_components(&root),
            Err(ComponentBuildError::DuplicateComponentName { .. })
        ));

        fs::remove_dir_all(root.path().join("components")).unwrap();
        fs::create_dir(root.path().join("components")).unwrap();
        write_triplet(&root, "components", "App Header", "pub fn invalid() {}");
        assert!(matches!(
            discover_components(&root),
            Err(ComponentBuildError::InvalidComponentName { .. })
        ));

        fs::remove_dir_all(root.path().join("components")).unwrap();
        fs::create_dir(root.path().join("components")).unwrap();
        write_triplet(&root, "components", "button", "pub fn builtin() {}");
        assert!(matches!(
            discover_components(&root),
            Err(ComponentBuildError::BuiltInElementCollision { .. })
        ));

        fs::remove_dir_all(root.path().join("components")).unwrap();
        fs::create_dir(root.path().join("components")).unwrap();
        write_triplet(&root, "pages", "page-name", "pub fn hyphenated() {}");
        write_triplet(&root, "pages", "page_name", "pub fn underscored() {}");
        assert!(matches!(
            discover_components(&root),
            Err(ComponentBuildError::ModuleIdentifierCollision { .. })
        ));
    }

    #[test]
    fn assigns_sorted_deterministic_ids_and_valid_module_identifiers() {
        let temporary = TemporaryDirectory::new();
        let root = temporary.root();
        write_triplet(&root, "pages", "zeta", "pub fn zeta() {}");
        write_triplet(&root, "components", "alpha-card", "pub fn alpha() {}");
        write_triplet(
            &root,
            "components/inventory",
            "inventory-slot",
            "pub fn slot() {}",
        );

        let components = discover_components(&root).expect("valid components");

        assert_eq!(components[0].name, "alpha-card");
        assert_eq!(components[0].id, ComponentId(0));
        assert_eq!(components[0].module_identifier, "alpha_card_component");
        assert_eq!(components[1].name, "inventory-slot");
        assert_eq!(components[1].id, ComponentId(1));
        assert_eq!(components[1].module_identifier, "inventory_slot_component");
        assert_eq!(components[2].name, "zeta");
        assert_eq!(components[2].id, ComponentId(2));
    }

    #[test]
    fn generated_manifest_references_original_logic_and_is_stable() {
        let temporary = TemporaryDirectory::new();
        let root = temporary.root();
        const LOGIC: &str = "pub const ORIGINAL_COMPONENT_LOGIC: &str = \"compiled\";\n";
        write_triplet(&root, "components", "app-header", LOGIC);
        let components = discover_components(&root).expect("valid components");
        let generated = generate_manifest(&components);

        assert!(generated.contains("#[path = "));
        assert!(generated.contains(components[0].logic_path.to_string_lossy().as_ref()));
        assert!(!generated.contains("ORIGINAL_COMPONENT_LOGIC"));
        assert!(generated.contains("TILT_UI_COMPONENTS"));
        assert!(generated.contains("match name"));

        let first_output = temporary.path().join("out-one");
        let second_output = temporary.path().join("out-two");
        fs::create_dir(&first_output).unwrap();
        fs::create_dir(&second_output).unwrap();
        let first = generate_components(&root, &first_output).expect("first generation");
        let second = generate_components(&root, &second_output).expect("second generation");
        assert_eq!(
            fs::read_to_string(first.generated_path).unwrap(),
            fs::read_to_string(second.generated_path).unwrap()
        );
    }

    #[test]
    fn rebuild_directives_cover_the_root_and_every_triplet_file() {
        let temporary = TemporaryDirectory::new();
        let root = temporary.root();
        write_triplet(&root, "components", "app-header", "pub fn logic() {}");
        let components = discover_components(&root).expect("valid components");
        let directives = cargo_rebuild_directives(root.path(), &components);

        assert_eq!(directives.len(), 4);
        assert_eq!(
            directives[0],
            format!("cargo:rerun-if-changed={}", root.path().display())
        );
        assert!(
            directives
                .iter()
                .any(|directive| directive.ends_with(".component.rs"))
        );
        assert!(
            directives
                .iter()
                .any(|directive| directive.ends_with(".component.html"))
        );
        assert!(
            directives
                .iter()
                .any(|directive| directive.ends_with(".component.css"))
        );
    }
}
