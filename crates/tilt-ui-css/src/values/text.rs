/// Identifies a font family supported by the TiltUI text renderer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontFamily {
    /// Uses the bundled proportional UI typeface.
    SansSerif,
    /// Uses the bundled UI symbol face.
    UiSymbols,
    /// Uses Bevy's default monospace typeface.
    Monospace,
    Serif,
    Cursive,
    Fantasy,
    SystemUi,
    Emoji,
    /// Resolves a named family from Bevy's font database.
    Named(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CssLineHeight {
    Normal,
    Pixels(f32),
    Relative(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextWrap {
    Wrap,
    NoWrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextTransform {
    None,
    Uppercase,
    Lowercase,
    Capitalize,
}

/// Describes the supported CSS font-weight values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    /// Uses the normal font face weight.
    Normal,
    /// Uses the bold font face weight.
    Bold,
    /// Uses an explicit numeric font weight.
    Number(u16),
}

/// Describes horizontal alignment of inline text content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    /// Aligns text to the start edge.
    Start,
    /// Aligns text to the end edge.
    End,
    /// Centers text.
    Center,
    /// Justifies text across each line.
    Justify,
}
