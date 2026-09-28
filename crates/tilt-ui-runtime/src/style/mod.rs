//! Component-scoped CSS matching, cascade resolution, and Bevy UI application.

mod animated;
mod apply;
mod backdrop;
mod background;
mod cascade;
pub(crate) mod convert;
mod inline;
mod matcher;
mod motion;
mod plugin;
mod state;

pub(crate) use inline::InlineStyle;
pub use motion::ActiveAnimations;
pub(crate) use plugin::author_style_pending;
pub use plugin::{TiltUiMediaEnvironment, TiltUiStyleRuntimePlugin, TiltUiStyleRuntimeSet};
pub use state::{CascadedStyle, RuntimeComputedStyle};
