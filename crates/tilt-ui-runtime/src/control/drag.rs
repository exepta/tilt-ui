//! Active pointer gesture state shared by numeric and resize controls.

use bevy::{
    ecs::{component::Component, entity::Entity},
    math::Vec2,
};
use bevy_picking::pointer::PointerId;

/// The active pointer gesture of one TiltUI control.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct ActiveControlDrag {
    /// Pointer that owns the gesture.
    pub pointer: PointerId,
    /// Widget-specific operation selected when the gesture began.
    pub kind: DragKind,
}

/// Describes the stable operation performed throughout a pointer drag.
#[derive(Debug, Clone, Copy)]
pub(crate) enum DragKind {
    /// Changes one thumb of a Slider.
    Slider { thumb: u8 },
    /// Changes a color through a persistent canvas or gradient track.
    Color { surface: Entity },
    /// Changes the dimensions of a TextArea.
    TextAreaResize {
        initial_size: Vec2,
        initial_pointer: Vec2,
    },
}
