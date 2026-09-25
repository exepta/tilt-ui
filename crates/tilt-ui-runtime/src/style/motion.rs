use bevy::ecs::{
    component::Component,
    entity::Entity,
    query::{Changed, Or, With},
    world::World,
};
use tilt_ui_css::{
    AnimationDirection, AnimationSpec, ComputedStyle, CssColor, CssTransform, Keyframe,
    KeyframesRule, Length, StyleDeclaration, StyleSheet, TimingFunction, TransitionProperty,
    TransitionSpec,
};

use crate::{ComponentAssetHandles, ComponentStyleOwner, RuntimeComputedStyle, UiStyleSheetAsset};

use super::apply::{PreviousComputedStyle, apply_motion_style};

/// Stores runtime progress for CSS animations currently affecting an element.
#[derive(Component, Debug, Clone)]
pub struct ActiveAnimations(Vec<ActiveAnimation>);

/// Stores runtime progress for CSS transitions currently affecting an element.
#[derive(Component, Debug, Clone)]
pub(crate) struct ActiveTransitions(Vec<ActiveTransition>);

#[derive(Component, Debug, Clone)]
struct MotionDisplayedStyle(ComputedStyle);

#[derive(Debug, Clone)]
struct ActiveAnimation {
    spec: AnimationSpec,
    source: KeyframesSource,
    elapsed: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyframesSource {
    Author,
    DefaultTheme,
}

#[derive(Debug, Clone)]
struct ActiveTransition {
    spec: TransitionSpec,
    from: ComputedStyle,
    to: ComputedStyle,
    elapsed: f32,
}

pub(crate) fn reconcile_scope(
    world: &mut World,
    owner: Entity,
    author: Option<&StyleSheet>,
    default_theme: Option<&StyleSheet>,
) {
    let entities = {
        let mut query = world.query_filtered::<
            (Entity, &ComponentStyleOwner, &RuntimeComputedStyle),
            Changed<RuntimeComputedStyle>,
        >();
        query
            .iter(world)
            .filter(|(_, style_owner, _)| style_owner.0 == owner)
            .map(|(entity, _, style)| (entity, style.0.clone()))
            .collect::<Vec<_>>()
    };
    for (entity, style) in entities {
        let specs = resolved_animations(&style);
        let active = specs
            .iter()
            .filter_map(|spec| {
                let name = spec.name.as_ref()?;
                let source = if author.and_then(|sheet| keyframes(sheet, &name.0)).is_some() {
                    Some(KeyframesSource::Author)
                } else if default_theme
                    .and_then(|sheet| keyframes(sheet, &name.0))
                    .is_some()
                {
                    Some(KeyframesSource::DefaultTheme)
                } else {
                    None
                };
                let source = source?;
                Some(ActiveAnimation {
                    spec: spec.clone(),
                    source,
                    elapsed: 0.0,
                })
            })
            .collect::<Vec<_>>();
        if active.is_empty() {
            world.entity_mut(entity).remove::<ActiveAnimations>();
        } else {
            let unchanged = world
                .get::<ActiveAnimations>(entity)
                .is_some_and(|current| {
                    current
                        .0
                        .iter()
                        .map(|animation| (&animation.spec, animation.source))
                        .eq(active
                            .iter()
                            .map(|animation| (&animation.spec, animation.source)))
                });
            if !unchanged {
                world.entity_mut(entity).insert(ActiveAnimations(active));
            }
        }
        reconcile_transitions(world, entity, &style);
    }
}

fn reconcile_transitions(world: &mut World, entity: Entity, style: &ComputedStyle) {
    let previous = world
        .get::<PreviousComputedStyle>(entity)
        .map(|style| style.0.clone())
        .unwrap_or_else(|| style.clone());
    let displayed = world
        .get::<MotionDisplayedStyle>(entity)
        .map(|style| style.0.clone())
        .unwrap_or_else(|| previous.clone());
    let transitions = resolved_transitions(style)
        .iter()
        .filter(|spec| spec.duration.0 > 0.0 && property_changed(&previous, style, spec.property))
        .cloned()
        .map(|spec| ActiveTransition {
            spec,
            from: displayed.clone(),
            to: style.clone(),
            elapsed: 0.0,
        })
        .collect::<Vec<_>>();
    if transitions.is_empty() {
        world.entity_mut(entity).remove::<ActiveTransitions>();
        if world.get::<ActiveAnimations>(entity).is_none() {
            world.entity_mut(entity).remove::<MotionDisplayedStyle>();
        }
    } else {
        world
            .entity_mut(entity)
            .insert(ActiveTransitions(transitions));
    }
}

fn resolved_transitions(style: &ComputedStyle) -> Vec<TransitionSpec> {
    if let Some(specifications) = &style.transition {
        return specifications.clone();
    }
    let Some(properties) = &style.transition_property else {
        return Vec::new();
    };
    properties
        .iter()
        .enumerate()
        .map(|(index, property)| TransitionSpec {
            property: *property,
            duration: style
                .transition_duration
                .as_ref()
                .and_then(|values| values.get(index).or_else(|| values.last()))
                .copied()
                .unwrap_or(tilt_ui_css::CssTime(0.0)),
            delay: style
                .transition_delay
                .as_ref()
                .and_then(|values| values.get(index).or_else(|| values.last()))
                .copied()
                .unwrap_or(tilt_ui_css::CssTime(0.0)),
            timing_function: style
                .transition_timing_function
                .as_ref()
                .and_then(|values| values.get(index).or_else(|| values.last()))
                .copied()
                .unwrap_or_default(),
        })
        .collect()
}

pub(crate) fn tick_animations(world: &mut World) {
    let delta = world
        .get_resource::<bevy::time::Time>()
        .map_or(0.0, bevy::time::Time::delta_secs);
    let entities = {
        let mut query = world.query_filtered::<(
            Entity,
            &ComponentStyleOwner,
            Option<&ActiveAnimations>,
            Option<&ActiveTransitions>,
            &RuntimeComputedStyle,
        ), Or<(With<ActiveAnimations>, With<ActiveTransitions>)>>();
        query
            .iter(world)
            .map(|(entity, owner, animations, transitions, base)| {
                (
                    entity,
                    owner.0,
                    animations.cloned(),
                    transitions.cloned(),
                    base.0.clone(),
                )
            })
            .collect::<Vec<_>>()
    };
    for (entity, owner, animations, transitions, base) in entities {
        let mut overlay = base.clone();
        let mut active_animations = animations.unwrap_or(ActiveAnimations(Vec::new()));
        let mut active_transitions = transitions.unwrap_or(ActiveTransitions(Vec::new()));
        let mut animations_running = false;
        let mut transitions_running = false;
        for transition in &mut active_transitions.0 {
            transition.elapsed += delta;
            let Some(progress) = transition_progress(transition) else {
                continue;
            };
            transitions_running = true;
            interpolate_property(
                &mut overlay,
                &transition.from,
                &transition.to,
                transition.spec.property,
                progress,
            );
        }
        for animation in &mut active_animations.0 {
            animation.elapsed += delta;
            let Some(progress) = animation_progress(animation) else {
                continue;
            };
            let Some(name) = animation.spec.name.as_ref() else {
                continue;
            };
            let Some(frames) = animation_frames(world, owner, animation.source, &name.0) else {
                continue;
            };
            animations_running = true;
            apply_keyframes(
                &mut overlay,
                &base,
                frames,
                progress,
                animation.spec.timing_function,
            );
        }
        if animations_running || transitions_running {
            apply_motion_style(world, entity, &overlay);
            world
                .entity_mut(entity)
                .insert(MotionDisplayedStyle(overlay));
            if animations_running {
                world.entity_mut(entity).insert(active_animations);
            } else {
                world.entity_mut(entity).remove::<ActiveAnimations>();
            }
            if transitions_running {
                world.entity_mut(entity).insert(active_transitions);
            } else {
                world.entity_mut(entity).remove::<ActiveTransitions>();
            }
        } else {
            apply_motion_style(world, entity, &base);
            world.entity_mut(entity).remove::<ActiveAnimations>();
            world.entity_mut(entity).remove::<ActiveTransitions>();
            world.entity_mut(entity).remove::<MotionDisplayedStyle>();
        }
    }
}

fn animation_frames<'a>(
    world: &'a World,
    owner: Entity,
    source: KeyframesSource,
    name: &str,
) -> Option<&'a [Keyframe]> {
    match source {
        KeyframesSource::Author => {
            let handles = world.get::<ComponentAssetHandles>(owner)?;
            let stylesheet = world
                .resource::<bevy::asset::Assets<UiStyleSheetAsset>>()
                .get(&handles.stylesheet)?
                .stylesheet();
            keyframes(stylesheet, name).map(|rule| rule.frames.as_slice())
        }
        KeyframesSource::DefaultTheme => world
            .get_resource::<crate::theme::DefaultThemeStyleSheet>()
            .and_then(|theme| keyframes(&theme.0, name))
            .map(|rule| rule.frames.as_slice()),
    }
}

fn keyframes<'a>(stylesheet: &'a StyleSheet, name: &str) -> Option<&'a KeyframesRule> {
    stylesheet
        .keyframes()
        .iter()
        .find(|rule| rule.name.0 == name)
}

fn resolved_animations(style: &ComputedStyle) -> Vec<AnimationSpec> {
    if let Some(specifications) = &style.animation {
        return specifications.clone();
    }
    let Some(names) = &style.animation_name else {
        return Vec::new();
    };
    names
        .iter()
        .enumerate()
        .map(|(index, name)| AnimationSpec {
            name: name.clone(),
            duration: style
                .animation_duration
                .as_ref()
                .and_then(|values| values.get(index).or_else(|| values.last()))
                .copied()
                .unwrap_or(tilt_ui_css::CssTime(0.0)),
            delay: style
                .animation_delay
                .as_ref()
                .and_then(|values| values.get(index).or_else(|| values.last()))
                .copied()
                .unwrap_or(tilt_ui_css::CssTime(0.0)),
            timing_function: style
                .animation_timing_function
                .as_ref()
                .and_then(|values| values.get(index).or_else(|| values.last()))
                .copied()
                .unwrap_or_default(),
            iteration_count: style
                .animation_iteration_count
                .as_ref()
                .and_then(|values| values.get(index).or_else(|| values.last()))
                .copied()
                .unwrap_or_default(),
            direction: style
                .animation_direction
                .as_ref()
                .and_then(|values| values.get(index).or_else(|| values.last()))
                .copied()
                .unwrap_or_default(),
        })
        .collect()
}

fn animation_progress(animation: &ActiveAnimation) -> Option<f32> {
    if animation.elapsed < animation.spec.delay.0 || animation.spec.duration.0 <= 0.0 {
        return None;
    }
    let elapsed = animation.elapsed - animation.spec.delay.0;
    let iteration = (elapsed / animation.spec.duration.0).floor();
    if let tilt_ui_css::IterationCount::Finite(count) = animation.spec.iteration_count
        && iteration >= count
    {
        return None;
    }
    let mut progress = (elapsed / animation.spec.duration.0).fract();
    let reverse = match animation.spec.direction {
        AnimationDirection::Normal => false,
        AnimationDirection::Reverse => true,
        AnimationDirection::Alternate => (iteration as i64) % 2 != 0,
        AnimationDirection::AlternateReverse => (iteration as i64) % 2 == 0,
    };
    if reverse {
        progress = 1.0 - progress;
    }
    Some(progress)
}

fn transition_progress(transition: &ActiveTransition) -> Option<f32> {
    if transition.elapsed < transition.spec.delay.0 {
        return Some(0.0);
    }
    let elapsed = transition.elapsed - transition.spec.delay.0;
    if elapsed >= transition.spec.duration.0 {
        return None;
    }
    Some(curve(
        elapsed / transition.spec.duration.0,
        transition.spec.timing_function,
    ))
}

fn property_changed(
    from: &ComputedStyle,
    to: &ComputedStyle,
    property: TransitionProperty,
) -> bool {
    match property {
        TransitionProperty::None => false,
        TransitionProperty::All => {
            from.background_color != to.background_color
                || from.color != to.color
                || from.opacity != to.opacity
                || from.width != to.width
                || from.height != to.height
                || from.font_size != to.font_size
                || from.transform != to.transform
                || from.border_color != to.border_color
                || from.border_radius != to.border_radius
        }
        TransitionProperty::Color => from.color != to.color,
        TransitionProperty::BackgroundColor => from.background_color != to.background_color,
        TransitionProperty::BorderColor => from.border_color != to.border_color,
        TransitionProperty::BorderRadius => from.border_radius != to.border_radius,
        TransitionProperty::FontSize => from.font_size != to.font_size,
        TransitionProperty::Opacity => from.opacity != to.opacity,
        TransitionProperty::Transform => from.transform != to.transform,
        TransitionProperty::Width => from.width != to.width,
        TransitionProperty::Height => from.height != to.height,
    }
}

fn interpolate_property(
    target: &mut ComputedStyle,
    from: &ComputedStyle,
    to: &ComputedStyle,
    property: TransitionProperty,
    t: f32,
) {
    match property {
        TransitionProperty::None => {}
        TransitionProperty::All => interpolate(target, from, to, t),
        TransitionProperty::Color => {
            target.color = blend_option(from.color, to.color, t, color);
        }
        TransitionProperty::BackgroundColor => {
            target.background_color =
                blend_option(from.background_color, to.background_color, t, color);
        }
        TransitionProperty::BorderColor => {
            target.border_color = blend_option(from.border_color, to.border_color, t, color);
        }
        TransitionProperty::BorderRadius => {
            target.border_radius = blend_option(from.border_radius, to.border_radius, t, radius);
        }
        TransitionProperty::FontSize => {
            target.font_size = blend_option(from.font_size, to.font_size, t, length);
        }
        TransitionProperty::Opacity => {
            target.opacity = blend_option(from.opacity, to.opacity, t, |a, b, progress| {
                Some(a + (b - a) * progress)
            });
        }
        TransitionProperty::Transform => {
            target.transform = blend_option(from.transform, to.transform, t, transform);
        }
        TransitionProperty::Width => {
            target.width = blend_option(from.width, to.width, t, length);
        }
        TransitionProperty::Height => {
            target.height = blend_option(from.height, to.height, t, length);
        }
    }
}

fn apply_keyframes(
    target: &mut ComputedStyle,
    base: &ComputedStyle,
    frames: &[Keyframe],
    progress: f32,
    timing: TimingFunction,
) {
    let start = frames.iter().rev().find(|frame| frame.offset <= progress);
    let end = frames.iter().find(|frame| frame.offset >= progress);
    let start_offset = start.map_or(0.0, |frame| frame.offset);
    let end_offset = end.map_or(1.0, |frame| frame.offset);
    let local = if (end_offset - start_offset).abs() <= f32::EPSILON {
        0.0
    } else {
        curve(
            (progress - start_offset) / (end_offset - start_offset),
            timing,
        )
    };
    let empty = [];
    let start_declarations = start.map_or(&empty[..], |frame| frame.declarations.as_slice());
    let end_declarations = end.map_or(&empty[..], |frame| frame.declarations.as_slice());
    let from = declarations_style(start_declarations, base);
    let to = declarations_style(end_declarations, base);
    interpolate_keyframe_properties(
        target,
        &from,
        &to,
        start_declarations,
        end_declarations,
        local,
    );
}

fn interpolate_keyframe_properties(
    target: &mut ComputedStyle,
    from: &ComputedStyle,
    to: &ComputedStyle,
    start: &[StyleDeclaration],
    end: &[StyleDeclaration],
    t: f32,
) {
    let declared = |property| {
        start.iter().chain(end).any(|declaration| match property {
            TransitionProperty::Color => matches!(declaration, StyleDeclaration::Color(_)),
            TransitionProperty::BackgroundColor => {
                matches!(declaration, StyleDeclaration::BackgroundColor(_))
            }
            TransitionProperty::Transform => matches!(declaration, StyleDeclaration::Transform(_)),
            TransitionProperty::Width => matches!(declaration, StyleDeclaration::Width(_)),
            TransitionProperty::Height => matches!(declaration, StyleDeclaration::Height(_)),
            TransitionProperty::BorderColor
            | TransitionProperty::BorderRadius
            | TransitionProperty::FontSize
            | TransitionProperty::Opacity
            | TransitionProperty::None
            | TransitionProperty::All => false,
        })
    };
    if declared(TransitionProperty::Color) {
        target.color = blend_option(from.color, to.color, t, color);
    }
    if declared(TransitionProperty::BackgroundColor) {
        target.background_color =
            blend_option(from.background_color, to.background_color, t, color);
    }
    if declared(TransitionProperty::Transform) {
        target.transform = blend_option(from.transform, to.transform, t, transform);
    }
    if declared(TransitionProperty::Width) {
        target.width = blend_option(from.width, to.width, t, length);
    }
    if declared(TransitionProperty::Height) {
        target.height = blend_option(from.height, to.height, t, length);
    }
    let opacity_declared = start
        .iter()
        .chain(end)
        .any(|declaration| matches!(declaration, StyleDeclaration::Opacity(_)));
    if opacity_declared {
        target.opacity = blend_option(from.opacity, to.opacity, t, |a, b, t| Some(a + (b - a) * t));
    }
    let font_size_declared = start
        .iter()
        .chain(end)
        .any(|declaration| matches!(declaration, StyleDeclaration::FontSize(_)));
    if font_size_declared {
        target.font_size = blend_option(from.font_size, to.font_size, t, length);
    }
}

fn declarations_style(declarations: &[StyleDeclaration], base: &ComputedStyle) -> ComputedStyle {
    let mut style = base.clone();
    for declaration in declarations {
        match declaration {
            StyleDeclaration::BackgroundColor(value) => style.background_color = Some(*value),
            StyleDeclaration::Color(value) => style.color = Some(*value),
            StyleDeclaration::Opacity(value) => style.opacity = Some(*value),
            StyleDeclaration::Transform(value) => style.transform = Some(*value),
            StyleDeclaration::Width(value) => style.width = Some(*value),
            StyleDeclaration::Height(value) => style.height = Some(*value),
            StyleDeclaration::FontSize(value) => style.font_size = Some(*value),
            StyleDeclaration::BorderRadius(value) => style.border_radius = Some(*value),
            _ => {}
        }
    }
    style
}

fn interpolate(target: &mut ComputedStyle, from: &ComputedStyle, to: &ComputedStyle, t: f32) {
    target.background_color = blend_option(from.background_color, to.background_color, t, color);
    target.color = blend_option(from.color, to.color, t, color);
    target.opacity = blend_option(from.opacity, to.opacity, t, |a, b, t| Some(a + (b - a) * t));
    target.width = blend_option(from.width, to.width, t, length);
    target.height = blend_option(from.height, to.height, t, length);
    target.font_size = blend_option(from.font_size, to.font_size, t, length);
    target.transform = blend_option(from.transform, to.transform, t, transform);
    target.border_color = blend_option(from.border_color, to.border_color, t, color);
    target.border_radius = blend_option(from.border_radius, to.border_radius, t, radius);
}

fn blend_option<T: Copy>(
    from: Option<T>,
    to: Option<T>,
    t: f32,
    blend: impl Fn(T, T, f32) -> Option<T>,
) -> Option<T> {
    match (from, to) {
        (Some(from), Some(to)) => blend(from, to, t),
        (_, Some(to)) if t >= 0.5 => Some(to),
        (Some(from), _) => Some(from),
        _ => None,
    }
}
fn color(a: CssColor, b: CssColor, t: f32) -> Option<CssColor> {
    Some(CssColor::rgba(
        a.red + (b.red - a.red) * t,
        a.green + (b.green - a.green) * t,
        a.blue + (b.blue - a.blue) * t,
        a.alpha + (b.alpha - a.alpha) * t,
    ))
}
fn length(a: Length, b: Length, t: f32) -> Option<Length> {
    match (a, b) {
        (Length::Px(a), Length::Px(b)) => Some(Length::Px(a + (b - a) * t)),
        (Length::Percent(a), Length::Percent(b)) => Some(Length::Percent(a + (b - a) * t)),
        (Length::Vw(a), Length::Vw(b)) => Some(Length::Vw(a + (b - a) * t)),
        (Length::Vh(a), Length::Vh(b)) => Some(Length::Vh(a + (b - a) * t)),
        _ if t >= 0.5 => Some(b),
        _ => Some(a),
    }
}
fn transform(a: CssTransform, b: CssTransform, t: f32) -> Option<CssTransform> {
    Some(CssTransform {
        translate_x: length(a.translate_x, b.translate_x, t)?,
        translate_y: length(a.translate_y, b.translate_y, t)?,
        scale_x: a.scale_x + (b.scale_x - a.scale_x) * t,
        scale_y: a.scale_y + (b.scale_y - a.scale_y) * t,
        rotation: a.rotation + (b.rotation - a.rotation) * t,
    })
}

fn radius(
    a: tilt_ui_css::BorderRadius,
    b: tilt_ui_css::BorderRadius,
    t: f32,
) -> Option<tilt_ui_css::BorderRadius> {
    Some(tilt_ui_css::BorderRadius {
        top_left: length(a.top_left, b.top_left, t)?,
        top_right: length(a.top_right, b.top_right, t)?,
        bottom_right: length(a.bottom_right, b.bottom_right, t)?,
        bottom_left: length(a.bottom_left, b.bottom_left, t)?,
    })
}
fn curve(t: f32, timing: TimingFunction) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match timing {
        TimingFunction::Linear => t,
        TimingFunction::Ease => cubic(t, 0.25, 0.1, 0.25, 1.0),
        TimingFunction::EaseIn => cubic(t, 0.42, 0.0, 1.0, 1.0),
        TimingFunction::EaseOut => cubic(t, 0.0, 0.0, 0.58, 1.0),
        TimingFunction::EaseInOut => cubic(t, 0.42, 0.0, 0.58, 1.0),
    }
}
fn cubic(x: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let mut t = x;
    for _ in 0..5 {
        let mt = 1.0 - t;
        let sample = 3.0 * mt * mt * t * x1 + 3.0 * mt * t * t * x2 + t * t * t;
        let derivative = 3.0 * mt * mt * x1 + 6.0 * mt * t * (x2 - x1) + 3.0 * t * t * (1.0 - x2);
        if derivative.abs() > 0.0001 {
            t = (t - (sample - x) / derivative).clamp(0.0, 1.0);
        }
    }
    let mt = 1.0 - t;
    3.0 * mt * mt * t * y1 + 3.0 * mt * t * t * y2 + t * t * t
}
