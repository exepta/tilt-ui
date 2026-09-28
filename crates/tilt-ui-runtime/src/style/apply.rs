use std::collections::HashMap;

use bevy::color::Alpha;
use bevy::{
    asset::{Assets, Handle, RenderAssetUsages},
    ecs::{
        entity::Entity,
        query::{Changed, Or},
        system::{Commands, Query},
        world::World,
    },
    image::{Image, ImageSampler},
    text::{FontSource, Justify, LineBreak, LineHeight, TextColor, TextFont, TextLayout},
    ui::{
        BackgroundColor, BorderColor, BoxShadow, Node, Outline, ShadowStyle, UiTransform, ZIndex,
        widget::{ImageNode, NodeImageMode, Text, TextShadow},
    },
    ui_render::ui_material::MaterialNode,
    window::{CursorIcon, SystemCursorIcon},
};
use bevy_picking::Pickable;
use tilt_ui_css::{
    ComputedStyle, CssBackgroundImage, CssGradient, CssOverflow, FontFamily, FontWeight,
    GradientDirection, TextAlign,
};
use wgpu_types::{Extent3d, TextureDimension, TextureFormat};

use crate::control::CssCursor;
use crate::style::animated::AnimatedFilters;
use crate::style::backdrop::BackdropFilter;
use crate::style::background::{
    BackgroundLayout, FilteredBackground, PositionedContain, PositionedContainMaterial,
};
use crate::{ControlPart, ControlPartKind, TiltText};

use super::{
    convert,
    motion::MotionDisplayedStyle,
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
    line_height: bool,
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

/// Product of this element's opacity and every styled ancestor's opacity.
#[derive(bevy::ecs::component::Component, Clone, Copy, PartialEq)]
struct ResolvedOpacity(f32);

#[derive(bevy::ecs::component::Component, Clone, Copy)]
struct OriginalPickable(Option<Pickable>);

#[derive(bevy::ecs::component::Component, Clone, Copy)]
struct OriginalZIndex(Option<ZIndex>);

#[derive(bevy::ecs::component::Component, Clone)]
struct OriginalBoxShadow(Option<BoxShadow>);

#[derive(bevy::ecs::component::Component, Clone, Copy)]
struct OriginalOutline(Option<Outline>);

#[derive(bevy::ecs::component::Component, Clone, Copy)]
struct OriginalTextShadow(Option<TextShadow>);

#[derive(bevy::ecs::component::Component, Clone, Copy)]
struct OriginalImageColor(bevy::color::Color);

#[derive(bevy::ecs::component::Component, Clone)]
pub(crate) struct OriginalText {
    source: String,
    rendered: String,
}

/// Keeps CSS text-transform in sync with text bindings and style changes.
pub(crate) fn sync_text_transform(
    mut commands: Commands,
    mut texts: Query<
        (
            Entity,
            &mut Text,
            &RuntimeComputedStyle,
            Option<&mut OriginalText>,
        ),
        Or<(Changed<Text>, Changed<RuntimeComputedStyle>)>,
    >,
) {
    for (entity, mut text, style, original) in &mut texts {
        let transform = style
            .0
            .text_transform
            .unwrap_or(tilt_ui_css::TextTransform::None);
        if transform == tilt_ui_css::TextTransform::None {
            if let Some(original) = original {
                if text.0 == original.rendered {
                    text.0 = original.source.clone();
                }
                commands.entity(entity).remove::<OriginalText>();
            }
            continue;
        }
        let source = original
            .as_ref()
            .filter(|original| original.rendered == text.0)
            .map_or_else(|| text.0.clone(), |original| original.source.clone());
        let rendered = transform_text(&source, transform);
        if text.0 != rendered {
            text.0 = rendered.clone();
        }
        if let Some(mut original) = original {
            if original.source != source || original.rendered != rendered {
                *original = OriginalText { source, rendered };
            }
        } else {
            commands
                .entity(entity)
                .insert(OriginalText { source, rendered });
        }
    }
}

fn transform_text(source: &str, transform: tilt_ui_css::TextTransform) -> String {
    match transform {
        tilt_ui_css::TextTransform::None => source.to_owned(),
        tilt_ui_css::TextTransform::Uppercase => source.to_uppercase(),
        tilt_ui_css::TextTransform::Lowercase => source.to_lowercase(),
        tilt_ui_css::TextTransform::Capitalize => {
            let mut result = String::new();
            let mut word_start = true;
            for ch in source.chars() {
                if ch.is_alphanumeric() && word_start {
                    result.extend(ch.to_uppercase());
                } else {
                    result.push(ch);
                }
                word_start = !ch.is_alphanumeric();
            }
            result
        }
    }
}

pub(crate) fn resolve_and_apply_tree(world: &mut World, root: Entity) {
    let mut current = root;
    let mut inherited = ComputedStyle::default();
    let mut inherited_opacity = 1.0;
    while let Some(parent) = super::backdrop::logical_parent(world, current) {
        if let Some(style) = world.get::<RuntimeComputedStyle>(parent) {
            inherited = style.0.clone();
            inherited_opacity = world
                .get::<ResolvedOpacity>(parent)
                .map_or(1.0, |value| value.0);
            break;
        }
        current = parent;
    }
    resolve_children(world, root, inherited, None, inherited_opacity);
}

fn resolve_children(
    world: &mut World,
    parent: Entity,
    inherited: ComputedStyle,
    inherited_font_size: Option<bevy::text::FontSize>,
    inherited_opacity: f32,
) {
    let children = super::backdrop::logical_children(world, parent);
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
        let opacity = inherited_opacity
            * if is_element || is_part {
                effective.opacity.unwrap_or(1.0)
            } else {
                1.0
            };
        let needs_apply = world
            .get::<RuntimeComputedStyle>(entity)
            .is_none_or(|previous| previous.0 != effective)
            || world
                .get::<ResolvedOpacity>(entity)
                .is_none_or(|previous| previous.0 != opacity);
        if world
            .get::<ResolvedOpacity>(entity)
            .is_none_or(|previous| previous.0 != opacity)
        {
            world.entity_mut(entity).insert(ResolvedOpacity(opacity));
        }
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
            opacity,
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
        local.font_family = parent.font_family.clone();
    }
    if local.text_align.is_none() {
        local.text_align = parent.text_align;
    }
    if local.line_height.is_none() {
        local.line_height = parent.line_height;
    }
    if local.text_wrap.is_none() {
        local.text_wrap = parent.text_wrap;
    }
    if local.text_transform.is_none() {
        local.text_transform = parent.text_transform;
    }
    if local.pointer_events.is_none() {
        local.pointer_events = parent.pointer_events;
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
    let parent_opacity = nearest_parent_opacity(world, entity);
    let opacity = parent_opacity * style.opacity.unwrap_or(1.0);
    let opacity_changed = world
        .get::<ResolvedOpacity>(entity)
        .is_none_or(|previous| previous.0 != opacity);
    if opacity_changed {
        world.entity_mut(entity).insert(ResolvedOpacity(opacity));
    }
    apply_element_visual(world, entity, style);
    if world.get::<Text>(entity).is_some() {
        apply_motion_text(world, entity, style);
    }
    if opacity_changed {
        reapply_descendant_opacity(world, entity, opacity);
    }
}

fn nearest_parent_opacity(world: &World, mut entity: Entity) -> f32 {
    while let Some(parent) = super::backdrop::logical_parent(world, entity) {
        if let Some(opacity) = world.get::<ResolvedOpacity>(parent) {
            return opacity.0;
        }
        entity = parent;
    }
    1.0
}

fn reapply_descendant_opacity(world: &mut World, parent: Entity, parent_opacity: f32) {
    let children = super::backdrop::logical_children(world, parent);
    for entity in children {
        let is_element = world.get::<crate::TiltElement>(entity).is_some()
            || world.get::<ControlPart>(entity).is_some();
        let style = world
            .get::<MotionDisplayedStyle>(entity)
            .map(|style| style.0.clone())
            .or_else(|| {
                world
                    .get::<RuntimeComputedStyle>(entity)
                    .map(|style| style.0.clone())
            });
        let opacity = parent_opacity
            * if is_element {
                style
                    .as_ref()
                    .and_then(|style| style.opacity)
                    .unwrap_or(1.0)
            } else {
                1.0
            };
        let changed = world
            .get::<ResolvedOpacity>(entity)
            .is_none_or(|previous| previous.0 != opacity);
        if changed {
            world.entity_mut(entity).insert(ResolvedOpacity(opacity));
            if let Some(style) = style.as_ref() {
                if is_element {
                    apply_element_visual(world, entity, style);
                }
                if world.get::<Text>(entity).is_some()
                    || world.get::<bevy::text::EditableText>(entity).is_some()
                {
                    apply_motion_text(world, entity, style);
                }
            }
        }
        reapply_descendant_opacity(world, entity, opacity);
    }
}

fn apply_element_visual(world: &mut World, entity: Entity, style: &ComputedStyle) {
    if let Some(effects) = style
        .animated_effects
        .as_ref()
        .filter(|effects| !effects.is_empty())
    {
        let filters = AnimatedFilters {
            effects: effects.clone(),
            quality: style
                .effect_quality
                .unwrap_or(tilt_ui_css::EffectQuality::Auto),
        };
        if world.get::<AnimatedFilters>(entity) != Some(&filters) {
            world.entity_mut(entity).insert(filters);
        }
    } else {
        world.entity_mut(entity).remove::<AnimatedFilters>();
    }
    if let Some(effects) = style
        .backdrop_filter
        .as_ref()
        .filter(|effects| !effects.is_empty())
    {
        let filter = BackdropFilter(effects.clone());
        if world.get::<BackdropFilter>(entity) != Some(&filter) {
            world.entity_mut(entity).insert(filter);
        }
    } else {
        world.entity_mut(entity).remove::<BackdropFilter>();
    }
    let opacity = world
        .get::<ResolvedOpacity>(entity)
        .map_or(1.0, |value| value.0);
    if let Some(mut node) = world.get_mut::<Node>(entity) {
        apply_node(&mut node, style);
    }
    if let Some(table) = world
        .get::<crate::widgets::structure::table::TableInfo>(entity)
        .copied()
        && let Some(mut node) = world.get_mut::<Node>(entity)
    {
        if style.display.is_none() {
            node.display = bevy::ui::Display::Grid;
        }
        if style.grid_template_columns.is_none() {
            node.grid_template_columns = vec![bevy::ui::RepeatedGridTrack::flex(
                u16::try_from(table.columns.max(1)).unwrap_or(u16::MAX),
                1.0,
            )];
        }
    }
    if let Some(cell) = world
        .get::<crate::widgets::structure::table_cell::TableCellInfo>(entity)
        .copied()
        && let Some(mut node) = world.get_mut::<Node>(entity)
    {
        if style.grid_column.is_none()
            && let Some(column) = cell.column
        {
            node.grid_column =
                bevy::ui::GridPlacement::start(i16::try_from(column + 1).unwrap_or(i16::MAX));
        }
        if style.grid_row.is_none()
            && let Some(row) = cell.row
        {
            node.grid_row =
                bevy::ui::GridPlacement::start(i16::try_from(row + 1).unwrap_or(i16::MAX));
        }
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
                        cursor.color = color_with_opacity(color, opacity);
                    } else {
                        cursor.selection_color = color_with_opacity(color, opacity);
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
                        .insert(TextColor(color_with_opacity(color, opacity)));
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
            .insert(BackgroundColor(color_with_opacity(value, opacity)));
        applied.background = true;
    } else if applied.background {
        world.entity_mut(entity).insert(BackgroundColor::DEFAULT);
        applied.background = false;
    }
    if let Some(source) = &style.background_image {
        if world
            .get::<crate::widgets::content::image::ImageMetadata>(entity)
            .is_none()
        {
            let handle = match source {
                CssBackgroundImage::LinearGradient(gradient) => {
                    Some(gradient_handle(world, gradient))
                }
                CssBackgroundImage::Url(source) => {
                    crate::widgets::content::image::load_image_handle(world, source)
                }
            };
            if let Some(handle) = handle {
                let effects = style
                    .background_filter
                    .as_ref()
                    .filter(|effects| !effects.is_empty());
                let filter_matches = effects.is_some_and(|effects| {
                    world
                        .get::<FilteredBackground>(entity)
                        .is_some_and(|current| {
                            current.source == handle && current.effects == *effects
                        })
                });
                if !filter_matches
                    && world
                        .get::<ImageNode>(entity)
                        .is_none_or(|node| node.image != handle)
                {
                    world
                        .entity_mut(entity)
                        .insert(ImageNode::new(handle.clone()).with_mode(NodeImageMode::Stretch));
                }
                let layout = BackgroundLayout {
                    size: style
                        .background_size
                        .unwrap_or(tilt_ui_css::BackgroundSize::Stretch),
                    position: style.background_position.unwrap_or_default(),
                    attachment: style
                        .background_attachment
                        .unwrap_or(tilt_ui_css::BackgroundAttachment::Scroll),
                };
                if world.get::<BackgroundLayout>(entity) != Some(&layout) {
                    world.entity_mut(entity).insert(layout);
                }
                let positioned = layout.size == tilt_ui_css::BackgroundSize::Contain
                    && layout.position != tilt_ui_css::BackgroundPosition::default();
                if positioned {
                    let marker = PositionedContain {
                        position: layout.position,
                        opacity,
                    };
                    if world.get::<PositionedContain>(entity) != Some(&marker) {
                        world.entity_mut(entity).insert(marker);
                    }
                    if world
                        .get::<MaterialNode<PositionedContainMaterial>>(entity)
                        .is_none()
                    {
                        world.init_resource::<Assets<PositionedContainMaterial>>();
                        let material = world
                            .resource_mut::<Assets<PositionedContainMaterial>>()
                            .add(PositionedContainMaterial {
                                geometry: bevy::math::Vec4::new(0.0, 0.0, 1.0, 1.0),
                                image: handle.clone(),
                                opacity: bevy::math::Vec4::new(opacity, 0.0, 0.0, 0.0),
                            });
                        world.entity_mut(entity).insert(MaterialNode(material));
                    }
                } else {
                    world
                        .entity_mut(entity)
                        .remove::<(PositionedContain, MaterialNode<PositionedContainMaterial>)>();
                }
                if let Some(mut node) = world.get_mut::<ImageNode>(entity) {
                    node.color = bevy::color::Color::srgba(
                        1.0,
                        1.0,
                        1.0,
                        if positioned { 0.0 } else { opacity },
                    );
                }
                if let Some(effects) = effects {
                    if !filter_matches {
                        world.entity_mut(entity).insert(FilteredBackground {
                            source: handle,
                            effects: effects.clone(),
                            output: None,
                        });
                    }
                } else {
                    world.entity_mut(entity).remove::<FilteredBackground>();
                }
                applied.gradient = true;
            } else if applied.gradient {
                world.entity_mut(entity).remove::<ImageNode>();
                world.entity_mut(entity).remove::<(
                    BackgroundLayout,
                    FilteredBackground,
                    PositionedContain,
                    MaterialNode<PositionedContainMaterial>,
                )>();
                applied.gradient = false;
            }
        }
    } else if applied.gradient {
        world.entity_mut(entity).remove::<ImageNode>();
        world.entity_mut(entity).remove::<(
            BackgroundLayout,
            FilteredBackground,
            PositionedContain,
            MaterialNode<PositionedContainMaterial>,
        )>();
        applied.gradient = false;
    }
    if !applied.gradient {
        apply_image_opacity(world, entity, opacity);
    }
    if let Some(edges) = style.border_color_edges {
        world.entity_mut(entity).insert(BorderColor {
            top: color_with_opacity(edges.top, opacity),
            right: color_with_opacity(edges.right, opacity),
            bottom: color_with_opacity(edges.bottom, opacity),
            left: color_with_opacity(edges.left, opacity),
        });
        applied.border_color = true;
    } else if let Some(value) = style.border_color {
        world
            .entity_mut(entity)
            .insert(BorderColor::all(color_with_opacity(value, opacity)));
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
    apply_interaction_style(world, entity, style, true);
    apply_box_effects(world, entity, style, opacity);
    world.entity_mut(entity).insert(applied);
}

fn apply_image_opacity(world: &mut World, entity: Entity, opacity: f32) {
    let Some(current) = world.get::<ImageNode>(entity).map(|node| node.color) else {
        return;
    };
    if opacity < 1.0 {
        let original = world
            .get::<OriginalImageColor>(entity)
            .map_or(current, |saved| saved.0);
        if world.get::<OriginalImageColor>(entity).is_none() {
            world
                .entity_mut(entity)
                .insert(OriginalImageColor(original));
        }
        if let Some(mut node) = world.get_mut::<ImageNode>(entity) {
            node.color = original.with_alpha(original.alpha() * opacity);
        }
    } else if let Some(original) = world.get::<OriginalImageColor>(entity).copied() {
        if let Some(mut node) = world.get_mut::<ImageNode>(entity) {
            node.color = original.0;
        }
        world.entity_mut(entity).remove::<OriginalImageColor>();
    }
}

fn apply_box_effects(world: &mut World, entity: Entity, style: &ComputedStyle, opacity: f32) {
    if let Some(shadows) = &style.box_shadow {
        if world.get::<OriginalBoxShadow>(entity).is_none() {
            let original = world.get::<BoxShadow>(entity).cloned();
            world.entity_mut(entity).insert(OriginalBoxShadow(original));
        }
        let shadows = shadows
            .iter()
            .map(|shadow| ShadowStyle {
                color: color_with_opacity(shadow.color, opacity),
                x_offset: convert::val(shadow.x),
                y_offset: convert::val(shadow.y),
                blur_radius: convert::val(shadow.blur),
                spread_radius: convert::val(shadow.spread),
            })
            .collect();
        world.entity_mut(entity).insert(BoxShadow(shadows));
    } else if let Some(original) = world.get::<OriginalBoxShadow>(entity).cloned() {
        if let Some(shadow) = original.0 {
            world.entity_mut(entity).insert(shadow);
        } else {
            world.entity_mut(entity).remove::<BoxShadow>();
        }
        world.entity_mut(entity).remove::<OriginalBoxShadow>();
    }
    if style.outline_width.is_some()
        || style.outline_color.is_some()
        || style.outline_offset.is_some()
    {
        if world.get::<OriginalOutline>(entity).is_none() {
            let original = world.get::<Outline>(entity).copied();
            world.entity_mut(entity).insert(OriginalOutline(original));
        }
        world.entity_mut(entity).insert(Outline::new(
            style
                .outline_width
                .map(convert::val)
                .unwrap_or(bevy::ui::Val::Px(1.0)),
            style
                .outline_offset
                .map(convert::val)
                .unwrap_or(bevy::ui::Val::Px(0.0)),
            style
                .outline_color
                .map(|color| color_with_opacity(color, opacity))
                .unwrap_or(bevy::color::Color::WHITE),
        ));
    } else if let Some(original) = world.get::<OriginalOutline>(entity).copied() {
        if let Some(outline) = original.0 {
            world.entity_mut(entity).insert(outline);
        } else {
            world.entity_mut(entity).remove::<Outline>();
        }
        world.entity_mut(entity).remove::<OriginalOutline>();
    }
}

fn apply_interaction_style(
    world: &mut World,
    entity: Entity,
    style: &ComputedStyle,
    layout_node: bool,
) {
    if let Some(value) = style.pointer_events {
        if world.get::<OriginalPickable>(entity).is_none() {
            let original = world.get::<Pickable>(entity).copied();
            world.entity_mut(entity).insert(OriginalPickable(original));
        }
        world.entity_mut(entity).insert(match value {
            tilt_ui_css::PointerEvents::Auto => Pickable::default(),
            tilt_ui_css::PointerEvents::None => Pickable::IGNORE,
        });
    } else if let Some(original) = world.get::<OriginalPickable>(entity).copied() {
        if let Some(pickable) = original.0 {
            world.entity_mut(entity).insert(pickable);
        } else {
            world.entity_mut(entity).remove::<Pickable>();
        }
        world.entity_mut(entity).remove::<OriginalPickable>();
    }
    if layout_node && let Some(cursor) = style.cursor {
        let icon = match cursor {
            tilt_ui_css::CssCursor::Auto => None,
            tilt_ui_css::CssCursor::Default => Some(SystemCursorIcon::Default),
            tilt_ui_css::CssCursor::Pointer => Some(SystemCursorIcon::Pointer),
            tilt_ui_css::CssCursor::Text => Some(SystemCursorIcon::Text),
            tilt_ui_css::CssCursor::Move => Some(SystemCursorIcon::Move),
            tilt_ui_css::CssCursor::Wait => Some(SystemCursorIcon::Wait),
            tilt_ui_css::CssCursor::Progress => Some(SystemCursorIcon::Progress),
            tilt_ui_css::CssCursor::Crosshair => Some(SystemCursorIcon::Crosshair),
            tilt_ui_css::CssCursor::Help => Some(SystemCursorIcon::Help),
            tilt_ui_css::CssCursor::Grab => Some(SystemCursorIcon::Grab),
            tilt_ui_css::CssCursor::Grabbing => Some(SystemCursorIcon::Grabbing),
            tilt_ui_css::CssCursor::NotAllowed => Some(SystemCursorIcon::NotAllowed),
            tilt_ui_css::CssCursor::ColResize => Some(SystemCursorIcon::ColResize),
            tilt_ui_css::CssCursor::RowResize => Some(SystemCursorIcon::RowResize),
        };
        world
            .entity_mut(entity)
            .insert(CssCursor(icon.map(CursorIcon::System)));
    } else if layout_node {
        world.entity_mut(entity).remove::<CssCursor>();
    }
    if layout_node {
        if let Some(index) = style.z_index {
            if world.get::<OriginalZIndex>(entity).is_none() {
                let original = world.get::<ZIndex>(entity).copied();
                world.entity_mut(entity).insert(OriginalZIndex(original));
            }
            world.entity_mut(entity).insert(ZIndex(index));
        } else if let Some(original) = world.get::<OriginalZIndex>(entity).copied() {
            if let Some(index) = original.0 {
                world.entity_mut(entity).insert(index);
            } else {
                world.entity_mut(entity).remove::<ZIndex>();
            }
            world.entity_mut(entity).remove::<OriginalZIndex>();
        }
    }
}

fn apply_motion_text(world: &mut World, entity: Entity, style: &ComputedStyle) {
    let opacity = world
        .get::<ResolvedOpacity>(entity)
        .map_or(1.0, |value| value.0);
    if let Some(value) = style.color {
        world
            .entity_mut(entity)
            .insert(TextColor(color_with_opacity(value, opacity)));
    }
    if let Some(value) = style
        .font_size
        .and_then(|value| convert::font_size(value, None))
    {
        let mut font = world.get::<TextFont>(entity).cloned().unwrap_or_default();
        font.font_size = value;
        world.entity_mut(entity).insert(font);
    }
    if let Some(shadow) = style.text_shadow {
        let (tilt_ui_css::Length::Px(x), tilt_ui_css::Length::Px(y)) = (shadow.x, shadow.y) else {
            return;
        };
        world.entity_mut(entity).insert(TextShadow {
            offset: bevy::math::Vec2::new(x, y),
            color: color_with_opacity(shadow.color, opacity),
        });
    }
}

pub(crate) fn apply_node(node: &mut Node, style: &ComputedStyle) {
    let defaults = Node::default();
    node.display = style
        .display
        .map(convert::display)
        .unwrap_or(defaults.display);
    node.box_sizing = style
        .box_sizing
        .map_or(defaults.box_sizing, |value| match value {
            tilt_ui_css::BoxSizing::ContentBox => bevy::ui::BoxSizing::ContentBox,
            tilt_ui_css::BoxSizing::BorderBox => bevy::ui::BoxSizing::BorderBox,
        });
    node.overflow.x = style
        .overflow_x
        .map(convert_overflow)
        .unwrap_or(defaults.overflow.x);
    node.overflow.y = style
        .overflow_y
        .map(convert_overflow)
        .unwrap_or(defaults.overflow.y);
    node.scrollbar_width = style.scroll_width.unwrap_or(defaults.scrollbar_width);
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
    node.flex_basis = style
        .flex_basis
        .map(convert::val)
        .unwrap_or(defaults.flex_basis);
    node.grid_template_rows = style.grid_template_rows.as_ref().map_or_else(
        || defaults.grid_template_rows.clone(),
        |groups| groups.iter().map(convert_grid_group).collect(),
    );
    node.grid_template_columns = style.grid_template_columns.as_ref().map_or_else(
        || defaults.grid_template_columns.clone(),
        |groups| groups.iter().map(convert_grid_group).collect(),
    );
    node.grid_auto_rows = style.grid_auto_rows.as_ref().map_or_else(
        || defaults.grid_auto_rows.clone(),
        |tracks| tracks.iter().map(convert_grid_track).collect(),
    );
    node.grid_auto_columns = style.grid_auto_columns.as_ref().map_or_else(
        || defaults.grid_auto_columns.clone(),
        |tracks| tracks.iter().map(convert_grid_track).collect(),
    );
    node.grid_auto_flow = style
        .grid_auto_flow
        .map_or(defaults.grid_auto_flow, |flow| match flow {
            tilt_ui_css::GridAutoFlow::Row => bevy::ui::GridAutoFlow::Row,
            tilt_ui_css::GridAutoFlow::Column => bevy::ui::GridAutoFlow::Column,
            tilt_ui_css::GridAutoFlow::RowDense => bevy::ui::GridAutoFlow::RowDense,
            tilt_ui_css::GridAutoFlow::ColumnDense => bevy::ui::GridAutoFlow::ColumnDense,
        });
    node.grid_row = style
        .grid_row
        .map_or(defaults.grid_row, convert_grid_placement);
    node.grid_column = style
        .grid_column
        .map_or(defaults.grid_column, convert_grid_placement);
    node.border_radius = style
        .border_radius
        .map(convert::radius)
        .unwrap_or(defaults.border_radius);
}

fn convert_grid_group(group: &tilt_ui_css::GridTrackGroup) -> bevy::ui::RepeatedGridTrack {
    let repetition = match group.repetition {
        tilt_ui_css::GridRepetition::Count(count) => bevy::ui::GridTrackRepetition::Count(count),
        tilt_ui_css::GridRepetition::AutoFill => bevy::ui::GridTrackRepetition::AutoFill,
        tilt_ui_css::GridRepetition::AutoFit => bevy::ui::GridTrackRepetition::AutoFit,
    };
    bevy::ui::RepeatedGridTrack::repeat_many(
        repetition,
        group
            .tracks
            .iter()
            .map(convert_grid_track)
            .collect::<Vec<_>>(),
    )
}

fn convert_grid_track(track: &tilt_ui_css::GridTrackSize) -> bevy::ui::GridTrack {
    use tilt_ui_css::{GridTrackSize as Track, Length};
    match track {
        Track::Auto => bevy::ui::GridTrack::auto(),
        Track::MinContent => bevy::ui::GridTrack::min_content(),
        Track::MaxContent => bevy::ui::GridTrack::max_content(),
        Track::Length(Length::Px(value)) => bevy::ui::GridTrack::px(*value),
        Track::Length(Length::Percent(value)) => bevy::ui::GridTrack::percent(*value),
        Track::Length(Length::Vw(value)) => bevy::ui::GridTrack::vw(*value),
        Track::Length(Length::Vh(value)) => bevy::ui::GridTrack::vh(*value),
        Track::Length(Length::Auto) => bevy::ui::GridTrack::auto(),
        Track::Fraction(value) => bevy::ui::GridTrack::fr(*value),
        Track::MinMax(min, max) => {
            bevy::ui::GridTrack::minmax(convert_grid_min(min), convert_grid_max(max))
        }
    }
}

fn convert_grid_min(track: &tilt_ui_css::GridTrackSize) -> bevy::ui::MinTrackSizingFunction {
    use bevy::ui::MinTrackSizingFunction as Min;
    use tilt_ui_css::{GridTrackSize as Track, Length};
    match track {
        Track::Auto => Min::Auto,
        Track::MinContent => Min::MinContent,
        Track::MaxContent => Min::MaxContent,
        Track::Length(Length::Px(value)) => Min::Px(*value),
        Track::Length(Length::Percent(value)) => Min::Percent(*value),
        Track::Length(Length::Vw(value)) => Min::Vw(*value),
        Track::Length(Length::Vh(value)) => Min::Vh(*value),
        _ => Min::Auto,
    }
}

fn convert_grid_max(track: &tilt_ui_css::GridTrackSize) -> bevy::ui::MaxTrackSizingFunction {
    use bevy::ui::MaxTrackSizingFunction as Max;
    use tilt_ui_css::{GridTrackSize as Track, Length};
    match track {
        Track::Auto => Max::Auto,
        Track::MinContent => Max::MinContent,
        Track::MaxContent => Max::MaxContent,
        Track::Length(Length::Px(value)) => Max::Px(*value),
        Track::Length(Length::Percent(value)) => Max::Percent(*value),
        Track::Length(Length::Vw(value)) => Max::Vw(*value),
        Track::Length(Length::Vh(value)) => Max::Vh(*value),
        Track::Fraction(value) => Max::Fraction(*value),
        Track::Length(Length::Auto) | Track::MinMax(_, _) => Max::Auto,
    }
}

fn convert_grid_placement(placement: tilt_ui_css::GridPlacement) -> bevy::ui::GridPlacement {
    use tilt_ui_css::GridLine as Line;
    match (placement.start, placement.end) {
        (Line::Auto, Line::Auto) => bevy::ui::GridPlacement::auto(),
        (Line::Index(start), Line::Auto) => bevy::ui::GridPlacement::start(start),
        (Line::Auto, Line::Index(end)) => bevy::ui::GridPlacement::end(end),
        (Line::Auto, Line::Span(span)) | (Line::Span(span), Line::Auto) => {
            bevy::ui::GridPlacement::span(span)
        }
        (Line::Index(start), Line::Index(end)) => bevy::ui::GridPlacement::start_end(start, end),
        (Line::Index(start), Line::Span(span)) => bevy::ui::GridPlacement::start_span(start, span),
        (Line::Span(span), Line::Index(end)) => bevy::ui::GridPlacement::end_span(end, span),
        (Line::Span(_), Line::Span(_)) => bevy::ui::GridPlacement::auto(),
    }
}

fn convert_overflow(value: CssOverflow) -> bevy::ui::OverflowAxis {
    match value {
        CssOverflow::Visible => bevy::ui::OverflowAxis::Visible,
        CssOverflow::Hidden => bevy::ui::OverflowAxis::Hidden,
        CssOverflow::Clip => bevy::ui::OverflowAxis::Clip,
        CssOverflow::Scroll | CssOverflow::Auto => bevy::ui::OverflowAxis::Scroll,
    }
}

fn color_with_opacity(value: tilt_ui_css::CssColor, opacity: f32) -> bevy::color::Color {
    convert::color(tilt_ui_css::CssColor::rgba(
        value.red,
        value.green,
        value.blue,
        value.alpha * opacity,
    ))
}

fn apply_text(
    world: &mut World,
    entity: Entity,
    style: &ComputedStyle,
    font_size: Option<bevy::text::FontSize>,
) {
    let opacity = world
        .get::<ResolvedOpacity>(entity)
        .map_or(1.0, |value| value.0);
    let mut applied = world
        .get::<AppliedStyleComponents>(entity)
        .copied()
        .unwrap_or_default();
    apply_interaction_style(world, entity, style, false);
    if let Some(value) = style.color {
        world
            .entity_mut(entity)
            .insert(TextColor(color_with_opacity(value, opacity)));
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
        if let Some(family) = style.font_family.as_ref() {
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
                FontFamily::Serif => FontSource::Serif,
                FontFamily::Cursive => FontSource::Cursive,
                FontFamily::Fantasy => FontSource::Fantasy,
                FontFamily::SystemUi => FontSource::SystemUi,
                FontFamily::Emoji => FontSource::Emoji,
                FontFamily::Named(name) => FontSource::Family(name.clone().into()),
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
    if style.text_align.is_some() || style.text_wrap.is_some() {
        let mut layout = world.get::<TextLayout>(entity).copied().unwrap_or_default();
        layout.justify = style
            .text_align
            .map_or(Justify::Start, |value| match value {
                TextAlign::Start => Justify::Start,
                TextAlign::End => Justify::End,
                TextAlign::Center => Justify::Center,
                TextAlign::Justify => Justify::Justified,
            });
        layout.linebreak = match style.text_wrap {
            Some(tilt_ui_css::TextWrap::NoWrap) => LineBreak::NoWrap,
            _ => LineBreak::WordBoundary,
        };
        world.entity_mut(entity).insert(layout);
        applied.text_layout = true;
    } else if applied.text_layout {
        world.entity_mut(entity).remove::<TextLayout>();
        applied.text_layout = false;
    }
    if let Some(value) = style.line_height {
        let height = match value {
            tilt_ui_css::CssLineHeight::Normal => LineHeight::default(),
            tilt_ui_css::CssLineHeight::Pixels(value) => LineHeight::Px(value),
            tilt_ui_css::CssLineHeight::Relative(value) => LineHeight::RelativeToFont(value),
        };
        world.entity_mut(entity).insert(height);
        applied.line_height = true;
    } else if applied.line_height {
        world.entity_mut(entity).remove::<LineHeight>();
        applied.line_height = false;
    }
    if let Some(shadow) = style.text_shadow {
        if world.get::<OriginalTextShadow>(entity).is_none() {
            let original = world.get::<TextShadow>(entity).copied();
            world
                .entity_mut(entity)
                .insert(OriginalTextShadow(original));
        }
        let (tilt_ui_css::Length::Px(x), tilt_ui_css::Length::Px(y)) = (shadow.x, shadow.y) else {
            unreachable!()
        };
        world.entity_mut(entity).insert(TextShadow {
            offset: bevy::math::Vec2::new(x, y),
            color: color_with_opacity(shadow.color, opacity),
        });
    } else if let Some(original) = world.get::<OriginalTextShadow>(entity).copied() {
        if let Some(shadow) = original.0 {
            world.entity_mut(entity).insert(shadow);
        } else {
            world.entity_mut(entity).remove::<TextShadow>();
        }
        world.entity_mut(entity).remove::<OriginalTextShadow>();
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

    use super::{apply_element_visual, apply_node};
    use crate::TableInfo;

    #[test]
    fn restyling_preserves_a_cached_filtered_background() {
        use bevy::{asset::Assets, image::Image, ui::widget::ImageNode};
        use tilt_ui_css::{
            BackgroundEffect, CssBackgroundImage, CssColor, CssGradient, GradientDirection,
        };

        let mut world = World::new();
        let entity = world.spawn(Node::default()).id();
        let style = ComputedStyle {
            background_image: Some(CssBackgroundImage::LinearGradient(CssGradient {
                direction: GradientDirection::Down,
                stops: vec![
                    CssColor::rgba(1.0, 0.0, 0.0, 1.0),
                    CssColor::rgba(0.0, 0.0, 1.0, 1.0),
                ],
            })),
            background_filter: Some(vec![BackgroundEffect::Invert(100)]),
            ..Default::default()
        };
        apply_element_visual(&mut world, entity, &style);
        let source = world
            .get::<super::FilteredBackground>(entity)
            .unwrap()
            .source
            .clone();
        let image = world
            .resource::<Assets<Image>>()
            .get(&source)
            .unwrap()
            .clone();
        let filtered = world.resource_mut::<Assets<Image>>().add(image);
        world.get_mut::<ImageNode>(entity).unwrap().image = filtered.clone();
        world
            .get_mut::<super::FilteredBackground>(entity)
            .unwrap()
            .output = Some(filtered.clone());

        apply_element_visual(&mut world, entity, &style);
        assert_eq!(world.get::<ImageNode>(entity).unwrap().image, filtered);
        assert_eq!(
            world
                .get::<super::FilteredBackground>(entity)
                .unwrap()
                .output,
            Some(filtered)
        );
    }

    #[test]
    fn text_transform_follows_binding_updates_and_restores_source() {
        use crate::style::state::RuntimeComputedStyle;
        use bevy::{
            app::{App, Update},
            ui::widget::Text,
        };
        use tilt_ui_css::TextTransform;
        let mut app = App::new();
        app.add_systems(Update, super::sync_text_transform);
        let entity = app
            .world_mut()
            .spawn((
                Text::new("hello world"),
                RuntimeComputedStyle(ComputedStyle {
                    text_transform: Some(TextTransform::Capitalize),
                    ..Default::default()
                }),
            ))
            .id();
        app.update();
        assert_eq!(app.world().get::<Text>(entity).unwrap().0, "Hello World");
        app.world_mut().get_mut::<Text>(entity).unwrap().0 = "new VALUE".into();
        app.update();
        assert_eq!(app.world().get::<Text>(entity).unwrap().0, "New VALUE");
        app.world_mut()
            .get_mut::<RuntimeComputedStyle>(entity)
            .unwrap()
            .0
            .text_transform = Some(TextTransform::None);
        app.update();
        assert_eq!(app.world().get::<Text>(entity).unwrap().0, "new VALUE");
    }

    #[test]
    fn css_pointer_and_z_index_restore_existing_components() {
        use bevy_picking::Pickable;
        use tilt_ui_css::{CssCursor, PointerEvents};
        let mut world = World::new();
        let entity = world
            .spawn((Node::default(), Pickable::IGNORE, bevy::ui::ZIndex(7)))
            .id();
        super::apply_element_visual(
            &mut world,
            entity,
            &ComputedStyle {
                pointer_events: Some(PointerEvents::Auto),
                cursor: Some(CssCursor::Help),
                z_index: Some(12),
                scroll_width: Some(8.0),
                ..Default::default()
            },
        );
        assert_eq!(world.get::<Pickable>(entity), Some(&Pickable::default()));
        assert_eq!(
            world.get::<bevy::ui::ZIndex>(entity),
            Some(&bevy::ui::ZIndex(12))
        );
        assert_eq!(world.get::<Node>(entity).unwrap().scrollbar_width, 8.0);
        assert!(world.get::<crate::control::CssCursor>(entity).is_some());
        super::apply_element_visual(&mut world, entity, &ComputedStyle::default());
        assert_eq!(world.get::<Pickable>(entity), Some(&Pickable::IGNORE));
        assert_eq!(
            world.get::<bevy::ui::ZIndex>(entity),
            Some(&bevy::ui::ZIndex(7))
        );
        assert_eq!(world.get::<Node>(entity).unwrap().scrollbar_width, 0.0);
        assert!(world.get::<crate::control::CssCursor>(entity).is_none());
    }

    #[test]
    fn css_shadow_and_outline_reach_bevy_components() {
        use bevy::ui::{BoxShadow, Outline};
        use tilt_ui_css::{CssBoxShadow, CssColor, Length};
        let mut world = World::new();
        let entity = world.spawn(Node::default()).id();
        let style = ComputedStyle {
            box_shadow: Some(vec![CssBoxShadow {
                x: Length::Px(1.0),
                y: Length::Px(2.0),
                blur: Length::Px(8.0),
                spread: Length::Px(0.0),
                color: CssColor::rgba(0.0, 0.0, 0.0, 0.5),
            }]),
            outline_width: Some(Length::Px(2.0)),
            outline_offset: Some(Length::Px(3.0)),
            outline_color: Some(CssColor::rgba(1.0, 0.0, 0.0, 1.0)),
            ..Default::default()
        };
        super::apply_element_visual(&mut world, entity, &style);
        assert_eq!(world.get::<BoxShadow>(entity).unwrap().0.len(), 1);
        assert_eq!(
            world.get::<Outline>(entity).unwrap().width,
            bevy::ui::Val::Px(2.0)
        );
        assert_eq!(
            world.get::<Outline>(entity).unwrap().offset,
            bevy::ui::Val::Px(3.0)
        );
        super::apply_element_visual(&mut world, entity, &ComputedStyle::default());
        assert!(world.get::<BoxShadow>(entity).is_none());
        assert!(world.get::<Outline>(entity).is_none());
    }

    #[test]
    fn opacity_multiplies_through_backgrounds_images_and_text() {
        use crate::{TiltElement, style::state::CascadedStyle};
        use bevy::{
            color::Alpha,
            text::TextColor,
            ui::{
                BackgroundColor,
                widget::{ImageNode, Text},
            },
        };
        use tilt_ui_core::ElementKind;
        use tilt_ui_css::CssColor;
        let mut world = World::new();
        let root = world.spawn_empty().id();
        let parent = world
            .spawn((
                Node::default(),
                TiltElement {
                    kind: ElementKind::Div,
                },
                CascadedStyle(ComputedStyle {
                    opacity: Some(0.5),
                    color: Some(CssColor::rgba(1.0, 1.0, 1.0, 1.0)),
                    background_color: Some(CssColor::rgba(1.0, 0.0, 0.0, 1.0)),
                    ..Default::default()
                }),
            ))
            .id();
        let child = world
            .spawn((
                Node::default(),
                TiltElement {
                    kind: ElementKind::Div,
                },
                ImageNode::solid_color(bevy::color::Color::WHITE),
                CascadedStyle(ComputedStyle {
                    opacity: Some(0.5),
                    ..Default::default()
                }),
            ))
            .id();
        let text = world.spawn(Text::new("hello")).id();
        world.entity_mut(root).add_child(parent);
        world.entity_mut(parent).add_child(child);
        world.entity_mut(child).add_child(text);
        super::resolve_and_apply_tree(&mut world, root);
        assert_eq!(world.get::<BackgroundColor>(parent).unwrap().0.alpha(), 0.5);
        assert_eq!(world.get::<ImageNode>(child).unwrap().color.alpha(), 0.25);
        assert_eq!(world.get::<TextColor>(text).unwrap().0.alpha(), 0.25);
        world.get_mut::<CascadedStyle>(parent).unwrap().0.opacity = None;
        super::resolve_and_apply_tree(&mut world, root);
        assert_eq!(world.get::<ImageNode>(child).unwrap().color.alpha(), 0.5);
        assert_eq!(world.get::<TextColor>(text).unwrap().0.alpha(), 0.5);
        let mut animated = world
            .get::<crate::style::state::RuntimeComputedStyle>(parent)
            .unwrap()
            .0
            .clone();
        animated.opacity = Some(0.25);
        super::apply_motion_style(&mut world, parent, &animated);
        assert_eq!(world.get::<ImageNode>(child).unwrap().color.alpha(), 0.125);
        assert_eq!(world.get::<TextColor>(text).unwrap().0.alpha(), 0.125);
    }

    #[test]
    fn authored_grid_and_flex_properties_reach_bevy_node() {
        use tilt_ui_css::{
            BoxSizing, GridAutoFlow, GridLine, GridPlacement, GridRepetition, GridTrackGroup,
            GridTrackSize, Length,
        };
        let mut node = Node::default();
        let style = ComputedStyle {
            display: Some(tilt_ui_css::Display::Grid),
            box_sizing: Some(BoxSizing::ContentBox),
            flex_basis: Some(Length::Px(140.0)),
            grid_template_columns: Some(vec![GridTrackGroup {
                repetition: GridRepetition::Count(3),
                tracks: vec![GridTrackSize::MinMax(
                    Box::new(GridTrackSize::Length(Length::Px(0.0))),
                    Box::new(GridTrackSize::Fraction(1.0)),
                )],
            }]),
            grid_auto_flow: Some(GridAutoFlow::ColumnDense),
            grid_column: Some(GridPlacement {
                start: GridLine::Index(2),
                end: GridLine::Span(2),
            }),
            ..Default::default()
        };
        apply_node(&mut node, &style);
        assert_eq!(node.display, Display::Grid);
        assert_eq!(node.box_sizing, bevy::ui::BoxSizing::ContentBox);
        assert_eq!(node.flex_basis, bevy::ui::Val::Px(140.0));
        assert_eq!(node.grid_template_columns.len(), 1);
        assert_eq!(node.grid_auto_flow, bevy::ui::GridAutoFlow::ColumnDense);
        assert_eq!(node.grid_column, bevy::ui::GridPlacement::start_span(2, 2));
        apply_node(&mut node, &ComputedStyle::default());
        assert!(node.grid_template_columns.is_empty());
        assert_eq!(node.grid_column, bevy::ui::GridPlacement::auto());
    }

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
        assert_eq!(
            world.get::<Node>(table).unwrap().grid_template_columns,
            vec![bevy::ui::RepeatedGridTrack::flex(2, 1.0)]
        );
    }

    #[test]
    fn table_cell_keeps_its_authored_position_after_style_application() {
        let mut world = World::new();
        let cell = world
            .spawn((
                Node::default(),
                crate::widgets::structure::table_cell::TableCellInfo {
                    row: Some(1),
                    column: Some(2),
                    ..Default::default()
                },
            ))
            .id();
        apply_element_visual(&mut world, cell, &ComputedStyle::default());
        let node = world.get::<Node>(cell).unwrap();
        assert_eq!(node.grid_column, bevy::ui::GridPlacement::start(3));
        assert_eq!(node.grid_row, bevy::ui::GridPlacement::start(2));
    }
}
