use std::{collections::BTreeMap, path::PathBuf};

use tilt_ui_core::{ComponentId, ComponentKind};
use tilt_ui_html::{is_builtin_element_tag, is_valid_component_name};

use super::{ComponentBuildError, DiscoveredComponent};

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
) -> Result<Vec<DiscoveredComponent>, ComponentBuildError> {
    let mut components = Vec::with_capacity(collected.len());

    for component in collected.into_values() {
        let base_path = component.base_path();
        let present_files = [
            component.logic_path.as_ref(),
            component.template_path.as_ref(),
            component.stylesheet_path.as_ref(),
        ];
        if present_files.iter().flatten().count() == 1 {
            return Err(ComponentBuildError::OrphanComponentFile {
                path: present_files
                    .iter()
                    .flatten()
                    .next()
                    .expect("component file count is one")
                    .to_path_buf(),
            });
        }
        let logic_path = component
            .logic_path
            .ok_or_else(|| ComponentBuildError::MissingLogic {
                path: base_path.clone(),
            })?;
        let template_path =
            component
                .template_path
                .ok_or_else(|| ComponentBuildError::MissingTemplate {
                    path: base_path.clone(),
                })?;
        let stylesheet_path = component
            .stylesheet_path
            .ok_or(ComponentBuildError::MissingStylesheet { path: base_path })?;

        if component.kind == ComponentKind::Component && !is_valid_component_name(&component.name) {
            return Err(ComponentBuildError::InvalidComponentName {
                name: component.name,
                path: logic_path,
            });
        }
        if component.kind == ComponentKind::Component && is_builtin_element_tag(&component.name) {
            return Err(ComponentBuildError::BuiltInElementCollision {
                name: component.name,
                path: logic_path,
            });
        }

        let module_identifier = module_identifier(&component.name);
        let template_asset_path =
            asset_path(&component.relative_directory, &component.name, "html");
        let stylesheet_asset_path =
            asset_path(&component.relative_directory, &component.name, "css");
        components.push(DiscoveredComponent {
            id: ComponentId(0),
            name: component.name,
            kind: component.kind,
            relative_directory: component.relative_directory,
            logic_path,
            template_path,
            stylesheet_path,
            module_identifier,
            template_asset_path,
            stylesheet_asset_path,
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

fn asset_path(relative_directory: &std::path::Path, name: &str, extension: &str) -> String {
    let directory = relative_directory.to_string_lossy().replace('\\', "/");
    if directory.is_empty() {
        format!("tilt-ui://{name}.component.{extension}")
    } else {
        format!("tilt-ui://{directory}/{name}.component.{extension}")
    }
}
