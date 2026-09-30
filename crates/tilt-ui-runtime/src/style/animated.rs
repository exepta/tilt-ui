//! Collect visible time-dependent CSS effects for the shared GPU pass.

use bevy::{
    app::{App, Plugin, PostUpdate},
    camera::Camera,
    ecs::{component::Component, resource::Resource, schedule::IntoScheduleConfigs},
    math::{Rect, Vec2, Vec4},
    time::Time,
    ui::{CalculatedClip, ComputedNode, IsDefaultUiCamera, UiGlobalTransform, UiSystems},
    window::{PrimaryWindow, Window},
};
use tilt_ui_css::{AnimatedEffect, AnimatedEffectKind, EffectQuality};

const MAX_EFFECTS: usize = 8;

#[derive(Component, Clone, PartialEq)]
pub(crate) struct AnimatedFilters {
    pub effects: Vec<AnimatedEffect>,
    pub quality: EffectQuality,
}

#[derive(Resource, Clone, Copy, Default)]
pub(crate) struct AnimatedPassState {
    pub control: Vec4,
    pub rects: [Vec4; MAX_EFFECTS],
    pub boxes: [Vec4; MAX_EFFECTS],
    pub radii: [Vec4; MAX_EFFECTS],
    pub specs: [Vec4; MAX_EFFECTS],
}

pub(crate) struct AnimatedRuntimePlugin;

impl Plugin for AnimatedRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AnimatedPassState>()
            .add_systems(PostUpdate, sync_animated_pass.after(UiSystems::PostLayout));
    }
}

pub(crate) fn sync_animated_pass(
    cameras: bevy::ecs::system::Query<
        (bevy::ecs::entity::Entity, &Camera),
        bevy::ecs::query::With<IsDefaultUiCamera>,
    >,
    windows: bevy::ecs::system::Query<&Window, bevy::ecs::query::With<PrimaryWindow>>,
    effects: bevy::ecs::system::Query<(
        &AnimatedFilters,
        &ComputedNode,
        &UiGlobalTransform,
        Option<&CalculatedClip>,
    )>,
    time: Option<bevy::ecs::system::Res<Time>>,
    mut state: bevy::ecs::system::ResMut<AnimatedPassState>,
) {
    let Some(time) = time else {
        if state.control.x != 0.0 {
            *state = AnimatedPassState::default();
        }
        return;
    };
    let Some((_, camera)) = cameras.iter().next() else {
        if state.control.x != 0.0 {
            *state = AnimatedPassState::default();
        }
        return;
    };
    let size = camera
        .physical_viewport_size()
        .map(|size| size.as_vec2())
        .or_else(|| {
            windows.iter().next().map(|window| {
                Vec2::new(
                    window.physical_width() as f32,
                    window.physical_height() as f32,
                )
            })
        })
        .unwrap_or(Vec2::ZERO);
    if size.min_element() <= 0.0 {
        if state.control.x != 0.0 {
            *state = AnimatedPassState::default();
        }
        return;
    }
    let mut pass = AnimatedPassState::default();
    for (filter, node, transform, clip) in &effects {
        if node.is_empty() {
            continue;
        }
        let rect = effect_rect(node, transform, clip, size);
        if rect.min.x >= rect.max.x || rect.min.y >= rect.max.y {
            continue;
        }
        let quality = match filter.quality {
            EffectQuality::Low => 0.0,
            EffectQuality::Medium => 1.0,
            EffectQuality::High => 2.0,
            EffectQuality::Auto => {
                if cfg!(target_arch = "wasm32")
                    || cfg!(target_os = "ios")
                    || cfg!(target_os = "android")
                    || size.x <= 800.0
                {
                    0.0
                } else {
                    1.0
                }
            }
        };
        for effect in &filter.effects {
            if effect.strength == 0 || pass.control.x as usize >= MAX_EFFECTS {
                continue;
            }
            let index = pass.control.x as usize;
            pass.rects[index] = Vec4::new(
                rect.min.x / size.x,
                rect.min.y / size.y,
                rect.max.x / size.x,
                rect.max.y / size.y,
            );
            let origin = transform.affine().transform_point2(-node.size() * 0.5);
            pass.boxes[index] = Vec4::new(
                origin.x / size.x,
                origin.y / size.y,
                (origin.x + node.size().x) / size.x,
                (origin.y + node.size().y) / size.y,
            );
            let radii = node.border_radius();
            pass.radii[index] = Vec4::new(
                radii.top_left,
                radii.top_right,
                radii.bottom_right,
                radii.bottom_left,
            );
            pass.specs[index] = Vec4::new(
                kind_id(effect.kind),
                effect.strength as f32 / 100.0,
                effect.speed as f32 / 100.0,
                quality,
            );
            pass.control.x += 1.0;
        }
    }
    if pass.control.x > 0.0 {
        pass.control.y = time.elapsed_secs();
    }
    if pass.control.x != 0.0 || state.control.x != 0.0 {
        *state = pass;
    }
}

pub(crate) fn effect_rect(
    node: &ComputedNode,
    transform: &UiGlobalTransform,
    clip: Option<&CalculatedClip>,
    viewport: Vec2,
) -> Rect {
    let half = node.size() * 0.5;
    let affine = transform.affine();
    let corners = [
        Vec2::new(-half.x, -half.y),
        Vec2::new(half.x, -half.y),
        Vec2::new(-half.x, half.y),
        Vec2::new(half.x, half.y),
    ];
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for corner in corners {
        let point = affine.transform_point2(corner);
        min = min.min(point);
        max = max.max(point);
    }
    if let Some(clip) = clip {
        min = min.max(clip.clip.min);
        max = max.min(clip.clip.max);
    }
    Rect {
        min: min.clamp(Vec2::ZERO, viewport),
        max: max.clamp(Vec2::ZERO, viewport),
    }
}

fn kind_id(kind: AnimatedEffectKind) -> f32 {
    match kind {
        AnimatedEffectKind::Noise => 1.0,
        AnimatedEffectKind::SignalLost | AnimatedEffectKind::RetroTv => 2.0,
        AnimatedEffectKind::OldMovie | AnimatedEffectKind::OldFilm => 3.0,
        AnimatedEffectKind::SideGlow => 4.0,
        AnimatedEffectKind::Bloom => 5.0,
        AnimatedEffectKind::WaterPearls => 6.0,
        AnimatedEffectKind::WaterWave => 7.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{time::Time, ui::Node};

    #[test]
    fn gpu_pass_exists_only_for_visible_active_effects() {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default());
        app.add_plugins(AnimatedRuntimePlugin);
        app.world_mut()
            .spawn((bevy::camera::Camera2d, IsDefaultUiCamera));
        app.world_mut().spawn((Window::default(), PrimaryWindow));
        let element = app
            .world_mut()
            .spawn((
                Node::default(),
                ComputedNode {
                    size: Vec2::new(100.0, 80.0),
                    ..Default::default()
                },
                UiGlobalTransform::from_xy(200.0, 150.0),
                AnimatedFilters {
                    effects: vec![AnimatedEffect {
                        kind: AnimatedEffectKind::Bloom,
                        strength: 80,
                        speed: 100,
                    }],
                    quality: EffectQuality::Low,
                },
            ))
            .id();
        app.update();
        let settings = app.world().resource::<AnimatedPassState>();
        assert_eq!(settings.control.x, 1.0);
        assert_eq!(settings.specs[0], Vec4::new(5.0, 0.8, 1.0, 0.0));

        app.world_mut()
            .entity_mut(element)
            .insert(UiGlobalTransform::from_xy(2000.0, 2000.0));
        app.update();
        assert_eq!(app.world().resource::<AnimatedPassState>().control.x, 0.0);
    }
}
