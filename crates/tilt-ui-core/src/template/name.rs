use std::fmt;

/// Identifies a user-defined TiltUI component, such as `<app-header />`.
///
/// Component names are distinct from built-in element kinds and do not refer
/// to runtime entities.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ComponentName(String);

impl ComponentName {
    /// Creates a component name without applying selector or tag validation.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Returns the component name as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ComponentName {
    fn from(name: String) -> Self {
        Self::new(name)
    }
}

impl From<&str> for ComponentName {
    fn from(name: &str) -> Self {
        Self::new(name)
    }
}

impl AsRef<str> for ComponentName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ComponentName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
