//! Editable CSS colors with a persistent saturation/value canvas and history.

use std::collections::HashMap;

use bevy::window::{PrimaryWindow, Window};
use bevy::{
    asset::{Assets, Handle, RenderAssetUsages},
    ecs::{
        component::Component,
        entity::Entity,
        hierarchy::ChildOf,
        message::{Message, MessageReader, Messages},
        system::Commands,
        world::World,
    },
    image::{Image, ImageSampler},
    math::Vec2,
    ui::{
        ComputedNode, ComputedUiRenderTargetInfo, GlobalZIndex, InteractionDisabled, Node,
        UiGlobalTransform, UiScale, Val,
        widget::{Button, ImageNode, NodeImageMode},
    },
};
use bevy_input_focus::{FocusCause, InputFocus, tab_navigation::TabIndex};
use bevy_picking::{
    Pickable,
    events::{Drag, DragEnd, Pointer, Press, Release},
    pointer::{PointerButton, PointerId},
};
use tilt_ui_core::TemplateAttribute;
use tilt_ui_css::{CssColor, Length, parse_color_value};
use wgpu_types::{Extent3d, TextureDimension, TextureFormat};

use crate::{
    ControlActivated, ControlPart, ControlPartKind, EditableTextChanged, EditableTextCommitted,
    ElementState, TiltControl, WidgetLayoutOverride,
    control::drag::{ActiveControlDrag, DragKind},
    widgets::{
        controls::{spawn_part, spawn_text_part},
        state::set_widget_display,
    },
};

const CANVAS_SIZE: u32 = 64;
const TRACK_WIDTH: u32 = 160;
const TRACK_HEIGHT: u32 = 12;

const PALETTE: [CssColor; 12] = [
    CssColor::rgba(0.66, 0.20, 0.92, 1.0),
    CssColor::rgba(0.45, 0.27, 0.82, 1.0),
    CssColor::rgba(0.20, 0.43, 0.88, 1.0),
    CssColor::rgba(0.10, 0.67, 0.72, 1.0),
    CssColor::rgba(0.13, 0.68, 0.43, 1.0),
    CssColor::rgba(0.77, 0.73, 0.18, 1.0),
    CssColor::rgba(0.93, 0.55, 0.18, 1.0),
    CssColor::rgba(0.85, 0.28, 0.38, 1.0),
    CssColor::rgba(0.30, 0.27, 0.47, 1.0),
    CssColor::rgba(0.57, 0.56, 0.66, 1.0),
    CssColor::rgba(0.93, 0.93, 0.96, 1.0),
    CssColor::rgba(0.10, 0.09, 0.20, 1.0),
];

/// Selects the visible representation of an RGBA color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorDisplayFormat {
    /// Six- or eight-digit hexadecimal notation.
    Hex,
    /// `rgb()` notation without alpha.
    Rgb,
    /// `rgba()` notation with alpha.
    Rgba,
}

/// Stores the authoritative RGBA value and persistent color parts.
#[derive(Component, Debug, Clone)]
pub struct ColorPickerState {
    /// Current normalized RGBA value.
    pub value: CssColor,
    /// Whether the palette is currently open.
    pub open: bool,
    /// Active text-display format.
    pub format: ColorDisplayFormat,
    /// Up to ten most recently committed colors, newest first.
    pub recent: Vec<CssColor>,
    preview: Entity,
    popup: Entity,
    canvas: Entity,
    canvas_thumb: Entity,
    hue_track: Entity,
    hue_thumb: Entity,
    alpha_track: Entity,
    alpha_thumb: Entity,
    canvas_image: Handle<Image>,
    alpha_image: Handle<Image>,
    formats: [Entity; 3],
    recent_parts: Vec<Entity>,
    /// Persistent selectable palette entries.
    pub swatches: Vec<Entity>,
}

/// Notification emitted when user editing or palette selection changes a color.
#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct ColorPickerChanged {
    /// ColorPicker entity.
    pub entity: Entity,
    /// New normalized RGBA value.
    pub value: CssColor,
}

#[derive(Component, Debug, Clone, Copy)]
enum ColorAction {
    Preview(Entity),
    Swatch(Entity, CssColor),
    Format(Entity, ColorDisplayFormat),
    Recent(Entity, usize),
}

fn color_part(
    world: &mut World,
    owner: Entity,
    kind: ControlPartKind,
    color: CssColor,
    action: ColorAction,
) -> Entity {
    let entity = spawn_part(world, owner, kind);
    world.entity_mut(entity).insert((
        Button,
        TiltControl,
        ElementState::default(),
        TabIndex(-1),
        Pickable::default(),
        ImageNode::solid_color(crate::style::convert::color(color)),
        action,
    ));
    entity
}

pub(crate) fn materialize(world: &mut World, entity: Entity, attributes: &[TemplateAttribute]) {
    crate::widgets::controls::input::materialize_parts(world, entity, attributes, false);
    let authored = crate::component::static_attribute_value(attributes, "value");
    let mut value = authored
        .and_then(|source| parse_color_value(source).ok())
        .unwrap_or(CssColor::rgba(0.66, 0.20, 0.92, 1.0));
    if let Some(alpha) = crate::component::static_attribute_value(attributes, "alpha")
        .and_then(|value| value.parse::<u8>().ok())
    {
        value.alpha = f32::from(alpha) / 255.0;
    }
    let format = match crate::component::static_attribute_value(attributes, "format") {
        Some("rgb") => ColorDisplayFormat::Rgb,
        Some("rgba") => ColorDisplayFormat::Rgba,
        _ => ColorDisplayFormat::Hex,
    };
    if world.get_resource::<Assets<Image>>().is_none() {
        world.init_resource::<Assets<Image>>();
    }
    let (hue, _, _) = rgb_to_hsv(value);
    let (canvas_image, hue_image, alpha_image) = {
        let mut images = world.resource_mut::<Assets<Image>>();
        (
            images.add(texture(CANVAS_SIZE, CANVAS_SIZE, canvas_pixels(hue))),
            images.add(texture(TRACK_WIDTH, TRACK_HEIGHT, hue_pixels())),
            images.add(texture(TRACK_WIDTH, TRACK_HEIGHT, alpha_pixels(value))),
        )
    };
    let preview = color_part(
        world,
        entity,
        ControlPartKind::Preview,
        value,
        ColorAction::Preview(entity),
    );
    let popup = spawn_part(world, entity, ControlPartKind::Popup);
    world.entity_mut(popup).insert(GlobalZIndex(100));
    set_widget_display(world, popup, false);
    let canvas = texture_part(
        world,
        entity,
        ControlPartKind::ColorCanvas,
        canvas_image.clone(),
    );
    let canvas_thumb = spawn_part(world, entity, ControlPartKind::ColorCanvasThumb);
    world.entity_mut(canvas).add_child(canvas_thumb);
    let hue_track = texture_part(world, entity, ControlPartKind::HueTrack, hue_image);
    let hue_thumb = spawn_part(world, entity, ControlPartKind::HueThumb);
    world.entity_mut(hue_track).add_child(hue_thumb);
    let alpha_track = texture_part(
        world,
        entity,
        ControlPartKind::AlphaTrack,
        alpha_image.clone(),
    );
    let alpha_thumb = spawn_part(world, entity, ControlPartKind::AlphaThumb);
    world.entity_mut(alpha_track).add_child(alpha_thumb);
    let formats_row = spawn_part(world, entity, ControlPartKind::ColorFormats);
    let formats = [
        ColorDisplayFormat::Hex,
        ColorDisplayFormat::Rgb,
        ColorDisplayFormat::Rgba,
    ]
    .map(|mode| {
        let label = match mode {
            ColorDisplayFormat::Hex => "HEX",
            ColorDisplayFormat::Rgb => "RGB",
            ColorDisplayFormat::Rgba => "RGBA",
        };
        let part = spawn_text_part(world, entity, ControlPartKind::ColorFormat, label);
        world.entity_mut(part).insert((
            Button,
            TiltControl,
            TabIndex(-1),
            Pickable::default(),
            ElementState {
                checked: mode == format,
                ..Default::default()
            },
            ColorAction::Format(entity, mode),
        ));
        world.entity_mut(formats_row).add_child(part);
        part
    });
    let palette = spawn_part(world, entity, ControlPartKind::ColorSwatches);
    let swatches = PALETTE
        .iter()
        .copied()
        .map(|color| {
            let swatch = color_part(
                world,
                entity,
                ControlPartKind::Swatch,
                color,
                ColorAction::Swatch(entity, color),
            );
            world.entity_mut(palette).add_child(swatch);
            swatch
        })
        .collect();
    let recent_row = spawn_part(world, entity, ControlPartKind::RecentColors);
    let recent_parts = (0..10)
        .map(|index| {
            let part = color_part(
                world,
                entity,
                ControlPartKind::RecentColor,
                value,
                ColorAction::Recent(entity, index),
            );
            world.entity_mut(recent_row).add_child(part);
            set_widget_display(world, part, false);
            part
        })
        .collect();
    for part in [
        canvas,
        hue_track,
        alpha_track,
        formats_row,
        palette,
        recent_row,
    ] {
        world.entity_mut(popup).add_child(part);
    }
    world.entity_mut(entity).add_child(preview).add_child(popup);
    world.entity_mut(entity).insert(ColorPickerState {
        value,
        open: false,
        format,
        recent: vec![value],
        preview,
        popup,
        canvas,
        canvas_thumb,
        hue_track,
        hue_thumb,
        alpha_track,
        alpha_thumb,
        canvas_image,
        alpha_image,
        formats,
        recent_parts,
        swatches,
    });
    refresh_color_visuals(world, entity, None);
    refresh_recent(world, entity);
    if authored.is_some_and(|source| parse_color_value(source).is_err()) {
        if let Some(mut state) = world.get_mut::<ElementState>(entity) {
            state.invalid = true;
        }
    } else {
        crate::set_editable_text(world, entity, format_color(value, format));
    }
}

fn texture_part(
    world: &mut World,
    owner: Entity,
    kind: ControlPartKind,
    image: Handle<Image>,
) -> Entity {
    let part = spawn_part(world, owner, kind);
    world.entity_mut(part).insert((
        ImageNode::new(image).with_mode(NodeImageMode::Stretch),
        Pickable::default(),
    ));
    part
}

/// Opens or closes a ColorPicker palette without emitting a user event.
pub fn set_color_picker_open(world: &mut World, entity: Entity, open: bool) -> bool {
    let Some(mut state) = world.get_mut::<ColorPickerState>(entity) else {
        return false;
    };
    if state.open == open {
        return false;
    }
    state.open = open;
    let popup = state.popup;
    if let Some(mut css) = world.get_mut::<ElementState>(entity) {
        css.open = open;
    }
    set_widget_display(world, popup, open);
    if open {
        place_open_color_pickers(world);
    }
    true
}

/// Places a palette next to its trigger, flipping at the right and bottom edges.
fn palette_position(
    anchor: Vec2,
    anchor_size: Vec2,
    popup_size: Vec2,
    viewport: Vec2,
    margin: f32,
    gap: f32,
) -> Vec2 {
    let below = anchor.y + anchor_size.y + gap;
    let above = anchor.y - popup_size.y - gap;
    let x = if anchor.x + popup_size.x > viewport.x - margin {
        anchor.x + anchor_size.x - popup_size.x
    } else {
        anchor.x
    };
    let y = if below + popup_size.y > viewport.y - margin && above >= margin {
        above
    } else {
        below
    };
    Vec2::new(
        x.clamp(margin, (viewport.x - popup_size.x - margin).max(margin)),
        y.clamp(margin, (viewport.y - popup_size.y - margin).max(margin)),
    )
}

/// Keeps open palettes inside their render target after layout and window resizing.
pub(crate) fn place_open_color_pickers(world: &mut World) {
    let palettes = world
        .query::<(Entity, &ColorPickerState)>()
        .iter(world)
        .filter_map(|(owner, state)| state.open.then_some((owner, state.popup, state.canvas)))
        .collect::<Vec<_>>();
    if palettes.is_empty() {
        return;
    }
    let viewport = world
        .query_filtered::<&Window, bevy::ecs::query::With<PrimaryWindow>>()
        .iter(world)
        .next()
        .map(|window| Vec2::new(window.width(), window.height()));
    let Some(viewport) = viewport else {
        return;
    };
    for (owner, popup, canvas) in palettes {
        let (Some(anchor_transform), Some(anchor_node), Some(popup_node)) = (
            world.get::<UiGlobalTransform>(owner),
            world.get::<ComputedNode>(owner),
            world.get::<ComputedNode>(popup),
        ) else {
            continue;
        };
        let Some(inverse) = anchor_transform.try_inverse() else {
            continue;
        };
        let anchor_size = anchor_node.size();
        if anchor_size.x <= 0.0 || anchor_size.y <= 0.0 {
            continue;
        }
        let scale = world
            .get::<ComputedUiRenderTargetInfo>(popup)
            .map_or(1.0, |target| target.scale_factor())
            / world.get_resource::<UiScale>().map_or(1.0, |scale| scale.0);
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let viewport = viewport * scale;
        let margin = 8.0 * scale;
        let available = (viewport - Vec2::splat(margin * 2.0)).max(Vec2::ONE);
        let natural_width = world
            .get::<crate::style::RuntimeComputedStyle>(popup)
            .and_then(|style| match style.0.width {
                Some(Length::Px(width)) => Some(width),
                _ => None,
            })
            .unwrap_or(224.0);
        let width = natural_width.min(available.x / scale).max(1.0);
        let measured_height = popup_node.size().y;
        let natural_canvas_height = world
            .get::<crate::style::RuntimeComputedStyle>(canvas)
            .and_then(|style| match style.0.height {
                Some(Length::Px(height)) => Some(height),
                _ => None,
            })
            .unwrap_or(160.0);
        let current_canvas_height = world
            .get::<ComputedNode>(canvas)
            .map_or(0.0, |node| node.size().y);
        let natural_height = if measured_height > 0.0 {
            measured_height
                + if current_canvas_height > 0.0 {
                    natural_canvas_height * scale - current_canvas_height
                } else {
                    0.0
                }
        } else {
            360.0 * scale
        };
        // Preserve every palette control: only the saturation canvas contracts
        // when a short window cannot accommodate its usual height.
        let canvas_height = (natural_canvas_height
            - (natural_height - available.y).max(0.0) / scale)
            .clamp(40.0, natural_canvas_height);
        let height = natural_height - (natural_canvas_height - canvas_height) * scale;
        let size = Vec2::new(width * scale, height);
        let anchor = anchor_transform.affine().translation - anchor_size * 0.5;
        let position = palette_position(anchor, anchor_size, size, viewport, margin, 6.0 * scale);
        let local = inverse.transform_point2(position) + anchor_size * 0.5;
        let layout = WidgetLayoutOverride {
            width: Some(Val::Px(width)),
            left: Some(Val::Px(local.x / scale)),
            top: Some(Val::Px(local.y / scale)),
            ..Default::default()
        };
        let canvas_layout = WidgetLayoutOverride {
            height: Some(Val::Px(canvas_height)),
            ..Default::default()
        };
        if !world
            .get::<WidgetLayoutOverride>(canvas)
            .is_some_and(|old| *old == canvas_layout)
        {
            if let Some(mut node) = world.get_mut::<Node>(canvas) {
                canvas_layout.apply(&mut node);
            }
            world.entity_mut(canvas).insert(canvas_layout);
        }
        if world
            .get::<WidgetLayoutOverride>(popup)
            .is_some_and(|old| *old == layout)
        {
            continue;
        }
        if let Some(mut node) = world.get_mut::<Node>(popup) {
            layout.apply(&mut node);
        }
        world.entity_mut(popup).insert(layout);
    }
}

/// Sets an RGBA value and refreshes the existing preview without emitting a user event.
pub fn set_color_value(world: &mut World, entity: Entity, value: CssColor) -> bool {
    update_color(world, entity, value, true)
}

/// Changes the visible color notation without changing its semantic value.
pub fn set_color_format(world: &mut World, entity: Entity, format: ColorDisplayFormat) -> bool {
    let Some(mut state) = world.get_mut::<ColorPickerState>(entity) else {
        return false;
    };
    if state.format == format {
        return false;
    }
    state.format = format;
    let (value, formats) = (state.value, state.formats);
    for (index, part) in formats.into_iter().enumerate() {
        if let Some(mut css) = world.get_mut::<ElementState>(part) {
            css.checked = index
                == match format {
                    ColorDisplayFormat::Hex => 0,
                    ColorDisplayFormat::Rgb => 1,
                    ColorDisplayFormat::Rgba => 2,
                };
        }
    }
    crate::set_editable_text(world, entity, format_color(value, format));
    true
}

fn update_color(world: &mut World, entity: Entity, value: CssColor, write_text: bool) -> bool {
    let Some(mut state) = world.get_mut::<ColorPickerState>(entity) else {
        return false;
    };
    if state.value == value {
        return false;
    }
    let previous = state.value;
    state.value = value;
    let format = state.format;
    refresh_color_visuals(world, entity, Some(previous));
    if write_text {
        crate::set_editable_text(world, entity, format_color(value, format));
    }
    if let Some(mut css) = world.get_mut::<ElementState>(entity)
        && css.invalid
    {
        css.invalid = false;
    }
    true
}

fn format_color(value: CssColor, format: ColorDisplayFormat) -> String {
    let channel = |channel: f32| (channel.clamp(0.0, 1.0) * 255.0).round() as u8;
    let (red, green, blue, alpha) = (
        channel(value.red),
        channel(value.green),
        channel(value.blue),
        channel(value.alpha),
    );
    match format {
        ColorDisplayFormat::Hex if alpha == 255 => format!("#{red:02X}{green:02X}{blue:02X}"),
        ColorDisplayFormat::Hex => format!("#{red:02X}{green:02X}{blue:02X}{alpha:02X}"),
        ColorDisplayFormat::Rgb => format!("rgb({red}, {green}, {blue})"),
        ColorDisplayFormat::Rgba => format!(
            "rgba({red}, {green}, {blue}, {:.3})",
            value.alpha.clamp(0.0, 1.0)
        ),
    }
}

fn position_thumb(world: &mut World, entity: Entity, left: f32, top: Option<f32>) {
    let layout = WidgetLayoutOverride {
        left: Some(Val::Percent(left.clamp(0.0, 100.0))),
        top: top.map(|top| Val::Percent(top.clamp(0.0, 100.0))),
        ..Default::default()
    };
    if world
        .get::<WidgetLayoutOverride>(entity)
        .is_some_and(|current| current.left == layout.left && current.top == layout.top)
    {
        return;
    }
    if let Some(mut node) = world.get_mut::<bevy::ui::Node>(entity) {
        layout.apply(&mut node);
    }
    world.entity_mut(entity).insert(layout);
}

fn refresh_color_visuals(world: &mut World, entity: Entity, previous: Option<CssColor>) {
    let Some(state) = world.get::<ColorPickerState>(entity) else {
        return;
    };
    let (value, preview, canvas_thumb, hue_thumb, alpha_thumb, canvas_image, alpha_image) = (
        state.value,
        state.preview,
        state.canvas_thumb,
        state.hue_thumb,
        state.alpha_thumb,
        state.canvas_image.clone(),
        state.alpha_image.clone(),
    );
    let (hue, saturation, lightness) = rgb_to_hsv(value);
    if let Some(mut image) = world.get_mut::<ImageNode>(preview) {
        image.color = crate::style::convert::color(value);
    }
    let previous_hue = previous.map(|previous| rgb_to_hsv(previous).0);
    if previous_hue.is_none_or(|old| hue_step(old) != hue_step(hue))
        && let Some(mut image) = world.resource_mut::<Assets<Image>>().get_mut(&canvas_image)
    {
        image.data = Some(canvas_pixels(hue_step(hue)));
    }
    if previous.is_none_or(|old| {
        [old.red, old.green, old.blue].map(color_byte)
            != [value.red, value.green, value.blue].map(color_byte)
    }) && let Some(mut image) = world.resource_mut::<Assets<Image>>().get_mut(&alpha_image)
    {
        image.data = Some(alpha_pixels(value));
    }
    position_thumb(
        world,
        canvas_thumb,
        saturation * 100.0,
        Some((1.0 - lightness) * 100.0),
    );
    position_thumb(world, hue_thumb, hue / 360.0 * 100.0, None);
    position_thumb(world, alpha_thumb, value.alpha * 100.0, None);
}

fn push_recent(world: &mut World, entity: Entity) {
    let Some(mut state) = world.get_mut::<ColorPickerState>(entity) else {
        return;
    };
    let value = state.value;
    state.recent.retain(|color| *color != value);
    state.recent.insert(0, value);
    state.recent.truncate(10);
    refresh_recent(world, entity);
}

fn refresh_recent(world: &mut World, entity: Entity) {
    let Some(state) = world.get::<ColorPickerState>(entity) else {
        return;
    };
    let (parts, recent) = (state.recent_parts.clone(), state.recent.clone());
    for (index, part) in parts.into_iter().enumerate() {
        let Some(color) = recent.get(index).copied() else {
            set_widget_display(world, part, false);
            continue;
        };
        if let Some(mut image) = world.get_mut::<ImageNode>(part) {
            image.color = crate::style::convert::color(color);
        }
        set_widget_display(world, part, true);
    }
}

fn rgb_to_hsv(color: CssColor) -> (f32, f32, f32) {
    let max = color.red.max(color.green).max(color.blue);
    let min = color.red.min(color.green).min(color.blue);
    let delta = max - min;
    let hue = if delta <= f32::EPSILON {
        0.0
    } else if max == color.red {
        60.0 * ((color.green - color.blue) / delta).rem_euclid(6.0)
    } else if max == color.green {
        60.0 * ((color.blue - color.red) / delta + 2.0)
    } else {
        60.0 * ((color.red - color.green) / delta + 4.0)
    };
    (
        hue,
        if max <= f32::EPSILON {
            0.0
        } else {
            delta / max
        },
        max,
    )
}

fn hsv_to_rgb(hue: f32, saturation: f32, value: f32, alpha: f32) -> CssColor {
    let chroma = value * saturation;
    let segment = (hue / 60.0).rem_euclid(6.0);
    let secondary = chroma * (1.0 - (segment.rem_euclid(2.0) - 1.0).abs());
    let (red, green, blue) = match segment {
        x if x < 1.0 => (chroma, secondary, 0.0),
        x if x < 2.0 => (secondary, chroma, 0.0),
        x if x < 3.0 => (0.0, chroma, secondary),
        x if x < 4.0 => (0.0, secondary, chroma),
        x if x < 5.0 => (secondary, 0.0, chroma),
        _ => (chroma, 0.0, secondary),
    };
    let offset = value - chroma;
    CssColor::rgba(red + offset, green + offset, blue + offset, alpha)
}

fn pixel(color: CssColor, pixels: &mut Vec<u8>) {
    for channel in [color.red, color.green, color.blue, color.alpha] {
        pixels.push(color_byte(channel));
    }
}

fn color_byte(channel: f32) -> u8 {
    (channel.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn hue_step(hue: f32) -> f32 {
    (hue / 360.0 * TRACK_WIDTH as f32).round() * 360.0 / TRACK_WIDTH as f32
}

fn canvas_pixels(hue: f32) -> Vec<u8> {
    let mut pixels = Vec::with_capacity((CANVAS_SIZE * CANVAS_SIZE * 4) as usize);
    let pure = hsv_to_rgb(hue, 1.0, 1.0, 1.0);
    for y in 0..CANVAS_SIZE {
        let value = 1.0 - y as f32 / (CANVAS_SIZE - 1) as f32;
        for x in 0..CANVAS_SIZE {
            let saturation = x as f32 / (CANVAS_SIZE - 1) as f32;
            let white = 1.0 - saturation;
            pixel(
                CssColor::rgba(
                    (white + pure.red * saturation) * value,
                    (white + pure.green * saturation) * value,
                    (white + pure.blue * saturation) * value,
                    1.0,
                ),
                &mut pixels,
            );
        }
    }
    pixels
}

fn hue_pixels() -> Vec<u8> {
    let mut pixels = Vec::with_capacity((TRACK_WIDTH * TRACK_HEIGHT * 4) as usize);
    for _ in 0..TRACK_HEIGHT {
        for x in 0..TRACK_WIDTH {
            pixel(
                hsv_to_rgb(x as f32 / (TRACK_WIDTH - 1) as f32 * 360.0, 1.0, 1.0, 1.0),
                &mut pixels,
            );
        }
    }
    pixels
}

fn alpha_pixels(color: CssColor) -> Vec<u8> {
    let mut pixels = Vec::with_capacity((TRACK_WIDTH * TRACK_HEIGHT * 4) as usize);
    for y in 0..TRACK_HEIGHT {
        for x in 0..TRACK_WIDTH {
            let alpha = x as f32 / (TRACK_WIDTH - 1) as f32;
            let base = if (x / 8 + y / 8) % 2 == 0 { 0.91 } else { 0.78 };
            pixel(
                CssColor::rgba(
                    base * (1.0 - alpha) + color.red * alpha,
                    base * (1.0 - alpha) + color.green * alpha,
                    base * (1.0 - alpha) + color.blue * alpha,
                    1.0,
                ),
                &mut pixels,
            );
        }
    }
    pixels
}

fn texture(width: u32, height: u32, pixels: Vec<u8>) -> Image {
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

fn color_surface(world: &World, target: Entity) -> Option<(Entity, Entity)> {
    let mut current = target;
    loop {
        if let Some(part) = world.get::<ControlPart>(current)
            && matches!(
                part.kind,
                ControlPartKind::ColorCanvas
                    | ControlPartKind::HueTrack
                    | ControlPartKind::AlphaTrack
            )
            && world
                .get::<ColorPickerState>(part.owner)
                .is_some_and(|state| {
                    [state.canvas, state.hue_track, state.alpha_track].contains(&current)
                })
        {
            return Some((part.owner, current));
        }
        current = world.get::<ChildOf>(current)?.parent();
    }
}

fn active_color_surface(world: &mut World, pointer: PointerId) -> Option<(Entity, Entity)> {
    let mut query = world.query::<(Entity, &ActiveControlDrag)>();
    query.iter(world).find_map(|(owner, active)| {
        (active.pointer == pointer)
            .then_some(active.kind)
            .and_then(|kind| match kind {
                DragKind::Color { surface } => Some((owner, surface)),
                _ => None,
            })
    })
}

fn color_fraction(world: &World, surface: Entity, position: Vec2) -> Option<Vec2> {
    let node = world.get::<ComputedNode>(surface)?;
    let transform = *world.get::<UiGlobalTransform>(surface)?;
    let target = world.get::<ComputedUiRenderTargetInfo>(surface)?;
    let ui_scale = world.get_resource::<UiScale>().map_or(1.0, |scale| scale.0);
    let point = node.normalize_point(transform, position * target.scale_factor() / ui_scale)?;
    Some((point + Vec2::splat(0.5)).clamp(Vec2::ZERO, Vec2::ONE))
}

fn update_from_pointer(world: &mut World, owner: Entity, surface: Entity, position: Vec2) {
    let Some(point) = color_fraction(world, surface, position) else {
        return;
    };
    let Some(state) = world.get::<ColorPickerState>(owner) else {
        return;
    };
    let current = state.value;
    let (hue, saturation, value) = rgb_to_hsv(current);
    let next = match world.get::<ControlPart>(surface).map(|part| part.kind) {
        Some(ControlPartKind::ColorCanvas) => {
            hsv_to_rgb(hue, point.x, 1.0 - point.y, current.alpha)
        }
        Some(ControlPartKind::HueTrack) => {
            hsv_to_rgb(point.x * 360.0, saturation, value, current.alpha)
        }
        Some(ControlPartKind::AlphaTrack) => {
            CssColor::rgba(current.red, current.green, current.blue, point.x)
        }
        _ => return,
    };
    if set_color_value(world, owner, next) {
        world
            .resource_mut::<Messages<ColorPickerChanged>>()
            .write(ColorPickerChanged {
                entity: owner,
                value: next,
            });
    }
}

fn finish_drag(commands: &mut Commands, pointer: PointerId) {
    commands.queue(move |world: &mut World| {
        let owners = {
            let mut query = world.query::<(Entity, &ActiveControlDrag)>();
            query
                .iter(world)
                .filter_map(|(entity, drag)| {
                    (drag.pointer == pointer && matches!(drag.kind, DragKind::Color { .. }))
                        .then_some(entity)
                })
                .collect::<Vec<_>>()
        };
        for owner in owners {
            world.entity_mut(owner).remove::<ActiveControlDrag>();
            push_recent(world, owner);
        }
    });
}

pub(crate) fn color_pointer_input(
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
            let (target, position, pointer) = (
                press.entity,
                press.pointer_location.position,
                press.pointer_id,
            );
            commands.queue(move |world: &mut World| {
                let Some((owner, surface)) = color_surface(world, target) else {
                    return;
                };
                if world.get::<InteractionDisabled>(owner).is_some() {
                    return;
                }
                set_color_picker_open(world, owner, true);
                if let Some(mut focus) = world.get_resource_mut::<InputFocus>() {
                    focus.set(owner, FocusCause::Pressed);
                }
                world.entity_mut(owner).insert(ActiveControlDrag {
                    pointer,
                    kind: DragKind::Color { surface },
                });
                update_from_pointer(world, owner, surface, position);
            });
        }
    }
    if let Some(drags) = drags.as_mut() {
        let mut latest = HashMap::new();
        for drag in drags.read() {
            if drag.button != PointerButton::Primary {
                continue;
            }
            latest.insert(drag.pointer_id, drag.pointer_location.position);
        }
        for (pointer, position) in latest {
            commands.queue(move |world: &mut World| {
                let Some((owner, surface)) = active_color_surface(world, pointer) else {
                    return;
                };
                if world.get::<InteractionDisabled>(owner).is_some() {
                    return;
                }
                update_from_pointer(world, owner, surface, position);
            });
        }
    }
    if let Some(ends) = ends.as_mut() {
        for end in ends.read() {
            if end.button == PointerButton::Primary {
                finish_drag(&mut commands, end.pointer_id);
            }
        }
    }
    if let Some(releases) = releases.as_mut() {
        for release in releases.read() {
            if release.button == PointerButton::Primary {
                finish_drag(&mut commands, release.pointer_id);
            }
        }
    }
}

pub(crate) fn process_color_edits(
    mut edits: MessageReader<EditableTextChanged>,
    mut commands: Commands,
) {
    for edit in edits.read() {
        let entity = edit.entity;
        let parsed = parse_color_value(&edit.value);
        let rgb_without_alpha = edit.value.trim_start().starts_with("rgb(");
        commands.queue(move |world: &mut World| {
            let Some(current) = world
                .get::<ColorPickerState>(entity)
                .map(|state| state.value)
            else {
                return;
            };
            if let Some(mut css) = world.get_mut::<ElementState>(entity)
                && css.invalid != parsed.is_err()
            {
                css.invalid = parsed.is_err();
            }
            let Ok(mut value) = parsed else {
                return;
            };
            if rgb_without_alpha {
                value.alpha = current.alpha;
            }
            if update_color(world, entity, value, false) {
                world
                    .resource_mut::<Messages<ColorPickerChanged>>()
                    .write(ColorPickerChanged { entity, value });
            }
        });
    }
}

pub(crate) fn process_color_commits(
    mut commits: MessageReader<EditableTextCommitted>,
    mut commands: Commands,
) {
    for commit in commits.read() {
        let (entity, parsed) = (commit.entity, parse_color_value(&commit.value));
        let rgb_without_alpha = commit.value.trim_start().starts_with("rgb(");
        commands.queue(move |world: &mut World| {
            let (Some(state), Ok(mut value)) = (world.get::<ColorPickerState>(entity), parsed)
            else {
                return;
            };
            let (format, alpha) = (state.format, state.value.alpha);
            if rgb_without_alpha {
                value.alpha = alpha;
            }
            update_color(world, entity, value, false);
            push_recent(world, entity);
            crate::set_editable_text(world, entity, format_color(value, format));
        });
    }
}

pub(crate) fn process_color_activation(
    mut activated: MessageReader<ControlActivated>,
    mut commands: Commands,
) {
    for activation in activated.read() {
        let entity = activation.entity;
        commands.queue(move |world: &mut World| {
            if world.get::<ColorPickerState>(entity).is_some() {
                if world.get::<InteractionDisabled>(entity).is_none() {
                    set_color_picker_open(world, entity, true);
                }
                return;
            }
            let Some(action) = world.get::<ColorAction>(entity).copied() else {
                return;
            };
            let owner = match action {
                ColorAction::Preview(owner)
                | ColorAction::Swatch(owner, _)
                | ColorAction::Format(owner, _)
                | ColorAction::Recent(owner, _) => owner,
            };
            if world.get::<InteractionDisabled>(owner).is_some() {
                return;
            }
            match action {
                ColorAction::Preview(owner) => {
                    set_color_picker_open(world, owner, true);
                }
                ColorAction::Swatch(owner, color) => {
                    if set_color_value(world, owner, color) {
                        world.resource_mut::<Messages<ColorPickerChanged>>().write(
                            ColorPickerChanged {
                                entity: owner,
                                value: color,
                            },
                        );
                    }
                    push_recent(world, owner);
                    set_color_picker_open(world, owner, false);
                    if let Some(mut focus) = world.get_resource_mut::<InputFocus>() {
                        focus.set(owner, FocusCause::Pressed);
                    }
                }
                ColorAction::Format(owner, format) => {
                    set_color_format(world, owner, format);
                    if let Some(mut focus) = world.get_resource_mut::<InputFocus>() {
                        focus.set(owner, FocusCause::Pressed);
                    }
                }
                ColorAction::Recent(owner, index) => {
                    let color = world
                        .get::<ColorPickerState>(owner)
                        .and_then(|state| state.recent.get(index).copied());
                    if let Some(color) = color {
                        if set_color_value(world, owner, color) {
                            world.resource_mut::<Messages<ColorPickerChanged>>().write(
                                ColorPickerChanged {
                                    entity: owner,
                                    value: color,
                                },
                            );
                        }
                        push_recent(world, owner);
                        set_color_picker_open(world, owner, false);
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{ColorDisplayFormat, ColorPickerChanged, ColorPickerState, format_color};
    use crate::{
        ControlActivated, EditableText, EditableTextCommitted, ElementState, TiltControl,
        TiltElement, TiltUiControlRuntimePlugin,
    };
    use bevy::{
        app::App,
        ecs::change_detection::DetectChanges,
        ecs::message::{MessageCursor, Messages},
        ui::{Display, Node},
    };
    use bevy_picking::pointer::PointerId;
    use tilt_ui_core::{ElementKind, TemplateAttribute};
    use tilt_ui_css::{CssColor, parse_color_value};

    #[test]
    fn popup_flips_at_right_and_bottom_edges() {
        use bevy::math::Vec2;
        let viewport = Vec2::new(400.0, 300.0);
        let popup = Vec2::new(224.0, 180.0);
        assert_eq!(
            super::palette_position(
                Vec2::new(350.0, 250.0),
                Vec2::new(40.0, 32.0),
                popup,
                viewport,
                8.0,
                6.0
            ),
            Vec2::new(166.0, 64.0),
        );
        assert_eq!(
            super::palette_position(
                Vec2::new(10.0, 10.0),
                Vec2::new(40.0, 32.0),
                popup,
                viewport,
                8.0,
                6.0
            ),
            Vec2::new(10.0, 48.0),
        );
    }

    #[test]
    fn popup_clamps_when_neither_side_has_full_room() {
        use bevy::math::Vec2;
        let position = super::palette_position(
            Vec2::new(210.0, 120.0),
            Vec2::new(30.0, 28.0),
            Vec2::new(224.0, 160.0),
            Vec2::new(250.0, 180.0),
            8.0,
            6.0,
        );
        assert_eq!(position, Vec2::new(16.0, 12.0));
    }

    #[test]
    fn open_palette_repositions_after_window_resize() {
        use bevy::{
            math::Vec2,
            ui::{ComputedNode, UiGlobalTransform, Val},
            window::{PrimaryWindow, Window},
        };
        let mut world = bevy::ecs::world::World::new();
        let window = world
            .spawn((
                Window {
                    resolution: (400, 300).into(),
                    ..Default::default()
                },
                PrimaryWindow,
            ))
            .id();
        let picker = world
            .spawn((
                Node::default(),
                ElementState::default(),
                ComputedNode {
                    size: Vec2::new(160.0, 40.0),
                    ..Default::default()
                },
                UiGlobalTransform::from_xy(350.0, 250.0),
            ))
            .id();
        super::materialize(&mut world, picker, &[]);
        let popup = world.get::<ColorPickerState>(picker).unwrap().popup;
        world.entity_mut(popup).insert(ComputedNode {
            size: Vec2::new(224.0, 360.0),
            ..Default::default()
        });
        assert!(super::set_color_picker_open(&mut world, picker, true));
        let layout = *world.get::<crate::WidgetLayoutOverride>(popup).unwrap();
        assert_eq!(layout.left, Some(Val::Px(-102.0)));
        assert_eq!(layout.top, Some(Val::Px(-222.0)));
        assert_eq!(layout.max_height, None);
        let canvas = world.get::<ColorPickerState>(picker).unwrap().canvas;
        assert_eq!(
            world
                .get::<crate::WidgetLayoutOverride>(canvas)
                .unwrap()
                .height,
            Some(Val::Px(84.0))
        );
        // Simulate the compact size produced by the next layout pass.
        world.entity_mut(popup).insert(ComputedNode {
            size: Vec2::new(224.0, 284.0),
            ..Default::default()
        });
        world.entity_mut(canvas).insert(ComputedNode {
            size: Vec2::new(202.0, 84.0),
            ..Default::default()
        });
        world.get_mut::<Window>(window).unwrap().resolution = (800, 800).into();
        super::place_open_color_pickers(&mut world);
        let layout = *world.get::<crate::WidgetLayoutOverride>(popup).unwrap();
        assert_eq!(layout.left, Some(Val::Px(0.0)));
        assert_eq!(layout.top, Some(Val::Px(46.0)));
        assert_eq!(
            world
                .get::<crate::WidgetLayoutOverride>(canvas)
                .unwrap()
                .height,
            Some(Val::Px(160.0))
        );
    }

    #[test]
    fn color_text_round_trips_through_css_parser() {
        let color = CssColor::rgba(1.0, 0.5, 0.0, 1.0);
        let parsed = parse_color_value(&format_color(color, ColorDisplayFormat::Hex)).unwrap();
        assert!((parsed.green - color.green).abs() < 0.003);
        assert_eq!(parsed.alpha, 1.0);
        assert!(parse_color_value("rgb(255, 0, 0)").is_ok());
        assert!(parse_color_value("rgba(255, 0, 0, 0.5)").is_ok());
        assert!(parse_color_value("broken").is_err());
    }

    #[test]
    fn active_color_drag_keeps_its_original_surface() {
        let mut world = bevy::ecs::world::World::new();
        let surface = world.spawn_empty().id();
        let picker = world
            .spawn(crate::control::drag::ActiveControlDrag {
                pointer: PointerId::Mouse,
                kind: crate::control::drag::DragKind::Color { surface },
            })
            .id();

        assert_eq!(
            super::active_color_surface(&mut world, PointerId::Mouse),
            Some((picker, surface))
        );
    }

    #[test]
    fn dragging_a_valid_color_does_not_invalidate_its_css_state() {
        let mut world = bevy::ecs::world::World::new();
        let picker = world.spawn((Node::default(), ElementState::default())).id();
        super::materialize(&mut world, picker, &[]);
        world.clear_trackers();

        assert!(super::set_color_value(
            &mut world,
            picker,
            CssColor::rgba(0.25, 0.5, 0.75, 1.0),
        ));
        assert!(
            !world
                .entity(picker)
                .get_ref::<ElementState>()
                .unwrap()
                .is_changed()
        );
    }

    #[test]
    fn swatch_selection_updates_existing_parts_and_emits_one_change() {
        let mut app = App::new();
        app.add_plugins(TiltUiControlRuntimePlugin);
        let picker = app
            .world_mut()
            .spawn((
                Node::default(),
                TiltElement {
                    kind: ElementKind::ColorPicker,
                },
                TiltControl,
                ElementState::default(),
            ))
            .id();
        super::materialize(
            app.world_mut(),
            picker,
            &[TemplateAttribute::Static {
                name: "value".into(),
                value: "#112233".into(),
            }],
        );
        let before = app.world().get::<ColorPickerState>(picker).unwrap().clone();
        app.world_mut()
            .resource_mut::<Messages<ControlActivated>>()
            .write(ControlActivated {
                entity: before.swatches[0],
            });
        app.update();
        let after = app.world().get::<ColorPickerState>(picker).unwrap();
        assert_ne!(after.value, before.value);
        assert_eq!(after.popup, before.popup);
        assert_eq!(after.preview, before.preview);
        assert_eq!(after.swatches, before.swatches);
        assert_eq!(
            app.world().get::<EditableText>(picker).unwrap().value,
            format_color(after.value, after.format)
        );
        let mut cursor = MessageCursor::<ColorPickerChanged>::default();
        assert_eq!(
            cursor
                .read(app.world().resource::<Messages<ColorPickerChanged>>())
                .count(),
            1
        );
    }

    #[test]
    fn static_alpha_initializes_semantic_and_visible_value_together() {
        let mut app = App::new();
        let picker = app
            .world_mut()
            .spawn((Node::default(), ElementState::default()))
            .id();
        super::materialize(
            app.world_mut(),
            picker,
            &[
                TemplateAttribute::Static {
                    name: "value".into(),
                    value: "#112233".into(),
                },
                TemplateAttribute::Static {
                    name: "alpha".into(),
                    value: "128".into(),
                },
            ],
        );
        let state = app.world().get::<ColorPickerState>(picker).unwrap();
        assert_eq!(state.value.alpha, 128.0 / 255.0);
        assert_eq!(
            app.world().get::<EditableText>(picker).unwrap().value,
            "#11223380"
        );
    }

    #[test]
    fn canvas_corners_and_hsv_conversion_represent_real_colors() {
        let pixels = super::canvas_pixels(0.0);
        assert_eq!(&pixels[0..4], &[255, 255, 255, 255]);
        let top_right = ((super::CANVAS_SIZE - 1) * 4) as usize;
        assert_eq!(&pixels[top_right..top_right + 4], &[255, 0, 0, 255]);
        let bottom_left = (((super::CANVAS_SIZE - 1) * super::CANVAS_SIZE) * 4) as usize;
        assert_eq!(&pixels[bottom_left..bottom_left + 4], &[0, 0, 0, 255]);
        let color = CssColor::rgba(0.2, 0.6, 0.9, 0.4);
        let (hue, saturation, value) = super::rgb_to_hsv(color);
        let round_trip = super::hsv_to_rgb(hue, saturation, value, color.alpha);
        assert!((round_trip.red - color.red).abs() < 0.001);
        assert!((round_trip.green - color.green).abs() < 0.001);
        assert!((round_trip.blue - color.blue).abs() < 0.001);
    }

    #[test]
    fn input_click_opens_popup_and_formats_share_one_color_value() {
        let mut app = App::new();
        app.add_plugins(TiltUiControlRuntimePlugin);
        let picker = app
            .world_mut()
            .spawn((
                Node::default(),
                TiltElement {
                    kind: ElementKind::ColorPicker,
                },
                TiltControl,
                ElementState::default(),
            ))
            .id();
        super::materialize(
            app.world_mut(),
            picker,
            &[TemplateAttribute::Static {
                name: "value".into(),
                value: "rgba(17, 34, 51, 0.5)".into(),
            }],
        );
        let popup = app.world().get::<ColorPickerState>(picker).unwrap().popup;
        assert_eq!(
            app.world().get::<Node>(popup).unwrap().display,
            Display::None
        );
        app.world_mut()
            .resource_mut::<Messages<ControlActivated>>()
            .write(ControlActivated { entity: picker });
        app.update();
        assert!(app.world().get::<ColorPickerState>(picker).unwrap().open);
        assert_ne!(
            app.world().get::<Node>(popup).unwrap().display,
            Display::None
        );
        let value = app.world().get::<ColorPickerState>(picker).unwrap().value;
        assert_eq!(
            app.world().get::<EditableText>(picker).unwrap().value,
            format_color(value, ColorDisplayFormat::Hex)
        );
        assert!(super::set_color_format(
            app.world_mut(),
            picker,
            ColorDisplayFormat::Rgb
        ));
        assert_eq!(
            app.world().get::<EditableText>(picker).unwrap().value,
            "rgb(17, 34, 51)"
        );
        app.world_mut()
            .resource_mut::<Messages<EditableTextCommitted>>()
            .write(EditableTextCommitted {
                entity: picker,
                name: None,
                value: "rgb(17, 34, 51)".into(),
            });
        app.update();
        assert_eq!(
            app.world()
                .get::<ColorPickerState>(picker)
                .unwrap()
                .value
                .alpha,
            value.alpha
        );
        assert!(super::set_color_format(
            app.world_mut(),
            picker,
            ColorDisplayFormat::Rgba
        ));
        assert!(
            app.world()
                .get::<EditableText>(picker)
                .unwrap()
                .value
                .starts_with("rgba(17, 34, 51,")
        );
        assert_eq!(
            app.world().get::<ColorPickerState>(picker).unwrap().value,
            value
        );
    }

    #[test]
    fn ten_recent_colors_reuse_the_same_parts() {
        let mut world = bevy::ecs::world::World::new();
        let picker = world.spawn((Node::default(), ElementState::default())).id();
        super::materialize(&mut world, picker, &[]);
        let parts = world
            .get::<ColorPickerState>(picker)
            .unwrap()
            .recent_parts
            .clone();
        for step in 0..12 {
            let color = CssColor::rgba(step as f32 / 12.0, 0.25, 0.5, 1.0);
            super::set_color_value(&mut world, picker, color);
            super::push_recent(&mut world, picker);
        }
        let state = world.get::<ColorPickerState>(picker).unwrap();
        assert_eq!(state.recent.len(), 10);
        assert_eq!(state.recent_parts, parts);
        assert_eq!(state.recent[0].red, 11.0 / 12.0);
    }
}
