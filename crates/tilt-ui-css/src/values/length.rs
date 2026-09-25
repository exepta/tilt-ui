/// Stores a CSS length without resolving it to physical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Length {
    /// Uses the property's automatic layout behavior.
    Auto,
    /// A length in CSS pixels.
    Px(f32),
    /// A length relative to the relevant containing dimension.
    Percent(f32),
    /// A length relative to the viewport width.
    Vw(f32),
    /// A length relative to the viewport height.
    Vh(f32),
}
