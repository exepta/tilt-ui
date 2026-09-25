//! Value formatting and persistent text for the TiltUI badge element.

use bevy::{
    ecs::{component::Component, entity::Entity, hierarchy::ChildOf, world::World},
    ui::{Node, PositionType, Val, widget::Text},
};
use tilt_ui_core::{BadgeAnchor, TemplateAttribute};

use crate::{ComponentElementIds, ComponentStyleOwner, ControlPartKind};

/// Target corner applied after normal CSS layout values.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BadgeAnchorPlacement(pub BadgeAnchor);

impl BadgeAnchorPlacement {
    pub(crate) fn apply(self, node: &mut Node) {
        node.position_type = PositionType::Absolute;
        node.top = Val::Auto;
        node.right = Val::Auto;
        node.bottom = Val::Auto;
        node.left = Val::Auto;
        match self.0 {
            BadgeAnchor::TopLeft => {
                node.top = Val::Px(-8.0);
                node.left = Val::Px(-8.0);
            }
            BadgeAnchor::TopRight => {
                node.top = Val::Px(-8.0);
                node.right = Val::Px(-8.0);
            }
            BadgeAnchor::BottomLeft => {
                node.bottom = Val::Px(-8.0);
                node.left = Val::Px(-8.0);
            }
            BadgeAnchor::BottomRight => {
                node.bottom = Val::Px(-8.0);
                node.right = Val::Px(-8.0);
            }
        }
    }
}

/// Stores badge value and static anchoring metadata.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct BadgeValue {
    /// Current numeric value.
    pub value: u32,
    /// Highest value displayed without a plus suffix.
    pub max: Option<u32>,
    /// Optional component-local target ID.
    pub target: Option<String>,
    /// Requested target corner.
    pub anchor: Option<String>,
    text: Entity,
}

fn label(value: u32, max: Option<u32>) -> String {
    match max {
        Some(max) if value > max => format!("{max}+"),
        _ => value.to_string(),
    }
}

pub(crate) fn materialize(world: &mut World, owner: Entity, attributes: &[TemplateAttribute]) {
    let value = crate::component::static_attribute_value(attributes, "value")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let max = crate::component::static_attribute_value(attributes, "max")
        .and_then(|value| value.parse().ok());
    let text = crate::widgets::controls::spawn_text_part(
        world,
        owner,
        ControlPartKind::Value,
        label(value, max),
    );
    world.entity_mut(owner).add_child(text);
    world.entity_mut(owner).insert(BadgeValue {
        value,
        max,
        target: crate::component::static_attribute_value(attributes, "for").map(str::to_owned),
        anchor: crate::component::static_attribute_value(attributes, "anchor").map(str::to_owned),
        text,
    });
}

pub(crate) fn resolve_targets(world: &mut World, scope: Entity) {
    let badges = {
        let mut query = world.query::<(Entity, &BadgeValue, &ComponentStyleOwner)>();
        query
            .iter(world)
            .filter(|(_, _, owner)| owner.0 == scope)
            .filter_map(|(entity, badge, _)| {
                badge
                    .target
                    .as_ref()
                    .map(|target| (entity, target.clone(), badge.anchor.clone()))
            })
            .collect::<Vec<_>>()
    };
    for (badge, target_id, anchor) in badges {
        let Some(target) = world
            .get::<ComponentElementIds>(scope)
            .and_then(|ids| ids.get(&target_id))
        else {
            continue;
        };
        if target == badge || is_ancestor(world, badge, target) {
            continue;
        }
        world.entity_mut(target).add_child(badge);
        let anchor = match anchor.as_deref() {
            Some("top-left") => BadgeAnchor::TopLeft,
            Some("bottom-left") => BadgeAnchor::BottomLeft,
            Some("bottom-right") => BadgeAnchor::BottomRight,
            _ => BadgeAnchor::TopRight,
        };
        world.entity_mut(badge).insert(BadgeAnchorPlacement(anchor));
        if let Some(mut node) = world.get_mut::<Node>(badge) {
            BadgeAnchorPlacement(anchor).apply(&mut node);
        }
    }
}

fn is_ancestor(world: &World, ancestor: Entity, mut node: Entity) -> bool {
    while let Some(parent) = world.get::<ChildOf>(node) {
        node = parent.parent();
        if node == ancestor {
            return true;
        }
    }
    false
}

/// Changes a badge value without replacing its text entity.
pub fn set_badge_value(world: &mut World, entity: Entity, value: u32) -> bool {
    let Some(mut badge) = world.get_mut::<BadgeValue>(entity) else {
        return false;
    };
    if badge.value == value {
        return false;
    }
    badge.value = value;
    let text = badge.text;
    let label = label(value, badge.max);
    if let Some(mut displayed) = world.get_mut::<Text>(text) {
        displayed.0 = label;
    }
    true
}

#[cfg(test)]
mod tests {
    use bevy::{
        ecs::{hierarchy::ChildOf, world::World},
        ui::{Node, Val},
    };

    use super::*;

    #[test]
    fn resolves_component_local_target_and_positions_badge() {
        let mut world = World::new();
        let scope = world.spawn(ComponentElementIds::default()).id();
        let target = world.spawn(Node::default()).id();
        world
            .get_mut::<ComponentElementIds>(scope)
            .unwrap()
            .insert("avatar".into(), target);
        let text = world.spawn(Text::new("3")).id();
        let badge = world
            .spawn((
                Node::default(),
                ComponentStyleOwner(scope),
                BadgeValue {
                    value: 3,
                    max: None,
                    target: Some("avatar".into()),
                    anchor: Some("bottom-left".into()),
                    text,
                },
            ))
            .id();
        world.entity_mut(scope).add_children(&[target, badge]);
        resolve_targets(&mut world, scope);
        assert_eq!(world.get::<ChildOf>(badge).unwrap().parent(), target);
        let node = world.get::<Node>(badge).unwrap();
        assert_eq!(node.bottom, Val::Px(-8.0));
        assert_eq!(node.left, Val::Px(-8.0));
    }
}
