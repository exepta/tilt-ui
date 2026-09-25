use thiserror::Error;
use tilt_ui_css::StyleParseError;

/// Describes a failure while loading a TiltUI `.component.css` asset.
#[derive(Debug, Error)]
pub enum UiStyleSheetAssetLoaderError {
    /// The asset reader could not read the source bytes.
    #[error("failed to read TiltUI stylesheet asset: {0}")]
    Io(#[from] std::io::Error),

    /// The source bytes are not valid UTF-8 text.
    #[error("TiltUI stylesheet asset is not valid UTF-8: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    /// The stylesheet source could not be parsed into the typed stylesheet model.
    #[error("failed to parse TiltUI stylesheet asset: {0}")]
    StyleParse(#[from] StyleParseError),
}
