//! Semantic range state and persistent anatomy for the TiltUI slider.

use bevy::ecs::{component::Component, entity::Entity, world::World};
use tilt_ui_core::{SliderDotAnchor, SliderType, TemplateAttribute};

use crate::{
    ControlPartKind, NumericRange,
    widgets::state::{NumericParts, RangeOrientation, set_layout, update_fill_width},
};

use super::{spawn_part, spawn_text_part};

/// Sets a single-value Slider without emitting a user-originated change event.
pub fn set_slider_value(world: &mut World, entity: Entity, value: f32) -> bool {
    if world
        .get::<SliderSettings>(entity)
        .is_none_or(|settings| settings.kind != SliderType::Default)
    {
        return false;
    }
    crate::set_numeric_value(world, entity, value)
}

/// Stores slider configuration and the selected interval in range mode.
#[derive(Component, Debug, Clone, Copy)]
pub struct SliderSettings {
    /// Single-value or two-thumb range behavior.
    pub kind: SliderType,
    /// Lower endpoint in range mode.
    pub lower: f32,
    /// Upper endpoint in range mode.
    pub upper: f32,
    /// Optional number of visible dot intervals.
    pub dots: Option<u32>,
    /// Whether endpoint labels are visible.
    pub show_labels: bool,
    /// Whether the current value is shown near the thumb.
    pub show_tip: bool,
    /// Side on which dots are anchored.
    pub dot_anchor: SliderDotAnchor,
    /// Thumb selected for keyboard changes in range mode.
    pub active_thumb: u8,
}

pub(crate) fn materialize_parts(
    world: &mut World,
    owner: Entity,
    attributes: &[TemplateAttribute],
) {
    let range = numeric_range(attributes);
    let orientation = orientation(attributes);
    let kind = if static_value(attributes, "type").is_some_and(|value| value == "range") {
        SliderType::Range
    } else {
        SliderType::Default
    };
    let lower = range.normalize(number(attributes, "range-start").unwrap_or(range.min));
    let upper = range.normalize(number(attributes, "range-end").unwrap_or(range.max));
    let settings = SliderSettings {
        kind,
        lower: lower.min(upper),
        upper: upper.max(lower),
        dots: number(attributes, "dots").and_then(|value| {
            (value.is_finite() && (1.0..=100.0).contains(&value)).then_some(value as u32)
        }),
        show_labels: crate::component::has_boolean_static_attribute(attributes, "show-labels"),
        show_tip: crate::component::has_boolean_static_attribute(attributes, "show-tip"),
        dot_anchor: if static_value(attributes, "dot-anchor").is_some_and(|value| value == "bottom")
        {
            SliderDotAnchor::Bottom
        } else {
            SliderDotAnchor::Top
        },
        active_thumb: 0,
    };
    let track = spawn_part(world, owner, ControlPartKind::Track);
    let fill = spawn_part(world, owner, ControlPartKind::Fill);
    let thumb = spawn_part(world, owner, ControlPartKind::Thumb);
    world.entity_mut(owner).add_child(track);
    world.entity_mut(track).add_child(fill);
    world.entity_mut(track).add_child(thumb);
    let second_thumb = (kind == SliderType::Range).then(|| {
        let entity = spawn_part(world, owner, ControlPartKind::Thumb);
        world.entity_mut(track).add_child(entity);
        entity
    });
    world.entity_mut(owner).insert((range, settings));

    if let Some(intervals) = settings.dots {
        for index in 0..=intervals {
            let dot = spawn_part(world, owner, ControlPartKind::Dot);
            world.entity_mut(track).add_child(dot);
            set_layout(world, dot, |layout| {
                layout.left = Some(bevy::ui::Val::Percent(
                    index as f32 * 100.0 / intervals as f32,
                ))
            });
        }
    }
    if settings.show_labels {
        for (fraction, value) in [(0.0, range.min), (1.0, range.max)] {
            let label = spawn_text_part(world, owner, ControlPartKind::Label, format!("{value}"));
            world.entity_mut(track).add_child(label);
            set_layout(world, label, |layout| {
                layout.left = Some(bevy::ui::Val::Percent(fraction * 100.0))
            });
        }
    }
    let tip = if settings.show_tip {
        let tip = spawn_text_part(
            world,
            owner,
            ControlPartKind::Tooltip,
            format!(
                "{}",
                if kind == SliderType::Range {
                    settings.lower
                } else {
                    range.value
                }
            ),
        );
        world.entity_mut(thumb).add_child(tip);
        Some(tip)
    } else {
        None
    };
    let second_tip = if settings.show_tip {
        second_thumb.map(|thumb| {
            let tip = spawn_text_part(
                world,
                owner,
                ControlPartKind::Tooltip,
                format!("{}", settings.upper),
            );
            world.entity_mut(thumb).add_child(tip);
            tip
        })
    } else {
        None
    };
    world.entity_mut(owner).insert(NumericParts {
        track,
        fill,
        thumb: Some(thumb),
        second_thumb,
        tip,
        second_tip,
        orientation,
    });
    if kind == SliderType::Range {
        super::super::state::update_range_layout(world, owner);
    } else {
        update_fill_width(world, owner, range.fraction());
    }
}

pub(crate) fn numeric_range(attributes: &[TemplateAttribute]) -> NumericRange {
    let min = number(attributes, "min").unwrap_or(0.0);
    let max = number(attributes, "max").unwrap_or(100.0);
    let value = number(attributes, "value").unwrap_or(min);
    let step = number(attributes, "step");
    NumericRange::new(min, max, value, step)
}

pub(crate) fn orientation(attributes: &[TemplateAttribute]) -> RangeOrientation {
    if static_value(attributes, "orientation")
        .or_else(|| static_value(attributes, "alignment"))
        .is_some_and(|value| value == "vertical")
    {
        RangeOrientation::Vertical
    } else {
        RangeOrientation::Horizontal
    }
}

pub(crate) fn number(attributes: &[TemplateAttribute], expected: &str) -> Option<f32> {
    static_value(attributes, expected).and_then(|value| value.parse().ok())
}

pub(crate) fn static_value<'a>(
    attributes: &'a [TemplateAttribute],
    expected: &str,
) -> Option<&'a str> {
    attributes.iter().find_map(|attribute| match attribute {
        TemplateAttribute::Static { name, value } if name == expected => Some(value.as_str()),
        _ => None,
    })
}
