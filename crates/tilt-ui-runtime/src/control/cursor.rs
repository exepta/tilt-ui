//! Per-element window cursors and built-in control defaults.

use bevy::{
    ecs::{
        entity::Entity,
        hierarchy::ChildOf,
        query::With,
        system::{Commands, Query, Res},
    },
    ui::widget::Button,
    window::{CursorIcon, SystemCursorIcon, Window},
};
use bevy_picking::{hover::HoverMap, pointer::PointerId};
use tilt_ui_core::ElementKind;

use crate::TiltElement;

/// Overrides the cursor while this element or a descendant is hovered.
/// The closest explicit override wins, before built-in control defaults.
/// Supports both system icons and Bevy custom image cursors.
#[derive(bevy::prelude::Component, Debug, Clone, PartialEq, Eq)]
pub struct UiCursor(pub CursorIcon);

/// Sets an element cursor; pass `None` to restore inherited/default behavior.
pub fn set_ui_cursor(
    world: &mut bevy::prelude::World,
    entity: Entity,
    cursor: Option<CursorIcon>,
) -> bool {
    let Ok(mut target) = world.get_entity_mut(entity) else {
        return false;
    };
    if let Some(cursor) = cursor {
        target.insert(UiCursor(cursor));
    } else {
        target.remove::<UiCursor>();
    }
    true
}

fn cursor_for_target(
    mut entity: Entity,
    parents: &Query<&ChildOf>,
    elements: &Query<&TiltElement>,
    buttons: &Query<(), With<Button>>,
    cursors: &Query<&UiCursor>,
) -> CursorIcon {
    let mut ancestor = entity;
    loop {
        if let Ok(cursor) = cursors.get(ancestor) {
            return cursor.0.clone();
        }
        let Ok(parent) = parents.get(ancestor) else {
            break;
        };
        ancestor = parent.parent();
    }
    loop {
        if let Ok(element) = elements.get(entity) {
            match element.kind {
                ElementKind::Input | ElementKind::TextArea => return SystemCursorIcon::Text.into(),
                ElementKind::Button
                | ElementKind::ToggleButton
                | ElementKind::SwitchButton
                | ElementKind::HyperLink => return SystemCursorIcon::Pointer.into(),
                ElementKind::Checkbox | ElementKind::RadioButton | ElementKind::Slider => {
                    return SystemCursorIcon::Default.into();
                }
                _ => {}
            }
        }
        if buttons.get(entity).is_ok() {
            return SystemCursorIcon::Pointer.into();
        }
        let Ok(parent) = parents.get(entity) else {
            return SystemCursorIcon::Default.into();
        };
        entity = parent.parent();
    }
}

pub(super) fn update_ui_cursor(
    mut commands: Commands,
    hover: Option<Res<HoverMap>>,
    parents: Query<&ChildOf>,
    elements: Query<&TiltElement>,
    buttons: Query<(), With<Button>>,
    cursors: Query<&UiCursor>,
    mut windows: Query<(Entity, &Window, Option<&mut CursorIcon>)>,
) {
    let hovered = hover
        .as_ref()
        .and_then(|hover| hover.get(&PointerId::Mouse))
        .and_then(|hits| {
            hits.iter()
                .min_by(|(_, left), (_, right)| left.depth.total_cmp(&right.depth))
                .map(|(entity, _)| *entity)
        });
    let hovered_icon =
        hovered.map(|entity| cursor_for_target(entity, &parents, &elements, &buttons, &cursors));
    for (entity, window, current) in &mut windows {
        let icon = if window.cursor_position().is_some() {
            hovered_icon.clone().unwrap_or_default()
        } else {
            CursorIcon::default()
        };
        if let Some(mut current) = current {
            if *current != icon {
                *current = icon;
            }
        } else {
            commands.entity(entity).insert(icon);
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        app::{App, Update},
        ecs::entity::Entity,
        math::Vec2,
        window::{CursorIcon, SystemCursorIcon, Window},
    };
    use bevy_picking::{backend::HitData, hover::HoverMap, pointer::PointerId};
    use tilt_ui_core::ElementKind;

    use crate::TiltElement;

    use super::update_ui_cursor;

    #[test]
    fn hovered_control_and_nested_text_use_the_requested_cursor() {
        let mut app = App::new();
        app.insert_resource(HoverMap::default())
            .add_systems(Update, update_ui_cursor);
        let mut window = Window::default();
        window.set_cursor_position(Some(Vec2::new(10.0, 10.0)));
        let window = app.world_mut().spawn(window).id();

        let cases = [
            (ElementKind::Input, SystemCursorIcon::Text),
            (ElementKind::TextArea, SystemCursorIcon::Text),
            (ElementKind::Checkbox, SystemCursorIcon::Default),
            (ElementKind::RadioButton, SystemCursorIcon::Default),
            (ElementKind::Button, SystemCursorIcon::Pointer),
            (ElementKind::Slider, SystemCursorIcon::Default),
        ];
        for (kind, expected) in cases {
            let owner = app.world_mut().spawn(TiltElement { kind }).id();
            let text = app.world_mut().spawn_empty().id();
            app.world_mut().entity_mut(owner).add_child(text);
            hover(&mut app, text, window);
            app.update();
            assert_eq!(
                app.world().get::<CursorIcon>(window),
                Some(&CursorIcon::System(expected)),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn custom_cursor_inherits_and_removal_restores_control_default() {
        use super::{UiCursor, set_ui_cursor};
        use bevy::window::{CustomCursor, CustomCursorImage};
        let mut app = App::new();
        app.insert_resource(HoverMap::default())
            .add_systems(Update, update_ui_cursor);
        let mut window = Window::default();
        window.set_cursor_position(Some(Vec2::ONE));
        let window = app.world_mut().spawn(window).id();
        let custom = CursorIcon::Custom(CustomCursor::Image(CustomCursorImage {
            hotspot: (3, 4),
            ..Default::default()
        }));
        let parent = app.world_mut().spawn(UiCursor(custom.clone())).id();
        let button = app
            .world_mut()
            .spawn(TiltElement {
                kind: ElementKind::Button,
            })
            .id();
        let text = app.world_mut().spawn_empty().id();
        app.world_mut().entity_mut(parent).add_child(button);
        app.world_mut().entity_mut(button).add_child(text);
        hover(&mut app, text, window);
        app.update();
        assert_eq!(app.world().get::<CursorIcon>(window), Some(&custom));
        set_ui_cursor(
            app.world_mut(),
            button,
            Some(SystemCursorIcon::Crosshair.into()),
        );
        app.update();
        assert_eq!(
            app.world().get::<CursorIcon>(window),
            Some(&SystemCursorIcon::Crosshair.into())
        );
        set_ui_cursor(app.world_mut(), button, None);
        set_ui_cursor(app.world_mut(), parent, None);
        app.update();
        assert_eq!(
            app.world().get::<CursorIcon>(window),
            Some(&SystemCursorIcon::Pointer.into())
        );
        app.world_mut().resource_mut::<HoverMap>().clear();
        app.update();
        assert_eq!(
            app.world().get::<CursorIcon>(window),
            Some(&CursorIcon::default())
        );
    }

    fn hover(app: &mut App, entity: Entity, camera: Entity) {
        let mut hover = app.world_mut().resource_mut::<HoverMap>();
        hover.clear();
        hover
            .entry(PointerId::Mouse)
            .or_default()
            .insert(entity, HitData::new(camera, 0.0, None, None));
    }
}
