//! A standalone, CSS-stylable loading indicator.

use bevy::ecs::component::Component;

/// Marker placed on a materialized `<spinner />` element.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TiltSpinner;
