use bevy::{
    ecs::{entity::Entity, message::Message},
    reflect::Reflect,
};

/// Notification emitted when a control completes an activation gesture.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub struct ControlActivated {
    /// Entity that completed the activation gesture.
    pub entity: Entity,
}
