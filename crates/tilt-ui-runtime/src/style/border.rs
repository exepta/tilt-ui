//! Procedural border patterns. Materials are only changed when style or layout changes.

use bevy::{
    app::{App, Plugin, PostUpdate},
    asset::{Asset, Assets, Handle},
    ecs::{
        component::Component,
        hierarchy::{ChildOf, Children},
        system::{Query, ResMut},
        world::World,
    },
    math::Vec4,
    prelude::IntoScheduleConfigs,
    reflect::TypePath,
    render::{RenderApp, render_resource::AsBindGroup},
    shader::{Shader, ShaderRef},
    ui::{ComputedNode, Node, PositionType, UiSystems, Val},
    ui_render::{
        UiMaterialPlugin,
        ui_material::{MaterialNode, UiMaterial},
    },
};
use bevy_picking::Pickable;
use tilt_ui_css::{BorderStyle, ComputedStyle, CssColor, Edges};

const BORDER_SHADER: Handle<Shader> =
    bevy::asset::uuid_handle!("97171a4d-324d-484f-b97c-401a3bba12b7");

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(crate) struct BorderMaterial {
    #[uniform(0)]
    pub geometry: Vec4, // width, height, brush strength, unused
    #[uniform(1)]
    pub widths: Vec4, // top, right, bottom, left
    #[uniform(2)]
    pub radii: Vec4, // top-left, top-right, bottom-right, bottom-left
    #[uniform(3)]
    pub styles: Vec4,
    #[uniform(4)]
    pub top: Vec4,
    #[uniform(5)]
    pub right: Vec4,
    #[uniform(6)]
    pub bottom: Vec4,
    #[uniform(7)]
    pub left: Vec4,
}

impl UiMaterial for BorderMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(BORDER_SHADER)
    }
}

#[derive(Component, Clone, Copy, PartialEq)]
pub(crate) struct BorderOverlay {
    styles: Edges<BorderStyle>,
    colors: Edges<CssColor>,
    strength: f32,
    opacity: f32,
}

#[derive(Component)]
struct BorderQuad;

pub(crate) struct BorderRuntimePlugin;

impl Plugin for BorderRuntimePlugin {
    fn build(&self, app: &mut App) {
        if app.get_sub_app(RenderApp).is_none() {
            return;
        }
        app.world_mut()
            .resource_mut::<Assets<Shader>>()
            .insert(
                BORDER_SHADER.id(),
                Shader::from_wgsl(include_str!("border.wgsl"), "tilt_ui_border.wgsl"),
            )
            .expect("unique embedded border shader");
        app.add_plugins(UiMaterialPlugin::<BorderMaterial>::default());
        app.add_systems(PostUpdate, update_border_materials.after(UiSystems::Layout));
    }
}

pub(crate) fn apply_border(
    world: &mut World,
    entity: bevy::ecs::entity::Entity,
    style: &ComputedStyle,
    opacity: f32,
) -> bool {
    let styles = style.border_style.unwrap_or(Edges::all(BorderStyle::Solid));
    let procedural = [styles.top, styles.right, styles.bottom, styles.left]
        .iter()
        .any(|style| !matches!(style, BorderStyle::None | BorderStyle::Solid));
    if !procedural || !world.contains_resource::<Assets<BorderMaterial>>() {
        if world.get::<BorderOverlay>(entity).is_some() {
            let children: Vec<_> = world
                .get::<Children>(entity)
                .into_iter()
                .flatten()
                .copied()
                .filter(|child| world.get::<BorderQuad>(*child).is_some())
                .collect();
            for child in children {
                world.despawn(child);
            }
            world.entity_mut(entity).remove::<BorderOverlay>();
        }
        return false;
    }
    let colors = style
        .border_color_edges
        .or_else(|| style.border_color.map(Edges::all))
        .unwrap_or(Edges::all(CssColor::rgba(0.0, 0.0, 0.0, 1.0)));
    let overlay = BorderOverlay {
        styles,
        colors,
        strength: style.border_brush_strength.unwrap_or(0.65),
        opacity,
    };
    if world.get::<BorderOverlay>(entity) == Some(&overlay) {
        return true;
    }
    world.entity_mut(entity).insert(overlay);
    if !world
        .get::<Children>(entity)
        .into_iter()
        .flatten()
        .any(|child| world.get::<BorderQuad>(*child).is_some())
    {
        let handle = world
            .resource_mut::<Assets<BorderMaterial>>()
            .add(BorderMaterial::empty());
        let child = world
            .spawn((
                BorderQuad,
                Node {
                    position_type: PositionType::Absolute,
                    ..Default::default()
                },
                MaterialNode(handle),
                Pickable::IGNORE,
            ))
            .id();
        world.entity_mut(entity).add_child(child);
    }
    true
}

impl BorderMaterial {
    fn empty() -> Self {
        Self {
            geometry: Vec4::ZERO,
            widths: Vec4::ZERO,
            radii: Vec4::ZERO,
            styles: Vec4::ZERO,
            top: Vec4::ZERO,
            right: Vec4::ZERO,
            bottom: Vec4::ZERO,
            left: Vec4::ZERO,
        }
    }
}

fn style_number(value: BorderStyle) -> f32 {
    match value {
        BorderStyle::None => 0.0,
        BorderStyle::Solid => 1.0,
        BorderStyle::Dotted => 2.0,
        BorderStyle::Dashed => 3.0,
        BorderStyle::DashDot => 4.0,
        BorderStyle::Skeleton => 5.0,
        BorderStyle::Brushed => 6.0,
    }
}

fn color(value: CssColor, opacity: f32) -> Vec4 {
    let linear =
        bevy::color::Color::srgba(value.red, value.green, value.blue, value.alpha).to_linear();
    Vec4::new(
        linear.red,
        linear.green,
        linear.blue,
        linear.alpha * opacity,
    )
}

fn update_border_materials(
    parents: Query<(&ComputedNode, &BorderOverlay)>,
    mut quads: Query<
        (&ChildOf, &MaterialNode<BorderMaterial>, &mut Node),
        bevy::ecs::query::With<BorderQuad>,
    >,
    mut materials: ResMut<Assets<BorderMaterial>>,
) {
    for (parent, handle, mut node) in &mut quads {
        let Ok((computed, overlay)) = parents.get(parent.parent()) else {
            continue;
        };
        if computed.is_empty() {
            continue;
        }
        let size = computed.size();
        let widths = computed.border();
        let radii = computed.border_radius();
        let desired = BorderMaterial {
            geometry: Vec4::new(size.x, size.y, overlay.strength, 0.0),
            widths: Vec4::new(
                widths.min_inset.y,
                widths.max_inset.x,
                widths.max_inset.y,
                widths.min_inset.x,
            ),
            radii: Vec4::new(
                radii.top_left,
                radii.top_right,
                radii.bottom_right,
                radii.bottom_left,
            ),
            styles: Vec4::new(
                style_number(overlay.styles.top),
                style_number(overlay.styles.right),
                style_number(overlay.styles.bottom),
                style_number(overlay.styles.left),
            ),
            top: color(overlay.colors.top, overlay.opacity),
            right: color(overlay.colors.right, overlay.opacity),
            bottom: color(overlay.colors.bottom, overlay.opacity),
            left: color(overlay.colors.left, overlay.opacity),
        };
        let changed = materials.get(&handle.0).is_some_and(|material| {
            material.geometry != desired.geometry
                || material.widths != desired.widths
                || material.radii != desired.radii
                || material.styles != desired.styles
                || material.top != desired.top
                || material.right != desired.right
                || material.bottom != desired.bottom
                || material.left != desired.left
        });
        if changed {
            if let Some(mut material) = materials.get_mut(&handle.0) {
                *material = desired;
            }
        }
        let scale = computed.inverse_scale_factor;
        let width = Val::Px(size.x * scale);
        let height = Val::Px(size.y * scale);
        let left = Val::Px(-widths.min_inset.x * scale);
        let top = Val::Px(-widths.min_inset.y * scale);
        if node.width != width || node.height != height || node.left != left || node.top != top {
            node.width = width;
            node.height = height;
            node.left = left;
            node.top = top;
        }
    }
}
