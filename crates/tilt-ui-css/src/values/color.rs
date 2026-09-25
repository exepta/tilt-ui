/// Represents an RGBA CSS color with normalized channel values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CssColor {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

impl CssColor {
    /// Creates a normalized RGBA color value.
    pub const fn rgba(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    /// Returns a fully transparent color.
    pub const fn transparent() -> Self {
        Self::rgba(0.0, 0.0, 0.0, 0.0)
    }
}
