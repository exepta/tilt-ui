//! Runtime semantics specific to the TiltUI button element.

use bevy::ecs::component::Component;

/// Marks a materialized TiltUI control with native button semantics.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TiltButton;
