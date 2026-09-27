use bevy::{asset::Asset, reflect::TypePath};
use tilt_ui_core::Template;
use tilt_ui_html::{DocumentHead, ParsedDocument};

/// Bevy asset containing a parsed TiltUI component template.
#[derive(Asset, TypePath, Debug)]
pub struct UiTemplateAsset {
    template: Template,
    document_head: Option<DocumentHead>,
}

impl UiTemplateAsset {
    /// Wraps a parsed TiltUI template as a Bevy asset.
    pub fn new(template: Template) -> Self {
        Self {
            template,
            document_head: None,
        }
    }

    /// Wraps the parsed entry document and retains its head metadata.
    pub fn from_document(document: ParsedDocument) -> Self {
        Self {
            template: document.template,
            document_head: Some(document.head),
        }
    }

    /// Returns head metadata only for an `index.html` asset.
    pub fn document_head(&self) -> Option<&DocumentHead> {
        self.document_head.as_ref()
    }

    /// Returns the parsed template retained by this asset.
    pub fn template(&self) -> &Template {
        &self.template
    }

    /// Consumes the asset and returns its parsed template.
    pub fn into_template(self) -> Template {
        self.template
    }
}

impl From<Template> for UiTemplateAsset {
    fn from(template: Template) -> Self {
        Self::new(template)
    }
}

#[cfg(test)]
mod tests {
    use super::UiTemplateAsset;
    use tilt_ui_html::parse_template;

    #[test]
    fn retains_the_parsed_template() {
        let template = parse_template("<button>Play</button>").expect("valid template");
        let asset = UiTemplateAsset::new(template);

        assert_eq!(asset.template().roots().len(), 1);
        assert_eq!(asset.template().nodes().len(), 2);
    }
}
