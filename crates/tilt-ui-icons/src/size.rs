//! Supported raster output sizes.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconSize {
    Px16,
    Px32,
    Px64,
}

impl IconSize {
    pub const fn pixels(self) -> u32 {
        match self {
            Self::Px16 => 16,
            Self::Px32 => 32,
            Self::Px64 => 64,
        }
    }

    pub const fn from_pixels(value: u32) -> Option<Self> {
        match value {
            16 => Some(Self::Px16),
            32 => Some(Self::Px32),
            64 => Some(Self::Px64),
            _ => None,
        }
    }
}
