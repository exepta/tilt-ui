//! Native TiltUI control element definitions.

pub mod button;
pub mod checkbox;
pub mod choice_box;
pub mod input;
pub mod list_box;
pub mod option;
pub mod radio_button;
pub mod slider;
pub mod switch_button;
pub mod text_area;
pub mod toggle_button;

pub use button::TiltButton;
pub use checkbox::TiltCheckbox;
pub use radio_button::TiltRadioButton;
pub use switch_button::TiltSwitchButton;
pub use toggle_button::TiltToggleButton;

use bevy::{
    ecs::{entity::Entity, world::World},
    ui::{Node, widget::Text},
};
use bevy_picking::Pickable;

use crate::{ComponentStyleOwner, ControlPart, ControlPartKind};

pub(crate) fn spawn_part(world: &mut World, owner: Entity, kind: ControlPartKind) -> Entity {
    let scope = world.get::<ComponentStyleOwner>(owner).copied();
    let part = world
        .spawn((
            Node::default(),
            Pickable::IGNORE,
            ControlPart { owner, kind },
        ))
        .id();
    if let Some(scope) = scope {
        world.entity_mut(part).insert(scope);
    }
    part
}

pub(crate) fn spawn_text_part(
    world: &mut World,
    owner: Entity,
    kind: ControlPartKind,
    value: impl Into<String>,
) -> Entity {
    let part = spawn_part(world, owner, kind);
    world.entity_mut(part).insert(Text::new(value));
    part
}
