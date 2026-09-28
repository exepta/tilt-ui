//! Runtime semantics specific to the TiltUI image element.

use std::{collections::HashMap, path::Path};

#[cfg(all(feature = "svg", not(target_arch = "wasm32")))]
use bevy::log::warn;
use bevy::{
    asset::{AssetServer, Assets, Handle, RenderAssetUsages},
    ecs::{
        component::Component,
        entity::Entity,
        hierarchy::ChildOf,
        message::MessageReader,
        resource::Resource,
        system::{Commands, Query},
        world::World,
    },
    image::{CompressedImageFormats, Image, ImageSampler, ImageType},
    ui::widget::ImageNode,
};
use tilt_ui_core::TemplateAttribute;
#[cfg(all(feature = "svg", not(target_arch = "wasm32")))]
use wgpu_types::{Extent3d, TextureDimension, TextureFormat};

use crate::{
    ComponentElementIds, ComponentStyleOwner,
    widgets::controls::input::{FileInputOptions, FileInputSelected},
};

#[cfg(all(feature = "svg", not(target_arch = "wasm32")))]
use resvg::{
    tiny_skia::{Pixmap, Transform},
    usvg::Options,
};

/// Stores semantic image metadata retained independently of Bevy image loading.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct ImageMetadata {
    /// Source path declared by the template, when present.
    pub source: Option<String>,
    /// Alternative text declared by the template, when present.
    pub alt: Option<String>,
    /// Component-local file input ID used for automatic selected-file previews.
    pub preview: Option<String>,
}

pub(crate) fn materialize_metadata(
    world: &mut World,
    entity: Entity,
    attributes: &[TemplateAttribute],
) {
    let mut metadata = ImageMetadata::default();
    for attribute in attributes {
        let TemplateAttribute::Static { name, value } = attribute else {
            continue;
        };
        match name.as_str() {
            "src" => metadata.source = Some(value.clone()),
            "alt" => metadata.alt = Some(value.clone()),
            "preview" => metadata.preview = normalize_preview_target(value),
            _ => {}
        }
    }
    world.entity_mut(entity).insert(metadata);
}

/// Stores the resolved file input entity that drives an image preview.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImagePreviewInput {
    /// File input entity referenced by `img preview="input-id"`.
    pub input: Entity,
}

#[derive(Resource, Default)]
struct RuntimeImageCache(HashMap<String, Handle<Image>>);

/// Updates an image source on its existing entity without rebuilding children.
pub fn set_image_source(world: &mut World, entity: Entity, source: Option<String>) -> bool {
    let Some(metadata) = world.get::<ImageMetadata>(entity) else {
        return false;
    };
    if metadata.source == source {
        return false;
    }
    let image = source
        .as_ref()
        .and_then(|source| load_image_handle(world, source));
    world.get_mut::<ImageMetadata>(entity).unwrap().source = source;
    if let Some(mut node) = world.get_mut::<ImageNode>(entity) {
        node.image = image.unwrap_or_default();
    }
    true
}

pub(crate) fn resolve_preview_targets(world: &mut World, scope: Entity) {
    let previews = {
        let mut query = world.query::<(Entity, &ImageMetadata, &ComponentStyleOwner)>();
        query
            .iter(world)
            .filter(|(_, _, owner)| owner.0 == scope)
            .filter_map(|(entity, metadata, _)| {
                metadata
                    .preview
                    .as_ref()
                    .map(|target| (entity, target.clone()))
            })
            .collect::<Vec<_>>()
    };
    for (image, target_id) in previews {
        let Some(input) = world
            .get::<ComponentElementIds>(scope)
            .and_then(|ids| ids.get(&target_id))
        else {
            continue;
        };
        if input == image || is_ancestor(world, image, input) {
            continue;
        }
        if world.get::<FileInputOptions>(input).is_none() {
            continue;
        }
        world.entity_mut(image).insert(ImagePreviewInput { input });
    }
}

/// Applies completed `input type=file` selections to linked `<img preview="...">` elements.
pub(crate) fn sync_file_input_previews(
    mut selected: MessageReader<FileInputSelected>,
    options: Query<&FileInputOptions>,
    previews: Query<(Entity, &ImagePreviewInput)>,
    mut commands: Commands,
) {
    for event in selected.read() {
        let Ok(input_options) = options.get(event.entity) else {
            continue;
        };
        let Some(source) = preview_source(input_options, &event.selection) else {
            continue;
        };
        let targets = previews
            .iter()
            .filter_map(|(image, preview)| (preview.input == event.entity).then_some(image))
            .collect::<Vec<_>>();
        if targets.is_empty() {
            continue;
        }
        commands.queue(move |world: &mut World| {
            for image in targets {
                set_image_source(world, image, Some(source.clone()));
            }
        });
    }
}

pub(crate) fn load_image_handle(world: &mut World, source: &str) -> Option<Handle<Image>> {
    let source = source.trim();
    if source.is_empty() {
        return None;
    }
    let resolved = world
        .get_resource::<crate::UiRuntimeConfiguration>()
        .map_or_else(|| source.to_owned(), |config| config.asset_path(source));

    if let Some(handle) = world
        .get_resource::<RuntimeImageCache>()
        .and_then(|cache| cache.0.get(&resolved).cloned())
    {
        return Some(handle);
    }

    if let Some(handle) = load_image_from_filesystem(world, source) {
        cache_image(world, &resolved, &handle);
        return Some(handle);
    }

    #[cfg(all(feature = "svg", not(target_arch = "wasm32")))]
    if path_is_svg(source) {
        if let Some(handle) = load_svg_image(world, source) {
            cache_image(world, &resolved, &handle);
            return Some(handle);
        }
        warn!("Failed to rasterize SVG image '{source}', falling back to AssetServer load.");
    }

    world
        .get_resource::<AssetServer>()
        .map(|server| server.load::<Image>(resolved))
}

fn cache_image(world: &mut World, source: &str, handle: &Handle<Image>) {
    world.init_resource::<RuntimeImageCache>();
    world
        .resource_mut::<RuntimeImageCache>()
        .0
        .insert(source.to_owned(), handle.clone());
}

fn load_image_from_filesystem(world: &mut World, source: &str) -> Option<Handle<Image>> {
    let path = Path::new(source);
    if !path.is_absolute() || !path.is_file() {
        return None;
    }
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)?;
    if !matches!(
        extension.as_str(),
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "ico"
    ) {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let image = Image::from_buffer(
        &bytes,
        ImageType::Extension(extension.as_str()),
        CompressedImageFormats::empty(),
        true,
        ImageSampler::default(),
        RenderAssetUsages::default(),
    )
    .ok()?;
    world.init_resource::<Assets<Image>>();
    Some(world.resource_mut::<Assets<Image>>().add(image))
}

#[cfg(all(feature = "svg", not(target_arch = "wasm32")))]
fn load_svg_image(world: &mut World, source: &str) -> Option<Handle<Image>> {
    let fs_path = resolve_image_path(source)?;
    let bytes = std::fs::read(fs_path).ok()?;
    let tree = resvg::usvg::Tree::from_data(&bytes, &Options::default()).ok()?;
    let size = tree.size().to_int_size();
    let mut pixmap = Pixmap::new(size.width(), size.height())?;
    resvg::render(&tree, Transform::default(), &mut pixmap.as_mut());
    let image = rgba8_srgb_linear_image(size.width(), size.height(), pixmap.take());
    world.init_resource::<Assets<Image>>();
    Some(world.resource_mut::<Assets<Image>>().add(image))
}

#[cfg(all(feature = "svg", not(target_arch = "wasm32")))]
fn path_is_svg(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
}

#[cfg(all(feature = "svg", not(target_arch = "wasm32")))]
fn resolve_image_path(source: &str) -> Option<std::path::PathBuf> {
    let raw = Path::new(source);
    if raw.is_absolute() && raw.is_file() {
        return Some(raw.to_path_buf());
    }
    if raw.is_file() {
        return Some(raw.to_path_buf());
    }
    let without_leading_slash = source.strip_prefix('/').unwrap_or(source);
    let asset_path = Path::new("assets").join(without_leading_slash);
    asset_path.is_file().then_some(asset_path)
}

#[cfg(all(feature = "svg", not(target_arch = "wasm32")))]
fn rgba8_srgb_linear_image(width: u32, height: u32, data: Vec<u8>) -> Image {
    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::linear();
    image
}

fn preview_source(
    options: &FileInputOptions,
    selection: &crate::widgets::controls::input::FileInputSelection,
) -> Option<String> {
    if options.folder {
        return None;
    }
    let path = selection.native_path.as_ref()?;
    if !is_supported_preview_source(path) {
        return None;
    }
    if !options.extensions.is_empty() && !input_allows_image_preview(options) {
        return None;
    }
    Some(path.to_string_lossy().replace('\\', "/"))
}

fn input_allows_image_preview(options: &FileInputOptions) -> bool {
    options
        .extensions
        .iter()
        .any(|extension| is_supported_preview_extension(extension))
}

fn is_supported_preview_source(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(is_supported_preview_extension)
}

fn is_supported_preview_extension(extension: &str) -> bool {
    let extension = extension
        .trim()
        .trim_start_matches('.')
        .to_ascii_lowercase();
    matches!(extension.as_str(), "jpg" | "jpeg" | "png")
        || cfg!(all(feature = "svg", not(target_arch = "wasm32"))) && extension == "svg"
}

fn normalize_preview_target(value: &str) -> Option<String> {
    let normalized = value.trim().trim_start_matches('#').trim();
    (!normalized.is_empty()).then(|| normalized.to_owned())
}

fn is_ancestor(world: &World, ancestor: Entity, mut node: Entity) -> bool {
    while let Some(parent) = world.get::<ChildOf>(node) {
        node = parent.parent();
        if node == ancestor {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use bevy::{
        app::{App, Update},
        ecs::message::Messages,
        ui::widget::ImageNode,
    };

    use super::*;

    #[test]
    fn image_metadata_retains_preview_target_without_hash() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        materialize_metadata(
            &mut world,
            entity,
            &[TemplateAttribute::Static {
                name: "preview".into(),
                value: "#avatar-file".into(),
            }],
        );

        assert_eq!(
            world
                .get::<ImageMetadata>(entity)
                .unwrap()
                .preview
                .as_deref(),
            Some("avatar-file")
        );
    }

    #[test]
    fn resolves_preview_targets_inside_component_scope() {
        let mut world = World::new();
        let scope = world.spawn(ComponentElementIds::default()).id();
        let input = world.spawn(FileInputOptions::default()).id();
        world
            .get_mut::<ComponentElementIds>(scope)
            .unwrap()
            .insert("avatar-file".into(), input);
        let image = world
            .spawn((
                ComponentStyleOwner(scope),
                ImageMetadata {
                    preview: Some("avatar-file".into()),
                    ..Default::default()
                },
            ))
            .id();
        world.entity_mut(scope).add_children(&[input, image]);

        resolve_preview_targets(&mut world, scope);

        assert_eq!(world.get::<ImagePreviewInput>(image).unwrap().input, input);
    }

    #[test]
    fn selected_image_file_updates_linked_preview_source() {
        let mut app = App::new();
        app.add_message::<FileInputSelected>()
            .add_systems(Update, sync_file_input_previews);
        let input = app.world_mut().spawn(FileInputOptions::default()).id();
        let image = app
            .world_mut()
            .spawn((
                ImageNode::default(),
                ImageMetadata::default(),
                ImagePreviewInput { input },
            ))
            .id();
        let source = std::env::temp_dir().join("tilt-ui-preview-test.png");
        std::fs::write(&source, b"not decoded by this test").unwrap();
        app.world_mut()
            .resource_mut::<Messages<FileInputSelected>>()
            .write(FileInputSelected {
                entity: input,
                selection: crate::widgets::controls::input::FileInputSelection {
                    name: "tilt-ui-preview-test.png".into(),
                    size_bytes: Some(24),
                    native_path: Some(source.clone()),
                },
            });

        app.update();

        let expected = source.to_string_lossy().replace('\\', "/");
        assert_eq!(
            world_source(app.world(), image).as_deref(),
            Some(expected.as_str())
        );
        let _ = std::fs::remove_file(source);
    }

    fn world_source(world: &World, entity: Entity) -> Option<String> {
        world.get::<ImageMetadata>(entity)?.source.clone()
    }
}
