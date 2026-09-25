//! Dismisses persistent popup controls when a pointer press lands outside them.

use bevy::ecs::{
    entity::Entity, hierarchy::ChildOf, message::MessageReader, system::Commands, world::World,
};
use bevy_picking::{
    events::{Pointer, Press},
    pointer::PointerButton,
};

use crate::widgets::{
    advanced::{
        color_picker::{ColorPickerState, set_color_picker_open},
        date_picker::{DatePickerState, DatePickerTrigger, set_date_picker_open},
    },
    controls::choice_box::{ChoiceBoxParts, set_choice_open},
};

pub(crate) fn dismiss_popups_on_press(
    mut presses: Option<MessageReader<Pointer<Press>>>,
    mut commands: Commands,
) {
    let Some(presses) = presses.as_mut() else {
        return;
    };
    for press in presses.read() {
        if press.button != PointerButton::Primary {
            continue;
        }
        let target = press.entity;
        commands.queue(move |world: &mut World| dismiss_outside(world, target));
    }
}

fn popup_owner(world: &World, target: Entity) -> Option<Entity> {
    let mut current = Some(target);
    while let Some(entity) = current {
        if world.get::<ChoiceBoxParts>(entity).is_some()
            || world.get::<DatePickerState>(entity).is_some()
            || world.get::<ColorPickerState>(entity).is_some()
        {
            return Some(entity);
        }
        if let Some(trigger) = world.get::<DatePickerTrigger>(entity) {
            return Some(trigger.0);
        }
        current = world.get::<ChildOf>(entity).map(|parent| parent.parent());
    }
    None
}

fn dismiss_outside(world: &mut World, target: Entity) {
    let inside = popup_owner(world, target);
    let open = {
        let mut choices = world.query::<(Entity, &ChoiceBoxParts)>();
        let mut dates = world.query::<(Entity, &DatePickerState)>();
        let mut colors = world.query::<(Entity, &ColorPickerState)>();
        let mut open = Vec::new();
        open.extend(
            choices
                .iter(world)
                .filter_map(|(entity, parts)| parts.open.then_some(entity)),
        );
        open.extend(
            dates
                .iter(world)
                .filter_map(|(entity, state)| state.open.then_some(entity)),
        );
        open.extend(
            colors
                .iter(world)
                .filter_map(|(entity, state)| state.open.then_some(entity)),
        );
        open
    };
    for entity in open.into_iter().filter(|entity| Some(*entity) != inside) {
        if world.get::<ChoiceBoxParts>(entity).is_some() {
            set_choice_open(world, entity, false);
        } else if world.get::<DatePickerState>(entity).is_some() {
            set_date_picker_open(world, entity, false);
        } else {
            set_color_picker_open(world, entity, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        app::{App, Update},
        camera::NormalizedRenderTarget,
        ecs::{hierarchy::Children, world::World},
        math::Vec2,
        ui::Node,
    };
    use bevy_picking::{
        backend::HitData,
        events::{Pointer, Press},
        pointer::{Location, PointerButton, PointerId},
    };

    use super::{dismiss_outside, dismiss_popups_on_press};
    use crate::widgets::{
        advanced::color_picker::{
            ColorPickerState, materialize as materialize_color, set_color_picker_open,
        },
        advanced::date_picker::{
            DatePickerState, DatePickerTrigger, materialize as materialize_date,
            set_date_picker_open,
        },
        controls::choice_box::{
            ChoiceBoxParts, materialize as materialize_choice, set_choice_open,
        },
    };
    use crate::{ControlPart, ControlPartKind};

    #[test]
    fn pointer_press_message_dismisses_open_choice() {
        let mut app = App::new();
        app.add_message::<Pointer<Press>>()
            .add_systems(Update, dismiss_popups_on_press);
        let choice = app.world_mut().spawn(Node::default()).id();
        materialize_choice(app.world_mut(), choice, &[]);
        set_choice_open(app.world_mut(), choice, true);
        let outside = app.world_mut().spawn(Node::default()).id();
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<Pointer<Press>>>()
            .write(Pointer::new(
                PointerId::Mouse,
                Location {
                    target: NormalizedRenderTarget::None {
                        width: 100,
                        height: 100,
                    },
                    position: Vec2::ZERO,
                },
                Press {
                    button: PointerButton::Primary,
                    hit: HitData::new(outside, 0.0, None, None),
                    count: 1,
                },
                outside,
            ));
        app.update();
        assert!(!app.world().get::<ChoiceBoxParts>(choice).unwrap().open);
    }

    #[test]
    fn press_inside_keeps_popup_open_and_press_outside_closes_it() {
        let mut world = World::new();
        let choice = world.spawn(Node::default()).id();
        materialize_choice(&mut world, choice, &[]);
        let choice_popup = world.get::<ChoiceBoxParts>(choice).unwrap().popup;
        let option = world.spawn(Node::default()).id();
        world.entity_mut(choice_popup).add_child(option);
        let date = world.spawn(Node::default()).id();
        materialize_date(&mut world, date, &[]);
        let date_popup = world
            .get::<Children>(date)
            .unwrap()
            .iter()
            .copied()
            .find(|child| {
                world
                    .get::<ControlPart>(*child)
                    .is_some_and(|part| part.kind == ControlPartKind::Popup)
            })
            .unwrap();
        let trigger = world.spawn((Node::default(), DatePickerTrigger(date))).id();
        let color = world.spawn(Node::default()).id();
        materialize_color(&mut world, color, &[]);
        let color_popup = world
            .get::<Children>(color)
            .unwrap()
            .iter()
            .copied()
            .find(|child| {
                world
                    .get::<ControlPart>(*child)
                    .is_some_and(|part| part.kind == ControlPartKind::Popup)
            })
            .unwrap();
        let outside = world.spawn(Node::default()).id();

        set_choice_open(&mut world, choice, true);
        dismiss_outside(&mut world, option);
        assert!(world.get::<ChoiceBoxParts>(choice).unwrap().open);

        set_date_picker_open(&mut world, date, true);
        dismiss_outside(&mut world, date_popup);
        assert!(!world.get::<ChoiceBoxParts>(choice).unwrap().open);
        assert!(world.get::<DatePickerState>(date).unwrap().open);
        dismiss_outside(&mut world, trigger);
        assert!(world.get::<DatePickerState>(date).unwrap().open);
        dismiss_outside(&mut world, outside);
        assert!(!world.get::<DatePickerState>(date).unwrap().open);

        set_color_picker_open(&mut world, color, true);
        dismiss_outside(&mut world, color_popup);
        assert!(world.get::<ColorPickerState>(color).unwrap().open);
        dismiss_outside(&mut world, outside);
        assert!(!world.get::<ColorPickerState>(color).unwrap().open);
    }
}
