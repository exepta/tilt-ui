use bevy::ecs::component::Component;

/// Marks a materialized element as an interactive TiltUI control.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TiltControl;

/// Stores the normal sequential tab index of a TiltUI control.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlTabIndex(pub i32);
