use bevy::prelude::Vec2;

/// Demonstrates ordinary Rust logic compiled from a reusable component source file.
pub struct HeaderLayout {
    /// Preferred header size in logical UI units.
    pub size: Vec2,
}

/// Returns the default layout used by the example header.
pub fn default_header_layout() -> HeaderLayout {
    HeaderLayout {
        size: Vec2::new(320.0, 48.0),
    }
}
