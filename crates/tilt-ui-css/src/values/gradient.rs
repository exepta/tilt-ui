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
