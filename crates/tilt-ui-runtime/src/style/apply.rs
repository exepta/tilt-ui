use std::collections::HashMap;

use bevy::{
    asset::{Assets, Handle, RenderAssetUsages},
    ecs::{
        entity::Entity,
        hierarchy::{ChildOf, Children},
        world::World,
    },
    image::{Image, ImageSampler},
    text::{FontSource, Justify, TextColor, TextFont, TextLayout},
    ui::{
        BackgroundColor, BorderColor, Node, UiTransform,
        widget::{ImageNode, NodeImageMode, Text},
    },
};
use tilt_ui_css::{
    ComputedStyle, CssGradient, CssOverflow, FontFamily, FontWeight, GradientDirection, TextAlign,
};
use wgpu_types::{Extent3d, TextureDimension, TextureFormat};

use crate::{ControlPart, ControlPartKind, TiltText};

use super::{
    convert,
    state::{CascadedStyle, RuntimeComputedStyle},
};

#[derive(bevy::ecs::component::Component, Default, Clone, Copy)]
pub(crate) struct AppliedStyleComponents {
    background: bool,
    border_color: bool,
    text_color: bool,
    text_font: bool,
    font_family: bool,
    text_layout: bool,
    transform: bool,
    gradient: bool,
}

#[derive(bevy::ecs::resource::Resource, Default)]
struct GradientImageCache(HashMap<GradientKey, Handle<Image>>);

#[derive(Hash, PartialEq, Eq)]
struct GradientKey {
    direction: GradientDirection,
    stops: Vec<[u32; 4]>,
}

impl From<&CssGradient> for GradientKey {
    fn from(gradient: &CssGradient) -> Self {
        Self {
            direction: gradient.direction,
            stops: gradient
                .stops
                .iter()
                .map(|color| {
                    [
                        color.red.to_bits(),
                        color.green.to_bits(),
                        color.blue.to_bits(),
                        color.alpha.to_bits(),
                    ]
                })
                .collect(),
        }
    }
}

fn gradient_texture(gradient: &CssGradient) -> Image {
    let horizontal = matches!(
        gradient.direction,
        GradientDirection::Left | GradientDirection::Right
    );
    let (width, height) = if horizontal { (256, 1) } else { (1, 256) };
    let mut pixels = Vec::with_capacity(256 * 4);
    for index in 0..256 {
        let fraction = index as f32 / 255.0;
        let fraction = if matches!(
            gradient.direction,
            GradientDirection::Left | GradientDirection::Up
        ) {
            1.0 - fraction
        } else {
            fraction
        };
        let position = fraction * (gradient.stops.len() - 1) as f32;
        let first = position.floor() as usize;
        let second = (first + 1).min(gradient.stops.len() - 1);
        let amount = position - first as f32;
        let a = gradient.stops[first];
        let b = gradient.stops[second];
        for (start, end) in [
            (a.red, b.red),
            (a.green, b.green),
            (a.blue, b.blue),
            (a.alpha, b.alpha),
        ] {
            pixels.push(((start + (end - start) * amount).clamp(0.0, 1.0) * 255.0).round() as u8);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::linear();
    image
}

fn gradient_handle(world: &mut World, gradient: &CssGradient) -> Handle<Image> {
    world.init_resource::<GradientImageCache>();
    let key = GradientKey::from(gradient);
    if let Some(handle) = world.resource::<GradientImageCache>().0.get(&key) {
        return handle.clone();
    }
    world.init_resource::<Assets<Image>>();
    let handle = world
        .resource_mut::<Assets<Image>>()
        .add(gradient_texture(gradient));
    world
        .resource_mut::<GradientImageCache>()
        .0
        .insert(key, handle.clone());
    handle
}

/// Retains the resolved style preceding the latest cascade update.
#[derive(bevy::ecs::component::Component, Clone)]
pub(crate) struct PreviousComputedStyle(pub ComputedStyle);

pub(crate) fn resolve_and_apply_tree(world: &mut World, root: Entity) {
    let mut current = root;
    let mut inherited = ComputedStyle::default();
    while let Some(parent) = world.get::<ChildOf>(current).map(|parent| parent.0) {
        if let Some(style) = world.get::<RuntimeComputedStyle>(parent) {
            inherited = style.0.clone();
            break;
        }
        current = parent;
    }
    resolve_children(world, root, inherited, None);
}

fn resolve_children(
    world: &mut World,
    parent: Entity,
    inherited: ComputedStyle,
    inherited_font_size: Option<bevy::text::FontSize>,
) {
    let children = world
        .get::<Children>(parent)
        .map(|children| children.to_vec())
        .unwrap_or_default();
    for entity in children {
        let is_element = world.get::<crate::TiltElement>(entity).is_some();
        let is_part = world.get::<ControlPart>(entity).is_some();
        let is_text = world.get::<TiltText>(entity).is_some()
            || world.get::<Text>(entity).is_some()
            || world.get::<bevy::text::EditableText>(entity).is_some();
        let mut effective = inherited.clone();
        if is_element || is_part {
            let local = world
                .get::<CascadedStyle>(entity)
                .map(|style| style.0.clone())
                .unwrap_or_default();
            effective = inherit(local, &inherited);
        }
        let needs_apply = world
            .get::<RuntimeComputedStyle>(entity)
            .is_none_or(|previous| previous.0 != effective);
        if (is_element || is_part) && needs_apply {
            apply_element(world, entity, &effective);
        }
        let next_font_size = effective
            .font_size
            .and_then(|value| convert::font_size(value, inherited_font_size));
        if is_text && needs_apply {
            apply_text(world, entity, &effective, next_font_size);
        }
        resolve_children(
            world,
            entity,
            effective,
            next_font_size.or(inherited_font_size),
        );
    }
}

fn inherit(mut local: ComputedStyle, parent: &ComputedStyle) -> ComputedStyle {
    if local.color.is_none() {
        local.color = parent.color;
    }
    if local.font_size.is_none() {
        local.font_size = parent.font_size;
    }
    if local.font_weight.is_none() {
        local.font_weight = parent.font_weight;
    }
    if local.font_family.is_none() {
        local.font_family = parent.font_family;
    }
    if local.text_align.is_none() {
        local.text_align = parent.text_align;
    }
    local
}

fn apply_element(world: &mut World, entity: Entity, style: &ComputedStyle) {
    if let Some(previous) = world
        .get::<RuntimeComputedStyle>(entity)
        .map(|style| style.0.clone())
    {
        world
            .entity_mut(entity)
            .insert(PreviousComputedStyle(previous));
    }
    apply_element_visual(world, entity, style);
    let applied = world
        .get::<AppliedStyleComponents>(entity)
        .copied()
        .unwrap_or_default();
    world
        .entity_mut(entity)
        .insert((RuntimeComputedStyle(style.clone()), applied));
}

/// Applies a temporary motion result without replacing the resolved base style.
pub(crate) fn apply_motion_style(world: &mut World, entity: Entity, style: &ComputedStyle) {
    apply_element_visual(world, entity, style);
    if world.get::<Text>(entity).is_some() {
        apply_motion_text(world, entity, style);
    }
}

fn apply_element_visual(world: &mut World, entity: Entity, style: &ComputedStyle) {
    if let Some(mut node) = world.get_mut::<Node>(entity) {
        apply_node(&mut node, style);
    }
    if world
        .get::<crate::widgets::structure::table::TableInfo>(entity)
        .is_some()
        && style.display.is_none()
        && let Some(mut node) = world.get_mut::<Node>(entity)
    {
        node.display = bevy::ui::Display::Grid;
    }
    if let Some(layout) = world
        .get::<crate::widgets::state::WidgetLayoutOverride>(entity)
        .copied()
        && let Some(mut node) = world.get_mut::<Node>(entity)
    {
        layout.apply(&mut node);
    }
    if let Some(anchor) = world
        .get::<crate::widgets::content::badge::BadgeAnchorPlacement>(entity)
        .copied()
        && let Some(mut node) = world.get_mut::<Node>(entity)
    {
        anchor.apply(&mut node);
    }
    if world
        .get::<crate::widgets::state::WidgetDisplayOverride>(entity)
        .is_some_and(|display| display.hidden)
        && let Some(mut node) = world.get_mut::<Node>(entity)
    {
        node.display = bevy::ui::Display::None;
    }
    if let Some(part) = world.get::<ControlPart>(entity).copied()
        && world.get::<bevy::text::EditableText>(part.owner).is_some()
    {
        match part.kind {
            ControlPartKind::Cursor | ControlPartKind::Selection => {
                if let Some(color) = style.background_color {
                    let mut cursor = world
                        .get::<bevy::text::TextCursorStyle>(part.owner)
                        .copied()
                        .unwrap_or_default();
                    if part.kind == ControlPartKind::Cursor {
                        cursor.color = convert::color(color);
                    } else {
                        cursor.selection_color = convert::color(color);
                    }
                    world.entity_mut(part.owner).insert(cursor);
                }
            }
            ControlPartKind::Value => {
                if world
                    .get::<crate::EditableText>(part.owner)
                    .is_some_and(|state| {
                        !matches!(
                            state.input_type,
                            tilt_ui_core::InputType::Password | tilt_ui_core::InputType::File
                        )
                    })
                    && let Some(color) = style.color
                {
                    world
                        .entity_mut(part.owner)
                        .insert(TextColor(convert::color(color)));
                }
            }
            _ => {}
        }
    }
    let mut applied = world
        .get::<AppliedStyleComponents>(entity)
        .copied()
        .unwrap_or_default();
    if let Some(value) = style.background_color {
        world
            .entity_mut(entity)
            .insert(BackgroundColor(convert::color(value)));
        applied.background = true;
    } else if applied.background {
        world.entity_mut(entity).insert(BackgroundColor::DEFAULT);
        applied.background = false;
    }
    if let Some(gradient) = &style.background_image {
        if world
            .get::<crate::widgets::content::image::ImageMetadata>(entity)
            .is_none()
        {
            let handle = gradient_handle(world, gradient);
            if world
                .get::<ImageNode>(entity)
                .is_none_or(|node| node.image != handle)
            {
                world
                    .entity_mut(entity)
                    .insert(ImageNode::new(handle).with_mode(NodeImageMode::Stretch));
            }
            applied.gradient = true;
        }
    } else if applied.gradient {
        world.entity_mut(entity).remove::<ImageNode>();
        applied.gradient = false;
    }
    if let Some(value) = style.border_color {
        world
            .entity_mut(entity)
            .insert(BorderColor::all(convert::color(value)));
        applied.border_color = true;
    } else if applied.border_color {
        world.entity_mut(entity).insert(BorderColor::DEFAULT);
        applied.border_color = false;
    }
    if let Some(value) = style.transform {
        world.entity_mut(entity).insert(convert::transform(value));
        applied.transform = true;
    } else if applied.transform {
        world.entity_mut(entity).insert(UiTransform::IDENTITY);
        applied.transform = false;
    }
    world.entity_mut(entity).insert(applied);
}

fn apply_motion_text(world: &mut World, entity: Entity, style: &ComputedStyle) {
    if let Some(value) = style.color {
        world
            .entity_mut(entity)
            .insert(TextColor(convert::color(value)));
    }
    if let Some(value) = style
        .font_size
        .and_then(|value| convert::font_size(value, None))
    {
        let mut font = world.get::<TextFont>(entity).cloned().unwrap_or_default();
        font.font_size = value;
        world.entity_mut(entity).insert(font);
    }
}

fn apply_node(node: &mut Node, style: &ComputedStyle) {
    let defaults = Node::default();
    node.display = style
        .display
        .map(convert::display)
        .unwrap_or(defaults.display);
    node.overflow.x = style
        .overflow_x
        .map(convert_overflow)
        .unwrap_or(defaults.overflow.x);
    node.overflow.y = style
        .overflow_y
        .map(convert_overflow)
        .unwrap_or(defaults.overflow.y);
    node.position_type = style
        .position
        .map(convert::position)
        .unwrap_or(defaults.position_type);
    node.width = style.width.map(convert::val).unwrap_or(defaults.width);
    node.height = style.height.map(convert::val).unwrap_or(defaults.height);
    node.min_width = style
        .min_width
        .map(convert::val)
        .unwrap_or(defaults.min_width);
    node.min_height = style
        .min_height
        .map(convert::val)
        .unwrap_or(defaults.min_height);
    node.max_width = style
        .max_width
        .map(convert::val)
        .unwrap_or(defaults.max_width);
    node.max_height = style
        .max_height
        .map(convert::val)
        .unwrap_or(defaults.max_height);
    let static_position = matches!(style.position, Some(tilt_ui_css::Position::Static));
    node.top = (!static_position)
        .then(|| style.top.map(convert::val))
        .flatten()
        .unwrap_or(defaults.top);
    node.right = (!static_position)
        .then(|| style.right.map(convert::val))
        .flatten()
        .unwrap_or(defaults.right);
    node.bottom = (!static_position)
        .then(|| style.bottom.map(convert::val))
        .flatten()
        .unwrap_or(defaults.bottom);
    node.left = (!static_position)
        .then(|| style.left.map(convert::val))
        .flatten()
        .unwrap_or(defaults.left);
    node.margin = style.margin.map(convert::rect).unwrap_or(defaults.margin);
    node.padding = style.padding.map(convert::rect).unwrap_or(defaults.padding);
    node.border = style
        .border_width
        .map(convert::rect)
        .unwrap_or(defaults.border);
    node.row_gap = style.row_gap.map(convert::val).unwrap_or(defaults.row_gap);
    node.column_gap = style
        .column_gap
        .map(convert::val)
        .unwrap_or(defaults.column_gap);
    node.flex_direction = style
        .flex_direction
        .map(convert::flex_direction)
        .unwrap_or(defaults.flex_direction);
    node.flex_wrap = style
        .flex_wrap
        .map(convert::flex_wrap)
        .unwrap_or(defaults.flex_wrap);
    node.justify_content = style
        .justify_content
        .map(convert::justify_content)
        .unwrap_or(defaults.justify_content);
    node.align_items = style
        .align_items
        .map(convert::align_items)
        .unwrap_or(defaults.align_items);
    node.align_self = style
        .align_self
        .map(convert::align_self)
        .unwrap_or(defaults.align_self);
    node.flex_grow = style.flex_grow.unwrap_or(defaults.flex_grow);
    node.flex_shrink = style.flex_shrink.unwrap_or(defaults.flex_shrink);
    node.border_radius = style
        .border_radius
        .map(convert::radius)
        .unwrap_or(defaults.border_radius);
}

fn convert_overflow(value: CssOverflow) -> bevy::ui::OverflowAxis {
    match value {
        CssOverflow::Visible => bevy::ui::OverflowAxis::Visible,
        CssOverflow::Hidden => bevy::ui::OverflowAxis::Hidden,
        CssOverflow::Clip => bevy::ui::OverflowAxis::Clip,
        CssOverflow::Scroll | CssOverflow::Auto => bevy::ui::OverflowAxis::Scroll,
    }
}

fn apply_text(
    world: &mut World,
    entity: Entity,
    style: &ComputedStyle,
    font_size: Option<bevy::text::FontSize>,
) {
    let mut applied = world
        .get::<AppliedStyleComponents>(entity)
        .copied()
        .unwrap_or_default();
    if let Some(value) = style.color {
        world
            .entity_mut(entity)
            .insert(TextColor(convert::color(value)));
        applied.text_color = true;
    } else if applied.text_color {
        world.entity_mut(entity).remove::<TextColor>();
        applied.text_color = false;
    }
    if world
        .get::<crate::EditableText>(entity)
        .is_some_and(|state| {
            matches!(
                state.input_type,
                tilt_ui_core::InputType::Password | tilt_ui_core::InputType::File
            )
        })
    {
        world
            .entity_mut(entity)
            .insert(TextColor(bevy::color::Color::NONE));
        applied.text_color = true;
    }
    if font_size.is_some() || style.font_weight.is_some() || style.font_family.is_some() {
        let mut font = world.get::<TextFont>(entity).cloned().unwrap_or_default();
        if let Some(size) = font_size {
            font.font_size = size;
        }
        if let Some(weight) = style.font_weight {
            font.weight = convert::font_weight(weight);
        }
        if let Some(family) = style.font_family {
            font.font = match family {
                FontFamily::SansSerif => world
                    .get_resource::<crate::theme::DefaultThemeFonts>()
                    .map(|fonts| {
                        let bold = matches!(style.font_weight, Some(FontWeight::Bold))
                            || matches!(style.font_weight, Some(FontWeight::Number(value)) if value >= 600);
                        FontSource::Handle(if bold { fonts.bold.clone() } else { fonts.regular.clone() })
                    })
                    .unwrap_or(FontSource::SansSerif),
                FontFamily::Monospace => FontSource::default(),
                FontFamily::UiSymbols => world
                    .get_resource::<crate::theme::DefaultThemeFonts>()
                    .map(|fonts| FontSource::Handle(fonts.symbols.clone()))
                    .unwrap_or(FontSource::SansSerif),
            };
            applied.font_family = true;
        } else if applied.font_family {
            font.font = FontSource::default();
            applied.font_family = false;
        }
        world.entity_mut(entity).insert(font);
        applied.text_font = true;
    } else if applied.text_font {
        world.entity_mut(entity).remove::<TextFont>();
        applied.text_font = false;
    }
    if let Some(value) = style.text_align {
        let justify = match value {
            TextAlign::Start => Justify::Start,
            TextAlign::End => Justify::End,
            TextAlign::Center => Justify::Center,
            TextAlign::Justify => Justify::Justified,
        };
        let mut layout = world.get::<TextLayout>(entity).copied().unwrap_or_default();
        layout.justify = justify;
        world.entity_mut(entity).insert(layout);
        applied.text_layout = true;
    } else if applied.text_layout {
        world.entity_mut(entity).remove::<TextLayout>();
        applied.text_layout = false;
    }
    world
        .entity_mut(entity)
        .insert((RuntimeComputedStyle(style.clone()), applied));
}

#[cfg(test)]
mod table_tests {
    use bevy::{
        ecs::world::World,
        ui::{Display, Node},
    };
    use tilt_ui_css::ComputedStyle;

    use super::apply_element_visual;
    use crate::TableInfo;

    #[test]
    fn table_remains_a_grid_without_default_theme_css() {
        let mut world = World::new();
        let table = world
            .spawn((
                Node::default(),
                TableInfo {
                    columns: 2,
                    rows: 1,
                },
            ))
            .id();
        apply_element_visual(&mut world, table, &ComputedStyle::default());
        assert_eq!(world.get::<Node>(table).unwrap().display, Display::Grid);
    }
}
