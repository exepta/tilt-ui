use tilt_ui_core::TemplateAttribute;

use super::TemplateParseError;

/// Classifies a decoded template attribute by its binding syntax.
pub(crate) fn parse_attribute(
    name: String,
    value: String,
) -> Result<TemplateAttribute, TemplateParseError> {
    if let Some(binding_name) = delimited_name(&name, '[', ']') {
        return Ok(TemplateAttribute::PropertyBinding {
            name: binding_name.to_owned(),
            expression: value.trim().to_owned(),
        });
    }
    if let Some(binding_name) = delimited_name(&name, '(', ')') {
        return Ok(TemplateAttribute::EventBinding {
            name: binding_name.to_owned(),
            expression: value.trim().to_owned(),
        });
    }
    if name.starts_with(['[', '(']) || name.ends_with([']', ')']) {
        return Err(TemplateParseError::InvalidBinding(name));
    }
    if name.is_empty() {
        return Err(TemplateParseError::InvalidAttribute(name));
    }

    Ok(TemplateAttribute::Static { name, value })
}

fn delimited_name(name: &str, start: char, end: char) -> Option<&str> {
    name.strip_prefix(start)
        .and_then(|name| name.strip_suffix(end))
        .filter(|name| !name.is_empty() && is_binding_name(name))
}

fn is_binding_name(name: &str) -> bool {
    name.chars()
        .all(|character| !character.is_whitespace() && !matches!(character, '[' | ']' | '(' | ')'))
}
