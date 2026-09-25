//! Pointer selection and clipboard copy for non-editable UI text.

use bevy::{
    clipboard::Clipboard,
    ecs::{
        change_detection::{DetectChanges, Ref},
        component::Component,
        entity::Entity,
        hierarchy::{ChildOf, Children},
        message::MessageReader,
        resource::Resource,
        system::{Query, Res, ResMut},
    },
    input::{
        ButtonInput, ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    math::{Rect, Vec2},
    text::{ComputedTextBlock, TextCursorStyle, TextLayoutInfo},
    ui::{ComputedNode, ComputedUiRenderTargetInfo, UiGlobalTransform, UiScale, widget::Text},
};
use bevy_input_focus::InputFocus;
use bevy_picking::{
    events::{Drag, Pointer, Press},
    pointer::{PointerButton, PointerId},
};
use parley::editing::{Cursor, Selection};

use crate::EditableText;

/// Enables mouse selection on a non-editable text node.
#[derive(Component, Debug, Default, Clone, Copy)]
pub struct SelectableStaticText {
    /// UTF-8 anchor offset.
    pub anchor: usize,
    /// UTF-8 focus offset.
    pub focus: usize,
}

impl SelectableStaticText {
    /// Returns the selected byte range, if nonempty.
    pub fn range(self) -> Option<std::ops::Range<usize>> {
        (self.anchor != self.focus)
            .then(|| self.anchor.min(self.focus)..self.anchor.max(self.focus))
    }
}

/// Tracks the one static text selection currently active in the UI.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct ActiveStaticTextSelection {
    pub(crate) entity: Option<Entity>,
    pointer: Option<PointerId>,
}

pub(crate) fn mark_selectable(world: &mut bevy::ecs::world::World, entity: Entity) {
    world.entity_mut(entity).insert((
        SelectableStaticText::default(),
        TextCursorStyle {
            selection_color: bevy::color::Color::srgba(0.66, 0.29, 0.91, 0.38),
            unfocused_selection_color: bevy::color::Color::srgba(0.66, 0.29, 0.91, 0.38),
            ..Default::default()
        },
    ));
}

fn selectable_target(
    mut entity: Entity,
    selectable: &Query<&SelectableStaticText>,
    parents: &Query<&ChildOf>,
    children: &Query<&Children>,
) -> Option<Entity> {
    loop {
        if selectable.get(entity).is_ok() {
            return Some(entity);
        }
        if let Ok(children) = children.get(entity)
            && let Some(child) = children
                .iter()
                .copied()
                .find(|child| selectable.get(*child).is_ok())
        {
            return Some(child);
        }
        entity = parents.get(entity).ok()?.parent();
    }
}

fn local_point(
    position: Vec2,
    node: &ComputedNode,
    target: &ComputedUiRenderTargetInfo,
    transform: &UiGlobalTransform,
    scale: f32,
) -> Option<Vec2> {
    transform.try_inverse().map(|inverse| {
        inverse.transform_point2(position * target.scale_factor() / scale) - node.content_box().min
    })
}

type TextGeometry<'w, 's> = Query<
    'w,
    's,
    (
        &'static ComputedTextBlock,
        &'static ComputedNode,
        &'static ComputedUiRenderTargetInfo,
        &'static UiGlobalTransform,
    ),
>;

pub(crate) fn select_static_text(
    mut presses: Option<MessageReader<Pointer<Press>>>,
    mut drags: Option<MessageReader<Pointer<Drag>>>,
    mut active: ResMut<ActiveStaticTextSelection>,
    mut selectable: Query<&mut SelectableStaticText>,
    geometry: TextGeometry,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    mut layouts: Query<&mut TextLayoutInfo>,
    mut focus: Option<ResMut<InputFocus>>,
    scale: Option<Res<UiScale>>,
) {
    let scale = scale.as_ref().map_or(1.0, |scale| scale.0);
    if let Some(presses) = presses.as_mut() {
        for press in presses.read() {
            if press.button != PointerButton::Primary {
                continue;
            }
            let target =
                selectable_target(press.entity, &selectable.as_readonly(), &parents, &children);
            if let Some(previous) = active.entity {
                if let Ok(mut previous_selection) = selectable.get_mut(previous) {
                    *previous_selection = SelectableStaticText::default();
                }
                if let Ok(mut layout) = layouts.get_mut(previous) {
                    layout.selection_rects.clear();
                }
            }
            active.entity = None;
            active.pointer = None;
            let Some(entity) = target else {
                continue;
            };
            let Ok((block, node, render_target, transform)) = geometry.get(entity) else {
                continue;
            };
            let Some(point) = local_point(
                press.pointer_location.position,
                node,
                render_target,
                transform,
                scale,
            ) else {
                continue;
            };
            let selection = match press.count {
                1 => Selection::from_point(block.buffer(), point.x, point.y),
                2 => Selection::word_from_point(block.buffer(), point.x, point.y),
                _ => Selection::line_from_point(block.buffer(), point.x, point.y),
            };
            if let Ok(mut state) = selectable.get_mut(entity) {
                state.anchor = selection.anchor().index();
                state.focus = selection.focus().index();
                active.entity = Some(entity);
                active.pointer = Some(press.pointer_id);
                if let Some(focus) = focus.as_deref_mut() {
                    focus.clear();
                }
            }
        }
    }
    if let Some(drags) = drags.as_mut() {
        for drag in drags.read() {
            if drag.button != PointerButton::Primary || active.pointer != Some(drag.pointer_id) {
                continue;
            }
            let Some(entity) = active.entity else {
                continue;
            };
            let Ok((block, node, render_target, transform)) = geometry.get(entity) else {
                continue;
            };
            let Some(point) = local_point(
                drag.pointer_location.position,
                node,
                render_target,
                transform,
                scale,
            ) else {
                continue;
            };
            if let Ok(mut state) = selectable.get_mut(entity) {
                state.focus = Cursor::from_point(block.buffer(), point.x, point.y).index();
            }
        }
    }
}

pub(crate) fn update_static_selection_highlight(
    active: Res<ActiveStaticTextSelection>,
    selections: Query<Ref<SelectableStaticText>>,
    blocks: Query<Ref<ComputedTextBlock>>,
    mut layouts: Query<&mut TextLayoutInfo>,
) {
    let Some(entity) = active.entity else {
        return;
    };
    let (Ok(state), Ok(block), Ok(mut layout)) = (
        selections.get(entity),
        blocks.get(entity),
        layouts.get_mut(entity),
    ) else {
        return;
    };
    if !state.is_changed() && !block.is_changed() && !layout.is_changed() {
        return;
    }
    let rectangles = state.range().map_or_else(Vec::new, |_| {
        let selection = Selection::new(
            Cursor::from_byte_index(
                block.buffer(),
                state.anchor,
                parley::layout::Affinity::Downstream,
            ),
            Cursor::from_byte_index(
                block.buffer(),
                state.focus,
                parley::layout::Affinity::Downstream,
            ),
        );
        selection
            .geometry(block.buffer())
            .into_iter()
            .map(|(box_, _)| {
                Rect::new(
                    box_.x0 as f32,
                    box_.y0 as f32,
                    box_.x1 as f32,
                    box_.y1 as f32,
                )
            })
            .collect::<Vec<_>>()
    });
    if layout.selection_rects != rectangles {
        layout.selection_rects = rectangles;
    }
}

pub(crate) fn selected_text(
    active: &ActiveStaticTextSelection,
    selections: &Query<&SelectableStaticText>,
    texts: &Query<&Text>,
) -> Option<String> {
    let entity = active.entity?;
    let range = selections.get(entity).ok()?.range()?;
    texts.get(entity).ok()?.0.get(range).map(str::to_owned)
}

pub(crate) fn copy_selected_text_shortcut(
    mut keys: Option<MessageReader<KeyboardInput>>,
    modifiers: Option<Res<ButtonInput<Key>>>,
    active: Res<ActiveStaticTextSelection>,
    selections: Query<&SelectableStaticText>,
    texts: Query<&Text>,
    focus: Option<Res<InputFocus>>,
    editable: Query<&EditableText>,
    mut clipboard: Option<ResMut<Clipboard>>,
) {
    let Some(keys) = keys.as_mut() else {
        return;
    };
    let command = modifiers.as_ref().is_some_and(|keys| {
        keys.pressed(if cfg!(target_os = "macos") {
            Key::Super
        } else {
            Key::Control
        })
    });
    let editable_focused = focus
        .as_ref()
        .and_then(|focus| focus.get())
        .is_some_and(|entity| editable.get(entity).is_ok());
    for key in keys.read() {
        if key.state != ButtonState::Pressed || !command || editable_focused {
            continue;
        }
        if matches!(&key.logical_key, Key::Character(value) if value.eq_ignore_ascii_case("c"))
            && let Some(value) = selected_text(&active, &selections, &texts)
            && let Some(clipboard) = clipboard.as_deref_mut()
        {
            let _ = clipboard.set_text(value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SelectableStaticText;

    #[test]
    fn selection_range_handles_both_drag_directions() {
        assert_eq!(
            SelectableStaticText {
                anchor: 8,
                focus: 2
            }
            .range(),
            Some(2..8)
        );
        assert_eq!(
            SelectableStaticText {
                anchor: 2,
                focus: 8
            }
            .range(),
            Some(2..8)
        );
        assert_eq!(
            SelectableStaticText {
                anchor: 2,
                focus: 2
            }
            .range(),
            None
        );
    }
}
