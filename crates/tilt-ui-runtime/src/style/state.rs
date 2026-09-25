use bevy::ecs::component::Component;
use tilt_ui_css::ComputedStyle;

/// Stores the selector-cascaded CSS values before inheritance is resolved.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct CascadedStyle(pub ComputedStyle);

/// Stores the resolved CSS values that apply to a materialized TiltUI node.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub struct RuntimeComputedStyle(pub ComputedStyle);
