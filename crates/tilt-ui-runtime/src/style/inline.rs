use bevy::ecs::component::Component;
use tilt_ui_css::{StyleDeclaration, parse_stylesheet};

/// Parsed declarations on one element, updated only when its authored source changes.
#[derive(Component, Debug, Clone)]
pub(crate) struct InlineStyle {
    pub static_source: String,
    pub dynamic_source: String,
    pub declarations: Vec<StyleDeclaration>,
}

impl InlineStyle {
    pub fn parse(source: &str) -> Result<Self, String> {
        Self::from_parts(source, "")
    }

    pub fn from_parts(static_source: &str, dynamic_source: &str) -> Result<Self, String> {
        let source = match (
            static_source.trim().is_empty(),
            dynamic_source.trim().is_empty(),
        ) {
            (true, true) => String::new(),
            (false, true) => static_source.to_owned(),
            (true, false) => dynamic_source.to_owned(),
            (false, false) => format!("{static_source};{dynamic_source}"),
        };
        if source.len() > 8192 {
            return Err("inline style exceeds 8192 bytes".into());
        }
        let declarations = if source.trim().is_empty() {
            Vec::new()
        } else {
            let wrapped = format!("* {{{source}}}");
            let sheet = parse_stylesheet(&wrapped).map_err(|error| error.to_string())?;
            sheet
                .rules
                .into_iter()
                .next()
                .ok_or("inline style has no declaration block")?
                .declarations
        };
        Ok(Self {
            static_source: static_source.to_owned(),
            dynamic_source: dynamic_source.to_owned(),
            declarations,
        })
    }
}
