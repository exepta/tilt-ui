use thiserror::Error;
use tilt_ui_html::TemplateParseError;

/// Describes a failure while loading a TiltUI `.component.html` asset.
#[derive(Debug, Error)]
pub enum UiTemplateAssetLoaderError {
    /// The asset reader could not read the source bytes.
    #[error("failed to read TiltUI template asset: {0}")]
    Io(#[from] std::io::Error),

    /// The source bytes are not valid UTF-8 text.
    #[error("TiltUI template asset is not valid UTF-8: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    /// The template source could not be parsed into the core template model.
    #[error("failed to parse TiltUI template asset: {0}")]
    TemplateParse(#[from] TemplateParseError),
}
