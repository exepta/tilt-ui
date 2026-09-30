//! Shared option selection for ChoiceBox and ListBox controls.

use bevy::{
    ecs::{
        entity::Entity,
        hierarchy::ChildOf,
        message::{Message, MessageReader, Messages},
        system::Commands,
        world::World,
    },
    input::{
        ButtonState,
        keyboard::{KeyCode, KeyboardInput},
    },
    ui::InteractionDisabled,
};
use bevy_input_focus::{FocusCause, InputFocus};
use tilt_ui_core::ElementKind;

use crate::{
    ControlActivated, ControlChecked, TiltElement,
    widgets::controls::{
        choice_box::{ChoiceBoxParts, refresh_value, set_choice_open},
        list_box::ListBoxMode,
        option::OptionData,
    },
};

/// Notification emitted when user interaction changes an option selection.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct OptionSelectionChanged {
    /// ChoiceBox or ListBox that owns the option.
    pub control: Entity,
    /// Option whose checked value changed.
    pub option: Entity,
    /// Semantic value of the option.
    pub value: String,
    /// New checked state.
    pub selected: bool,
}

fn option_owner(world: &World, option: Entity) -> Option<Entity> {
    let mut current = option;
    while let Some(parent) = world.get::<ChildOf>(current) {
        current = parent.parent();
        if world.get::<TiltElement>(current).is_some_and(|element| {
            matches!(element.kind, ElementKind::ChoiceBox | ElementKind::ListBox)
        }) {
            return Some(current);
        }
    }
    None
}

fn options(world: &World, control: Entity) -> Vec<Entity> {
    let parent = world
        .get::<ChoiceBoxParts>(control)
        .map_or(control, |parts| parts.popup);
    crate::widgets::controls::option::descendants(world, parent)
}

fn change_option(
    world: &mut World,
    control: Entity,
    option: Entity,
    selected: bool,
    user: bool,
) -> bool {
    if !crate::set_control_checked(world, option, selected) {
        return false;
    }
    if user && let Some(data) = world.get::<OptionData>(option) {
        let value = data.value.clone();
        world
            .resource_mut::<Messages<OptionSelectionChanged>>()
            .write(OptionSelectionChanged {
                control,
                option,
                value,
                selected,
            });
    }
    true
}

/// Selects or deselects an option without emitting user-originated messages.
pub fn set_option_selected(world: &mut World, option: Entity, selected: bool) -> bool {
    select_option(world, option, selected, false)
}

fn select_option(world: &mut World, option: Entity, selected: bool, user: bool) -> bool {
    let Some(control) = option_owner(world, option) else {
        return false;
    };
    let multiple = world
        .get::<ListBoxMode>(control)
        .is_some_and(|mode| mode.multiple);
    let mut changed = false;
    if selected && !multiple {
        for other in options(world, control)
            .into_iter()
            .filter(|other| *other != option)
        {
            changed |= change_option(world, control, other, false, user);
        }
    }
    changed |= change_option(world, control, option, selected, user);
    if world.get::<ChoiceBoxParts>(control).is_some() {
        refresh_value(world, control);
    }
    changed
}

pub(crate) fn process_option_activation(
    mut activated: MessageReader<ControlActivated>,
    mut commands: Commands,
) {
    for activation in activated.read() {
        let entity = activation.entity;
        commands.queue(move |world: &mut World| {
            let Some(kind) = world.get::<TiltElement>(entity).map(|element| element.kind) else {
                return;
            };
            match kind {
                ElementKind::ChoiceBox => {
                    let open = world
                        .get::<ChoiceBoxParts>(entity)
                        .is_some_and(|parts| !parts.open);
                    set_choice_open(world, entity, open);
                }
                ElementKind::Option => {
                    if world.get::<InteractionDisabled>(entity).is_some() {
                        return;
                    }
                    let Some(control) = option_owner(world, entity) else {
                        return;
                    };
                    if world.get::<InteractionDisabled>(control).is_some() {
                        return;
                    }
                    let selected = world
                        .get::<ControlChecked>(entity)
                        .is_some_and(|state| state.0);
                    let multiple = world
                        .get::<ListBoxMode>(control)
                        .is_some_and(|mode| mode.multiple);
                    select_option(world, entity, if multiple { !selected } else { true }, true);
                    if world.get::<ChoiceBoxParts>(control).is_some() {
                        set_choice_open(world, control, false);
                        if let Some(mut focus) = world.get_resource_mut::<InputFocus>() {
                            focus.set(control, FocusCause::Pressed);
                        }
                    }
                }
                _ => {}
            }
        });
    }
}

pub(crate) fn option_keyboard_input(
    mut keys: Option<MessageReader<KeyboardInput>>,
    focus: Option<bevy::ecs::system::Res<InputFocus>>,
    mut commands: Commands,
) {
    let Some(focused) = focus.and_then(|focus| focus.get()) else {
        return;
    };
    let Some(keys) = keys.as_mut() else {
        return;
    };
    for key in keys.read() {
        if key.state != ButtonState::Pressed {
            continue;
        }
        let direction = match key.key_code {
            KeyCode::ArrowDown => 1,
            KeyCode::ArrowUp => -1,
            KeyCode::Home => -2,
            KeyCode::End => 2,
            KeyCode::Escape => 0,
            _ => continue,
        };
        commands.queue(move |world: &mut World| {
            let kind = world
                .get::<TiltElement>(focused)
                .map(|element| element.kind);
            let control = match kind {
                Some(ElementKind::Option) => option_owner(world, focused),
                Some(ElementKind::ChoiceBox | ElementKind::ListBox) => Some(focused),
                _ => None,
            };
            let Some(control) = control else {
                return;
            };
            if world.get::<InteractionDisabled>(control).is_some() {
                return;
            }
            if direction == 0 {
                if world.get::<ChoiceBoxParts>(control).is_some() {
                    set_choice_open(world, control, false);
                    if let Some(mut focus) = world.get_resource_mut::<InputFocus>() {
                        focus.set(control, FocusCause::Pressed);
                    }
                }
                return;
            }
            let choices = options(world, control)
                .into_iter()
                .filter(|option| world.get::<InteractionDisabled>(*option).is_none())
                .collect::<Vec<_>>();
            if choices.is_empty() {
                return;
            }
            let current = choices
                .iter()
                .position(|option| *option == focused)
                .or_else(|| {
                    choices.iter().position(|option| {
                        world
                            .get::<ControlChecked>(*option)
                            .is_some_and(|checked| checked.0)
                    })
                });
            let next = match direction {
                -2 => 0,
                2 => choices.len() - 1,
                1 => current.map_or(0, |index| (index + 1).min(choices.len() - 1)),
                _ => current.map_or(choices.len() - 1, |index| index.saturating_sub(1)),
            };
            let option = choices[next];
            if world.get::<ChoiceBoxParts>(control).is_some()
                || !world
                    .get::<ListBoxMode>(control)
                    .is_some_and(|mode| mode.multiple)
            {
                select_option(world, option, true, true);
            }
            if world.get::<ListBoxMode>(control).is_some()
                && let Some(mut focus) = world.get_resource_mut::<InputFocus>()
            {
                focus.set(option, FocusCause::Pressed);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        app::App,
        ecs::message::{MessageCursor, Messages},
        input::{
            ButtonState,
            keyboard::{Key, KeyCode, KeyboardInput},
        },
    };
    use bevy_input_focus::{FocusCause, InputFocus};
    use tilt_ui_core::ElementKind;

    use crate::{
        ControlActivated, ControlChecked, ElementState, TiltElement, TiltUiControlRuntimePlugin,
        widgets::controls::{
            choice_box::{ChoiceBoxParts, set_choice_open},
            option::OptionData,
        },
    };

    use super::{OptionSelectionChanged, set_option_selected};

    #[test]
    fn choice_box_uses_authored_options_and_emits_precise_changes() {
        let mut app = App::new();
        app.add_plugins(TiltUiControlRuntimePlugin)
            .add_message::<KeyboardInput>();
        let choice = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::ChoiceBox,
                },
                ElementState::default(),
            ))
            .id();
        crate::widgets::controls::choice_box::materialize(app.world_mut(), choice, &[]);
        let a = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Option,
                },
                OptionData {
                    value: "a".into(),
                    label: "First".into(),
                },
                ControlChecked(true),
                ElementState {
                    checked: true,
                    ..Default::default()
                },
            ))
            .id();
        let b = app
            .world_mut()
            .spawn((
                TiltElement {
                    kind: ElementKind::Option,
                },
                OptionData {
                    value: "b".into(),
                    label: "Second".into(),
                },
                ControlChecked(false),
                ElementState::default(),
            ))
            .id();
        app.world_mut().entity_mut(choice).add_child(a).add_child(b);
        crate::widgets::controls::choice_box::finish(app.world_mut(), choice);
        let popup = app.world().get::<ChoiceBoxParts>(choice).unwrap().popup;
        assert!(
            app.world()
                .get::<bevy::ecs::hierarchy::ChildOf>(a)
                .is_some_and(|parent| parent.parent() == popup)
        );

        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<ControlActivated>>()
            .write(ControlActivated { entity: choice });
        app.update();
        assert!(app.world().get::<ChoiceBoxParts>(choice).unwrap().open);
        assert!(app.world().get::<ElementState>(choice).unwrap().open);

        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<ControlActivated>>()
            .write(ControlActivated { entity: b });
        app.update();
        assert!(!app.world().get::<ChoiceBoxParts>(choice).unwrap().open);
        assert!(!app.world().get::<ControlChecked>(a).unwrap().0);
        assert!(app.world().get::<ControlChecked>(b).unwrap().0);
        let mut cursor = MessageCursor::<OptionSelectionChanged>::default();
        let changes = cursor
            .read(
                app.world()
                    .resource::<bevy::ecs::message::Messages<OptionSelectionChanged>>(),
            )
            .collect::<Vec<_>>();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].option, a);
        assert_eq!(changes[1].option, b);

        assert!(set_option_selected(app.world_mut(), a, true));
        assert!(!app.world().get::<ControlChecked>(b).unwrap().0);
        assert!(!set_choice_open(app.world_mut(), choice, false));

        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(choice, FocusCause::Pressed);
        let window = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<Messages<KeyboardInput>>()
            .write(KeyboardInput {
                key_code: KeyCode::ArrowDown,
                logical_key: Key::ArrowDown,
                state: ButtonState::Pressed,
                text: None,
                repeat: false,
                window,
            });
        app.update();
        assert!(!app.world().get::<ControlChecked>(a).unwrap().0);
        assert!(app.world().get::<ControlChecked>(b).unwrap().0);
    }
}
