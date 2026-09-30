//! Runtime semantics and optional SVG markers for TiltUI checkboxes.

use bevy::{
    asset::Handle,
    ecs::{component::Component, entity::Entity, query::Changed, world::World},
    image::Image,
    text::TextColor,
    ui::widget::{ImageNode, NodeImageMode, Text},
};
use tilt_ui_core::TemplateAttribute;

use crate::{ControlChecked, ControlPartKind};

use super::spawn_part;

/// Identifies a materialized checkbox control.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TiltCheckbox;

#[derive(Component, Clone)]
pub(crate) struct CheckboxMarker {
    mark: Entity,
    checked_source: Option<String>,
    unchecked_source: Option<String>,
    checked_image: Option<Handle<Image>>,
    unchecked_image: Option<Handle<Image>>,
}

#[derive(Component)]
struct CheckboxMarkerPart;

pub(crate) fn materialize_parts(
    world: &mut World,
    owner: Entity,
    attributes: &[TemplateAttribute],
) {
    let indicator = spawn_part(world, owner, ControlPartKind::Indicator);
    let mark = spawn_part(world, owner, ControlPartKind::Mark);
    world
        .entity_mut(mark)
        .insert((Text::new("✓"), CheckboxMarkerPart));
    world.entity_mut(indicator).add_child(mark);
    world.entity_mut(owner).add_child(indicator);

    let checked_source = crate::component::static_attribute_value(attributes, "checked-icon")
        .filter(|source| !source.trim().is_empty())
        .map(str::to_owned);
    let unchecked_source = crate::component::static_attribute_value(attributes, "unchecked-icon")
        .filter(|source| !source.trim().is_empty())
        .map(str::to_owned);
    if checked_source.is_some() || unchecked_source.is_some() {
        let checked_image = checked_source
            .as_deref()
            .and_then(|source| load_marker_icon(world, source));
        let unchecked_image = unchecked_source
            .as_deref()
            .and_then(|source| load_marker_icon(world, source));
        world.entity_mut(owner).insert(CheckboxMarker {
            mark,
            checked_source,
            unchecked_source,
            checked_image,
            unchecked_image,
        });
        apply_marker_state(world, owner);
    }
}

fn apply_marker_state(world: &mut World, owner: Entity) {
    let checked = world
        .get::<ControlChecked>(owner)
        .is_some_and(|value| value.0);
    let (mark, image) = {
        let Some(marker) = world.get::<CheckboxMarker>(owner) else {
            return;
        };
        (
            marker.mark,
            if checked {
                marker.checked_image.clone()
            } else {
                marker.unchecked_image.clone()
            },
        )
    };
    let fallback = if checked && image.is_none() {
        "✓"
    } else {
        ""
    };
    if let Some(mut text) = world.get_mut::<Text>(mark)
        && text.0 != fallback
    {
        text.0 = fallback.into();
    }
    if let Some(image) = image {
        let color = world
            .get::<TextColor>(mark)
            .map_or(bevy::color::Color::WHITE, |color| color.0);
        if let Some(mut node) = world.get_mut::<ImageNode>(mark) {
            if node.image != image {
                node.image = image;
            }
            if node.color != color {
                node.color = color;
            }
        } else {
            let mut node = ImageNode::new(image).with_mode(NodeImageMode::Stretch);
            node.color = color;
            world.entity_mut(mark).insert(node);
        }
    } else if world.get::<ImageNode>(mark).is_some() {
        world.entity_mut(mark).remove::<ImageNode>();
    }
}

/// Only changed selection state or marker color needs to touch the GPU image node.
pub(crate) fn sync_markers(world: &mut World) {
    let changed_checkboxes = {
        let mut query = world.query_filtered::<Entity, (
            bevy::ecs::query::With<CheckboxMarker>,
            Changed<ControlChecked>,
        )>();
        query.iter(world).collect::<Vec<_>>()
    };
    for owner in changed_checkboxes {
        apply_marker_state(world, owner);
    }
    let recolored_marks = {
        let mut query = world.query_filtered::<(Entity, &TextColor), (
            bevy::ecs::query::With<CheckboxMarkerPart>,
            Changed<TextColor>,
        )>();
        query
            .iter(world)
            .map(|(entity, color)| (entity, color.0))
            .collect::<Vec<_>>()
    };
    for (mark, color) in recolored_marks {
        if let Some(mut image) = world.get_mut::<ImageNode>(mark)
            && image.color != color
        {
            image.color = color;
        }
    }
}

pub(crate) fn refresh_marker_asset_paths(world: &mut World) {
    let sources = {
        let mut query = world.query::<(Entity, &CheckboxMarker)>();
        query
            .iter(world)
            .map(|(owner, marker)| {
                (
                    owner,
                    marker.checked_source.clone(),
                    marker.unchecked_source.clone(),
                )
            })
            .collect::<Vec<_>>()
    };
    for (owner, checked, unchecked) in sources {
        let checked_image = checked
            .as_deref()
            .and_then(|source| load_marker_icon(world, source));
        let unchecked_image = unchecked
            .as_deref()
            .and_then(|source| load_marker_icon(world, source));
        if let Some(mut marker) = world.get_mut::<CheckboxMarker>(owner) {
            marker.checked_image = checked_image;
            marker.unchecked_image = unchecked_image;
        }
        apply_marker_state(world, owner);
    }
}

#[cfg(all(feature = "svg", not(target_arch = "wasm32")))]
mod svg {
    use std::{
        collections::HashMap,
        path::{Component, Path, PathBuf},
    };

    use bevy::{
        asset::{Assets, Handle, RenderAssetUsages},
        ecs::{resource::Resource, world::World},
        image::{Image, ImageSampler},
        log::warn,
    };
    use resvg::{
        tiny_skia::{Pixmap, Transform},
        usvg::Options,
    };
    use wgpu_types::{Extent3d, TextureDimension, TextureFormat};

    use crate::{UiRuntimeConfiguration, assets::UiAssetSourceRoot};

    const ICON_TEXTURE_SIZE: u32 = 96;
    const MAX_SVG_BYTES: u64 = 1_048_576;

    #[derive(Resource, Default)]
    struct MarkerIconCache(HashMap<PathBuf, Option<Handle<Image>>>);

    pub(super) fn load_marker_icon(world: &mut World, source: &str) -> Option<Handle<Image>> {
        let Some(path) = icon_path(world, source) else {
            warn!("Unsupported checkbox icon '{source}'; expected a local .svg file");
            return None;
        };
        if let Some(cached) = world
            .get_resource::<MarkerIconCache>()
            .and_then(|cache| cache.0.get(&path))
        {
            return cached.clone();
        }
        let image = rasterize_icon(&path).map(|image| {
            world.init_resource::<Assets<Image>>();
            world.resource_mut::<Assets<Image>>().add(image)
        });
        if image.is_none() {
            warn!(
                "Could not rasterize checkbox SVG '{}'; using the default marker",
                path.display()
            );
        }
        world.init_resource::<MarkerIconCache>();
        world
            .resource_mut::<MarkerIconCache>()
            .0
            .insert(path, image.clone());
        image
    }

    fn icon_path(world: &World, source: &str) -> Option<PathBuf> {
        let source = source.trim();
        let path = if let Some(relative) = source.strip_prefix("tilt-ui://") {
            let path = Path::new(relative);
            if path
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
            {
                return None;
            }
            world.get_resource::<UiAssetSourceRoot>()?.0.join(path)
        } else {
            let path = Path::new(source);
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                if path
                    .components()
                    .any(|part| !matches!(part, Component::Normal(_)))
                {
                    return None;
                }
                let root = &world.get_resource::<UiAssetSourceRoot>()?.0;
                let assets = &world.get_resource::<UiRuntimeConfiguration>()?.assets_path;
                root.join(assets).join(path)
            }
        };
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
            .then_some(path)
    }

    fn rasterize_icon(path: &Path) -> Option<Image> {
        if std::fs::metadata(path).ok()?.len() > MAX_SVG_BYTES {
            return None;
        }
        let bytes = std::fs::read(path).ok()?;
        let tree = resvg::usvg::Tree::from_data(&bytes, &Options::default()).ok()?;
        let size = tree.size();
        let width = size.width();
        let height = size.height();
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return None;
        }
        let scale = (ICON_TEXTURE_SIZE as f32 - 8.0) / width.max(height);
        let x = (ICON_TEXTURE_SIZE as f32 - width * scale) * 0.5;
        let y = (ICON_TEXTURE_SIZE as f32 - height * scale) * 0.5;
        let transform = Transform::from_row(scale, 0.0, 0.0, scale, x, y);
        let mut pixmap = Pixmap::new(ICON_TEXTURE_SIZE, ICON_TEXTURE_SIZE)?;
        resvg::render(&tree, transform, &mut pixmap.as_mut());
        let mut pixels = pixmap.take();
        for pixel in pixels.chunks_exact_mut(4) {
            pixel[0] = 255;
            pixel[1] = 255;
            pixel[2] = 255;
        }
        let mut image = Image::new(
            Extent3d {
                width: ICON_TEXTURE_SIZE,
                height: ICON_TEXTURE_SIZE,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            pixels,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        image.sampler = ImageSampler::linear();
        Some(image)
    }
}

#[cfg(all(feature = "svg", not(target_arch = "wasm32")))]
use svg::load_marker_icon;

#[cfg(not(all(feature = "svg", not(target_arch = "wasm32"))))]
fn load_marker_icon(_world: &mut World, _source: &str) -> Option<Handle<Image>> {
    None
}

#[cfg(test)]
mod tests {
    #[cfg(all(feature = "svg", not(target_arch = "wasm32")))]
    use bevy::{color::Color, text::TextColor};
    use bevy::{
        ecs::world::World,
        ui::{
            Node,
            widget::{ImageNode, Text},
        },
    };
    use tilt_ui_core::TemplateAttribute;

    use super::{CheckboxMarker, materialize_parts, sync_markers};
    use crate::ControlChecked;

    fn attribute(name: &str, value: &str) -> TemplateAttribute {
        TemplateAttribute::Static {
            name: name.into(),
            value: value.into(),
        }
    }

    #[cfg(all(feature = "svg", not(target_arch = "wasm32")))]
    #[test]
    fn svg_marker_switches_images_and_tracks_css_color() {
        use crate::{UiRuntimeConfiguration, assets::UiAssetSourceRoot};

        let directory = std::env::temp_dir().join(format!(
            "tilt-ui-checkbox-svg-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        std::fs::create_dir_all(directory.join("media")).unwrap();
        std::fs::write(
            directory.join("media/heart-outline.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M3 6L12 21L21 6" fill="none" stroke="black" stroke-width="2"/></svg>"#,
        )
        .unwrap();
        std::fs::write(
            directory.join("media/heart-filled.svg"),
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M3 6L12 21L21 6Z" fill="black"/></svg>"#,
        )
        .unwrap();

        let mut world = World::new();
        world.insert_resource(UiAssetSourceRoot(directory.clone()));
        world.insert_resource(
            UiRuntimeConfiguration::default()
                .with_assets_path("media")
                .unwrap(),
        );
        let checkbox = world.spawn((Node::default(), ControlChecked(false))).id();
        materialize_parts(
            &mut world,
            checkbox,
            &[
                attribute("checked-icon", "heart-filled.svg"),
                attribute("unchecked-icon", "heart-outline.svg"),
            ],
        );
        let marker = world.get::<CheckboxMarker>(checkbox).unwrap().clone();
        let outlined = world.get::<ImageNode>(marker.mark).unwrap().image.clone();
        assert_eq!(world.get::<Text>(marker.mark).unwrap().0, "");
        world
            .entity_mut(marker.mark)
            .insert(TextColor(Color::srgb(0.8, 0.1, 0.4)));
        sync_markers(&mut world);
        assert_eq!(
            world.get::<ImageNode>(marker.mark).unwrap().color,
            Color::srgb(0.8, 0.1, 0.4)
        );

        world.get_mut::<ControlChecked>(checkbox).unwrap().0 = true;
        sync_markers(&mut world);
        let filled = world.get::<ImageNode>(marker.mark).unwrap().image.clone();
        assert_ne!(outlined, filled);
        assert_eq!(
            world.get::<ImageNode>(marker.mark).unwrap().color,
            Color::srgb(0.8, 0.1, 0.4)
        );

        world.get_mut::<ControlChecked>(checkbox).unwrap().0 = false;
        sync_markers(&mut world);
        assert_eq!(world.get::<ImageNode>(marker.mark).unwrap().image, outlined);

        let second = world.spawn((Node::default(), ControlChecked(true))).id();
        materialize_parts(
            &mut world,
            second,
            &[
                attribute("checked-icon", "heart-filled.svg"),
                attribute("unchecked-icon", "heart-outline.svg"),
            ],
        );
        assert_eq!(
            world
                .resource::<bevy::asset::Assets<bevy::image::Image>>()
                .len(),
            2
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn missing_or_unsupported_svg_uses_the_standard_marker() {
        let mut world = World::new();
        let checkbox = world.spawn((Node::default(), ControlChecked(true))).id();
        materialize_parts(
            &mut world,
            checkbox,
            &[attribute("checked-icon", "missing.png")],
        );
        let mark = world.get::<CheckboxMarker>(checkbox).unwrap().mark;
        assert_eq!(world.get::<Text>(mark).unwrap().0, "✓");
        assert!(world.get::<ImageNode>(mark).is_none());

        world.get_mut::<ControlChecked>(checkbox).unwrap().0 = false;
        sync_markers(&mut world);
        assert_eq!(world.get::<Text>(mark).unwrap().0, "");
    }
}
