//! Semantic destinations and activation events for TiltUI hyperlinks.

use bevy::ecs::{
    component::Component,
    entity::Entity,
    hierarchy::Children,
    message::{Message, MessageReader, MessageWriter},
    system::Query,
    world::World,
};
use tilt_ui_core::TemplateAttribute;

use crate::ControlActivated;
use crate::{ControlPartKind, widgets::controls::spawn_text_part};

/// Stores the destination retained by a hyperlink control.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct LinkTarget(pub String);

#[derive(Component)]
struct LinkIcon(Entity);

/// Notification emitted when a hyperlink is activated.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct LinkActivated {
    /// Hyperlink entity.
    pub entity: Entity,
    /// Authored destination.
    pub href: String,
}

pub(crate) fn materialize(world: &mut World, entity: Entity, attributes: &[TemplateAttribute]) {
    let href = crate::component::static_attribute_value(attributes, "href")
        .unwrap_or_default()
        .to_owned();
    world.entity_mut(entity).insert(LinkTarget(href));
    let icon = match crate::component::static_attribute_value(attributes, "icon") {
        Some("none") => None,
        Some("external") | None => Some("➚"),
        Some("arrow-right") => Some("➜"),
        Some("download") => Some("⬇"),
        Some(value) if value.trim().is_empty() => None,
        Some(value) => Some(value),
    };
    if let Some(icon) = icon {
        let part = spawn_text_part(world, entity, ControlPartKind::Indicator, icon);
        world.entity_mut(entity).add_child(part);
        world.entity_mut(entity).insert(LinkIcon(part));
    }
}

pub(crate) fn finalize_icons(world: &mut World) {
    let links = {
        let mut query = world.query::<(Entity, &LinkIcon)>();
        query
            .iter(world)
            .map(|(link, icon)| (link, icon.0))
            .collect::<Vec<_>>()
    };
    for (link, icon) in links {
        let Some(children) = world.get::<Children>(link) else {
            continue;
        };
        let mut ordered = children
            .iter()
            .copied()
            .filter(|child| *child != icon)
            .collect::<Vec<_>>();
        ordered.push(icon);
        world.entity_mut(link).replace_children(&ordered);
        world.entity_mut(link).remove::<LinkIcon>();
    }
}

/// Updates an existing hyperlink destination without emitting activation.
pub fn set_link_href(world: &mut World, entity: Entity, href: impl Into<String>) -> bool {
    let href = href.into();
    let Some(mut target) = world.get_mut::<LinkTarget>(entity) else {
        return false;
    };
    if target.0 == href {
        return false;
    }
    target.0 = href;
    true
}

pub(crate) fn process_link_activation(
    mut activated: MessageReader<ControlActivated>,
    targets: Query<&LinkTarget>,
    mut links: MessageWriter<LinkActivated>,
) {
    for activation in activated.read() {
        let Ok(target) = targets.get(activation.entity) else {
            continue;
        };
        if target.0.is_empty() {
            continue;
        }
        links.write(LinkActivated {
            entity: activation.entity,
            href: target.0.clone(),
        });
        open_external(&target.0);
    }
}

fn open_external(href: &str) {
    if !(href.starts_with("https://") || href.starts_with("http://")) {
        return;
    }
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(href).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(href).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("rundll32")
        .arg("url.dll,FileProtocolHandler")
        .arg(href)
        .spawn();
}

#[cfg(test)]
mod tests {
    use bevy::ecs::{hierarchy::Children, world::World};
    use bevy::ui::widget::Text;
    use tilt_ui_core::TemplateAttribute;

    use super::{finalize_icons, materialize};

    #[test]
    fn default_icon_follows_link_text_and_none_omits_it() {
        let mut world = World::new();
        let link = world.spawn_empty().id();
        materialize(&mut world, link, &[]);
        let label = world.spawn(Text::new("Open in new tab")).id();
        world.entity_mut(link).add_child(label);
        finalize_icons(&mut world);
        let children = world.get::<Children>(link).unwrap();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0], label);
        assert_eq!(world.get::<Text>(children[1]).unwrap().0, "➚");

        let no_icon = world.spawn_empty().id();
        materialize(
            &mut world,
            no_icon,
            &[TemplateAttribute::Static {
                name: "icon".into(),
                value: "none".into(),
            }],
        );
        assert!(world.get::<Children>(no_icon).is_none());
    }
}
