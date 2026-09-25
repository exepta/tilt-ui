//! Runtime semantics specific to the TiltUI toggle-button element.

use bevy::ecs::component::Component;

/// Identifies a materialized toggle button control.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TiltToggleButton;
