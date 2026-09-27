//! Optional legacy component metadata embedded in a Rust struct literal.

use std::{fs, path::Path};
use syn::{Expr, ExprArray, ExprLit, ExprStruct, Item, Lit};

use super::ComponentBuildError;

#[derive(Debug, Clone)]
pub(super) struct ComponentDefinition {
    pub name: String,
    pub template_file: String,
    pub styles: Vec<String>,
}

pub(super) fn read(path: &Path) -> Result<Option<ComponentDefinition>, ComponentBuildError> {
    let source =
        fs::read_to_string(path).map_err(|error| ComponentBuildError::ReadComponentDefinition {
            path: path.to_owned(),
            source: error,
        })?;
    let file = syn::parse_file(&source).map_err(|error| {
        ComponentBuildError::InvalidComponentDefinition {
            path: path.to_owned(),
            reason: error.to_string(),
        }
    })?;
    for item in file.items {
        let expression = match item {
            Item::Const(item) => item.expr,
            Item::Static(item) => item.expr,
            _ => continue,
        };
        let Expr::Struct(fields) = *expression else {
            continue;
        };
        if !fields.fields.iter().any(
            |field| matches!(&field.member, syn::Member::Named(name) if name == "template_name"),
        ) {
            continue;
        }
        return parse_definition(path, &fields).map(Some);
    }
    Ok(None)
}

fn parse_definition(
    path: &Path,
    definition: &ExprStruct,
) -> Result<ComponentDefinition, ComponentBuildError> {
    let mut name = None;
    let mut template_file = None;
    let mut styles = None;
    for field in &definition.fields {
        let syn::Member::Named(key) = &field.member else {
            continue;
        };
        match key.to_string().as_str() {
            "template_name" => name = string(&field.expr),
            "template_file" => template_file = string(&field.expr),
            "styles" => styles = strings(&field.expr),
            _ => {}
        }
    }
    let invalid = |reason: &str| ComponentBuildError::InvalidComponentDefinition {
        path: path.to_owned(),
        reason: reason.to_owned(),
    };
    Ok(ComponentDefinition {
        name: name
            .filter(|name| !name.is_empty())
            .ok_or_else(|| invalid("template_name must be a non-empty string literal"))?,
        template_file: template_file
            .filter(|name| !name.is_empty())
            .ok_or_else(|| invalid("template_file must be a non-empty string literal"))?,
        styles: styles.ok_or_else(|| invalid("styles must be an array of string literals"))?,
    })
}

fn string(expression: &Expr) -> Option<String> {
    if let Expr::Lit(ExprLit {
        lit: Lit::Str(value),
        ..
    }) = expression
    {
        Some(value.value())
    } else {
        None
    }
}

fn strings(expression: &Expr) -> Option<Vec<String>> {
    let expression = if let Expr::Reference(reference) = expression {
        &*reference.expr
    } else {
        expression
    };
    let Expr::Array(ExprArray { elems, .. }) = expression else {
        return None;
    };
    elems.iter().map(string).collect()
}
