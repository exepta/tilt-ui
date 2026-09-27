use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use tilt_ui_core::{ComponentId, ComponentKind};
use tilt_ui_html::{is_builtin_element_tag, is_valid_component_name};

use super::{ComponentBuildError, DiscoveredComponent, definition};

pub(crate) struct CollectedComponent {
    pub(crate) name: String,
    pub(crate) kind: ComponentKind,
    pub(crate) relative_directory: PathBuf,
    pub(crate) logic_path: Option<PathBuf>,
    pub(crate) template_path: Option<PathBuf>,
    pub(crate) stylesheet_path: Option<PathBuf>,
}

impl CollectedComponent {
    pub(crate) fn new(name: String, kind: ComponentKind, relative_directory: PathBuf) -> Self {
        Self {
            name,
            kind,
            relative_directory,
            logic_path: None,
            template_path: None,
            stylesheet_path: None,
        }
    }

    fn base_path(&self) -> PathBuf {
        self.relative_directory.join(&self.name)
    }
}

pub(crate) fn validate_components(
    collected: BTreeMap<PathBuf, CollectedComponent>,
    root: &Path,
) -> Result<Vec<DiscoveredComponent>, ComponentBuildError> {
    let mut components = Vec::with_capacity(collected.len());

    let mut referenced_styles = HashSet::new();
    let mut referenced_templates = HashSet::new();
    for component in collected
        .values()
        .filter(|component| component.logic_path.is_some())
    {
        let logic_path = component.logic_path.as_ref().unwrap();
        let definition = definition::read(logic_path)?;
        let name = definition.as_ref().map_or_else(
            || component.name.clone(),
            |definition| definition.name.clone(),
        );
        let template_path = if let Some(definition) = &definition {
            resolve_asset(root, logic_path, &definition.template_file)?
        } else {
            component
                .template_path
                .clone()
                .ok_or_else(|| ComponentBuildError::MissingTemplate {
                    path: component.base_path(),
                })?
        };
        let stylesheet_paths = if let Some(definition) = definition {
            definition
                .styles
                .iter()
                .map(|style| resolve_asset(root, logic_path, style))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            vec![component.stylesheet_path.clone().ok_or_else(|| {
                ComponentBuildError::MissingStylesheet {
                    path: component.base_path(),
                }
            })?]
        };
        referenced_templates.insert(template_path.clone());
        referenced_styles.extend(stylesheet_paths.iter().cloned());
        if component.kind == ComponentKind::Component && !is_valid_component_name(&name) {
            return Err(ComponentBuildError::InvalidComponentName {
                name,
                path: logic_path.clone(),
            });
        }
        if component.kind == ComponentKind::Component && is_builtin_element_tag(&name) {
            return Err(ComponentBuildError::BuiltInElementCollision {
                name,
                path: logic_path.clone(),
            });
        }
        let template_asset_path = asset_path_for_file(root, &template_path);
        let stylesheet_asset_paths = stylesheet_paths
            .iter()
            .map(|path| asset_path_for_file(root, path))
            .collect::<Vec<_>>();
        let stylesheet_path = stylesheet_paths.first().cloned().unwrap_or_default();
        let stylesheet_asset_path = stylesheet_asset_paths.first().cloned().unwrap_or_default();
        components.push(DiscoveredComponent {
            id: ComponentId(0),
            name: name.clone(),
            kind: component.kind,
            relative_directory: component.relative_directory.clone(),
            logic_path: logic_path.clone(),
            template_path,
            stylesheet_path,
            stylesheet_paths,
            stylesheet_asset_paths,
            module_identifier: module_identifier(&name),
            template_asset_path,
            stylesheet_asset_path,
        });
    }
    for component in collected
        .values()
        .filter(|component| component.logic_path.is_none())
    {
        let template_referenced = component
            .template_path
            .as_ref()
            .is_none_or(|template| referenced_templates.contains(template));
        let style_referenced = component
            .stylesheet_path
            .as_ref()
            .is_none_or(|style| referenced_styles.contains(style));
        if template_referenced && style_referenced {
            continue;
        }
        let present = component
            .template_path
            .as_ref()
            .or(component.stylesheet_path.as_ref())
            .unwrap();
        if component.template_path.is_some() && component.stylesheet_path.is_some() {
            return Err(ComponentBuildError::MissingLogic {
                path: component.base_path(),
            });
        }
        return Err(ComponentBuildError::OrphanComponentFile {
            path: present.clone(),
        });
    }

    components.sort_by(|left, right| {
        left.relative_directory
            .cmp(&right.relative_directory)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.kind.cmp(&right.kind))
    });

    let mut names = BTreeMap::new();
    let mut modules = BTreeMap::new();
    for (index, component) in components.iter_mut().enumerate() {
        let id =
            ComponentId(u32::try_from(index).map_err(|_| ComponentBuildError::TooManyComponents)?);
        component.id = id;

        if let Some(first) = names.insert(component.name.clone(), component.logic_path.clone()) {
            return Err(ComponentBuildError::DuplicateComponentName {
                name: component.name.clone(),
                first,
                second: component.logic_path.clone(),
            });
        }
        if let Some(first) = modules.insert(
            component.module_identifier.clone(),
            component.logic_path.clone(),
        ) {
            return Err(ComponentBuildError::ModuleIdentifierCollision {
                identifier: component.module_identifier.clone(),
                first,
                second: component.logic_path.clone(),
            });
        }
    }

    Ok(components)
}

pub(crate) fn module_identifier(name: &str) -> String {
    let mut identifier = String::new();
    for character in name.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            identifier.push(character.to_ascii_lowercase());
        } else {
            identifier.push('_');
        }
    }
    if identifier
        .chars()
        .next()
        .is_none_or(|character| character.is_ascii_digit())
    {
        identifier.insert(0, '_');
    }
    identifier.push_str("_component");
    identifier
}

fn asset_path_for_file(root: &Path, path: &Path) -> String {
    format!(
        "tilt-ui://{}",
        path.strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/")
    )
}

fn resolve_asset(root: &Path, logic: &Path, name: &str) -> Result<PathBuf, ComponentBuildError> {
    let source = Path::new(name);
    if source.is_absolute() || !name.ends_with(".html") && !name.ends_with(".css") {
        return Err(ComponentBuildError::InvalidComponentDefinition {
            path: logic.to_owned(),
            reason: format!("unsupported asset path {name:?}"),
        });
    }
    let path = logic.parent().unwrap().join(source);
    let resolved = fs::canonicalize(&path).map_err(|error| {
        ComponentBuildError::InvalidComponentDefinition {
            path: logic.to_owned(),
            reason: format!("{}: {error}", path.display()),
        }
    })?;
    if !resolved.starts_with(root) || !resolved.is_file() {
        return Err(ComponentBuildError::InvalidComponentDefinition {
            path: logic.to_owned(),
            reason: format!(
                "asset {} must be a file within {}",
                resolved.display(),
                root.display()
            ),
        });
    }
    Ok(resolved)
}
