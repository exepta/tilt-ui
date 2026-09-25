//! Component-scoped CSS matching, cascade resolution, and Bevy UI application.

mod apply;
mod cascade;
pub(crate) mod convert;
mod matcher;
mod motion;
mod plugin;
mod state;

pub use motion::ActiveAnimations;
pub use plugin::{TiltUiMediaEnvironment, TiltUiStyleRuntimePlugin};
pub use state::{CascadedStyle, RuntimeComputedStyle};
