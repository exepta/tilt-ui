use crate::{AnimationName, StyleDeclaration};

/// Stores one normalized keyframe block.
#[derive(Debug, Clone)]
pub struct Keyframe {
    /// Offset normalized to the inclusive range `0.0..=1.0`.
    pub offset: f32,
    /// Typed declarations evaluated by the motion runtime.
    pub declarations: Vec<StyleDeclaration>,
    /// Stable source order for duplicate offsets.
    pub source_order: usize,
}

/// Stores a stylesheet-local keyframe animation definition.
#[derive(Debug, Clone)]
pub struct KeyframesRule {
    /// Animation name local to the owning stylesheet.
    pub name: AnimationName,
    /// Sorted normalized keyframes.
    pub frames: Vec<Keyframe>,
}
