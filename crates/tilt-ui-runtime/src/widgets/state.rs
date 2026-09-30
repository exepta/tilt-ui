//! Shared semantic state used by value-oriented TiltUI widgets.

use bevy::{
    ecs::{
        component::Component,
        entity::Entity,
        message::Message,
        system::{Commands, Query, Res},
        world::World,
    },
    prelude::Visibility,
    text::{EditableText as BevyEditableText, TextEdit},
    time::Time,
    ui::{Display, Node, Val, widget::Text},
};
use tilt_ui_core::InputType;

use crate::{ElementState, widgets::controls::slider::SliderSettings};

/// Stores the normalized numeric value contract shared by range-oriented widgets.
#[derive(bevy::ecs::component::Component, Debug, Clone, Copy, PartialEq)]
pub struct NumericRange {
    /// Lowest accepted value.
    pub min: f32,
    /// Highest accepted value.
    pub max: f32,
    /// Current clamped and snapped value.
    pub value: f32,
    /// Optional positive increment used for snapping.
    pub step: Option<f32>,
}

impl NumericRange {
    /// Creates a range with a normalized domain and value.
    pub fn new(min: f32, max: f32, value: f32, step: Option<f32>) -> Self {
        let min = if min.is_finite() { min } else { 0.0 };
        let max = if max.is_finite() { max } else { min };
        let (min, max) = if min <= max { (min, max) } else { (max, min) };
        let step = step.filter(|step| *step > 0.0 && step.is_finite());
        let mut range = Self {
            min,
            max,
            value: min,
            step,
        };
        range.value = range.normalize(value);
        range
    }

    /// Returns the value normalized into the inclusive range and optional step grid.
    pub fn normalize(self, value: f32) -> f32 {
        if !value.is_finite() {
            return self.min;
        }
        let value = value.clamp(self.min, self.max);
        match self.step {
            Some(step) => {
                (self.min + ((value - self.min) / step).round() * step).clamp(self.min, self.max)
            }
            None => value,
        }
    }

    /// Returns the current position as a value in the inclusive `0.0..=1.0` interval.
    pub fn fraction(self) -> f32 {
        let span = self.max - self.min;
        if span <= f32::EPSILON {
            0.0
        } else {
            ((self.value - self.min) / span).clamp(0.0, 1.0)
        }
    }
}

/// Orientation of a numeric value control.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RangeOrientation {
    /// Values increase from left to right.
    #[default]
    Horizontal,
    /// Values increase from bottom to top.
    Vertical,
}

/// References the persistent geometry of a numeric widget.
#[derive(bevy::ecs::component::Component, Debug, Clone, Copy)]
pub struct NumericParts {
    /// Track entity used for pointer coordinate mapping.
    pub track: Entity,
    /// Fill entity representing the selected portion.
    pub fill: Entity,
    /// Primary thumb for an interactive slider.
    pub thumb: Option<Entity>,
    /// Secondary thumb for a range slider.
    pub second_thumb: Option<Entity>,
    /// Optional persistent text displaying the current value.
    pub tip: Option<Entity>,
    /// Optional persistent text displaying the upper range endpoint.
    pub second_tip: Option<Entity>,
    /// Layout orientation.
    pub orientation: RangeOrientation,
}

/// Stores semantic layout overrides that must survive CSS restyling.
#[derive(bevy::ecs::component::Component, Debug, Clone, Copy, Default, PartialEq)]
pub struct WidgetLayoutOverride {
    /// Value-derived width.
    pub width: Option<Val>,
    /// Value-derived height.
    pub height: Option<Val>,
    /// Value-derived maximum height.
    pub max_height: Option<Val>,
    /// Value-derived left offset.
    pub left: Option<Val>,
    /// Value-derived top offset.
    pub top: Option<Val>,
    /// Value-derived bottom offset.
    pub bottom: Option<Val>,
}

/// Durations for small interaction animations, in seconds. Set a duration to zero to disable it.
#[derive(bevy::ecs::resource::Resource, Debug, Clone, Copy)]
pub struct UiMotionSettings {
    pub slider_seconds: f32,
    pub wheel_seconds: f32,
    pub caret_seconds: f32,
    pub input_text_seconds: f32,
}

impl Default for UiMotionSettings {
    fn default() -> Self {
        Self {
            slider_seconds: 0.10,
            wheel_seconds: 0.08,
            caret_seconds: 0.075,
            input_text_seconds: 0.28,
        }
    }
}

#[derive(Component)]
pub(crate) struct WidgetLayoutMotion {
    from: WidgetLayoutOverride,
    elapsed: f32,
}

fn mix_val(from: Val, to: Val, amount: f32) -> Val {
    match (from, to) {
        (Val::Px(from), Val::Px(to)) => Val::Px(from + (to - from) * amount),
        (Val::Percent(from), Val::Percent(to)) => Val::Percent(from + (to - from) * amount),
        _ => to,
    }
}

fn mix_field(field: &mut Val, from: Option<Val>, to: Option<Val>, amount: f32) {
    if let (Some(from), Some(to)) = (from, to) {
        *field = mix_val(from, to, amount);
    }
}

pub(crate) fn animate_widget_layout(
    time: Option<Res<Time>>,
    settings: Res<UiMotionSettings>,
    mut controls: Query<(
        Entity,
        &WidgetLayoutOverride,
        &mut Node,
        &mut WidgetLayoutMotion,
    )>,
    mut commands: Commands,
) {
    let delta = time.as_ref().map_or(1.0 / 60.0, |time| time.delta_secs());
    for (entity, target, mut node, mut motion) in &mut controls {
        motion.elapsed += delta;
        let duration = settings.slider_seconds.max(f32::EPSILON);
        let progress = (motion.elapsed / duration).clamp(0.0, 1.0);
        let amount = 1.0 - (1.0 - progress).powi(3);
        mix_field(&mut node.width, motion.from.width, target.width, amount);
        mix_field(&mut node.height, motion.from.height, target.height, amount);
        mix_field(&mut node.left, motion.from.left, target.left, amount);
        mix_field(&mut node.top, motion.from.top, target.top, amount);
        mix_field(&mut node.bottom, motion.from.bottom, target.bottom, amount);
        if progress >= 1.0 {
            target.apply(&mut node);
            commands.entity(entity).remove::<WidgetLayoutMotion>();
        }
    }
}

impl WidgetLayoutOverride {
    pub(crate) fn apply(self, node: &mut bevy::ui::Node) {
        if let Some(value) = self.width {
            node.width = value;
        }
        if let Some(value) = self.height {
            node.height = value;
        }
        if let Some(value) = self.max_height {
            node.max_height = value;
        }
        if let Some(value) = self.left {
            node.left = value;
        }
        if let Some(value) = self.top {
            node.top = value;
        }
        if let Some(value) = self.bottom {
            node.bottom = value;
        }
    }
}

/// Hides a persistent widget surface without removing its authored CSS style.
#[derive(bevy::ecs::component::Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidgetDisplayOverride {
    /// Whether the surface is removed from Bevy UI layout and picking.
    pub hidden: bool,
}

/// Opens or closes a persistent widget surface without rebuilding its hierarchy.
pub(crate) fn set_widget_display(world: &mut World, entity: Entity, visible: bool) {
    let display = if visible {
        world
            .get::<crate::style::RuntimeComputedStyle>(entity)
            .and_then(|style| style.0.display)
            .map(crate::style::convert::display)
            .unwrap_or(Display::Flex)
    } else {
        Display::None
    };
    if world
        .get::<Node>(entity)
        .is_some_and(|node| node.display != display)
    {
        world.get_mut::<Node>(entity).unwrap().display = display;
    }
    if world
        .get::<WidgetDisplayOverride>(entity)
        .is_none_or(|old| old.hidden != !visible)
    {
        world
            .entity_mut(entity)
            .insert(WidgetDisplayOverride { hidden: !visible });
    }
}

/// Stores an editable string and cursor data for text controls.
#[derive(bevy::ecs::component::Component, Debug, Clone, PartialEq, Eq)]
pub struct EditableText {
    /// Semantic unmasked text value.
    pub value: String,
    /// UTF-8 byte offset of the caret.
    pub cursor: usize,
    /// Optional UTF-8 byte offset that anchors the selection.
    pub selection_anchor: Option<usize>,
    /// Input-specific presentation and validation semantics.
    pub input_type: InputType,
    /// Indicates whether the value cannot be edited by user input.
    pub readonly: bool,
    /// Indicates whether line breaks are accepted.
    pub multiline: bool,
}

/// Stores immutable template configuration for an editable control.
#[derive(bevy::ecs::component::Component, Debug, Clone, PartialEq, Eq, Default)]
pub struct EditableTextOptions {
    /// Optional form name.
    pub name: Option<String>,
    /// Whether a nonempty value is required.
    pub required: bool,
    /// Minimum accepted character count.
    pub min_length: Option<usize>,
    /// Maximum accepted character count.
    pub max_characters: Option<usize>,
    /// Maximum logical line count for a text area.
    pub max_lines: Option<usize>,
    /// Regular expression that must match the complete nonempty value.
    pub pattern: Option<String>,
    /// Optional numeric lower bound.
    pub min: Option<String>,
    /// Optional numeric upper bound.
    pub max: Option<String>,
    /// Optional numeric increment.
    pub step: Option<String>,
}

/// References the persistent visual entities used by an editable text control.
#[derive(bevy::ecs::component::Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EditableTextParts {
    pub(crate) value: Entity,
    pub(crate) placeholder: Entity,
    pub(crate) selection: Entity,
    pub(crate) cursor: Entity,
}

impl EditableText {
    /// Creates an editable value with its caret at the end of the supplied text.
    pub fn new(value: String, input_type: InputType, readonly: bool, multiline: bool) -> Self {
        let cursor = value.len();
        Self {
            value,
            cursor,
            selection_anchor: None,
            input_type,
            readonly,
            multiline,
        }
    }
}

/// Returns whether an editable value violates its static semantic constraints.
pub(crate) fn editable_invalid(
    value: &str,
    input_type: InputType,
    options: &EditableTextOptions,
) -> bool {
    let count = value.chars().count();
    if options.required && value.is_empty() {
        return true;
    }
    if value.is_empty() {
        return false;
    }
    if options.min_length.is_some_and(|min| count < min)
        || options.max_characters.is_some_and(|max| count > max)
        || options
            .max_lines
            .is_some_and(|max| value.split('\n').count() > max)
    {
        return true;
    }
    if options.pattern.as_ref().is_some_and(|pattern| {
        regex::Regex::new(&format!("^(?:{pattern})$")).map_or(true, |rule| !rule.is_match(value))
    }) {
        return true;
    }
    match input_type {
        InputType::Email => {
            let mut pieces = value.split('@');
            let Some(local) = pieces.next() else {
                return true;
            };
            let Some(domain) = pieces.next() else {
                return true;
            };
            local.is_empty()
                || domain.split('.').any(str::is_empty)
                || !domain.contains('.')
                || pieces.next().is_some()
                || value.chars().any(char::is_whitespace)
        }
        InputType::Number | InputType::Range => {
            let number = if input_type == InputType::Number {
                crate::widgets::controls::input::number_expression(value)
            } else {
                value.parse::<f64>().ok()
            };
            let Some(number) = number.filter(|number| number.is_finite()) else {
                return true;
            };
            options
                .min
                .as_deref()
                .and_then(|min| min.parse::<f64>().ok())
                .is_some_and(|min| number < min)
                || options
                    .max
                    .as_deref()
                    .and_then(|max| max.parse::<f64>().ok())
                    .is_some_and(|max| number > max)
                || options
                    .step
                    .as_deref()
                    .and_then(|step| step.parse::<f64>().ok())
                    .filter(|step| *step > 0.0)
                    .is_some_and(|step| {
                        let base = options
                            .min
                            .as_deref()
                            .and_then(|min| min.parse::<f64>().ok())
                            .unwrap_or(0.0);
                        let steps = (number - base) / step;
                        (steps - steps.round()).abs() > 1e-7
                    })
        }
        InputType::Date => !valid_iso_date(value),
        _ => false,
    }
}

pub(crate) fn masked_password(value: &str) -> String {
    "*".repeat(value.chars().count())
}

fn valid_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    if bytes
        .iter()
        .enumerate()
        .any(|(index, byte)| index != 4 && index != 7 && !byte.is_ascii_digit())
    {
        return false;
    }
    let (Ok(year), Ok(month), Ok(day)) = (
        value[0..4].parse::<u32>(),
        value[5..7].parse::<u32>(),
        value[8..10].parse::<u32>(),
    ) else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    day >= 1 && day <= days
}

/// Updates a range value without emitting a user-originated message.
///
/// Returns `true` when the entity owns a numeric range and its value changed.
pub fn set_numeric_value(world: &mut World, entity: Entity, value: f32) -> bool {
    let Some(fraction) = ({
        let Some(mut range) = world.get_mut::<NumericRange>(entity) else {
            return false;
        };
        let normalized = range.normalize(value);
        if (range.value - normalized).abs() <= f32::EPSILON {
            return false;
        }
        range.value = normalized;
        Some(range.fraction())
    }) else {
        return false;
    };
    update_fill_width(world, entity, fraction);
    if let Some(tip) = world
        .get::<NumericParts>(entity)
        .and_then(|parts| parts.tip)
        && let Some(value) = world.get::<NumericRange>(entity).map(|range| range.value)
        && let Some(mut text) = world.get_mut::<Text>(tip)
    {
        text.0 = format!("{value}");
    }
    true
}

/// Reconfigures an existing numeric widget while retaining its entity and parts.
pub(crate) fn set_numeric_bound(
    world: &mut World,
    entity: Entity,
    field: &str,
    value: f32,
) -> bool {
    if !value.is_finite() || (field == "step" && value <= 0.0) {
        return false;
    }
    let Some(old) = world.get::<NumericRange>(entity).copied() else {
        return false;
    };
    let (min, max, step) = match field {
        "min" => (value, old.max, old.step),
        "max" => (old.min, value, old.step),
        "step" => (old.min, old.max, Some(value)),
        _ => return false,
    };
    let next = NumericRange::new(min, max, old.value, step);
    if next == old {
        return false;
    }
    *world.get_mut::<NumericRange>(entity).unwrap() = next;
    if let Some(settings) = world.get::<SliderSettings>(entity).copied()
        && settings.kind == tilt_ui_core::SliderType::Range
    {
        crate::set_slider_values(world, entity, settings.lower, settings.upper);
        update_range_layout(world, entity);
    } else {
        update_fill_width(world, entity, next.fraction());
        if let Some(tip) = world
            .get::<NumericParts>(entity)
            .and_then(|parts| parts.tip)
            && let Some(mut text) = world.get_mut::<Text>(tip)
        {
            text.0 = format!("{}", next.value);
        }
    }
    true
}

/// Updates both endpoints of a range slider without emitting a user event.
pub fn set_slider_values(world: &mut World, entity: Entity, lower: f32, upper: f32) -> bool {
    let Some(range) = world.get::<NumericRange>(entity).copied() else {
        return false;
    };
    let low = range.normalize(lower).min(range.normalize(upper));
    let high = range.normalize(upper).max(low);
    let Some(mut settings) = world.get_mut::<SliderSettings>(entity) else {
        return false;
    };
    if settings.kind != tilt_ui_core::SliderType::Range
        || (settings.lower == low && settings.upper == high)
    {
        return false;
    }
    settings.lower = low;
    settings.upper = high;
    if let Some(parts) = world.get::<NumericParts>(entity).copied() {
        for (tip, value) in [(parts.tip, low), (parts.second_tip, high)] {
            if let Some(tip) = tip
                && let Some(mut text) = world.get_mut::<Text>(tip)
            {
                text.0 = format!("{value}");
            }
        }
    }
    update_range_layout(world, entity);
    true
}

pub(crate) fn update_range_layout(world: &mut World, entity: Entity) {
    let Some(range) = world.get::<NumericRange>(entity).copied() else {
        return;
    };
    let Some(settings) = world.get::<SliderSettings>(entity).copied() else {
        return;
    };
    let Some(parts) = world.get::<NumericParts>(entity).copied() else {
        return;
    };
    let fraction = |value: f32| {
        if range.max <= range.min {
            0.0
        } else {
            ((value - range.min) / (range.max - range.min)).clamp(0.0, 1.0)
        }
    };
    let low = fraction(settings.lower) * 100.0;
    let high = fraction(settings.upper) * 100.0;
    set_layout(world, parts.fill, |layout| match parts.orientation {
        RangeOrientation::Horizontal => {
            layout.left = Some(Val::Percent(low));
            layout.width = Some(Val::Percent(high - low));
        }
        RangeOrientation::Vertical => {
            layout.bottom = Some(Val::Percent(low));
            layout.height = Some(Val::Percent(high - low));
        }
    });
    if let Some(thumb) = parts.thumb {
        set_layout(world, thumb, |layout| match parts.orientation {
            RangeOrientation::Horizontal => layout.left = Some(Val::Percent(low)),
            RangeOrientation::Vertical => layout.bottom = Some(Val::Percent(low)),
        });
    }
    if let Some(thumb) = parts.second_thumb {
        set_layout(world, thumb, |layout| match parts.orientation {
            RangeOrientation::Horizontal => layout.left = Some(Val::Percent(high)),
            RangeOrientation::Vertical => layout.bottom = Some(Val::Percent(high)),
        });
    }
}

/// Updates an editable value without emitting a user-originated message.
///
/// Returns `true` when the entity owns editable text and its value changed.
pub fn set_editable_text(world: &mut World, entity: Entity, value: impl Into<String>) -> bool {
    let value = value.into();
    if world
        .get::<EditableText>(entity)
        .is_some_and(|state| state.input_type == InputType::Number)
        && !crate::widgets::controls::input::number_characters_allowed(&value)
    {
        return false;
    }
    {
        let Some(mut state) = world.get_mut::<EditableText>(entity) else {
            return false;
        };
        if state.value == value {
            return false;
        }
        state.cursor = value.len();
        state.selection_anchor = None;
        state.value = value.clone();
    }

    if let Some(mut history) = world.get_mut::<crate::control::EditableHistory>(entity) {
        history.clear();
    }
    world
        .entity_mut(entity)
        .remove::<crate::scroll::UserTextScroll>();

    if let Some(mut native) = world.get_mut::<BevyEditableText>(entity) {
        native.editor_mut().set_text(&value);
        native.queue_edit(TextEdit::TextEnd(false));
    }
    if let Some(parts) = world.get::<EditableTextParts>(entity)
        && let Some(mut placeholder) = world.get_mut::<Visibility>(parts.placeholder)
    {
        *placeholder = if value.is_empty() {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    let displayed = world
        .get::<EditableText>(entity)
        .map(|state| {
            if state.input_type == InputType::Password {
                masked_password(&state.value)
            } else {
                state.value.clone()
            }
        })
        .unwrap_or_default();
    if let Some(parts) = world.get::<EditableTextParts>(entity)
        && let Some(mut text) = world.get_mut::<Text>(parts.value)
    {
        text.0 = displayed;
    }
    let invalid = match (
        world.get::<EditableText>(entity),
        world.get::<EditableTextOptions>(entity),
    ) {
        (Some(state), Some(options)) => editable_invalid(&state.value, state.input_type, options),
        _ => false,
    };
    #[cfg(feature = "component")]
    let managed_by_form = crate::widgets::structure::form::parent_form(world, entity).is_some();
    #[cfg(not(feature = "component"))]
    let managed_by_form = false;
    if !managed_by_form {
        if let Some(mut state) = world.get_mut::<ElementState>(entity) {
            if state.invalid != invalid {
                state.invalid = invalid;
            }
        }
    }
    #[cfg(feature = "component")]
    crate::widgets::structure::form::mark_form_dirty(world, entity);
    true
}

/// Changes readonly editing behavior and its CSS projection without emitting a user event.
pub fn set_editable_readonly(world: &mut World, entity: Entity, readonly: bool) -> bool {
    let Some(mut editable) = world.get_mut::<EditableText>(entity) else {
        return false;
    };
    if editable.readonly == readonly {
        return false;
    }
    editable.readonly = readonly;
    if let Some(mut css_state) = world.get_mut::<ElementState>(entity) {
        css_state.readonly = readonly;
    }
    true
}

/// Applies a user-resized TextArea size without changing the author stylesheet.
pub fn set_text_area_size(world: &mut World, entity: Entity, width: f32, height: f32) -> bool {
    if !world
        .get::<EditableText>(entity)
        .is_some_and(|state| state.multiline)
        || !width.is_finite()
        || !height.is_finite()
    {
        return false;
    }
    let (min_width, max_width, min_height, max_height) = match world.get::<bevy::ui::Node>(entity) {
        Some(node) => (
            px(node.min_width).unwrap_or(100.0),
            px(node.max_width).unwrap_or(f32::MAX),
            px(node.min_height).unwrap_or(60.0),
            px(node.max_height).unwrap_or(f32::MAX),
        ),
        None => return false,
    };
    let width = width.clamp(min_width, max_width.max(min_width));
    let height = height.clamp(min_height, max_height.max(min_height));
    set_layout(world, entity, |layout| {
        layout.width = Some(Val::Px(width));
        layout.height = Some(Val::Px(height));
    });
    true
}

fn px(value: Val) -> Option<f32> {
    match value {
        Val::Px(value) => Some(value),
        _ => None,
    }
}

/// Synchronizes a range fill part after a semantic numeric value change.
pub(crate) fn update_fill_width(world: &mut World, owner: Entity, fraction: f32) {
    let Some(parts) = world.get::<NumericParts>(owner).copied() else {
        return;
    };
    let percent = Val::Percent(fraction.clamp(0.0, 1.0) * 100.0);
    set_layout(world, parts.fill, |layout| match parts.orientation {
        RangeOrientation::Horizontal => layout.width = Some(percent),
        RangeOrientation::Vertical => layout.height = Some(percent),
    });
    if let Some(thumb) = parts.thumb {
        set_layout(world, thumb, |layout| match parts.orientation {
            RangeOrientation::Horizontal => layout.left = Some(percent),
            RangeOrientation::Vertical => layout.bottom = Some(percent),
        });
    }
}

pub(crate) fn set_layout(
    world: &mut World,
    entity: Entity,
    change: impl FnOnce(&mut WidgetLayoutOverride),
) {
    let previous = world.get::<WidgetLayoutOverride>(entity).copied();
    let mut layout = previous.unwrap_or_default();
    change(&mut layout);
    if previous == Some(layout) {
        return;
    }
    let animate = previous.is_some()
        && world
            .get_resource::<UiMotionSettings>()
            .is_some_and(|settings| settings.slider_seconds > 0.0);
    let from = world.get::<Node>(entity).map(|node| WidgetLayoutOverride {
        width: layout.width.map(|_| node.width),
        height: layout.height.map(|_| node.height),
        max_height: None,
        left: layout.left.map(|_| node.left),
        top: layout.top.map(|_| node.top),
        bottom: layout.bottom.map(|_| node.bottom),
    });
    if animate && let Some(from) = from {
        world
            .entity_mut(entity)
            .insert((layout, WidgetLayoutMotion { from, elapsed: 0.0 }));
    } else {
        if let Some(mut node) = world.get_mut::<Node>(entity) {
            layout.apply(&mut node);
        }
        world.entity_mut(entity).insert(layout);
    }
}

/// Notification emitted when user interaction changes a numeric widget value.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct NumericValueChanged {
    /// Entity whose numeric value changed.
    pub entity: Entity,
    /// New normalized numeric value.
    pub value: f32,
}

/// Notification emitted while user interaction changes a slider value.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct SliderChanged {
    /// Slider entity.
    pub entity: Entity,
    /// Current single value or lower range endpoint.
    pub value: f32,
    /// Upper endpoint when the slider is in range mode.
    pub upper: Option<f32>,
}

/// Notification emitted when a slider gesture finishes.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct SliderCommitted {
    /// Slider entity.
    pub entity: Entity,
    /// Committed single value or lower range endpoint.
    pub value: f32,
    /// Upper endpoint when the slider is in range mode.
    pub upper: Option<f32>,
}

/// Notification emitted when user interaction changes editable text.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct EditableTextChanged {
    /// Entity whose editable value changed.
    pub entity: Entity,
    /// Optional form name configured on the editable control.
    pub name: Option<String>,
    /// New semantic text value.
    pub value: String,
}

/// Notification emitted when a text control completes an editing gesture.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct EditableTextCommitted {
    /// Entity whose editable value was committed.
    pub entity: Entity,
    /// Optional form name configured on the editable control.
    pub name: Option<String>,
    /// Committed semantic text value.
    pub value: String,
}

#[cfg(test)]
mod tests {
    use bevy::{
        app::{App, Update},
        ecs::world::World,
        ui::{Node, Val},
    };

    use super::{
        NumericRange, UiMotionSettings, animate_widget_layout, set_editable_text, set_layout,
        set_numeric_value,
    };
    use crate::{ControlPart, ControlPartKind, EditableText, widgets::controls::spawn_text_part};

    #[test]
    fn value_layout_moves_between_start_and_target_then_settles() {
        let mut app = App::new();
        app.init_resource::<UiMotionSettings>()
            .add_systems(Update, animate_widget_layout);
        let fill = app.world_mut().spawn(Node::default()).id();
        set_layout(app.world_mut(), fill, |layout| {
            layout.width = Some(Val::Percent(0.0))
        });
        set_layout(app.world_mut(), fill, |layout| {
            layout.width = Some(Val::Percent(100.0))
        });
        assert_eq!(
            app.world().get::<Node>(fill).unwrap().width,
            Val::Percent(0.0)
        );
        app.update();
        assert!(
            matches!(app.world().get::<Node>(fill).unwrap().width, Val::Percent(value) if value > 0.0 && value < 100.0)
        );
        for _ in 0..8 {
            app.update();
        }
        assert_eq!(
            app.world().get::<Node>(fill).unwrap().width,
            Val::Percent(100.0)
        );
    }

    #[test]
    fn numeric_range_clamps_snaps_and_updates_its_persistent_fill() {
        let mut world = World::new();
        let owner = world
            .spawn(NumericRange::new(0.0, 10.0, 0.0, Some(2.0)))
            .id();
        let fill = world
            .spawn((
                Node::default(),
                ControlPart {
                    owner,
                    kind: ControlPartKind::Fill,
                },
            ))
            .id();
        world.entity_mut(owner).insert(super::NumericParts {
            track: fill,
            fill,
            thumb: None,
            second_thumb: None,
            tip: None,
            second_tip: None,
            orientation: super::RangeOrientation::Horizontal,
        });

        assert!(set_numeric_value(&mut world, owner, 7.1));
        assert_eq!(world.get::<NumericRange>(owner).unwrap().value, 8.0);
        assert_eq!(world.get::<Node>(fill).unwrap().width, Val::Percent(80.0));
    }

    #[test]
    fn programmatic_text_update_reuses_the_existing_value_part() {
        let mut world = World::new();
        let owner = world
            .spawn(EditableText::new(
                String::from("old"),
                tilt_ui_core::InputType::Text,
                false,
                false,
            ))
            .id();
        let value = spawn_text_part(&mut world, owner, ControlPartKind::Value, "old");
        world.entity_mut(owner).insert(super::EditableTextParts {
            value,
            placeholder: value,
            selection: value,
            cursor: value,
        });

        assert!(set_editable_text(&mut world, owner, "new"));
        assert_eq!(world.get::<EditableText>(owner).unwrap().value, "new");
        assert_eq!(world.get::<bevy::ui::widget::Text>(value).unwrap().0, "new");
    }

    #[test]
    fn editable_setter_preserves_native_text_and_static_initialization_does_not_reapply() {
        use crate::render::materialize_element;
        use tilt_ui_core::{ElementKind, TemplateAttribute};

        let mut world = World::new();
        let entity = world.spawn_empty().id();
        materialize_element(
            &mut world,
            entity,
            ElementKind::Input,
            &[TemplateAttribute::Static {
                name: "value".into(),
                value: "TiltUI".into(),
            }],
        );
        let parts = *world.get::<super::EditableTextParts>(entity).unwrap();
        assert!(set_editable_text(&mut world, entity, "TiltUI Framework"));
        assert_eq!(
            world.get::<EditableText>(entity).unwrap().value,
            "TiltUI Framework"
        );
        assert_eq!(
            world
                .get::<bevy::text::EditableText>(entity)
                .unwrap()
                .value()
                .to_string(),
            "TiltUI Framework"
        );
        assert_eq!(
            *world
                .get::<bevy::prelude::Visibility>(parts.placeholder)
                .unwrap(),
            bevy::prelude::Visibility::Hidden
        );
        assert!(!set_editable_text(&mut world, entity, "TiltUI Framework"));
        assert_eq!(
            world.get::<EditableText>(entity).unwrap().value,
            "TiltUI Framework"
        );
    }

    #[test]
    fn password_setter_masks_only_display_text() {
        use crate::render::materialize_element;
        use tilt_ui_core::{ElementKind, TemplateAttribute};

        let mut world = World::new();
        let entity = world.spawn_empty().id();
        materialize_element(
            &mut world,
            entity,
            ElementKind::Input,
            &[
                TemplateAttribute::Static {
                    name: "type".into(),
                    value: "password".into(),
                },
                TemplateAttribute::Static {
                    name: "value".into(),
                    value: "secret".into(),
                },
            ],
        );
        let parts = *world.get::<super::EditableTextParts>(entity).unwrap();
        assert_eq!(
            world.get::<bevy::ui::widget::Text>(parts.value).unwrap().0,
            "******"
        );
        assert!(set_editable_text(&mut world, entity, "new"));
        assert_eq!(world.get::<EditableText>(entity).unwrap().value, "new");
        assert_eq!(
            world.get::<bevy::ui::widget::Text>(parts.value).unwrap().0,
            "***"
        );
    }

    #[test]
    fn validation_keeps_number_drafts_but_marks_them_invalid() {
        use crate::render::materialize_element;
        use tilt_ui_core::{ElementKind, TemplateAttribute};

        let mut world = World::new();
        let entity = world.spawn_empty().id();
        materialize_element(
            &mut world,
            entity,
            ElementKind::Input,
            &[TemplateAttribute::Static {
                name: "type".into(),
                value: "number".into(),
            }],
        );
        for draft in ["-", "10.", "-0."] {
            assert!(set_editable_text(&mut world, entity, draft));
            assert_eq!(world.get::<EditableText>(entity).unwrap().value, draft);
        }
        assert!(set_editable_text(&mut world, entity, "-"));
        assert!(world.get::<crate::ElementState>(entity).unwrap().invalid);
        assert!(set_editable_text(&mut world, entity, "10.5"));
        assert!(!world.get::<crate::ElementState>(entity).unwrap().invalid);
    }

    #[test]
    fn progress_and_slider_setters_keep_parts_and_clamp_values() {
        use crate::render::materialize_element;
        use tilt_ui_core::{ElementKind, TemplateAttribute};

        let mut world = World::new();
        let slider = world.spawn_empty().id();
        materialize_element(
            &mut world,
            slider,
            ElementKind::Slider,
            &[
                TemplateAttribute::Static {
                    name: "min".into(),
                    value: "0".into(),
                },
                TemplateAttribute::Static {
                    name: "max".into(),
                    value: "100".into(),
                },
                TemplateAttribute::Static {
                    name: "step".into(),
                    value: "5".into(),
                },
            ],
        );
        let slider_parts = *world.get::<super::NumericParts>(slider).unwrap();
        assert!(crate::widgets::controls::slider::set_slider_value(
            &mut world, slider, 62.0
        ));
        assert_eq!(world.get::<NumericRange>(slider).unwrap().value, 60.0);
        assert!(
            matches!(world.get::<bevy::ui::Node>(slider_parts.fill).unwrap().width, Val::Percent(value) if (value - 60.0).abs() < 0.001)
        );
        assert_eq!(
            world.get::<super::NumericParts>(slider).unwrap().thumb,
            slider_parts.thumb
        );

        let progress = world.spawn_empty().id();
        materialize_element(&mut world, progress, ElementKind::ProgressBar, &[]);
        let progress_parts = *world.get::<super::NumericParts>(progress).unwrap();
        assert!(crate::widgets::advanced::progress_bar::set_progress_value(
            &mut world, progress, 120.0
        ));
        assert_eq!(world.get::<NumericRange>(progress).unwrap().value, 100.0);
        assert_eq!(
            world
                .get::<bevy::ui::Node>(progress_parts.fill)
                .unwrap()
                .width,
            Val::Percent(100.0)
        );
        assert_eq!(
            world.get::<super::NumericParts>(progress).unwrap().fill,
            progress_parts.fill
        );

        let vertical = world.spawn_empty().id();
        materialize_element(
            &mut world,
            vertical,
            ElementKind::ProgressBar,
            &[TemplateAttribute::Static {
                name: "orientation".into(),
                value: "vertical".into(),
            }],
        );
        let vertical_parts = *world.get::<super::NumericParts>(vertical).unwrap();
        assert!(crate::widgets::advanced::progress_bar::set_progress_value(
            &mut world, vertical, 72.0
        ));
        assert_eq!(
            vertical_parts.orientation,
            super::RangeOrientation::Vertical
        );
        assert_eq!(
            world
                .get::<bevy::ui::Node>(vertical_parts.fill)
                .unwrap()
                .height,
            Val::Percent(72.0)
        );
    }

    #[test]
    fn zero_or_nonfinite_ranges_never_propagate_nan_to_layout() {
        let zero = NumericRange::new(5.0, 5.0, f32::NAN, None);
        assert_eq!(zero.value, 5.0);
        assert_eq!(zero.fraction(), 0.0);
        let invalid = NumericRange::new(f32::NAN, f32::INFINITY, 4.0, Some(f32::NAN));
        assert_eq!(invalid.min, 0.0);
        assert_eq!(invalid.max, 0.0);
        assert_eq!(invalid.value, 0.0);
    }

    #[test]
    fn textarea_resize_clamps_and_preserves_handle_identity() {
        use crate::render::materialize_element;
        use tilt_ui_core::ElementKind;

        let mut world = World::new();
        let area = world.spawn_empty().id();
        materialize_element(&mut world, area, ElementKind::TextArea, &[]);
        let original = *world.get::<super::EditableTextParts>(area).unwrap();
        let handle = world
            .get::<bevy::ecs::hierarchy::Children>(area)
            .unwrap()
            .iter()
            .find(|child| {
                world
                    .get::<ControlPart>(**child)
                    .is_some_and(|part| part.kind == ControlPartKind::ResizeHandle)
            })
            .copied()
            .unwrap();
        assert!(super::set_text_area_size(&mut world, area, 420.0, 220.0));
        assert_eq!(
            world.get::<bevy::ui::Node>(area).unwrap().width,
            Val::Px(420.0)
        );
        assert!(super::set_text_area_size(&mut world, area, 10.0, 10.0));
        assert_eq!(
            world.get::<bevy::ui::Node>(area).unwrap().width,
            Val::Px(100.0)
        );
        assert_eq!(
            *world.get::<super::EditableTextParts>(area).unwrap(),
            original
        );
        assert!(world.get::<ControlPart>(handle).is_some());
    }
}
