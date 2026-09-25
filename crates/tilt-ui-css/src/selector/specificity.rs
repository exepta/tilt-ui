/// Stores a selector specificity value calculated by Servo's `selectors` crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Specificity(pub u32);

impl Specificity {
    /// Returns the count of ID-like selector components.
    pub const fn id_count(self) -> u16 {
        (self.0 >> 20) as u16
    }

    /// Returns the count of class-like selector components.
    pub const fn class_count(self) -> u16 {
        ((self.0 >> 10) & 0x03ff) as u16
    }

    /// Returns the count of type-like selector components.
    pub const fn type_count(self) -> u16 {
        (self.0 & 0x03ff) as u16
    }
}
