//! Persistent trigger and popup anatomy for the TiltUI ChoiceBox.

use bevy::{
    ecs::{component::Component, entity::Entity, world::World},
    text::{LineBreak, TextLayout},
    ui::{GlobalZIndex, Val, widget::Text},
};

use crate::{ControlChecked, ControlPartKind, TiltElement, WidgetLayoutOverride};
use tilt_ui_core::{ElementKind, TemplateAttribute};

use super::{spawn_part, spawn_text_part};

/// Stores the popup and value text belonging to one ChoiceBox.
#[derive(Component, Debug, Clone, Copy)]
pub struct ChoiceBoxParts {
    /// Text part showing the selected option.
    pub value: Entity,
    /// Popup container holding authored option entities.
    pub popup: Entity,
    /// Whether the popup is currently visible.
    pub open: bool,
}

pub(crate) fn materialize(world: &mut World, entity: Entity, attributes: &[TemplateAttribute]) {
    let value = spawn_text_part(world, entity, ControlPartKind::Value, "Select");
    world
        .entity_mut(value)
        .insert(TextLayout::linebreak(LineBreak::NoWrap));
    let indicator = spawn_text_part(world, entity, ControlPartKind::Indicator, "▾");
    let popup = spawn_part(world, entity, ControlPartKind::Popup);
    let visible_items = crate::component::static_attribute_value(attributes, "max-visible-items")
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(3);
    let layout = WidgetLayoutOverride {
        max_height: Some(Val::Px(f32::from(visible_items) * 32.0 + 2.0)),
        ..Default::default()
    };
    if let Some(mut node) = world.get_mut::<bevy::ui::Node>(popup) {
        layout.apply(&mut node);
    }
    world.entity_mut(popup).insert(layout);
    world.entity_mut(popup).insert(GlobalZIndex(100));
    crate::widgets::state::set_widget_display(world, popup, false);
    for part in [value, indicator, popup] {
        world.entity_mut(entity).add_child(part);
    }
    world.entity_mut(entity).insert(ChoiceBoxParts {
        value,
        popup,
        open: false,
    });
}

/// Opens or closes a ChoiceBox without emitting a user-originated event.
pub fn set_choice_open(world: &mut World, entity: Entity, open: bool) -> bool {
    let Some(mut parts) = world.get_mut::<ChoiceBoxParts>(entity) else {
        return false;
    };
    if parts.open == open {
        return false;
    }
    parts.open = open;
    let popup = parts.popup;
    if let Some(mut state) = world.get_mut::<crate::ElementState>(entity) {
        state.open = open;
    }
    crate::widgets::state::set_widget_display(world, popup, open);
    true
}

pub(crate) fn finish(world: &mut World, entity: Entity) {
    let Some(parts) = world.get::<ChoiceBoxParts>(entity).copied() else {
        return;
    };
    let options = world
        .get::<bevy::ecs::hierarchy::Children>(entity)
        .map(|children| {
            children
                .iter()
                .copied()
                .filter(|child| {
                    world
                        .get::<TiltElement>(*child)
                        .is_some_and(|item| item.kind == ElementKind::Option)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !options.iter().any(|option| {
        world
            .get::<ControlChecked>(*option)
            .is_some_and(|value| value.0)
    }) && let Some(first) = options.first()
    {
        crate::set_control_checked(world, *first, true);
    }
    for option in options {
        world.entity_mut(parts.popup).add_child(option);
    }
    refresh_value(world, entity);
}

pub(crate) fn refresh_value(world: &mut World, entity: Entity) {
    let Some(parts) = world.get::<ChoiceBoxParts>(entity).copied() else {
        return;
    };
    let selected = world
        .get::<bevy::ecs::hierarchy::Children>(parts.popup)
        .and_then(|children| {
            children.iter().copied().find(|child| {
                world
                    .get::<ControlChecked>(*child)
                    .is_some_and(|checked| checked.0)
            })
        })
        .and_then(|option| world.get::<super::option::OptionData>(option))
        .map(|option| option.label.clone())
        .unwrap_or_else(|| "Select".into());
    if let Some(mut text) = world.get_mut::<Text>(parts.value)
        && text.0 != selected
    {
        text.0 = selected;
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        ecs::world::World,
        text::{LineBreak, TextLayout},
        ui::{Node, Val, widget::Text},
    };
    use tilt_ui_core::TemplateAttribute;

    use super::{ChoiceBoxParts, materialize};

    #[test]
    fn popup_limits_visible_options_and_uses_a_supported_indicator() {
        let mut world = World::new();
        let choice = world.spawn(Node::default()).id();
        materialize(&mut world, choice, &[]);
        let parts = *world.get::<ChoiceBoxParts>(choice).unwrap();
        assert_eq!(
            world.get::<Node>(parts.popup).unwrap().max_height,
            Val::Px(98.0)
        );
        assert_eq!(
            world.get::<TextLayout>(parts.value).unwrap().linebreak,
            LineBreak::NoWrap
        );
        let indicator = world
            .get::<bevy::ecs::hierarchy::Children>(choice)
            .unwrap()
            .iter()
            .copied()
            .find(|child| world.get::<Text>(*child).is_some_and(|text| text.0 == "▾"));
        assert!(indicator.is_some());

        let custom = world.spawn(Node::default()).id();
        materialize(
            &mut world,
            custom,
            &[TemplateAttribute::Static {
                name: "max-visible-items".into(),
                value: "5".into(),
            }],
        );
        let popup = world.get::<ChoiceBoxParts>(custom).unwrap().popup;
        assert_eq!(world.get::<Node>(popup).unwrap().max_height, Val::Px(162.0));
    }
}
