//! Cached image filters and layout-only background cropping.

use std::collections::{HashMap, HashSet};

use bevy::{
    app::{App, Plugin, PostUpdate, Update},
    asset::{Asset, AssetEvent, AssetId, Assets, Handle, RenderAssetUsages},
    ecs::{
        change_detection::DetectChanges,
        component::Component,
        lifecycle::RemovedComponents,
        message::MessageReader,
        query::{Changed, Or},
        resource::Resource,
        system::{Query, Res, ResMut},
    },
    image::Image,
    math::{Rect, Vec2, Vec4},
    prelude::IntoScheduleConfigs,
    reflect::TypePath,
    render::{RenderApp, render_resource::AsBindGroup},
    shader::{Shader, ShaderRef},
    tasks::{AsyncComputeTaskPool, Task, TaskPool, futures::check_ready},
    ui::{
        ComputedNode, UiGlobalTransform, UiSystems,
        widget::{ImageNode, NodeImageMode},
    },
    ui_render::{
        UiMaterialPlugin,
        ui_material::{MaterialNode, UiMaterial},
    },
    window::{PrimaryWindow, Window},
};
use image::{DynamicImage, RgbaImage, imageops};
use tilt_ui_css::{BackgroundAttachment, BackgroundEffect, BackgroundPosition, BackgroundSize};
use wgpu_types::TextureFormat;

use super::plugin::TiltUiStyleRuntimeSet;

#[derive(Component, Clone)]
pub(crate) struct FilteredBackground {
    pub source: Handle<Image>,
    pub effects: Vec<BackgroundEffect>,
    pub output: Option<Handle<Image>>,
}

#[derive(Component, Clone, Copy, PartialEq)]
pub(crate) struct BackgroundLayout {
    pub size: BackgroundSize,
    pub position: BackgroundPosition,
    pub attachment: BackgroundAttachment,
}

const CONTAIN_SHADER: Handle<Shader> =
    bevy::asset::uuid_handle!("f94ab890-5a99-4729-925a-3699d7fcf327");

/// A cheap GPU quad used when a contained background needs non-centered placement.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(crate) struct PositionedContainMaterial {
    #[uniform(0)]
    pub geometry: Vec4,
    #[texture(1)]
    #[sampler(2)]
    pub image: Handle<Image>,
    #[uniform(3)]
    pub opacity: Vec4,
}

impl UiMaterial for PositionedContainMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(CONTAIN_SHADER)
    }
}

#[derive(Component, Clone, Copy, PartialEq)]
pub(crate) struct PositionedContain {
    pub position: BackgroundPosition,
    pub opacity: f32,
}

type FilterKey = (AssetId<Image>, Vec<BackgroundEffect>);

#[derive(Resource, Default)]
struct FilteredBackgroundCache {
    ready: HashMap<FilterKey, Handle<Image>>,
    pending: HashMap<FilterKey, Task<Option<Image>>>,
    failed: HashSet<FilterKey>,
}

pub(crate) struct BackgroundRuntimePlugin;

impl Plugin for BackgroundRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<AssetEvent<Image>>()
            .init_resource::<FilteredBackgroundCache>()
            .add_systems(
                Update,
                refresh_filtered_backgrounds.after(TiltUiStyleRuntimeSet::Apply),
            )
            .add_systems(PostUpdate, update_background_rects.after(UiSystems::Layout));
        app.add_systems(
            PostUpdate,
            update_positioned_contain.after(UiSystems::Layout),
        );
        if app.get_sub_app(RenderApp).is_none() {
            return;
        }
        app.world_mut()
            .resource_mut::<Assets<Shader>>()
            .insert(
                CONTAIN_SHADER.id(),
                Shader::from_wgsl(
                    include_str!("positioned_contain.wgsl"),
                    "tilt_ui_positioned_contain.wgsl",
                ),
            )
            .expect("unique embedded contain shader");
        app.add_plugins(UiMaterialPlugin::<PositionedContainMaterial>::default());
    }
}

fn update_positioned_contain(
    images: Option<Res<Assets<Image>>>,
    materials: Option<ResMut<Assets<PositionedContainMaterial>>>,
    backgrounds: Query<(
        &PositionedContain,
        &ComputedNode,
        &ImageNode,
        &MaterialNode<PositionedContainMaterial>,
    )>,
) {
    let (Some(images), Some(mut materials)) = (images, materials) else {
        return;
    };
    for (positioned, computed, image_node, material_node) in &backgrounds {
        let Some(image) = images.get(&image_node.image) else {
            continue;
        };
        let source = Vec2::new(
            image.texture_descriptor.size.width as f32,
            image.texture_descriptor.size.height as f32,
        );
        let area = computed.size();
        if source.min_element() <= 0.0 || area.min_element() <= 0.0 {
            continue;
        }
        let geometry = contain_geometry(source, area, positioned.position);
        let opacity = Vec4::new(positioned.opacity, 0.0, 0.0, 0.0);
        let changed = materials.get(&material_node.0).is_some_and(|material| {
            material.geometry != geometry
                || material.opacity != opacity
                || material.image != image_node.image
        });
        if changed && let Some(mut material) = materials.get_mut(&material_node.0) {
            material.geometry = geometry;
            material.opacity = opacity;
            material.image = image_node.image.clone();
        }
    }
}

fn contain_geometry(source: Vec2, area: Vec2, position: BackgroundPosition) -> Vec4 {
    let scale = (area / source).min_element();
    let draw = source * scale;
    let margin = (area - draw) * Vec2::new(position.x, position.y);
    Vec4::new(
        margin.x / area.x,
        margin.y / area.y,
        draw.x / area.x,
        draw.y / area.y,
    )
}

fn refresh_filtered_backgrounds(
    mut events: MessageReader<AssetEvent<Image>>,
    images: Option<ResMut<Assets<Image>>>,
    mut cache: ResMut<FilteredBackgroundCache>,
    mut removed: RemovedComponents<FilteredBackground>,
    mut backgrounds: Query<(&mut FilteredBackground, &mut ImageNode)>,
) {
    let Some(mut images) = images else {
        return;
    };
    let changed: HashSet<_> = events
        .read()
        .filter_map(|event| match event {
            AssetEvent::Modified { id } | AssetEvent::Removed { id } => Some(*id),
            AssetEvent::Added { .. }
            | AssetEvent::Unused { .. }
            | AssetEvent::LoadedWithDependencies { .. } => None,
        })
        .collect();
    if !changed.is_empty() {
        cache.ready.retain(|(source, _), output| {
            if changed.contains(source) {
                images.remove(output.id());
                false
            } else {
                true
            }
        });
        cache
            .pending
            .retain(|(source, _), _| !changed.contains(source));
        cache.failed.retain(|(source, _)| !changed.contains(source));
    }
    let requests_changed = backgrounds
        .iter_mut()
        .any(|(background, _)| background.is_changed());
    let requests_removed = removed.read().count() > 0;
    if requests_changed || requests_removed {
        let active: HashSet<_> = backgrounds
            .iter()
            .map(|(background, _)| (background.source.id(), background.effects.clone()))
            .collect();
        cache.ready.retain(|key, output| {
            if active.contains(key) {
                true
            } else {
                images.remove(output.id());
                false
            }
        });
        cache.pending.retain(|key, _| active.contains(key));
        cache.failed.retain(|key| active.contains(key));
    }
    let completed: Vec<_> = cache
        .pending
        .iter_mut()
        .filter_map(|(key, task)| check_ready(task).map(|image| (key.clone(), image)))
        .collect();
    for (key, image) in completed {
        cache.pending.remove(&key);
        if let Some(image) = image {
            let handle = images.add(image);
            cache.ready.insert(key, handle);
        } else {
            cache.failed.insert(key);
        }
    }
    for (mut background, mut node) in &mut backgrounds {
        if changed.contains(&background.source.id()) {
            background.output = None;
            node.image = background.source.clone();
        }
        if let Some(output) = &background.output {
            if node.image != *output {
                node.image = output.clone();
            }
            continue;
        }
        let key = (background.source.id(), background.effects.clone());
        let output = if let Some(output) = cache.ready.get(&key) {
            output.clone()
        } else {
            if !cache.pending.contains_key(&key)
                && !cache.failed.contains(&key)
                && cache.pending.len() < 2
            {
                if let Some(source) = images.get(&background.source) {
                    let source = source.clone();
                    let effects = background.effects.clone();
                    let task = AsyncComputeTaskPool::get_or_init(TaskPool::new)
                        .spawn(async move { process_image(&source, &effects) });
                    cache.pending.insert(key, task);
                }
            }
            continue;
        };
        node.image = output.clone();
        background.output = Some(output);
    }
}

fn process_image(source: &Image, effects: &[BackgroundEffect]) -> Option<Image> {
    let converted = source.convert(TextureFormat::Rgba8UnormSrgb)?;
    let size = converted.texture_descriptor.size;
    let mut pixels = RgbaImage::from_raw(size.width, size.height, converted.data?)?;
    // A filtered background does not need a larger texture than a desktop UI panel.
    // Bounding the work also prevents a multi-megapixel oil/blur job at startup.
    let longest = pixels.width().max(pixels.height());
    let limit = if effects
        .iter()
        .any(|effect| matches!(effect, BackgroundEffect::OilPaint(_)))
    {
        1024
    } else {
        1536
    };
    if longest > limit {
        let scale = limit as f32 / longest as f32;
        pixels = imageops::resize(
            &pixels,
            (pixels.width() as f32 * scale).round().max(1.0) as u32,
            (pixels.height() as f32 * scale).round().max(1.0) as u32,
            imageops::FilterType::Triangle,
        );
    }
    for effect in effects {
        match *effect {
            BackgroundEffect::Blur(radius) if radius > 0 => {
                pixels = imageops::blur(&pixels, radius as f32);
            }
            BackgroundEffect::Grayscale(amount) => {
                for pixel in pixels.pixels_mut() {
                    let gray = (0.2126 * pixel[0] as f32
                        + 0.7152 * pixel[1] as f32
                        + 0.0722 * pixel[2] as f32)
                        .round();
                    for channel in &mut pixel.0[..3] {
                        *channel = (*channel as f32
                            + (gray - *channel as f32) * amount as f32 / 100.0)
                            .round() as u8;
                    }
                }
            }
            BackgroundEffect::Contrast(amount) => {
                let factor = amount as f32 / 100.0;
                for pixel in pixels.pixels_mut() {
                    for channel in &mut pixel.0[..3] {
                        *channel = ((*channel as f32 - 128.0) * factor + 128.0)
                            .clamp(0.0, 255.0)
                            .round() as u8;
                    }
                }
            }
            BackgroundEffect::Invert(amount) => {
                let factor = amount as f32 / 100.0;
                for pixel in pixels.pixels_mut() {
                    for channel in &mut pixel.0[..3] {
                        *channel = (*channel as f32 * (1.0 - factor)
                            + (255 - *channel) as f32 * factor)
                            .round() as u8;
                    }
                }
            }
            BackgroundEffect::OilPaint(radius) => pixels = oil_paint(&pixels, radius),
            _ => {}
        }
    }
    Some(Image::from_dynamic(
        DynamicImage::ImageRgba8(pixels),
        true,
        RenderAssetUsages::default(),
    ))
}

fn oil_paint(source: &RgbaImage, radius: u8) -> RgbaImage {
    let mut output = source.clone();
    if source.width() == 0 || source.height() == 0 {
        return output;
    }
    let radius = radius as i32;
    let width = source.width() as i32;
    let height = source.height() as i32;
    for y in 0..source.height() {
        let y = y as i32;
        let mut counts = [0u16; 8];
        let mut sums = [[0u32; 3]; 8];
        for dy in -radius..=radius {
            let sy = (y + dy).clamp(0, height - 1) as u32;
            for dx in -radius..=radius {
                let sx = dx.clamp(0, width - 1) as u32;
                add_oil_sample(source.get_pixel(sx, sy), &mut counts, &mut sums);
            }
        }
        for x in 0..source.width() {
            let bucket = counts
                .iter()
                .enumerate()
                .max_by_key(|(_, count)| *count)
                .map(|(index, _)| index)
                .unwrap_or(0);
            let pixel = output.get_pixel_mut(x, y as u32);
            for channel in 0..3 {
                let average = sums[bucket][channel] / counts[bucket].max(1) as u32;
                pixel[channel] = ((average + 8) / 16 * 16).min(255) as u8;
            }
            if x + 1 < source.width() {
                let outgoing = (x as i32 - radius).clamp(0, width - 1) as u32;
                let incoming = (x as i32 + radius + 1).clamp(0, width - 1) as u32;
                for dy in -radius..=radius {
                    let sy = (y + dy).clamp(0, height - 1) as u32;
                    remove_oil_sample(source.get_pixel(outgoing, sy), &mut counts, &mut sums);
                    add_oil_sample(source.get_pixel(incoming, sy), &mut counts, &mut sums);
                }
            }
        }
    }
    output
}

fn oil_bucket(color: &image::Rgba<u8>) -> usize {
    ((color[0] as usize * 2 + color[1] as usize * 5 + color[2] as usize) / 8) / 32
}

fn add_oil_sample(color: &image::Rgba<u8>, counts: &mut [u16; 8], sums: &mut [[u32; 3]; 8]) {
    let bucket = oil_bucket(color);
    counts[bucket] += 1;
    for channel in 0..3 {
        sums[bucket][channel] += color[channel] as u32;
    }
}

fn remove_oil_sample(color: &image::Rgba<u8>, counts: &mut [u16; 8], sums: &mut [[u32; 3]; 8]) {
    let bucket = oil_bucket(color);
    counts[bucket] -= 1;
    for channel in 0..3 {
        sums[bucket][channel] -= color[channel] as u32;
    }
}

fn update_background_rects(
    images: Option<Res<Assets<Image>>>,
    windows: Query<&Window, bevy::ecs::query::With<PrimaryWindow>>,
    mut backgrounds: Query<
        (
            &BackgroundLayout,
            &ComputedNode,
            Option<&UiGlobalTransform>,
            &mut ImageNode,
        ),
        Or<(
            Changed<BackgroundLayout>,
            Changed<ComputedNode>,
            Changed<UiGlobalTransform>,
            Changed<ImageNode>,
        )>,
    >,
) {
    let Some(images) = images else {
        return;
    };
    let viewport = windows
        .iter()
        .next()
        .map(|window| Vec2::new(window.resolution.width(), window.resolution.height()));
    for (layout, computed, transform, mut node) in &mut backgrounds {
        let Some(image) = images.get(&node.image) else {
            continue;
        };
        let source = Vec2::new(
            image.texture_descriptor.size.width as f32,
            image.texture_descriptor.size.height as f32,
        );
        let box_size = computed.size();
        if source.min_element() <= 0.0 || box_size.min_element() <= 0.0 {
            continue;
        }
        let (mode, rect) = match layout.size {
            BackgroundSize::Stretch => (NodeImageMode::Stretch, None),
            BackgroundSize::Contain => (NodeImageMode::Auto, None),
            BackgroundSize::Cover => {
                let center = transform.map_or(Vec2::ZERO, |transform| transform.translation);
                (
                    NodeImageMode::Stretch,
                    Some(cover_rect(source, box_size, *layout, viewport, center)),
                )
            }
        };
        if node.image_mode != mode || node.rect != rect {
            node.image_mode = mode;
            node.rect = rect;
        }
    }
}

fn cover_rect(
    source: Vec2,
    box_size: Vec2,
    layout: BackgroundLayout,
    viewport: Option<Vec2>,
    center: Vec2,
) -> Rect {
    let fixed = layout.attachment == BackgroundAttachment::Fixed;
    let area = if fixed {
        viewport.unwrap_or(box_size)
    } else {
        box_size
    };
    let scale = (area / source).max_element();
    let visible = area / scale;
    let origin = (source - visible) * Vec2::new(layout.position.x, layout.position.y);
    let min = if fixed {
        origin + (center - box_size / 2.0) / scale
    } else {
        origin
    };
    Rect {
        min,
        max: min + box_size / scale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::app::App;
    use bevy::image::Image;

    #[test]
    fn sliding_oil_window_matches_the_original_neighborhood() {
        let source = RgbaImage::from_fn(7, 5, |x, y| {
            image::Rgba([
                (x * 37 + y * 11) as u8,
                (x * 13 + y * 47) as u8,
                (x * 29 + y * 19) as u8,
                255,
            ])
        });
        for radius in [0u8, 1, 3, 6] {
            let expected = RgbaImage::from_fn(source.width(), source.height(), |x, y| {
                let mut counts = [0u16; 8];
                let mut sums = [[0u32; 3]; 8];
                for dy in -(radius as i32)..=radius as i32 {
                    for dx in -(radius as i32)..=radius as i32 {
                        let sx = (x as i32 + dx).clamp(0, source.width() as i32 - 1) as u32;
                        let sy = (y as i32 + dy).clamp(0, source.height() as i32 - 1) as u32;
                        add_oil_sample(source.get_pixel(sx, sy), &mut counts, &mut sums);
                    }
                }
                let bucket = counts
                    .iter()
                    .enumerate()
                    .max_by_key(|(_, count)| *count)
                    .map(|(index, _)| index)
                    .unwrap();
                let mut pixel = *source.get_pixel(x, y);
                for channel in 0..3 {
                    let average = sums[bucket][channel] / counts[bucket] as u32;
                    pixel[channel] = ((average + 8) / 16 * 16).min(255) as u8;
                }
                pixel
            });
            assert_eq!(oil_paint(&source, radius), expected);
        }
    }

    #[test]
    fn effects_are_applied_once_to_a_derived_image() {
        let source = Image::new_fill(
            wgpu_types::Extent3d {
                width: 2,
                height: 1,
                depth_or_array_layers: 1,
            },
            wgpu_types::TextureDimension::D2,
            &[10, 100, 200, 255],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        let result = process_image(&source, &[BackgroundEffect::Invert(100)]).unwrap();
        assert_eq!(&result.data.unwrap()[..4], &[245, 155, 55, 255]);
    }

    #[test]
    fn cover_crop_respects_position_and_fixed_viewport() {
        let layout = BackgroundLayout {
            size: BackgroundSize::Cover,
            position: BackgroundPosition { x: 1.0, y: 0.5 },
            attachment: BackgroundAttachment::Scroll,
        };
        let rect = cover_rect(
            Vec2::new(640.0, 360.0),
            Vec2::splat(320.0),
            layout,
            None,
            Vec2::ZERO,
        );
        assert_eq!(rect.min, Vec2::new(280.0, 0.0));
        assert_eq!(rect.max, Vec2::new(640.0, 360.0));

        let fixed = BackgroundLayout {
            position: BackgroundPosition::default(),
            attachment: BackgroundAttachment::Fixed,
            ..layout
        };
        let rect = cover_rect(
            Vec2::new(640.0, 360.0),
            Vec2::new(200.0, 100.0),
            fixed,
            Some(Vec2::new(800.0, 600.0)),
            Vec2::new(400.0, 300.0),
        );
        assert_eq!(rect.min, Vec2::new(260.0, 150.0));
        assert_eq!(rect.max, Vec2::new(380.0, 210.0));
    }

    #[test]
    fn contained_image_can_align_to_the_right_edge() {
        let geometry = contain_geometry(
            Vec2::new(200.0, 100.0),
            Vec2::new(300.0, 100.0),
            BackgroundPosition { x: 1.0, y: 1.0 },
        );
        assert!((geometry.x - 1.0 / 3.0).abs() < 0.0001);
        assert_eq!(geometry.y, 0.0);
        assert!((geometry.z - 2.0 / 3.0).abs() < 0.0001);
        assert_eq!(geometry.w, 1.0);
    }

    #[test]
    fn unchanged_background_reuses_the_same_image_asset() {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>()
            .add_plugins(BackgroundRuntimePlugin);
        let source = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::new_fill(
                wgpu_types::Extent3d {
                    width: 2,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                wgpu_types::TextureDimension::D2,
                &[10, 100, 200, 255],
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::default(),
            ));
        let entity = app
            .world_mut()
            .spawn((
                FilteredBackground {
                    source: source.clone(),
                    effects: vec![BackgroundEffect::Invert(100)],
                    output: None,
                },
                ImageNode::new(source.clone()),
            ))
            .id();
        for _ in 0..100 {
            app.update();
            if app.world().get::<ImageNode>(entity).unwrap().image != source {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let first = app.world().get::<ImageNode>(entity).unwrap().image.clone();
        assert_ne!(first, source);
        assert_eq!(app.world().resource::<Assets<Image>>().len(), 2);
        app.update();
        assert_eq!(app.world().get::<ImageNode>(entity).unwrap().image, first);
        assert_eq!(app.world().resource::<Assets<Image>>().len(), 2);

        app.world_mut()
            .resource_mut::<Assets<Image>>()
            .get_mut(&source)
            .unwrap()
            .data = Some(vec![40, 50, 60, 255, 40, 50, 60, 255]);
        app.world_mut()
            .resource_mut::<bevy::ecs::message::Messages<AssetEvent<Image>>>()
            .write(AssetEvent::Modified { id: source.id() });
        app.update();
        assert_eq!(app.world().get::<ImageNode>(entity).unwrap().image, source);
        for _ in 0..100 {
            app.update();
            let image = &app.world().get::<ImageNode>(entity).unwrap().image;
            if *image != first && *image != source {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let second = app.world().get::<ImageNode>(entity).unwrap().image.clone();
        assert_ne!(second, first);
        assert_eq!(app.world().resource::<Assets<Image>>().len(), 2);
        assert_eq!(
            &app.world()
                .resource::<Assets<Image>>()
                .get(&second)
                .unwrap()
                .data
                .as_ref()
                .unwrap()[..4],
            &[215, 205, 195, 255]
        );
        app.world_mut().despawn(entity);
        app.update();
        assert_eq!(app.world().resource::<Assets<Image>>().len(), 1);
    }
}
