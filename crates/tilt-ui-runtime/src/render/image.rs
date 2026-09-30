use bevy::{
    ecs::{component::Component, entity::Entity, world::World},
    ui::{Node, Val, widget::ImageNode},
};
use bevy_picking::Pickable;
use tilt_ui_core::TemplateAttribute;

/// Creates an image node from a static template source when one is available.
pub(crate) fn image_node(world: &mut World, attributes: &[TemplateAttribute]) -> ImageNode {
    let Some(source) = attributes.iter().find_map(|attribute| match attribute {
        TemplateAttribute::Static { name, value } if name == "src" && !value.is_empty() => {
            Some(value)
        }
        _ => None,
    }) else {
        return ImageNode::default();
    };

    crate::widgets::content::image::load_image_handle(world, source)
        .map(ImageNode::new)
        .unwrap_or_default()
}

#[derive(Component, Clone, Copy)]
pub(crate) struct IconElementSize(pub u32);

/// Defers an icon texture until its node is near a scroll viewport.
#[cfg(feature = "tilt-icons")]
#[derive(Component, Clone, Copy)]
pub(crate) struct LazyIcon {
    pub(crate) buffer: f32,
}

#[cfg(feature = "tilt-icons")]
#[derive(Component, Clone, Copy)]
pub(crate) struct AnimatedIcon(pub(crate) tilt_ui_icons::Icon);

#[cfg(feature = "tilt-icons")]
pub(crate) fn update_icon_motion(
    world: &mut World,
    entity: Entity,
    icon: Option<tilt_ui_icons::Icon>,
) {
    use bevy::ui::UiTransform;

    let icon = icon.filter(|icon| icon.is_animated());
    if let Some(icon) = icon {
        world
            .entity_mut(entity)
            .insert((AnimatedIcon(icon), UiTransform::default()));
    } else if world.get::<AnimatedIcon>(entity).is_some() {
        world.entity_mut(entity).remove::<AnimatedIcon>();
        if let Some(mut transform) = world.get_mut::<UiTransform>(entity) {
            *transform = UiTransform::default();
        }
    }
}

#[cfg(feature = "tilt-icons")]
pub(crate) fn animate_icons(
    time: bevy::ecs::system::Res<bevy::time::Time>,
    mut icons: bevy::ecs::system::Query<(
        &AnimatedIcon,
        &mut bevy::ui::UiTransform,
        &bevy::ui::widget::ImageNode,
        Option<&LazyIcon>,
        Option<&bevy::prelude::InheritedVisibility>,
    )>,
) {
    // Quantize motion to 30 FPS so a page of animated previews does not dirty UI transforms every render frame.
    let seconds = (time.elapsed_secs() * 30.0).floor() / 30.0;
    for (icon, mut transform, image, lazy, visibility) in &mut icons {
        if visibility.is_some_and(|visibility| !visibility.get())
            || lazy.is_some() && image.image == bevy::ui::widget::ImageNode::default().image
        {
            continue;
        }
        let next = animated_transform(icon.0, seconds);
        if *transform != next {
            *transform = next;
        }
    }
}

#[cfg(feature = "tilt-icons")]
fn animated_transform(icon: tilt_ui_icons::Icon, seconds: f32) -> bevy::ui::UiTransform {
    use bevy::{
        math::{Rot2, Vec2},
        ui::{UiTransform, Val2},
    };
    use std::f32::consts::TAU;

    let mut transform = UiTransform::default();
    match icon {
        tilt_ui_icons::Icon::LoadingSpin => transform.rotation = Rot2::radians(seconds * TAU / 1.4),
        tilt_ui_icons::Icon::LoadingDots => {
            transform.scale = Vec2::splat(0.78 + 0.22 * (seconds * 5.0).sin().abs())
        }
        tilt_ui_icons::Icon::HeartBeat => {
            transform.scale = Vec2::splat(1.0 + 0.15 * (seconds * 6.0).sin().max(0.0))
        }
        tilt_ui_icons::Icon::BellRing => {
            transform.rotation = Rot2::radians(0.21 * (seconds * 8.0).sin())
        }
        tilt_ui_icons::Icon::SparklePulse => {
            transform.rotation = Rot2::radians(0.18 * (seconds * 3.0).sin());
            transform.scale = Vec2::splat(0.87 + 0.13 * (seconds * 4.0).sin().abs());
        }
        tilt_ui_icons::Icon::HourglassFlip => {
            transform.rotation = Rot2::radians((seconds * 0.55).floor() * std::f32::consts::PI)
        }
        tilt_ui_icons::Icon::WifiPulse => {
            transform.scale = Vec2::splat(0.82 + 0.18 * (seconds * 4.0).sin().abs())
        }
        tilt_ui_icons::Icon::OrbitSpin => transform.rotation = Rot2::radians(seconds * TAU / 3.2),
        tilt_ui_icons::Icon::FlameDance => {
            transform.scale = Vec2::new(
                0.96 + 0.04 * (seconds * 7.0).sin().abs(),
                0.9 + 0.1 * (seconds * 6.0).sin().abs(),
            );
            transform.rotation = Rot2::radians(0.07 * (seconds * 5.0).sin());
        }
        tilt_ui_icons::Icon::ArrowSlide => {
            transform.translation = Val2::px(3.5 * (seconds * 4.0).sin(), 0.0)
        }
        tilt_ui_icons::Icon::BevyFlight => {
            transform.translation = Val2::px(
                3.5 * (seconds * 1.7).sin(),
                -3.0 * (seconds * 2.2).sin().abs(),
            );
            transform.rotation = Rot2::radians(0.15 * (seconds * 2.0).sin());
            transform.scale = Vec2::splat(0.96 + 0.04 * (seconds * 3.0).sin().abs());
        }
        tilt_ui_icons::Icon::DiscordPulse => {
            transform.scale = Vec2::splat(0.92 + 0.12 * (seconds * 3.6).sin().abs());
            transform.rotation = Rot2::radians(0.05 * (seconds * 2.1).sin());
        }
        tilt_ui_icons::Icon::CatBounce => {
            transform.translation = Val2::px(0.0, -3.0 * (seconds * 4.0).sin().abs())
        }
        tilt_ui_icons::Icon::DogWag => {
            transform.rotation = Rot2::radians(0.16 * (seconds * 5.0).sin())
        }
        tilt_ui_icons::Icon::BirdFlap => {
            transform.scale = Vec2::new(1.0, 0.83 + 0.17 * (seconds * 8.0).sin().abs())
        }
        tilt_ui_icons::Icon::RabbitHop => {
            transform.translation = Val2::px(
                1.3 * (seconds * 2.9).sin(),
                -4.0 * (seconds * 2.9).sin().abs(),
            )
        }
        tilt_ui_icons::Icon::PawStep => {
            transform.translation = Val2::px(
                2.4 * (seconds * 2.8).sin(),
                -1.8 * (seconds * 2.8).sin().abs(),
            )
        }
        tilt_ui_icons::Icon::GamepadShake => {
            transform.translation = Val2::px(1.4 * (seconds * 12.0).sin(), 0.0);
            transform.rotation = Rot2::radians(0.08 * (seconds * 8.0).sin());
        }
        tilt_ui_icons::Icon::DiceRoll => transform.rotation = Rot2::radians(seconds * TAU / 2.2),
        tilt_ui_icons::Icon::SwordSwing => {
            transform.rotation = Rot2::radians(0.35 * (seconds * 3.4).sin())
        }
        tilt_ui_icons::Icon::BookPulse => {
            transform.scale = Vec2::new(0.9 + 0.1 * (seconds * 3.0).sin().abs(), 1.0)
        }
        tilt_ui_icons::Icon::MusicBounce => {
            transform.translation = Val2::px(0.0, 2.8 * (seconds * 4.6).sin())
        }
        tilt_ui_icons::Icon::CameraFlash => {
            transform.scale = Vec2::splat(1.0 + 0.16 * (seconds * 2.7).sin().max(0.0).powi(8))
        }
        tilt_ui_icons::Icon::CloudDrift => {
            transform.translation = Val2::px(4.0 * (seconds * 1.4).sin(), 0.0)
        }
        tilt_ui_icons::Icon::SunSpin => transform.rotation = Rot2::radians(seconds * TAU / 8.0),
        tilt_ui_icons::Icon::MoonRock => {
            transform.rotation = Rot2::radians(0.22 * (seconds * 2.2).sin())
        }
        tilt_ui_icons::Icon::RocketLaunch => {
            transform.translation = Val2::px(0.0, -4.5 * (seconds * 2.0).sin().abs());
            transform.rotation = Rot2::radians(0.1 * (seconds * 2.0).sin());
        }
        tilt_ui_icons::Icon::MessagePop => {
            transform.scale = Vec2::splat(0.86 + 0.14 * (seconds * 3.3).sin().abs())
        }
        tilt_ui_icons::Icon::CheckBounce => {
            transform.translation = Val2::px(0.0, -2.5 * (seconds * 4.2).sin().abs())
        }
        tilt_ui_icons::Icon::DownloadDrop => {
            transform.translation = Val2::px(0.0, 3.4 * (seconds * 3.0).sin())
        }
        _ => {}
    }
    transform
}

/// Switches a semantic `<icon>` to another catalog resolution in place.
#[cfg(feature = "tilt-icons")]
pub fn set_icon_size(world: &mut World, entity: Entity, size: tilt_ui_icons::IconSize) -> bool {
    if world.get::<IconElementSize>(entity).is_none() {
        return false;
    }
    let Some((icon, current_size)) = world
        .get::<crate::widgets::content::image::ImageMetadata>(entity)
        .and_then(|metadata| metadata.source.as_deref())
        .and_then(tilt_ui_icons::parse_source)
    else {
        return false;
    };
    if current_size == size {
        return false;
    }
    if !crate::widgets::content::image::set_image_source(world, entity, Some(icon.source(size))) {
        return false;
    }
    world
        .entity_mut(entity)
        .insert(IconElementSize(size.pixels()));
    let (css_width, css_height) = world
        .get::<crate::RuntimeComputedStyle>(entity)
        .map(|style| (style.0.width.is_some(), style.0.height.is_some()))
        .unwrap_or((false, false));
    if let Some(mut node) = world.get_mut::<Node>(entity) {
        if !css_width {
            node.width = Val::Px(size.pixels() as f32);
        }
        if !css_height {
            node.height = Val::Px(size.pixels() as f32);
        }
    }
    true
}

/// Materializes a catalog icon as a tintable Bevy image. Without the optional
/// feature the semantic element remains empty and brings in no icon assets.
pub(crate) fn materialize_icon(
    world: &mut World,
    entity: Entity,
    attributes: &[TemplateAttribute],
) {
    let attribute = |key: &str| {
        attributes.iter().find_map(|attribute| match attribute {
            TemplateAttribute::Static { name, value } if name == key => Some(value.as_str()),
            _ => None,
        })
    };
    let pixels = attribute("size")
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|size| matches!(size, 16 | 32 | 64))
        .unwrap_or(32);
    #[cfg(feature = "tilt-icons")]
    let source = attribute("name")
        .and_then(tilt_ui_icons::Icon::from_name)
        .and_then(|icon| {
            tilt_ui_icons::IconSize::from_pixels(pixels).map(|size| icon.source(size))
        });
    #[cfg(not(feature = "tilt-icons"))]
    let source: Option<String> = None;
    #[cfg(feature = "tilt-icons")]
    let motion_icon = source
        .as_deref()
        .and_then(tilt_ui_icons::parse_source)
        .map(|(icon, _)| icon);
    #[cfg(feature = "tilt-icons")]
    let lazy = attribute("loading").is_some_and(|value| value.eq_ignore_ascii_case("lazy"));
    #[cfg(not(feature = "tilt-icons"))]
    let lazy = false;
    let image = if lazy {
        ImageNode::default()
    } else {
        source
            .as_deref()
            .and_then(|source| crate::widgets::content::image::load_image_handle(world, source))
            .map(ImageNode::new)
            .unwrap_or_default()
    };
    world.entity_mut(entity).insert((
        image,
        Node {
            width: Val::Px(pixels as f32),
            height: Val::Px(pixels as f32),
            ..Default::default()
        },
        IconElementSize(pixels),
        Pickable::IGNORE,
        crate::widgets::content::image::ImageMetadata {
            source,
            alt: attribute("alt").map(str::to_owned),
            preview: None,
        },
    ));
    #[cfg(feature = "tilt-icons")]
    if lazy {
        let buffer = attribute("lazy-buffer")
            .and_then(|value| value.parse::<f32>().ok())
            .filter(|value| value.is_finite())
            .unwrap_or(600.0)
            .clamp(0.0, 4096.0);
        world.entity_mut(entity).insert(LazyIcon { buffer });
    }
    #[cfg(feature = "tilt-icons")]
    update_icon_motion(world, entity, motion_icon);
}

/// Loads nearby lazy icons and drops handles once they leave the buffered area.
#[cfg(feature = "tilt-icons")]
pub(crate) fn refresh_lazy_icons(world: &mut World) {
    use bevy::ui::widget::ImageNode;

    let empty_image = ImageNode::default().image;
    let changes = {
        let mut query = world.query::<(Entity, &LazyIcon, &crate::ImageMetadata, &ImageNode)>();
        query
            .iter(world)
            .filter_map(|(entity, lazy, metadata, image)| {
                if metadata.source.is_none() {
                    return None;
                }
                let near = icon_near_scroll_viewport(world, entity, lazy.buffer);
                let loaded = image.image != empty_image;
                (near != loaded).then(|| (entity, near, metadata.source.clone()))
            })
            .collect::<Vec<_>>()
    };
    for (entity, near, source) in changes {
        let handle = if near {
            source
                .as_deref()
                .and_then(|source| crate::widgets::content::image::load_image_handle(world, source))
        } else {
            None
        };
        if let Some(mut image) = world.get_mut::<ImageNode>(entity) {
            image.image = handle.unwrap_or_else(|| empty_image.clone());
        }
    }
}

#[cfg(feature = "tilt-icons")]
fn icon_near_scroll_viewport(world: &World, entity: Entity, buffer: f32) -> bool {
    use bevy::{
        ecs::hierarchy::ChildOf,
        prelude::{InheritedVisibility, Visibility},
        ui::{ComputedNode, OverflowAxis, UiGlobalTransform},
    };

    let hidden = |entity| {
        world
            .get::<Node>(entity)
            .is_some_and(|node| node.display == bevy::ui::Display::None)
            || world.get::<Visibility>(entity) == Some(&Visibility::Hidden)
            || world
                .get::<InheritedVisibility>(entity)
                .is_some_and(|visibility| !visibility.get())
    };
    if hidden(entity) {
        return false;
    }
    let Some(icon_node) = world.get::<ComputedNode>(entity) else {
        return false;
    };
    let Some(icon_transform) = world.get::<UiGlobalTransform>(entity) else {
        return false;
    };
    let icon_y = icon_transform.affine().translation.y;
    let icon_half_height = icon_node.size().y * 0.5;
    let mut current = entity;
    while let Some(parent) = world.get::<ChildOf>(current).map(ChildOf::parent) {
        current = parent;
        if hidden(current) {
            return false;
        }
        let scrolls_y = world
            .get::<Node>(current)
            .is_some_and(|node| matches!(node.overflow.y, OverflowAxis::Scroll));
        if !scrolls_y {
            continue;
        }
        let (Some(view), Some(transform)) = (
            world.get::<ComputedNode>(current),
            world.get::<UiGlobalTransform>(current),
        ) else {
            return false;
        };
        let view_y = transform.affine().translation.y;
        let buffer_physical = if view.inverse_scale_factor > 0.0 {
            buffer / view.inverse_scale_factor
        } else {
            buffer
        };
        let limit = view.size().y * 0.5 + icon_half_height + buffer_physical;
        return (icon_y - view_y).abs() <= limit;
    }
    true
}

#[cfg(all(test, feature = "tilt-icons"))]
mod lazy_tests {
    use super::*;
    use bevy::{
        asset::{Assets, Handle},
        ecs::hierarchy::ChildOf,
        image::Image,
        math::Vec2,
        prelude::InheritedVisibility,
        ui::{ComputedNode, Overflow, UiGlobalTransform},
    };
    use std::sync::Arc;
    use tilt_ui_core::TemplateAttribute;

    #[test]
    fn lazy_icon_loads_with_buffer_and_releases_handle_offscreen() {
        let mut world = World::new();
        let scroll = world
            .spawn((
                Node {
                    overflow: Overflow::scroll_y(),
                    ..Default::default()
                },
                ComputedNode {
                    size: Vec2::new(500.0, 400.0),
                    ..Default::default()
                },
                UiGlobalTransform::from_xy(0.0, 0.0),
                InheritedVisibility::VISIBLE,
            ))
            .id();
        let icon = world.spawn_empty().id();
        materialize_icon(
            &mut world,
            icon,
            &[
                TemplateAttribute::Static {
                    name: "name".into(),
                    value: "home".into(),
                },
                TemplateAttribute::Static {
                    name: "size".into(),
                    value: "32".into(),
                },
                TemplateAttribute::Static {
                    name: "loading".into(),
                    value: "lazy".into(),
                },
                TemplateAttribute::Static {
                    name: "lazy-buffer".into(),
                    value: "100".into(),
                },
            ],
        );
        world.entity_mut(icon).insert((
            ChildOf(scroll),
            ComputedNode {
                size: Vec2::splat(32.0),
                ..Default::default()
            },
            UiGlobalTransform::from_xy(0.0, 260.0),
            InheritedVisibility::VISIBLE,
        ));
        assert!(world.get_resource::<Assets<Image>>().is_none());
        assert_eq!(
            world
                .get::<crate::ImageMetadata>(icon)
                .unwrap()
                .source
                .as_deref(),
            Some("tilt-icon:home@32")
        );
        assert!(icon_near_scroll_viewport(&world, icon, 100.0));
        refresh_lazy_icons(&mut world);
        let weak = match &world.get::<ImageNode>(icon).unwrap().image {
            Handle::Strong(handle) => Arc::downgrade(handle),
            _ => panic!("buffered icon was not loaded"),
        };

        world
            .entity_mut(icon)
            .insert(UiGlobalTransform::from_xy(0.0, 350.0));
        refresh_lazy_icons(&mut world);
        assert_eq!(
            world.get::<ImageNode>(icon).unwrap().image,
            ImageNode::default().image
        );
        assert!(
            weak.upgrade().is_none(),
            "offscreen icon must not retain a strong handle"
        );

        world
            .entity_mut(icon)
            .insert(UiGlobalTransform::from_xy(0.0, 260.0));
        refresh_lazy_icons(&mut world);
        assert_ne!(
            world.get::<ImageNode>(icon).unwrap().image,
            ImageNode::default().image
        );
        assert!(set_icon_size(
            &mut world,
            icon,
            tilt_ui_icons::IconSize::Px64
        ));
        assert_eq!(
            world.get::<ImageNode>(icon).unwrap().image,
            ImageNode::default().image
        );
        refresh_lazy_icons(&mut world);
        assert_eq!(
            world
                .get::<crate::ImageMetadata>(icon)
                .unwrap()
                .source
                .as_deref(),
            Some("tilt-icon:home@64")
        );
        assert_ne!(
            world.get::<ImageNode>(icon).unwrap().image,
            ImageNode::default().image
        );

        world.get_mut::<Node>(scroll).unwrap().display = bevy::ui::Display::None;
        refresh_lazy_icons(&mut world);
        assert_eq!(
            world.get::<ImageNode>(icon).unwrap().image,
            ImageNode::default().image
        );
    }
    #[test]
    fn new_animated_variants_change_transform_without_new_svg_frames() {
        for name in [
            "bevy-flight",
            "discord-pulse",
            "cat-bounce",
            "dog-wag",
            "bird-flap",
            "rabbit-hop",
            "paw-step",
            "gamepad-shake",
            "dice-roll",
            "sword-swing",
            "book-pulse",
            "music-bounce",
            "camera-flash",
            "cloud-drift",
            "sun-spin",
            "moon-rock",
            "rocket-launch",
            "message-pop",
            "check-bounce",
            "download-drop",
        ] {
            let icon = tilt_ui_icons::Icon::from_name(name).unwrap();
            assert!(icon.is_animated(), "{name}");
            assert_ne!(
                animated_transform(icon, 0.1),
                animated_transform(icon, 0.43),
                "{name}"
            );
        }
    }

    #[test]
    fn animated_icon_tracks_source_changes_without_replacing_its_node() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        materialize_icon(
            &mut world,
            entity,
            &[
                TemplateAttribute::Static {
                    name: "name".into(),
                    value: "loading-spin".into(),
                },
                TemplateAttribute::Static {
                    name: "size".into(),
                    value: "32".into(),
                },
                TemplateAttribute::Static {
                    name: "loading".into(),
                    value: "lazy".into(),
                },
            ],
        );
        assert_eq!(
            world.get::<AnimatedIcon>(entity).unwrap().0,
            tilt_ui_icons::Icon::LoadingSpin
        );
        assert!(crate::widgets::content::image::set_image_source(
            &mut world,
            entity,
            Some(tilt_ui_icons::Icon::Home.source(tilt_ui_icons::IconSize::Px32))
        ));
        assert!(world.get::<AnimatedIcon>(entity).is_none());
        assert!(crate::widgets::content::image::set_image_source(
            &mut world,
            entity,
            Some(tilt_ui_icons::Icon::HeartBeat.source(tilt_ui_icons::IconSize::Px32))
        ));
        assert_eq!(
            world.get::<AnimatedIcon>(entity).unwrap().0,
            tilt_ui_icons::Icon::HeartBeat
        );
        assert!(world.get::<bevy::ui::UiTransform>(entity).is_some());
    }
}
