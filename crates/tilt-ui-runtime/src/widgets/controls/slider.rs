//! Semantic range state and persistent anatomy for the TiltUI slider.

use bevy::ecs::{component::Component, entity::Entity, hierarchy::Children, world::World};
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

/// Reconfigures only the affected slider parts; the slider entity and value survive.
pub(crate) fn reconfigure(world: &mut World, owner: Entity, name: &str, value: &str) {
    let (Some(mut settings), Some(mut parts), Some(range)) = (
        world.get::<SliderSettings>(owner).copied(),
        world.get::<NumericParts>(owner).copied(),
        world.get::<NumericRange>(owner).copied(),
    ) else {
        return;
    };
    match name {
        "type" => {
            let kind = if value == "range" {
                SliderType::Range
            } else {
                SliderType::Default
            };
            if settings.kind == kind {
                return;
            }
            settings.kind = kind;
            if kind == SliderType::Range {
                let thumb = spawn_part(world, owner, ControlPartKind::Thumb);
                world.entity_mut(parts.track).add_child(thumb);
                parts.second_thumb = Some(thumb);
                if settings.show_tip {
                    let tip = spawn_text_part(
                        world,
                        owner,
                        ControlPartKind::Tooltip,
                        settings.upper.to_string(),
                    );
                    world.entity_mut(thumb).add_child(tip);
                    parts.second_tip = Some(tip);
                }
            } else {
                if let Some(thumb) = parts.second_thumb.take() {
                    world.entity_mut(thumb).despawn();
                }
                parts.second_tip = None;
                settings.active_thumb = 0;
            }
        }
        "orientation" | "alignment" => {
            let next = if value == "vertical" {
                RangeOrientation::Vertical
            } else {
                RangeOrientation::Horizontal
            };
            if parts.orientation == next {
                return;
            }
            parts.orientation = next;
            // Dots and labels use axis-specific positions.
            position_markers(world, parts.track, next);
        }
        "dots" => {
            let next = value
                .parse::<f32>()
                .ok()
                .filter(|count| {
                    count.is_finite() && (1.0..=100.0).contains(count) && count.fract() == 0.0
                })
                .map(|count| count as u32);
            if settings.dots == next {
                return;
            }
            remove_parts(world, parts.track, ControlPartKind::Dot);
            settings.dots = next;
            if let Some(intervals) = next {
                for index in 0..=intervals {
                    let dot = spawn_part(world, owner, ControlPartKind::Dot);
                    world.entity_mut(parts.track).add_child(dot);
                    position_marker(
                        world,
                        dot,
                        parts.orientation,
                        index as f32 / intervals as f32,
                    );
                }
            }
        }
        "show-labels" => {
            let next = bool_value(value);
            if settings.show_labels == next {
                return;
            }
            remove_parts(world, parts.track, ControlPartKind::Label);
            settings.show_labels = next;
            if next {
                for (fraction, label) in [(0.0, range.min), (1.0, range.max)] {
                    let part =
                        spawn_text_part(world, owner, ControlPartKind::Label, label.to_string());
                    world.entity_mut(parts.track).add_child(part);
                    position_marker(world, part, parts.orientation, fraction);
                }
            }
        }
        "show-tip" => {
            let next = bool_value(value);
            if settings.show_tip == next {
                return;
            }
            settings.show_tip = next;
            for (thumb, tip, number) in [
                (
                    parts.thumb,
                    &mut parts.tip,
                    if settings.kind == SliderType::Range {
                        settings.lower
                    } else {
                        range.value
                    },
                ),
                (parts.second_thumb, &mut parts.second_tip, settings.upper),
            ] {
                if next {
                    if let Some(thumb) = thumb {
                        let part = spawn_text_part(
                            world,
                            owner,
                            ControlPartKind::Tooltip,
                            number.to_string(),
                        );
                        world.entity_mut(thumb).add_child(part);
                        *tip = Some(part);
                    }
                } else if let Some(part) = tip.take() {
                    world.entity_mut(part).despawn();
                }
            }
        }
        "dot-anchor" => {
            let next = if value == "bottom" {
                SliderDotAnchor::Bottom
            } else {
                SliderDotAnchor::Top
            };
            if settings.dot_anchor == next {
                return;
            }
            settings.dot_anchor = next;
        }
        _ => return,
    }
    world.entity_mut(owner).insert((settings, parts));
    if settings.kind == SliderType::Range {
        super::super::state::update_range_layout(world, owner);
    } else {
        update_fill_width(world, owner, range.fraction());
    }
}

fn bool_value(value: &str) -> bool {
    !matches!(value, "" | "false" | "0" | "off")
}

fn remove_parts(world: &mut World, track: Entity, kind: ControlPartKind) {
    let children = world
        .get::<Children>(track)
        .map(|children| children.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default();
    for child in children {
        if world
            .get::<crate::ControlPart>(child)
            .is_some_and(|part| part.kind == kind)
        {
            world.entity_mut(child).despawn();
        }
    }
}

fn position_markers(world: &mut World, track: Entity, orientation: RangeOrientation) {
    let children = world
        .get::<Children>(track)
        .map(|children| children.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default();
    let dots = children
        .iter()
        .copied()
        .filter(|child| {
            world
                .get::<crate::ControlPart>(*child)
                .is_some_and(|part| part.kind == ControlPartKind::Dot)
        })
        .collect::<Vec<_>>();
    for (index, dot) in dots.iter().enumerate() {
        let fraction = if dots.len() <= 1 {
            0.0
        } else {
            index as f32 / (dots.len() - 1) as f32
        };
        position_marker(world, *dot, orientation, fraction);
    }
    let labels = children
        .iter()
        .copied()
        .filter(|child| {
            world
                .get::<crate::ControlPart>(*child)
                .is_some_and(|part| part.kind == ControlPartKind::Label)
        })
        .collect::<Vec<_>>();
    for (index, label) in labels.iter().enumerate() {
        position_marker(world, *label, orientation, index as f32);
    }
}

fn position_marker(world: &mut World, part: Entity, orientation: RangeOrientation, fraction: f32) {
    if let Some(mut node) = world.get_mut::<bevy::ui::Node>(part) {
        if orientation == RangeOrientation::Vertical {
            node.left = bevy::ui::Val::Auto;
        } else {
            node.bottom = bevy::ui::Val::Auto;
        }
    }
    set_layout(world, part, |layout| {
        if orientation == RangeOrientation::Vertical {
            layout.left = None;
            layout.bottom = Some(bevy::ui::Val::Percent(fraction * 100.0));
        } else {
            layout.bottom = None;
            layout.left = Some(bevy::ui::Val::Percent(fraction * 100.0));
        }
    });
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
            position_marker(world, dot, orientation, index as f32 / intervals as f32);
        }
    }
    if settings.show_labels {
        for (fraction, value) in [(0.0, range.min), (1.0, range.max)] {
            let label = spawn_text_part(world, owner, ControlPartKind::Label, format!("{value}"));
            world.entity_mut(track).add_child(label);
            position_marker(world, label, orientation, fraction);
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
