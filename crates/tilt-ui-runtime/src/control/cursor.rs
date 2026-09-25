//! Window cursor shapes for built-in UI controls.

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

fn cursor_for_target(
    mut entity: Entity,
    parents: &Query<&ChildOf>,
    elements: &Query<&TiltElement>,
    buttons: &Query<(), With<Button>>,
) -> SystemCursorIcon {
    loop {
        if let Ok(element) = elements.get(entity) {
            match element.kind {
                ElementKind::Input | ElementKind::TextArea => return SystemCursorIcon::Text,
                ElementKind::Button
                | ElementKind::ToggleButton
                | ElementKind::SwitchButton
                | ElementKind::HyperLink => return SystemCursorIcon::Pointer,
                ElementKind::Checkbox | ElementKind::RadioButton | ElementKind::Slider => {
                    return SystemCursorIcon::Default;
                }
                _ => {}
            }
        }
        if buttons.get(entity).is_ok() {
            return SystemCursorIcon::Pointer;
        }
        let Ok(parent) = parents.get(entity) else {
            return SystemCursorIcon::Default;
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
        hovered.map(|entity| cursor_for_target(entity, &parents, &elements, &buttons));
    for (entity, window, current) in &mut windows {
        let icon = CursorIcon::System(if window.cursor_position().is_some() {
            hovered_icon.unwrap_or_default()
        } else {
            SystemCursorIcon::Default
        });
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

    fn hover(app: &mut App, entity: Entity, camera: Entity) {
        let mut hover = app.world_mut().resource_mut::<HoverMap>();
        hover.clear();
        hover
            .entry(PointerId::Mouse)
            .or_default()
            .insert(entity, HitData::new(camera, 0.0, None, None));
    }
}
