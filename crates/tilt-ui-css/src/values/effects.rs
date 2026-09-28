use crate::{CssColor, Length};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CssBoxShadow {
    pub x: Length,
    pub y: Length,
    pub blur: Length,
    pub spread: Length,
    pub color: CssColor,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CssTextShadow {
    pub x: Length,
    pub y: Length,
    pub color: CssColor,
}

/// Filters applied once to an image asset before it is uploaded for a CSS background.
/// Integer parameters make identical filter chains reusable across elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackgroundEffect {
    Blur(u8),
    Grayscale(u8),
    OilPaint(u8),
    Contrast(u16),
    Invert(u8),
}

/// Time-dependent GPU treatment applied to the pixels occupied by an element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnimatedEffectKind {
    Noise,
    RetroTv,
    OldFilm,
    SideGlow,
    Bloom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnimatedEffect {
    pub kind: AnimatedEffectKind,
    /// 0..=100, corresponding to 0..=1 in CSS.
    pub strength: u8,
    /// 0..=400, corresponding to 0..=4 in CSS.
    pub speed: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectQuality {
    Auto,
    Low,
    Medium,
    High,
}
