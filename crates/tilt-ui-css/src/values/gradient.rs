use super::CssColor;

/// Cardinal direction of a two-or-more-stop CSS linear gradient.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GradientDirection {
    Up,
    Right,
    Down,
    Left,
}

/// Equally spaced color stops for a linear background image.
#[derive(Debug, Clone, PartialEq)]
pub struct CssGradient {
    pub direction: GradientDirection,
    pub stops: Vec<CssColor>,
}

/// Image source supported by the UI background renderer.
#[derive(Debug, Clone, PartialEq)]
pub enum CssBackgroundImage {
    LinearGradient(CssGradient),
    Url(String),
}

/// How a background image fills the element's box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundSize {
    Stretch,
    Cover,
    Contain,
}

/// Position of an image within the available background area, from 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BackgroundPosition {
    pub x: f32,
    pub y: f32,
}

impl Default for BackgroundPosition {
    fn default() -> Self {
        Self { x: 0.5, y: 0.5 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundAttachment {
    Scroll,
    Fixed,
}
