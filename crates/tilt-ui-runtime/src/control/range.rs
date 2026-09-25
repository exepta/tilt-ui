//! Shared pointer and keyboard value handling for numeric controls.

use bevy::{
    ecs::{
        hierarchy::ChildOf,
        message::{MessageReader, Messages},
        system::{Commands, Query, Res},
        world::World,
    },
    input::{
        ButtonState,
        keyboard::{KeyCode, KeyboardInput},
    },
    math::Vec2,
    ui::{
        ComputedNode, ComputedUiRenderTargetInfo, InteractionDisabled, UiGlobalTransform, UiScale,
    },
};
use bevy_input_focus::{FocusCause, InputFocus};
use bevy_picking::{
    events::{Drag, DragEnd, Pointer, Press, Release},
    pointer::{PointerButton, PointerId},
};
use tilt_ui_core::SliderType;

use super::drag::{ActiveControlDrag, DragKind};
use crate::{
    NumericRange, NumericValueChanged,
    widgets::{
        controls::slider::SliderSettings,
        state::{
            NumericParts, RangeOrientation, SliderChanged, SliderCommitted, set_numeric_value,
            set_slider_values,
        },
    },
};

fn slider_root(
    world: &World,
    target: bevy::ecs::entity::Entity,
) -> Option<bevy::ecs::entity::Entity> {
    let mut entity = target;
    loop {
        if world.get::<SliderSettings>(entity).is_some() {
            return Some(entity);
        }
        entity = world.get::<ChildOf>(entity)?.parent();
    }
}

fn pointer_fraction(
    world: &World,
    entity: bevy::ecs::entity::Entity,
    position: Vec2,
) -> Option<f32> {
    let parts = world.get::<NumericParts>(entity)?;
    let node = world.get::<ComputedNode>(parts.track)?;
    let transform = *world.get::<UiGlobalTransform>(parts.track)?;
    let target = world.get::<ComputedUiRenderTargetInfo>(parts.track)?;
    let scale = world.get_resource::<UiScale>().map_or(1.0, |scale| scale.0);
    let point = node.normalize_point(transform, position * target.scale_factor() / scale)?;
    Some(
        match parts.orientation {
            RangeOrientation::Horizontal => point.x + 0.5,
            RangeOrientation::Vertical => 0.5 - point.y,
        }
        .clamp(0.0, 1.0),
    )
}

fn slider_snapshot(world: &World, entity: bevy::ecs::entity::Entity) -> Option<(f32, Option<f32>)> {
    let range = world.get::<NumericRange>(entity)?;
    let settings = world.get::<SliderSettings>(entity)?;
    Some(if settings.kind == SliderType::Range {
        (settings.lower, Some(settings.upper))
    } else {
        (range.value, None)
    })
}

fn apply_slider_value(
    world: &mut World,
    entity: bevy::ecs::entity::Entity,
    value: f32,
    thumb: u8,
) -> bool {
    let Some(settings) = world.get::<SliderSettings>(entity).copied() else {
        return false;
    };
    let changed = if settings.kind == SliderType::Range {
        if thumb == 0 {
            set_slider_values(world, entity, value.min(settings.upper), settings.upper)
        } else {
            set_slider_values(world, entity, settings.lower, value.max(settings.lower))
        }
    } else {
        set_numeric_value(world, entity, value)
    };
    if changed && let Some((value, upper)) = slider_snapshot(world, entity) {
        world
            .resource_mut::<Messages<SliderChanged>>()
            .write(SliderChanged {
                entity,
                value,
                upper,
            });
        if upper.is_none() {
            world
                .resource_mut::<Messages<NumericValueChanged>>()
                .write(NumericValueChanged { entity, value });
        }
    }
    changed
}

fn choose_thumb(world: &World, entity: bevy::ecs::entity::Entity, value: f32) -> u8 {
    let Some(settings) = world.get::<SliderSettings>(entity) else {
        return 0;
    };
    if settings.kind == SliderType::Range
        && (value - settings.upper).abs() < (value - settings.lower).abs()
    {
        1
    } else {
        0
    }
}

/// Converts pointer press and drag events into semantic slider values.
pub(crate) fn range_pointer_input(
    mut presses: Option<MessageReader<Pointer<Press>>>,
    mut drags: Option<MessageReader<Pointer<Drag>>>,
    mut ends: Option<MessageReader<Pointer<DragEnd>>>,
    mut releases: Option<MessageReader<Pointer<Release>>>,
    mut commands: Commands,
) {
    if let Some(presses) = presses.as_mut() {
        for press in presses.read() {
            if press.button != PointerButton::Primary {
                continue;
            }
            let target = press.entity;
            let position = press.pointer_location.position;
            let pointer = press.pointer_id;
            commands.queue(move |world: &mut World| {
                let Some(entity) = slider_root(world, target) else {
                    return;
                };
                if world.get::<InteractionDisabled>(entity).is_some() {
                    return;
                }
                let Some(fraction) = pointer_fraction(world, entity, position) else {
                    return;
                };
                let Some(range) = world.get::<NumericRange>(entity).copied() else {
                    return;
                };
                let value = range.normalize(range.min + fraction * (range.max - range.min));
                let thumb = choose_thumb(world, entity, value);
                if let Some(mut focus) = world.get_resource_mut::<InputFocus>() {
                    focus.set(entity, FocusCause::Pressed);
                }
                world.entity_mut(entity).insert(ActiveControlDrag {
                    pointer,
                    kind: DragKind::Slider { thumb },
                });
                if let Some(mut settings) = world.get_mut::<SliderSettings>(entity) {
                    settings.active_thumb = thumb;
                }
                apply_slider_value(world, entity, value, thumb);
            });
        }
    }
    if let Some(drags) = drags.as_mut() {
        for drag in drags.read() {
            if drag.button != PointerButton::Primary {
                continue;
            }
            let target = drag.entity;
            let position = drag.pointer_location.position;
            let pointer = drag.pointer_id;
            commands.queue(move |world: &mut World| {
                let Some(entity) = slider_root(world, target) else {
                    return;
                };
                let Some(ActiveControlDrag {
                    kind: DragKind::Slider { thumb },
                    ..
                }) = world
                    .get::<ActiveControlDrag>(entity)
                    .copied()
                    .filter(|active| active.pointer == pointer)
                else {
                    return;
                };
                if world.get::<InteractionDisabled>(entity).is_some() {
                    return;
                }
                let Some(fraction) = pointer_fraction(world, entity, position) else {
                    return;
                };
                let Some(range) = world.get::<NumericRange>(entity).copied() else {
                    return;
                };
                apply_slider_value(
                    world,
                    entity,
                    range.min + fraction * (range.max - range.min),
                    thumb,
                );
            });
        }
    }
    if let Some(ends) = ends.as_mut() {
        for end in ends.read() {
            if end.button != PointerButton::Primary {
                continue;
            }
            queue_commit(&mut commands, end.entity, end.pointer_id);
        }
    }
    if let Some(releases) = releases.as_mut() {
        for release in releases.read() {
            if release.button != PointerButton::Primary {
                continue;
            }
            queue_commit(&mut commands, release.entity, release.pointer_id);
        }
    }
}

fn queue_commit(commands: &mut Commands, target: bevy::ecs::entity::Entity, pointer: PointerId) {
    commands.queue(move |world: &mut World| {
        let Some(entity) = slider_root(world, target) else {
            return;
        };
        if !world.get::<ActiveControlDrag>(entity).is_some_and(|drag| {
            drag.pointer == pointer && matches!(drag.kind, DragKind::Slider { .. })
        }) {
            return;
        }
        world.entity_mut(entity).remove::<ActiveControlDrag>();
        if let Some((value, upper)) = slider_snapshot(world, entity) {
            world
                .resource_mut::<Messages<SliderCommitted>>()
                .write(SliderCommitted {
                    entity,
                    value,
                    upper,
                });
        }
    });
}

/// Maps focused Slider arrow/Home/End keys to the shared range setter.
pub(crate) fn range_keyboard_input(
    mut events: Option<MessageReader<KeyboardInput>>,
    focus: Option<Res<InputFocus>>,
    sliders: Query<Option<&InteractionDisabled>, bevy::ecs::query::With<SliderSettings>>,
    mut commands: Commands,
) {
    let Some(events) = events.as_mut() else {
        return;
    };
    let focused = focus.as_ref().and_then(|focus| focus.get());
    let Some(entity) = focused else {
        events.read().for_each(drop);
        return;
    };
    let Ok(disabled) = sliders.get(entity) else {
        events.read().for_each(drop);
        return;
    };
    if disabled.is_some() {
        events.read().for_each(drop);
        return;
    }
    for key in events.read() {
        if key.state != ButtonState::Pressed {
            continue;
        }
        let code = match key.key_code {
            KeyCode::ArrowLeft
            | KeyCode::ArrowDown
            | KeyCode::ArrowRight
            | KeyCode::ArrowUp
            | KeyCode::PageDown
            | KeyCode::PageUp
            | KeyCode::Home
            | KeyCode::End => key.key_code,
            _ => continue,
        };
        commands.queue(move |world: &mut World| {
            let Some(range) = world.get::<NumericRange>(entity).copied() else {
                return;
            };
            let Some(settings) = world.get::<SliderSettings>(entity).copied() else {
                return;
            };
            if world.get::<InteractionDisabled>(entity).is_some() {
                return;
            }
            let current = if settings.kind == SliderType::Range {
                if settings.active_thumb == 0 {
                    settings.lower
                } else {
                    settings.upper
                }
            } else {
                range.value
            };
            let step = range
                .step
                .unwrap_or(((range.max - range.min) / 100.0).max(1.0));
            let target = match code {
                KeyCode::ArrowLeft | KeyCode::ArrowDown => current - step,
                KeyCode::ArrowRight | KeyCode::ArrowUp => current + step,
                KeyCode::PageDown => current - step * 10.0,
                KeyCode::PageUp => current + step * 10.0,
                KeyCode::Home => range.min,
                KeyCode::End => range.max,
                _ => unreachable!(),
            };
            let thumb = settings.active_thumb;
            if apply_slider_value(world, entity, target, thumb)
                && let Some((value, upper)) = slider_snapshot(world, entity)
            {
                world
                    .resource_mut::<Messages<SliderCommitted>>()
                    .write(SliderCommitted {
                        entity,
                        value,
                        upper,
                    });
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        app::App,
        ecs::{
            message::{MessageCursor, Messages},
            world::World,
        },
        input::{
            ButtonState,
            keyboard::{Key, KeyCode, KeyboardInput},
        },
        math::Vec2,
        ui::{ComputedNode, ComputedUiRenderTargetInfo, UiGlobalTransform},
    };
    use bevy_input_focus::{FocusCause, InputFocus};
    use tilt_ui_core::{ElementKind, TemplateAttribute};

    use super::{apply_slider_value, pointer_fraction};
    use crate::{
        NumericRange, SliderChanged,
        widgets::{
            controls::slider::SliderSettings,
            state::{NumericParts, RangeOrientation},
        },
    };

    #[test]
    fn pointer_geometry_maps_horizontal_and_vertical_endpoints() {
        let mut world = World::new();
        let slider = world.spawn_empty().id();
        crate::render::materialize_element(&mut world, slider, ElementKind::Slider, &[]);
        let track = world.get::<NumericParts>(slider).unwrap().track;
        world.entity_mut(track).insert((
            ComputedNode {
                size: Vec2::new(100.0, 10.0),
                ..Default::default()
            },
            UiGlobalTransform::from_xy(50.0, 5.0),
            ComputedUiRenderTargetInfo::default(),
        ));
        assert_eq!(
            pointer_fraction(&world, slider, Vec2::new(0.0, 5.0)),
            Some(0.0)
        );
        assert_eq!(
            pointer_fraction(&world, slider, Vec2::new(100.0, 5.0)),
            Some(1.0)
        );
        world.get_mut::<NumericParts>(slider).unwrap().orientation = RangeOrientation::Vertical;
        world.get_mut::<ComputedNode>(track).unwrap().size = Vec2::new(10.0, 100.0);
        world
            .entity_mut(track)
            .insert(UiGlobalTransform::from_xy(5.0, 50.0));
        assert_eq!(
            pointer_fraction(&world, slider, Vec2::new(5.0, 0.0)),
            Some(1.0)
        );
        assert_eq!(
            pointer_fraction(&world, slider, Vec2::new(5.0, 100.0)),
            Some(0.0)
        );
    }

    #[test]
    fn range_thumb_changes_only_its_endpoint_and_emits_one_change() {
        let mut world = World::new();
        world.init_resource::<Messages<SliderChanged>>();
        world.init_resource::<Messages<crate::NumericValueChanged>>();
        let slider = world.spawn_empty().id();
        crate::render::materialize_element(
            &mut world,
            slider,
            ElementKind::Slider,
            &[
                TemplateAttribute::Static {
                    name: "type".into(),
                    value: "range".into(),
                },
                TemplateAttribute::Static {
                    name: "range-start".into(),
                    value: "25".into(),
                },
                TemplateAttribute::Static {
                    name: "range-end".into(),
                    value: "75".into(),
                },
                TemplateAttribute::Static {
                    name: "show-tip".into(),
                    value: String::new(),
                },
            ],
        );
        let parts = *world.get::<NumericParts>(slider).unwrap();
        assert!(parts.second_thumb.is_some());
        assert!(parts.tip.is_some());
        assert!(parts.second_tip.is_some());
        assert!(apply_slider_value(&mut world, slider, 40.0, 0));
        let settings = world.get::<SliderSettings>(slider).unwrap();
        assert_eq!((settings.lower, settings.upper), (40.0, 75.0));
        assert!(!apply_slider_value(&mut world, slider, 40.0, 0));
        assert!(apply_slider_value(&mut world, slider, 90.0, 1));
        let settings = world.get::<SliderSettings>(slider).unwrap();
        assert_eq!((settings.lower, settings.upper), (40.0, 90.0));
        assert_eq!(world.get::<NumericRange>(slider).unwrap().value, 0.0);
        assert_eq!(
            world.get::<NumericParts>(slider).unwrap().second_thumb,
            parts.second_thumb
        );
        assert_eq!(
            world
                .get::<bevy::ui::widget::Text>(parts.tip.unwrap())
                .unwrap()
                .0,
            "40"
        );
        assert_eq!(
            world
                .get::<bevy::ui::widget::Text>(parts.second_tip.unwrap())
                .unwrap()
                .0,
            "90"
        );
        let mut cursor = MessageCursor::<SliderChanged>::default();
        assert_eq!(
            cursor
                .read(world.resource::<Messages<SliderChanged>>())
                .count(),
            2
        );
    }

    #[test]
    fn keyboard_events_in_one_frame_accumulate_from_the_latest_value() {
        let mut app = App::new();
        app.add_plugins(crate::TiltUiControlRuntimePlugin)
            .add_message::<KeyboardInput>();
        let slider = app.world_mut().spawn_empty().id();
        crate::render::materialize_element(
            app.world_mut(),
            slider,
            ElementKind::Slider,
            &[
                TemplateAttribute::Static {
                    name: "value".into(),
                    value: "10".into(),
                },
                TemplateAttribute::Static {
                    name: "step".into(),
                    value: "5".into(),
                },
            ],
        );
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(slider, FocusCause::Pressed);
        let window = app.world_mut().spawn_empty().id();
        for _ in 0..2 {
            app.world_mut()
                .resource_mut::<Messages<KeyboardInput>>()
                .write(KeyboardInput {
                    key_code: KeyCode::ArrowRight,
                    logical_key: Key::ArrowRight,
                    state: ButtonState::Pressed,
                    text: None,
                    repeat: false,
                    window,
                });
        }
        app.update();
        assert_eq!(app.world().get::<NumericRange>(slider).unwrap().value, 20.0);
        let mut cursor = MessageCursor::<SliderChanged>::default();
        assert_eq!(
            cursor
                .read(app.world().resource::<Messages<SliderChanged>>())
                .count(),
            2
        );
    }
}
