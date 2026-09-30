//! Focused text editing shared by Input and TextArea.

use std::collections::HashMap;

use bevy::{
    ecs::{
        change_detection::DetectChanges,
        component::Component,
        entity::Entity,
        event::EntityEvent,
        hierarchy::ChildOf,
        message::{MessageReader, MessageWriter},
        observer::On,
        system::{Commands, Local, Query, Res, ResMut, SystemParam},
    },
    input::{
        ButtonInput, ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    math::{Rect, Vec2},
    prelude::Visibility,
    text::{
        EditableText as NativeEditableText, FontCx, LayoutCx, PreeditCursor, TextEdit,
        TextEditChange, TextLayoutInfo,
    },
    ui::{
        ComputedNode, ComputedUiRenderTargetInfo, InteractionDisabled, UiGlobalTransform, UiScale,
        widget::{Text, TextScroll},
    },
    window::{Ime, PrimaryWindow, Window},
};
use bevy_input_focus::{FocusCause, FocusLost, InputFocus};
use bevy_picking::{
    events::{Drag, Pointer, Press},
    pointer::PointerButton,
};

use crate::{
    ControlPart, EditableText, EditableTextChanged, EditableTextCommitted, ElementState,
    TiltControl,
    scroll::ActiveScrollbarDrag,
    widgets::state::{EditableTextOptions, EditableTextParts, editable_invalid, masked_password},
};

/// Enables the default per-glyph entrance animation on editable controls.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct AnimateInputText;

#[derive(Component)]
pub(crate) struct GlyphEntrance {
    first: usize,
    count: usize,
    elapsed: f32,
    targets: Vec<(usize, Vec2)>,
}

fn inserted_char_range(old: &str, new: &str) -> Option<(usize, usize)> {
    let old = old.chars().collect::<Vec<_>>();
    let new = new.chars().collect::<Vec<_>>();
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let inserted_end = new.len().saturating_sub(suffix);
    let first = new[..prefix]
        .iter()
        .filter(|character| !character.is_whitespace())
        .count();
    let inserted = new[prefix..inserted_end]
        .iter()
        .filter(|character| !character.is_whitespace())
        .count();
    (inserted > 0).then_some((first, inserted))
}

pub(crate) fn animate_input_text(
    mut commands: Commands,
    time: Option<Res<bevy::time::Time>>,
    settings: Res<crate::widgets::state::UiMotionSettings>,
    mut entries: Query<(Entity, &mut GlyphEntrance, &mut TextLayoutInfo)>,
) {
    let delta = time.as_ref().map_or(1.0 / 60.0, |time| time.delta_secs());
    for (entity, mut entrance, mut layout) in &mut entries {
        if entrance.targets.is_empty() {
            let end = (entrance.first + entrance.count).min(layout.glyphs.len());
            entrance.targets = (entrance.first..end)
                .map(|index| (index, layout.glyphs[index].position))
                .collect();
        }
        let duration = settings.input_text_seconds;
        entrance.elapsed += delta;
        let t = if duration <= 0.0 {
            1.0
        } else {
            (entrance.elapsed / duration).clamp(0.0, 1.0)
        };
        let settle = (1.0 - t) * (1.0 - t);
        for (order, (index, target)) in entrance.targets.iter().enumerate() {
            let Some(glyph) = layout.glyphs.get_mut(*index) else {
                continue;
            };
            let wave = (t * 18.0 + order as f32 * 0.45).sin() * (1.0 - t);
            glyph.position = *target + Vec2::new(5.0 * settle, -4.0 * settle + 1.6 * wave);
        }
        if t >= 1.0 || entrance.targets.is_empty() {
            commands.entity(entity).remove::<GlyphEntrance>();
        }
    }
}

pub(crate) fn animate_editable_cursor(
    focus: Option<Res<InputFocus>>,
    settings: Res<crate::widgets::state::UiMotionSettings>,
    time: Option<Res<bevy::time::Time>>,
    mut layouts: Query<&mut TextLayoutInfo>,
    mut previous: Local<HashMap<Entity, Rect>>,
) {
    let Some(focused) = focus.and_then(|focus| focus.get()) else {
        previous.clear();
        return;
    };
    previous.retain(|entity, _| *entity == focused);
    let Ok(mut layout) = layouts.get_mut(focused) else {
        return;
    };
    let Some((visible, target)) = layout.cursor else {
        return;
    };
    let delta = time.as_ref().map_or(1.0 / 60.0, |time| time.delta_secs());
    let duration = settings.caret_seconds;
    let amount = if duration <= 0.0 {
        1.0
    } else {
        1.0 - (-delta / duration).exp()
    };
    let from = previous.get(&focused).copied().unwrap_or(target);
    let next = Rect {
        min: from.min.lerp(target.min, amount),
        max: from.max.lerp(target.max, amount),
    };
    previous.insert(focused, next);
    if next != target {
        layout.cursor = Some((visible, next));
    }
}

#[derive(Clone)]
struct EditSnapshot {
    value: String,
    cursor: usize,
    selection_anchor: Option<usize>,
}

impl From<&EditableText> for EditSnapshot {
    fn from(state: &EditableText) -> Self {
        Self {
            value: state.value.clone(),
            cursor: state.cursor,
            selection_anchor: state.selection_anchor,
        }
    }
}

/// Keeps a bounded undo history for one editable control.
#[derive(bevy::ecs::component::Component, Default)]
pub(crate) struct EditableHistory {
    undo: Vec<EditSnapshot>,
    redo: Vec<EditSnapshot>,
    restoring: bool,
}

type FocusedEditorQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static EditableText,
        &'static EditableTextOptions,
        &'static mut NativeEditableText,
        Option<&'static mut EditableHistory>,
        Option<&'static InteractionDisabled>,
    ),
>;

type NativeEditSyncQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut EditableText,
        &'static EditableTextOptions,
        &'static mut NativeEditableText,
        &'static EditableTextParts,
        &'static mut ElementState,
        Option<&'static mut EditableHistory>,
        Option<&'static AnimateInputText>,
    ),
>;

impl EditableHistory {
    pub(crate) fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.restoring = false;
    }

    fn record(&mut self, state: &EditableText) {
        const MAX_UNDO: usize = 100;
        if self.restoring {
            self.restoring = false;
            return;
        }
        if self.undo.len() == MAX_UNDO {
            self.undo.remove(0);
        }
        self.undo.push(state.into());
        self.redo.clear();
    }
}

fn restore_edit(
    state: &EditableText,
    history: &mut EditableHistory,
    native: &mut NativeEditableText,
    font: &mut FontCx,
    layout: &mut LayoutCx,
    redo: bool,
) {
    let (from, to) = if redo {
        (&mut history.redo, &mut history.undo)
    } else {
        (&mut history.undo, &mut history.redo)
    };
    let Some(snapshot) = from.pop() else { return };
    to.push(state.into());
    history.restoring = true;
    native.pending_edits.clear();
    native.pending_paste = None;
    native.editor_mut().set_text(&snapshot.value);
    let mut driver = native.editor_mut().driver(font, &mut layout.0);
    if let Some(anchor) = snapshot.selection_anchor {
        driver.select_byte_range(anchor, snapshot.cursor);
    } else {
        driver.move_to_byte(snapshot.cursor);
    }
}

/// Queues Unicode-aware edits on the currently focused editable control.
pub(crate) fn edit_focused_text(
    mut events: Option<MessageReader<KeyboardInput>>,
    focus: Option<Res<InputFocus>>,
    modifiers: Option<Res<ButtonInput<Key>>>,
    mut editors: FocusedEditorQuery,
    mut font: Option<ResMut<FontCx>>,
    mut layout: Option<ResMut<LayoutCx>>,
    mut committed: MessageWriter<EditableTextCommitted>,
) {
    let Some(events) = events.as_mut() else {
        return;
    };
    if events.is_empty() {
        return;
    }
    let focused = focus.as_ref().and_then(|focus| focus.get());
    let keys = events.read();
    let Some(entity) = focused else {
        keys.for_each(drop);
        return;
    };
    let Ok((state, options, mut native, mut history, disabled)) = editors.get_mut(entity) else {
        keys.for_each(drop);
        return;
    };
    if disabled.is_some() || state.input_type == tilt_ui_core::InputType::File {
        keys.for_each(drop);
        return;
    }
    let command = modifiers.as_ref().is_some_and(|keys| {
        keys.pressed(if cfg!(target_os = "macos") {
            Key::Super
        } else {
            Key::Control
        })
    });
    let word = modifiers.as_ref().is_some_and(|keys| {
        keys.pressed(if cfg!(target_os = "macos") {
            Key::Alt
        } else {
            Key::Control
        })
    });
    let shift = modifiers
        .as_ref()
        .is_some_and(|keys| keys.pressed(Key::Shift));
    for key in keys {
        if key.state != ButtonState::Pressed || native.is_composing() {
            continue;
        }
        let history_action = match &key.logical_key {
            Key::Character(value) if command && value.eq_ignore_ascii_case("z") => Some(shift),
            Key::Character(value) if command && value.eq_ignore_ascii_case("y") => Some(true),
            _ => None,
        };
        if let Some(redo) = history_action {
            if !state.readonly
                && let (Some(history), Some(font), Some(layout)) = (
                    history.as_deref_mut(),
                    font.as_deref_mut(),
                    layout.as_deref_mut(),
                )
            {
                restore_edit(state, history, &mut native, font, layout, redo);
            }
            continue;
        }
        let action = match &key.logical_key {
            Key::ArrowLeft if word => Some(TextEdit::WordLeft(shift)),
            Key::ArrowRight if word => Some(TextEdit::WordRight(shift)),
            Key::ArrowLeft => Some(TextEdit::Left(shift)),
            Key::ArrowRight => Some(TextEdit::Right(shift)),
            Key::ArrowUp if command => Some(TextEdit::TextStart(shift)),
            Key::ArrowDown if command => Some(TextEdit::TextEnd(shift)),
            Key::ArrowUp => Some(TextEdit::Up(shift)),
            Key::ArrowDown => Some(TextEdit::Down(shift)),
            Key::Home if command => Some(TextEdit::TextStart(shift)),
            Key::End if command => Some(TextEdit::TextEnd(shift)),
            Key::Home => Some(TextEdit::LineStart(shift)),
            Key::End => Some(TextEdit::LineEnd(shift)),
            Key::Backspace if !state.readonly && word => Some(TextEdit::BackspaceWord),
            Key::Delete if !state.readonly && word => Some(TextEdit::DeleteWord),
            Key::Backspace if !state.readonly => Some(TextEdit::Backspace),
            Key::Delete if !state.readonly => Some(TextEdit::Delete),
            Key::Character(value) if command && value.eq_ignore_ascii_case("a") => {
                Some(TextEdit::SelectAll)
            }
            Key::Character(value) if command && value.eq_ignore_ascii_case("c") => {
                Some(TextEdit::Copy)
            }
            Key::Character(value)
                if command && value.eq_ignore_ascii_case("x") && !state.readonly =>
            {
                Some(TextEdit::Cut)
            }
            Key::Character(value)
                if command && value.eq_ignore_ascii_case("v") && !state.readonly =>
            {
                Some(TextEdit::Paste)
            }
            Key::Escape => Some(TextEdit::CollapseSelection),
            Key::Enter if state.multiline && !state.readonly => Some(TextEdit::Insert("\n".into())),
            Key::Enter if !state.multiline => {
                committed.write(EditableTextCommitted {
                    entity,
                    name: options.name.clone(),
                    value: state.value.clone(),
                });
                None
            }
            Key::Character(_) | Key::Space if !state.readonly && !command && !word => key
                .text
                .as_ref()
                .filter(|text| !text.is_empty())
                .map(|text| TextEdit::Insert(text.clone())),
            _ => None,
        };
        if let Some(action) = action {
            native.queue_edit(action);
        }
    }
}

/// Synchronizes completed native edits into the TiltUI semantic value.
pub(crate) fn sync_native_edit(
    trigger: On<TextEditChange>,
    mut commands: Commands,
    mut editors: NativeEditSyncQuery,
    #[cfg(feature = "component")] parents: Query<&ChildOf>,
    #[cfg(feature = "component")] forms: Query<(), bevy::ecs::query::With<crate::FormSettings>>,
    mut placeholders: Query<&mut Visibility>,
    mut text_parts: Query<&mut Text>,
    mut changed: MessageWriter<EditableTextChanged>,
) {
    let entity = trigger.event_target();
    let Ok((mut state, options, mut native, parts, mut css_state, mut history, animate)) =
        editors.get_mut(entity)
    else {
        return;
    };
    commands
        .entity(entity)
        .remove::<crate::scroll::UserTextScroll>();
    let value = native.value().to_string();
    if state.input_type == tilt_ui_core::InputType::Number
        && !crate::widgets::controls::input::number_characters_allowed(&value)
    {
        native.editor_mut().set_text(&state.value);
        native.queue_edit(TextEdit::TextEnd(false));
        return;
    }
    if options
        .max_lines
        .is_some_and(|limit| value.split('\n').count() > limit)
    {
        native.editor_mut().set_text(&state.value);
        native.queue_edit(TextEdit::TextEnd(false));
        return;
    }
    let selection = native.editor().raw_selection();
    if value == state.value {
        state.cursor = selection.focus().index();
        state.selection_anchor = (!selection.is_collapsed()).then(|| selection.anchor().index());
        return;
    }
    if let Some(history) = history.as_deref_mut() {
        history.record(&state);
    }
    if animate.is_some()
        && let Some((first, count)) = inserted_char_range(&state.value, &value)
    {
        commands.entity(entity).insert(GlyphEntrance {
            first,
            count,
            elapsed: 0.0,
            targets: Vec::new(),
        });
    }
    state.cursor = selection.focus().index();
    state.selection_anchor = (!selection.is_collapsed()).then(|| selection.anchor().index());
    state.value = value.clone();
    if state.input_type == tilt_ui_core::InputType::Password
        && let Ok(mut display) = text_parts.get_mut(parts.value)
    {
        display.0 = masked_password(&value);
    }
    #[cfg(feature = "component")]
    let managed_by_form = {
        let mut current = entity;
        loop {
            let Ok(parent) = parents.get(current) else {
                break false;
            };
            current = parent.parent();
            if forms.contains(current) {
                break true;
            }
        }
    };
    #[cfg(not(feature = "component"))]
    let managed_by_form = false;
    if !managed_by_form {
        let invalid = editable_invalid(&value, state.input_type, options);
        if css_state.invalid != invalid {
            css_state.invalid = invalid;
        }
    }
    if let Ok(mut placeholder) = placeholders.get_mut(parts.placeholder) {
        *placeholder = if value.is_empty() {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    changed.write(EditableTextChanged {
        entity,
        name: options.name.clone(),
        value,
    });
}

fn nearest_control(
    target: bevy::ecs::entity::Entity,
    hierarchy: &EditableHierarchy,
) -> Option<bevy::ecs::entity::Entity> {
    let mut entity = target;
    loop {
        let (control, parent, _) = hierarchy.get(entity).ok()?;
        if control.is_some() {
            return Some(entity);
        }
        entity = parent?.parent();
    }
}

type EditableHierarchy<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static TiltControl>,
        Option<&'static ChildOf>,
        Option<&'static ControlPart>,
    ),
>;

#[derive(SystemParam)]
pub(crate) struct EditablePointerContext<'w, 's> {
    hierarchy: EditableHierarchy<'w, 's>,
    scroll_drags: Query<'w, 's, &'static ActiveScrollbarDrag>,
}

type EditorGeometry<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut NativeEditableText,
        &'static ComputedNode,
        &'static ComputedUiRenderTargetInfo,
        &'static UiGlobalTransform,
        &'static TextScroll,
        Option<&'static InteractionDisabled>,
    ),
>;

fn local_point(
    position: bevy::math::Vec2,
    node: &ComputedNode,
    target: &ComputedUiRenderTargetInfo,
    transform: &UiGlobalTransform,
    scroll: &TextScroll,
    scale: f32,
) -> Option<bevy::math::Vec2> {
    transform.try_inverse().map(|inverse| {
        inverse.transform_point2(position * target.scale_factor() / scale) - node.content_box().min
            + scroll.0
    })
}

/// Places the caret and focus on primary pointer press.
pub(crate) fn editable_pointer_press(
    mut events: Option<MessageReader<Pointer<Press>>>,
    context: EditablePointerContext,
    mut editors: EditorGeometry,
    states: Query<&EditableText>,
    mut focus: Option<ResMut<InputFocus>>,
    scale: Option<Res<UiScale>>,
    modifiers: Option<Res<ButtonInput<Key>>>,
) {
    let Some(events) = events.as_mut() else {
        return;
    };
    let scale = scale.as_ref().map_or(1.0, |scale| scale.0);
    for press in events.read() {
        if press.button != PointerButton::Primary {
            continue;
        }
        if context
            .scroll_drags
            .iter()
            .any(|drag| drag.pointer == press.pointer_id)
        {
            continue;
        }
        if context
            .hierarchy
            .get(press.entity)
            .is_ok_and(|(_, _, part)| part.is_some_and(|part| part.kind.handles_own_pointer()))
        {
            continue;
        }
        let Some(entity) = nearest_control(press.entity, &context.hierarchy) else {
            continue;
        };
        if states
            .get(entity)
            .is_ok_and(|state| state.input_type == tilt_ui_core::InputType::File)
        {
            continue;
        }
        let Ok((mut native, node, target, transform, scroll, disabled)) = editors.get_mut(entity)
        else {
            continue;
        };
        if disabled.is_some() {
            continue;
        }
        if let Some(focus) = focus.as_deref_mut() {
            focus.set(entity, FocusCause::Pressed);
        }
        if native.is_composing() {
            continue;
        }
        let Some(point) = local_point(
            press.pointer_location.position,
            node,
            target,
            transform,
            scroll,
            scale,
        ) else {
            continue;
        };
        let action = match press.count {
            1 if modifiers
                .as_ref()
                .is_some_and(|keys| keys.pressed(Key::Shift)) =>
            {
                TextEdit::ShiftClickExtension(point)
            }
            1 => TextEdit::MoveToPoint(point),
            2 => TextEdit::SelectWordAtPoint(point),
            _ => TextEdit::SelectAll,
        };
        native.queue_edit(action);
    }
}

/// Extends selected text while a pointer drags inside an editable control.
pub(crate) fn editable_pointer_drag(
    mut events: Option<MessageReader<Pointer<Drag>>>,
    context: EditablePointerContext,
    mut editors: EditorGeometry,
    scale: Option<Res<UiScale>>,
) {
    let Some(events) = events.as_mut() else {
        return;
    };
    let scale = scale.as_ref().map_or(1.0, |scale| scale.0);
    for drag in events.read() {
        if drag.button != PointerButton::Primary {
            continue;
        }
        if context
            .scroll_drags
            .iter()
            .any(|active| active.pointer == drag.pointer_id)
        {
            continue;
        }
        if context
            .hierarchy
            .get(drag.entity)
            .is_ok_and(|(_, _, part)| part.is_some_and(|part| part.kind.handles_own_pointer()))
        {
            continue;
        }
        let Some(entity) = nearest_control(drag.entity, &context.hierarchy) else {
            continue;
        };
        let Ok((mut native, node, target, transform, scroll, disabled)) = editors.get_mut(entity)
        else {
            continue;
        };
        if disabled.is_some() || native.is_composing() {
            continue;
        }
        let Some(point) = local_point(
            drag.pointer_location.position,
            node,
            target,
            transform,
            scroll,
            scale,
        ) else {
            continue;
        };
        native.queue_edit(TextEdit::ExtendSelectionToPoint(point));
    }
}

/// Routes IME composition to the focused editable control.
pub(crate) fn editable_ime(
    mut events: Option<MessageReader<Ime>>,
    focus: Option<Res<InputFocus>>,
    mut editors: Query<(
        &EditableText,
        &mut NativeEditableText,
        Option<&InteractionDisabled>,
    )>,
) {
    let Some(events) = events.as_mut() else {
        return;
    };
    let focused = focus.as_ref().and_then(|focus| focus.get());
    let Some(entity) = focused else {
        events.read().for_each(drop);
        return;
    };
    let Ok((state, mut native, disabled)) = editors.get_mut(entity) else {
        events.read().for_each(drop);
        return;
    };
    if disabled.is_some() {
        events.read().for_each(drop);
        return;
    }
    for event in events.read() {
        match event {
            Ime::Preedit { value, cursor, .. } if !state.readonly => {
                native.queue_edit(TextEdit::ImeSetCompose {
                    value: value.as_str().into(),
                    cursor: cursor.map(|(anchor, focus)| PreeditCursor { anchor, focus }),
                })
            }
            Ime::Commit { value, .. } if !state.readonly => {
                native.queue_edit(TextEdit::ImeCommit {
                    value: value.as_str().into(),
                })
            }
            Ime::Disabled { .. } | Ime::Enabled { .. } => {
                native.queue_edit(TextEdit::clear_ime_compose())
            }
            _ => {}
        }
    }
}

/// Enables native IME only while a TiltUI editable control is focused.
pub(crate) fn update_ime_window(
    focus: Option<Res<InputFocus>>,
    editors: Query<(), bevy::ecs::query::With<NativeEditableText>>,
    mut windows: Query<&mut Window, bevy::ecs::query::With<PrimaryWindow>>,
) {
    let Some(focus) = focus else {
        return;
    };
    if !focus.is_changed() {
        return;
    }
    if let Ok(mut window) = windows.single_mut() {
        window.ime_enabled = focus.get().is_some_and(|entity| editors.contains(entity));
    }
}

/// Positions the native IME candidate popup next to the focused caret.
pub(crate) fn update_ime_candidate_position(
    focus: Option<Res<InputFocus>>,
    editors: Query<(
        &NativeEditableText,
        &ComputedNode,
        &UiGlobalTransform,
        &ComputedUiRenderTargetInfo,
        &TextScroll,
    )>,
    mut windows: Query<&mut Window, bevy::ecs::query::With<PrimaryWindow>>,
    scale: Option<Res<UiScale>>,
) {
    let Some(entity) = focus.as_ref().and_then(|focus| focus.get()) else {
        return;
    };
    let Ok((native, node, transform, target, scroll)) = editors.get(entity) else {
        return;
    };
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    let area = native.editor().ime_cursor_area();
    let point =
        bevy::math::Vec2::new(area.x0 as f32, area.y1 as f32) + node.content_box().min - scroll.0;
    window.ime_position = transform.affine().transform_point2(point)
        * scale.as_ref().map_or(1.0, |scale| scale.0)
        / target.scale_factor();
}

/// Clears composition and commits the value when editing focus leaves a control.
pub(crate) fn editable_focus_lost(
    trigger: On<FocusLost>,
    mut commands: Commands,
    mut editors: Query<(&EditableText, &EditableTextOptions, &mut NativeEditableText)>,
    mut committed: MessageWriter<EditableTextCommitted>,
) {
    if let Ok((state, options, mut native)) = editors.get_mut(trigger.entity) {
        native.queue_edit(TextEdit::clear_ime_compose());
        native.queue_edit(TextEdit::CollapseSelection);
        committed.write(EditableTextCommitted {
            entity: trigger.entity,
            name: options.name.clone(),
            value: state.value.clone(),
        });
        let entity = trigger.entity;
        commands.queue(move |world: &mut bevy::ecs::world::World| {
            if !world
                .get::<crate::widgets::controls::input::InputFieldOptions>(entity)
                .is_some_and(|options| options.clear_on_blur)
            {
                return;
            }
            if crate::set_editable_text(world, entity, "") {
                let name = world
                    .get::<EditableTextOptions>(entity)
                    .and_then(|options| options.name.clone());
                world
                    .resource_mut::<bevy::ecs::message::Messages<EditableTextChanged>>()
                    .write(EditableTextChanged {
                        entity,
                        name,
                        value: String::new(),
                    });
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        MinimalPlugins,
        app::App,
        asset::AssetPlugin,
        camera::NormalizedRenderTarget,
        ecs::{
            message::{MessageCursor, Messages},
            world::World,
        },
        input::{
            ButtonInput, ButtonState,
            keyboard::{Key, KeyCode, KeyboardInput},
        },
        math::Vec2,
        text::{EditableText as NativeEditableText, TextEdit, TextPlugin},
        ui::{
            ComputedNode, ComputedUiRenderTargetInfo, Node, UiGlobalTransform, widget::TextScroll,
        },
    };
    use bevy_input_focus::{FocusCause, InputFocus};
    use bevy_picking::{
        events::{Drag, Pointer},
        pointer::{Location, PointerButton, PointerId},
    };
    use tilt_ui_core::InputType;

    use crate::{EditableText, EditableTextCommitted, TiltControl, TiltUiControlRuntimePlugin};

    use crate::scroll::ActiveScrollbarDrag;

    #[test]
    fn inserted_unicode_chars_get_a_short_glyph_entrance() {
        use super::{GlyphEntrance, animate_input_text, inserted_char_range};
        use crate::widgets::state::UiMotionSettings;
        use bevy::{
            app::PostUpdate,
            math::Rect,
            text::{GlyphAtlasInfo, PositionedGlyph, TextLayoutInfo},
        };

        assert_eq!(inserted_char_range("ab", "aéb"), Some((1, 1)));
        assert_eq!(inserted_char_range("a ", "a b"), Some((1, 1)));
        assert_eq!(inserted_char_range("a", "a "), None);
        assert_eq!(inserted_char_range("abc", "ac"), None);
        let mut app = App::new();
        app.init_resource::<UiMotionSettings>()
            .add_systems(PostUpdate, animate_input_text);
        let entity = app
            .world_mut()
            .spawn((
                GlyphEntrance {
                    first: 0,
                    count: 1,
                    elapsed: 0.0,
                    targets: Vec::new(),
                },
                TextLayoutInfo {
                    glyphs: vec![PositionedGlyph {
                        position: Vec2::new(20.0, 20.0),
                        atlas_info: GlyphAtlasInfo {
                            texture: Default::default(),
                            rect: Rect::default(),
                            offset: Vec2::ZERO,
                            is_alpha_mask: true,
                        },
                        section_index: 0,
                        line_index: 0,
                    }],
                    ..Default::default()
                },
            ))
            .id();
        app.update();
        assert_ne!(
            app.world().get::<TextLayoutInfo>(entity).unwrap().glyphs[0].position,
            Vec2::new(20.0, 20.0)
        );
        for _ in 0..30 {
            app.update();
        }
        assert_eq!(
            app.world().get::<TextLayoutInfo>(entity).unwrap().glyphs[0].position,
            Vec2::new(20.0, 20.0)
        );
        assert!(app.world().get::<GlyphEntrance>(entity).is_none());
    }

    fn focused_editor(app: &mut App, multiline: bool, readonly: bool) -> bevy::ecs::entity::Entity {
        let entity = app
            .world_mut()
            .spawn((
                Node::default(),
                TiltControl,
                EditableText::new(String::new(), InputType::Text, readonly, multiline),
                crate::EditableTextOptions::default(),
                NativeEditableText::default(),
            ))
            .id();
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(entity, FocusCause::Pressed);
        entity
    }

    fn key(world: &mut World, code: KeyCode, logical: Key, text: Option<&str>) {
        let window = world.spawn_empty().id();
        world
            .resource_mut::<Messages<KeyboardInput>>()
            .write(KeyboardInput {
                key_code: code,
                logical_key: logical,
                state: ButtonState::Pressed,
                text: text.map(Into::into),
                repeat: false,
                window,
            });
    }

    fn shortcut_key() -> Key {
        if cfg!(target_os = "macos") {
            Key::Super
        } else {
            Key::Control
        }
    }

    #[test]
    fn text_input_queues_layout_aware_edits_and_commits_enter() {
        let mut app = App::new();
        app.add_plugins(TiltUiControlRuntimePlugin)
            .add_message::<KeyboardInput>();
        let entity = focused_editor(&mut app, false, false);
        key(
            app.world_mut(),
            KeyCode::KeyX,
            Key::Character("x".into()),
            Some("x"),
        );
        key(app.world_mut(), KeyCode::ArrowLeft, Key::ArrowLeft, None);
        app.world_mut()
            .get_mut::<NativeEditableText>(entity)
            .unwrap()
            .queue_edit(TextEdit::Backspace);
        key(app.world_mut(), KeyCode::Enter, Key::Enter, None);
        app.update();
        let edits = &app
            .world()
            .get::<NativeEditableText>(entity)
            .unwrap()
            .pending_edits;
        assert!(edits.contains(&TextEdit::Insert("x".into())));
        assert!(edits.contains(&TextEdit::Left(false)));
        assert!(edits.contains(&TextEdit::Backspace));
        let mut cursor = MessageCursor::<EditableTextCommitted>::default();
        assert_eq!(
            cursor
                .read(app.world().resource::<Messages<EditableTextCommitted>>())
                .count(),
            1
        );
    }

    #[test]
    fn textarea_enter_inserts_newline_while_readonly_blocks_mutation() {
        let mut app = App::new();
        app.add_plugins(TiltUiControlRuntimePlugin)
            .add_message::<KeyboardInput>();
        let area = focused_editor(&mut app, true, false);
        key(app.world_mut(), KeyCode::Enter, Key::Enter, None);
        app.update();
        assert!(
            app.world()
                .get::<NativeEditableText>(area)
                .unwrap()
                .pending_edits
                .contains(&TextEdit::Insert("\n".into()))
        );

        app.world_mut()
            .get_mut::<EditableText>(area)
            .unwrap()
            .readonly = true;
        key(
            app.world_mut(),
            KeyCode::KeyX,
            Key::Character("x".into()),
            Some("x"),
        );
        app.update();
        assert!(
            !app.world()
                .get::<NativeEditableText>(area)
                .unwrap()
                .pending_edits
                .contains(&TextEdit::Insert("x".into()))
        );
    }

    #[test]
    fn native_editor_updates_semantic_value_and_preserves_unicode_boundaries() {
        use crate::render::materialize_element;
        use tilt_ui_core::ElementKind;

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .add_plugins(TextPlugin)
            .add_plugins(TiltUiControlRuntimePlugin)
            .add_message::<KeyboardInput>();
        let entity = app.world_mut().spawn_empty().id();
        materialize_element(app.world_mut(), entity, ElementKind::Input, &[]);
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(entity, FocusCause::Pressed);
        key(
            app.world_mut(),
            KeyCode::KeyA,
            Key::Character("é".into()),
            Some("é"),
        );
        app.update();
        assert_eq!(app.world().get::<EditableText>(entity).unwrap().value, "é");
        assert_eq!(
            app.world()
                .get::<NativeEditableText>(entity)
                .unwrap()
                .editor()
                .raw_selection()
                .focus()
                .index(),
            2
        );
    }

    #[test]
    fn textarea_line_cap_rejects_extra_user_newlines_without_resetting_prior_text() {
        use crate::render::materialize_element;
        use tilt_ui_core::{ElementKind, TemplateAttribute};

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .add_plugins(TextPlugin)
            .add_plugins(TiltUiControlRuntimePlugin)
            .add_message::<KeyboardInput>();
        let entity = app.world_mut().spawn_empty().id();
        materialize_element(
            app.world_mut(),
            entity,
            ElementKind::TextArea,
            &[TemplateAttribute::Static {
                name: "max-lines".into(),
                value: "2".into(),
            }],
        );
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(entity, FocusCause::Pressed);
        key(app.world_mut(), KeyCode::Enter, Key::Enter, None);
        app.update();
        assert_eq!(app.world().get::<EditableText>(entity).unwrap().value, "\n");
        key(app.world_mut(), KeyCode::Enter, Key::Enter, None);
        app.update();
        assert_eq!(app.world().get::<EditableText>(entity).unwrap().value, "\n");
    }

    #[test]
    fn disabled_input_does_not_accept_native_keyboard_edits() {
        use crate::render::materialize_element;
        use tilt_ui_core::{ElementKind, TemplateAttribute};

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .add_plugins(TextPlugin)
            .add_plugins(TiltUiControlRuntimePlugin)
            .add_message::<KeyboardInput>();
        let entity = app.world_mut().spawn_empty().id();
        materialize_element(
            app.world_mut(),
            entity,
            ElementKind::Input,
            &[TemplateAttribute::Static {
                name: "disabled".into(),
                value: "true".into(),
            }],
        );
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(entity, FocusCause::Pressed);
        key(
            app.world_mut(),
            KeyCode::KeyA,
            Key::Character("a".into()),
            Some("a"),
        );
        app.update();
        assert!(
            app.world()
                .get::<EditableText>(entity)
                .unwrap()
                .value
                .is_empty()
        );
    }

    #[test]
    fn selection_replacement_and_backspace_commands_reach_the_native_editor() {
        use crate::render::materialize_element;
        use tilt_ui_core::{ElementKind, TemplateAttribute};

        let mut app = App::new();
        app.add_plugins(TiltUiControlRuntimePlugin)
            .add_message::<KeyboardInput>()
            .insert_resource(ButtonInput::<Key>::default());
        let entity = app.world_mut().spawn_empty().id();
        materialize_element(
            app.world_mut(),
            entity,
            ElementKind::Input,
            &[TemplateAttribute::Static {
                name: "value".into(),
                value: "TiltUI".into(),
            }],
        );
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(entity, FocusCause::Pressed);
        app.world_mut()
            .resource_mut::<ButtonInput<Key>>()
            .press(shortcut_key());
        key(
            app.world_mut(),
            KeyCode::KeyA,
            Key::Character("a".into()),
            Some("a"),
        );
        app.update();
        assert!(
            app.world()
                .get::<NativeEditableText>(entity)
                .unwrap()
                .pending_edits
                .contains(&TextEdit::SelectAll)
        );
        app.world_mut()
            .resource_mut::<ButtonInput<Key>>()
            .release(shortcut_key());
        key(
            app.world_mut(),
            KeyCode::KeyX,
            Key::Character("X".into()),
            Some("X"),
        );
        app.update();
        key(app.world_mut(), KeyCode::Backspace, Key::Backspace, None);
        app.update();
        let edits = &app
            .world()
            .get::<NativeEditableText>(entity)
            .unwrap()
            .pending_edits;
        assert!(edits.contains(&TextEdit::Insert("X".into())));
        assert!(edits.contains(&TextEdit::Backspace));
    }

    #[test]
    fn undo_and_redo_restore_editable_values_without_reapplying_static_text() {
        use crate::render::materialize_element;
        use tilt_ui_core::{ElementKind, TemplateAttribute};

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .add_plugins(TextPlugin)
            .add_plugins(TiltUiControlRuntimePlugin)
            .add_message::<KeyboardInput>()
            .insert_resource(ButtonInput::<Key>::default());
        let entity = app.world_mut().spawn_empty().id();
        materialize_element(
            app.world_mut(),
            entity,
            ElementKind::Input,
            &[TemplateAttribute::Static {
                name: "value".into(),
                value: "TiltUI".into(),
            }],
        );
        app.world_mut()
            .resource_mut::<InputFocus>()
            .set(entity, FocusCause::Pressed);
        app.update();

        key(
            app.world_mut(),
            KeyCode::KeyX,
            Key::Character("x".into()),
            Some("x"),
        );
        app.update();
        assert_eq!(
            app.world().get::<EditableText>(entity).unwrap().value,
            "TiltUIx"
        );

        app.world_mut()
            .resource_mut::<ButtonInput<Key>>()
            .press(shortcut_key());
        key(
            app.world_mut(),
            KeyCode::KeyZ,
            Key::Character("z".into()),
            None,
        );
        app.update();
        assert_eq!(
            app.world().get::<EditableText>(entity).unwrap().value,
            "TiltUI"
        );

        key(
            app.world_mut(),
            KeyCode::KeyY,
            Key::Character("y".into()),
            None,
        );
        app.update();
        assert_eq!(
            app.world().get::<EditableText>(entity).unwrap().value,
            "TiltUIx"
        );
    }

    #[test]
    fn standard_selection_and_clipboard_shortcuts_reach_native_editor() {
        let mut app = App::new();
        app.add_plugins(TiltUiControlRuntimePlugin)
            .add_message::<KeyboardInput>()
            .insert_resource(ButtonInput::<Key>::default());
        let entity = focused_editor(&mut app, false, false);
        app.world_mut()
            .resource_mut::<ButtonInput<Key>>()
            .press(shortcut_key());
        for (code, character) in [
            (KeyCode::KeyA, "a"),
            (KeyCode::KeyC, "c"),
            (KeyCode::KeyX, "x"),
            (KeyCode::KeyV, "v"),
        ] {
            key(
                app.world_mut(),
                code,
                Key::Character(character.into()),
                None,
            );
        }
        app.update();
        let edits = &app
            .world()
            .get::<NativeEditableText>(entity)
            .unwrap()
            .pending_edits;
        assert!(edits.contains(&TextEdit::SelectAll));
        assert!(edits.contains(&TextEdit::Copy));
        assert!(edits.contains(&TextEdit::Cut));
        assert!(edits.contains(&TextEdit::Paste));
    }

    #[test]
    fn captured_scrollbar_drag_never_extends_text_selection() {
        let mut app = App::new();
        app.add_plugins(TiltUiControlRuntimePlugin)
            .add_message::<Pointer<Drag>>();
        let entity = app
            .world_mut()
            .spawn((
                Node::default(),
                TiltControl,
                EditableText::new("hello".into(), InputType::Text, false, true),
                NativeEditableText::new("hello"),
                TextScroll::default(),
                ComputedNode {
                    size: Vec2::new(120.0, 50.0),
                    ..Default::default()
                },
                ComputedUiRenderTargetInfo::default(),
                UiGlobalTransform::default(),
                ActiveScrollbarDrag {
                    pointer: PointerId::Mouse,
                    vertical: true,
                    grab_offset: 0.0,
                },
            ))
            .id();
        app.world_mut()
            .get_mut::<NativeEditableText>(entity)
            .unwrap()
            .pending_edits
            .clear();
        let drag = || {
            Pointer::new(
                PointerId::Mouse,
                Location {
                    target: NormalizedRenderTarget::None {
                        width: 120,
                        height: 50,
                    },
                    position: Vec2::new(10.0, 10.0),
                },
                Drag {
                    button: PointerButton::Primary,
                    distance: Vec2::new(0.0, 10.0),
                    delta: Vec2::new(0.0, 10.0),
                },
                entity,
            )
        };
        app.world_mut()
            .resource_mut::<Messages<Pointer<Drag>>>()
            .write(drag());
        app.update();
        assert!(
            app.world()
                .get::<NativeEditableText>(entity)
                .unwrap()
                .pending_edits
                .is_empty()
        );

        app.world_mut()
            .entity_mut(entity)
            .remove::<ActiveScrollbarDrag>();
        app.world_mut()
            .resource_mut::<Messages<Pointer<Drag>>>()
            .write(drag());
        app.update();
        assert!(
            app.world()
                .get::<NativeEditableText>(entity)
                .unwrap()
                .pending_edits
                .iter()
                .any(|edit| matches!(edit, TextEdit::ExtendSelectionToPoint(_)))
        );
    }
}
