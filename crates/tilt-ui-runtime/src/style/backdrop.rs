//! Live dialog backdrop rendering. The base camera is blurred after its UI pass;
//! open dialogs are rendered by a later camera so their contents stay sharp.

use bevy::{
    app::{App, Plugin, PostUpdate},
    asset::{Assets, Handle},
    camera::{
        Camera, Camera2d, CameraOutputMode, ClearColorConfig, Hdr, RenderTarget,
        visibility::RenderLayers,
    },
    color::Color,
    core_pipeline::{
        Core2d, Core3d,
        fullscreen_material::{
            FullscreenMaterial, FullscreenMaterialPlugin, fullscreen_material_system,
        },
        upscaling::upscaling,
    },
    ecs::{
        component::Component,
        entity::Entity,
        hierarchy::{ChildOf, Children},
        schedule::IntoScheduleConfigs,
        system::BoxedSystem,
        world::World,
    },
    math::{Vec2, Vec4},
    render::{
        RenderApp,
        extract_component::ExtractComponent,
        render_resource::{BlendState, ShaderType},
    },
    shader::{Shader, ShaderRef},
    ui::{
        CalculatedClip, ComputedNode, IsDefaultUiCamera, Node, PositionType, UiGlobalTransform,
        UiSystems, UiTargetCamera, Val,
    },
    ui_render::render_pass::ui_pass,
    window::{PrimaryWindow, Window},
};
use tilt_ui_css::BackgroundEffect;

use crate::{DialogRenderer, DialogState};

use super::animated::{AnimatedPassState, effect_rect, sync_animated_pass};

const EFFECTS_SHADER: Handle<Shader> =
    bevy::asset::uuid_handle!("8d12d321-a528-47ca-9685-5cdb0dcd9621");

/// Effects sampled from pixels behind an element.
#[derive(Component, Clone, PartialEq)]
pub(crate) struct BackdropFilter(pub Vec<BackgroundEffect>);

#[derive(Component, Clone, Copy)]
struct DetachedDialog(Entity);

#[derive(Component, Clone)]
pub(crate) struct DetachedBackdrop {
    pub(crate) parent: Entity,
    placeholder: Entity,
    original_node: Node,
    index: usize,
}

#[derive(Component, Clone, Copy)]
struct BackdropPlaceholder(Entity);

#[derive(Component, Clone, Copy)]
struct RootBackdropTarget(Option<Entity>);

pub(crate) fn logical_parent(world: &World, entity: Entity) -> Option<Entity> {
    world
        .get::<ChildOf>(entity)
        .map(ChildOf::parent)
        .or_else(|| {
            world
                .get::<DetachedBackdrop>(entity)
                .map(|marker| marker.parent)
        })
}

pub(crate) fn logical_children(world: &World, entity: Entity) -> Vec<Entity> {
    world
        .get::<Children>(entity)
        .map(|children| {
            children
                .iter()
                .map(|child| {
                    world
                        .get::<BackdropPlaceholder>(*child)
                        .map_or(*child, |placeholder| placeholder.0)
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn logical_sibling_entity(world: &World, entity: Entity) -> Entity {
    world
        .get::<DetachedBackdrop>(entity)
        .map_or(entity, |marker| marker.placeholder)
}

#[derive(bevy::ecs::resource::Resource)]
struct OverlayCamera(Entity);

#[derive(Component, Clone, Copy, Default, ExtractComponent, ShaderType)]
struct LiveUiEffects {
    control: Vec4,
    bounds: Vec4,
    rects: [Vec4; 8],
    boxes: [Vec4; 8],
    radii: [Vec4; 8],
    filters: [Vec4; 8],
    animated_rects: [Vec4; 8],
    animated_boxes: [Vec4; 8],
    animated_radii: [Vec4; 8],
    animated_specs: [Vec4; 8],
}

impl FullscreenMaterial for LiveUiEffects {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(EFFECTS_SHADER)
    }

    fn schedule() -> impl bevy::ecs::schedule::ScheduleLabel + Clone {
        Core2d
    }

    fn schedule_configs(
        system: bevy::ecs::schedule::ScheduleConfigs<BoxedSystem>,
    ) -> bevy::ecs::schedule::ScheduleConfigs<BoxedSystem> {
        system.after(ui_pass).before(upscaling)
    }
}

pub(crate) struct BackdropRuntimePlugin;

impl Plugin for BackdropRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            sync_dialog_backdrops
                .after(UiSystems::PostLayout)
                .after(sync_animated_pass),
        );
        install_render_effects(app);
    }

    fn finish(&self, app: &mut App) {
        install_render_effects(app);
    }
}

fn install_render_effects(app: &mut App) {
    // TiltUiPlugin adds the runtime from its own finish hook. Plugins added
    // there are built immediately but do not receive a second finish call.
    if app.get_sub_app(RenderApp).is_none()
        || !app.world().contains_resource::<Assets<Shader>>()
        || app.is_plugin_added::<FullscreenMaterialPlugin<LiveUiEffects>>()
    {
        return;
    }
    app.world_mut()
        .resource_mut::<Assets<Shader>>()
        .insert(
            EFFECTS_SHADER.id(),
            Shader::from_wgsl(
                include_str!("live_effects.wgsl"),
                "tilt_ui_live_effects.wgsl",
            ),
        )
        .expect("unique embedded backdrop blur shader");
    app.add_plugins(FullscreenMaterialPlugin::<LiveUiEffects>::default());
    app.get_sub_app_mut(RenderApp).unwrap().add_systems(
        Core3d,
        fullscreen_material_system::<LiveUiEffects>
            .after(ui_pass)
            .before(upscaling),
    );
}

fn sync_dialog_backdrops(world: &mut World) {
    let base_camera = {
        let mut cameras = world.query_filtered::<
            (Entity, &Camera, Option<&RenderTarget>, Option<&Hdr>),
            bevy::ecs::query::With<IsDefaultUiCamera>,
        >();
        cameras
            .iter(world)
            .next()
            .map(|(entity, camera, target, hdr)| {
                (entity, camera.order, target.cloned(), hdr.is_some())
            })
    };
    let Some((base, base_order, target, hdr)) = base_camera else {
        return;
    };
    let open = {
        let mut dialogs = world.query::<(Entity, &DialogState, Option<&BackdropFilter>)>();
        let mut open = Vec::new();
        for (entity, state, filter) in dialogs.iter(world) {
            if state.open && state.renderer == DialogRenderer::Bevy {
                if let Some(filter) = filter
                    && filter_parameters(&filter.0).is_some()
                {
                    open.push((entity, filter.0.clone()));
                }
            }
        }
        open
    };
    let viewport = world
        .get::<Camera>(base)
        .and_then(Camera::physical_viewport_size)
        .map(|size| size.as_vec2())
        .or_else(|| {
            let mut windows =
                world.query_filtered::<&Window, bevy::ecs::query::With<PrimaryWindow>>();
            windows.iter(world).next().map(|window| {
                Vec2::new(
                    window.physical_width() as f32,
                    window.physical_height() as f32,
                )
            })
        });
    let mut desired = LiveUiEffects::default();
    if let Some(animation) = world.get_resource::<AnimatedPassState>() {
        desired.control.y = animation.control.x;
        desired.control.z = animation.control.y;
        desired.animated_rects = animation.rects;
        desired.animated_boxes = animation.boxes;
        desired.animated_radii = animation.radii;
        desired.animated_specs = animation.specs;
    }
    for (_, filters) in &open {
        if desired.control.x >= 8.0 {
            break;
        }
        let index = desired.control.x as usize;
        desired.rects[index] = Vec4::new(0.0, 0.0, 1.0, 1.0);
        desired.boxes[index] = desired.rects[index];
        desired.filters[index] = filter_parameters(filters).unwrap_or(Vec4::ZERO);
        desired.control.x += 1.0;
    }
    let generic = {
        let mut query = world.query::<(
            Entity,
            &BackdropFilter,
            &Node,
            &ComputedNode,
            &UiGlobalTransform,
            Option<&CalculatedClip>,
            Option<&DetachedBackdrop>,
            Option<&DialogState>,
        )>();
        query
            .iter(world)
            .filter_map(
                |(entity, filter, node, computed, transform, clip, detached, dialog)| {
                    if dialog.is_some() || computed.is_empty() {
                        return None;
                    }
                    let params = filter_parameters(&filter.0)?;
                    let viewport = viewport?;
                    let (rect, size, scale, origin, radii) = if let Some(detached) = detached {
                        let proxy_node = world.get::<ComputedNode>(detached.placeholder)?;
                        let proxy_transform =
                            world.get::<UiGlobalTransform>(detached.placeholder)?;
                        let proxy_clip = world.get::<CalculatedClip>(detached.placeholder);
                        (
                            effect_rect(proxy_node, proxy_transform, proxy_clip, viewport),
                            proxy_node.size(),
                            proxy_node.inverse_scale_factor,
                            proxy_transform
                                .affine()
                                .transform_point2(-proxy_node.size() * 0.5),
                            proxy_node.border_radius(),
                        )
                    } else {
                        (
                            effect_rect(computed, transform, clip, viewport),
                            computed.size(),
                            computed.inverse_scale_factor,
                            transform.affine().transform_point2(-computed.size() * 0.5),
                            computed.border_radius(),
                        )
                    };
                    if detached.is_none() && (rect.min.x >= rect.max.x || rect.min.y >= rect.max.y)
                    {
                        return None;
                    }
                    Some((
                        entity,
                        params,
                        rect,
                        size,
                        scale,
                        origin,
                        radii,
                        node.clone(),
                    ))
                },
            )
            .collect::<Vec<_>>()
    };

    let has_visible_generic = generic
        .iter()
        .any(|(_, _, rect, _, _, _, _, _)| rect.min.x < rect.max.x && rect.min.y < rect.max.y);
    if desired.control.x == 0.0 && !has_visible_generic {
        restore_dialogs(world);
        restore_unused_backdrops(world);
        restore_root_backdrops(world);
        for (entity, _, _, size, scale, origin, _, _) in generic {
            if world.get::<DetachedBackdrop>(entity).is_some() {
                position_backdrop_node(world, entity, origin, size, scale);
            }
        }
        if desired.control.y > 0.0 {
            desired.bounds = effect_bounds(&desired);
            world.entity_mut(base).insert(desired);
        } else {
            world.entity_mut(base).remove::<LiveUiEffects>();
        }
        if let Some(overlay) = world.get_resource::<OverlayCamera>() {
            if let Some(mut camera) = world.get_mut::<Camera>(overlay.0) {
                camera.is_active = false;
            }
        }
        return;
    }

    let overlay = if let Some(overlay) = world.get_resource::<OverlayCamera>() {
        overlay.0
    } else {
        let mut camera = Camera {
            order: base_order.saturating_add(1),
            clear_color: ClearColorConfig::Custom(Color::srgba(0.0, 0.0, 0.0, 0.0)),
            output_mode: CameraOutputMode::Write {
                blend_state: Some(BlendState::ALPHA_BLENDING),
                clear_color: ClearColorConfig::None,
            },
            ..Default::default()
        };
        camera.is_active = true;
        let entity = world.spawn((Camera2d, camera, RenderLayers::none())).id();
        if let Some(target) = &target {
            world.entity_mut(entity).insert(target.clone());
        }
        if hdr {
            world.entity_mut(entity).insert(Hdr);
        }
        world.insert_resource(OverlayCamera(entity));
        entity
    };
    if let Some(mut camera) = world.get_mut::<Camera>(overlay) {
        camera.is_active = true;
        camera.order = base_order.saturating_add(1);
    }
    if hdr && world.get::<Hdr>(overlay).is_none() {
        world.entity_mut(overlay).insert(Hdr);
    } else if !hdr {
        world.entity_mut(overlay).remove::<Hdr>();
    }
    for (entity, _) in open {
        if world.get::<DetachedDialog>(entity).is_some() {
            continue;
        }
        let Some(parent) = world.get::<ChildOf>(entity).map(ChildOf::parent) else {
            continue;
        };
        world.entity_mut(entity).remove::<ChildOf>();
        world
            .entity_mut(entity)
            .insert((DetachedDialog(parent), UiTargetCamera(overlay)));
    }
    restore_closed_dialogs(world);
    for (entity, params, rect, size, scale, origin, radii, original_node) in generic {
        if rect.min.x >= rect.max.x || rect.min.y >= rect.max.y || desired.control.x >= 8.0 {
            if world.get::<DetachedBackdrop>(entity).is_some() {
                position_backdrop_node(world, entity, origin, size, scale);
            }
            continue;
        }
        let index = desired.control.x as usize;
        let viewport = viewport.unwrap();
        desired.rects[index] = Vec4::new(
            rect.min.x / viewport.x,
            rect.min.y / viewport.y,
            rect.max.x / viewport.x,
            rect.max.y / viewport.y,
        );
        desired.boxes[index] = Vec4::new(
            origin.x / viewport.x,
            origin.y / viewport.y,
            (origin.x + size.x) / viewport.x,
            (origin.y + size.y) / viewport.y,
        );
        desired.radii[index] = Vec4::new(
            radii.top_left,
            radii.top_right,
            radii.bottom_right,
            radii.bottom_left,
        );
        desired.filters[index] = params;
        desired.control.x += 1.0;
        if world.get::<DetachedBackdrop>(entity).is_none() {
            let Some(parent) = world.get::<ChildOf>(entity).map(ChildOf::parent) else {
                if world.get::<RootBackdropTarget>(entity).is_none() {
                    let previous = world.get::<UiTargetCamera>(entity).map(|target| target.0);
                    world
                        .entity_mut(entity)
                        .insert((RootBackdropTarget(previous), UiTargetCamera(overlay)));
                }
                continue;
            };
            let index = world
                .get::<Children>(parent)
                .and_then(|children| children.iter().position(|child| *child == entity))
                .unwrap_or(0);
            let mut proxy = original_node.clone();
            proxy.width = Val::Px(size.x * scale);
            proxy.height = Val::Px(size.y * scale);
            let placeholder = world.spawn((proxy, BackdropPlaceholder(entity))).id();
            world.entity_mut(entity).remove::<ChildOf>();
            world
                .entity_mut(parent)
                .insert_children(index, &[placeholder]);
            world.entity_mut(entity).insert((
                DetachedBackdrop {
                    parent,
                    placeholder,
                    original_node,
                    index,
                },
                UiTargetCamera(overlay),
            ));
        }
        position_backdrop_node(world, entity, origin, size, scale);
    }
    restore_unused_backdrops(world);
    restore_root_backdrops(world);
    desired.bounds = effect_bounds(&desired);
    if desired.control.x == 0.0 && desired.control.y == 0.0 {
        world.entity_mut(base).remove::<LiveUiEffects>();
    } else if world.get::<LiveUiEffects>(base).is_none_or(|current| {
        current.control != desired.control
            || current.bounds != desired.bounds
            || current.rects != desired.rects
            || current.boxes != desired.boxes
            || current.radii != desired.radii
            || current.filters != desired.filters
            || current.animated_rects != desired.animated_rects
            || current.animated_boxes != desired.animated_boxes
            || current.animated_radii != desired.animated_radii
            || current.animated_specs != desired.animated_specs
    }) {
        world.entity_mut(base).insert(desired);
    }
}

fn restore_root_backdrops(world: &mut World) {
    let mut query = world.query::<(Entity, &RootBackdropTarget, Option<&BackdropFilter>)>();
    let roots = query
        .iter(world)
        .filter(|(_, _, filter)| filter.is_none_or(|filter| filter_parameters(&filter.0).is_none()))
        .map(|(entity, marker, _)| (entity, marker.0))
        .collect::<Vec<_>>();
    for (entity, previous) in roots {
        if let Some(camera) = previous {
            world.entity_mut(entity).insert(UiTargetCamera(camera));
        } else {
            world.entity_mut(entity).remove::<UiTargetCamera>();
        }
        world.entity_mut(entity).remove::<RootBackdropTarget>();
    }
}

fn effect_bounds(effects: &LiveUiEffects) -> Vec4 {
    let mut minimum = Vec2::ONE;
    let mut maximum = Vec2::ZERO;
    for rect in effects.rects.iter().take(effects.control.x as usize).chain(
        effects
            .animated_rects
            .iter()
            .take(effects.control.y as usize),
    ) {
        minimum = minimum.min(Vec2::new(rect.x, rect.y));
        maximum = maximum.max(Vec2::new(rect.z, rect.w));
    }
    Vec4::new(minimum.x, minimum.y, maximum.x, maximum.y)
}

fn position_backdrop_node(world: &mut World, entity: Entity, origin: Vec2, size: Vec2, scale: f32) {
    let left = Val::Px(origin.x * scale);
    let top = Val::Px(origin.y * scale);
    let width = Val::Px(size.x * scale);
    let height = Val::Px(size.y * scale);
    if world.get::<Node>(entity).is_some_and(|node| {
        node.position_type != PositionType::Absolute
            || node.left != left
            || node.top != top
            || node.width != width
            || node.height != height
    }) && let Some(mut node) = world.get_mut::<Node>(entity)
    {
        node.position_type = PositionType::Absolute;
        node.left = left;
        node.top = top;
        node.width = width;
        node.height = height;
    }
}

fn filter_parameters(effects: &[BackgroundEffect]) -> Option<Vec4> {
    let mut params = Vec4::new(0.0, 0.0, 1.0, 0.0);
    for effect in effects {
        match *effect {
            BackgroundEffect::Blur(value) => params.x = params.x.max(value as f32),
            BackgroundEffect::Grayscale(value) => params.y = (value as f32 / 100.0).max(params.y),
            BackgroundEffect::Contrast(value) => params.z = value as f32 / 100.0,
            BackgroundEffect::Invert(value) => params.w = (value as f32 / 100.0).max(params.w),
            BackgroundEffect::OilPaint(_) => {}
        }
    }
    (params != Vec4::new(0.0, 0.0, 1.0, 0.0)).then_some(params)
}

fn restore_unused_backdrops(world: &mut World) {
    let mut query = world.query::<(Entity, &DetachedBackdrop, Option<&BackdropFilter>)>();
    let detached = query
        .iter(world)
        .filter(|(_, marker, filter)| {
            world.get_entity(marker.parent).is_err()
                || world.get_entity(marker.placeholder).is_err()
                || filter.is_none_or(|filter| filter_parameters(&filter.0).is_none())
        })
        .map(|(entity, detached, _)| (entity, detached.clone()))
        .collect::<Vec<_>>();
    for (entity, detached) in detached {
        if world.get_entity(detached.parent).is_ok() {
            let mut node = detached.original_node;
            if let Some(style) = world.get::<super::state::RuntimeComputedStyle>(entity) {
                super::apply::apply_node(&mut node, &style.0);
            }
            world.entity_mut(entity).insert(node);
            world
                .entity_mut(detached.parent)
                .insert_children(detached.index, &[entity]);
        }
        if world.get_entity(detached.placeholder).is_ok() {
            world.entity_mut(detached.placeholder).despawn();
        }
        if world.get_entity(entity).is_ok() {
            if world.get_entity(detached.parent).is_ok() {
                world
                    .entity_mut(entity)
                    .remove::<(DetachedBackdrop, UiTargetCamera)>();
            } else {
                world.entity_mut(entity).despawn();
            }
        }
    }
}

fn restore_dialogs(world: &mut World) {
    let detached = {
        let mut query = world.query::<(Entity, &DetachedDialog)>();
        query
            .iter(world)
            .map(|(entity, parent)| (entity, parent.0))
            .collect::<Vec<_>>()
    };
    for (entity, parent) in detached {
        restore_dialog(world, entity, parent);
    }
}

fn restore_closed_dialogs(world: &mut World) {
    let detached = {
        let mut query = world.query::<(Entity, &DetachedDialog, &DialogState)>();
        query
            .iter(world)
            .filter(|(_, _, state)| !state.open)
            .map(|(entity, parent, _)| (entity, parent.0))
            .collect::<Vec<_>>()
    };
    for (entity, parent) in detached {
        restore_dialog(world, entity, parent);
    }
}

fn restore_dialog(world: &mut World, entity: Entity, parent: Entity) {
    if world.get_entity(parent).is_ok() {
        world.entity_mut(parent).add_child(entity);
        world
            .entity_mut(entity)
            .remove::<(DetachedDialog, UiTargetCamera)>();
    } else {
        world.entity_mut(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ComponentStyleOwner;
    use bevy::ui::Node;
    use bevy::window::Window;

    #[test]
    fn plugin_builds_without_a_renderer() {
        let mut app = App::new();
        app.add_plugins(BackdropRuntimePlugin);
    }

    #[test]
    fn shader_registers_when_renderer_is_added_after_tilt_ui() {
        let mut app = App::new();
        app.add_plugins(BackdropRuntimePlugin);
        app.insert_resource(Assets::<Shader>::default());
        app.insert_sub_app(RenderApp, bevy::app::SubApp::new());

        app.finish();

        assert!(
            app.world()
                .resource::<Assets<Shader>>()
                .get(EFFECTS_SHADER.id())
                .is_some()
        );
        assert!(app.is_plugin_added::<FullscreenMaterialPlugin<LiveUiEffects>>());
    }

    #[test]
    fn shader_registers_when_runtime_is_added_during_plugin_finish() {
        struct DeferredRuntime;

        impl Plugin for DeferredRuntime {
            fn build(&self, _: &mut App) {}

            fn finish(&self, app: &mut App) {
                app.add_plugins(BackdropRuntimePlugin);
            }
        }

        let mut app = App::new();
        app.add_plugins(DeferredRuntime);
        app.insert_resource(Assets::<Shader>::default());
        app.insert_sub_app(RenderApp, bevy::app::SubApp::new());

        app.finish();

        assert!(
            app.world()
                .resource::<Assets<Shader>>()
                .get(EFFECTS_SHADER.id())
                .is_some()
        );
        assert!(app.is_plugin_added::<FullscreenMaterialPlugin<LiveUiEffects>>());
    }

    #[test]
    fn dialog_uses_overlay_only_while_live_blur_is_needed() {
        let mut app = App::new();
        let camera = app.world_mut().spawn((Camera2d, IsDefaultUiCamera)).id();
        let scope = app.world_mut().spawn_empty().id();
        let parent = app.world_mut().spawn(Node::default()).id();
        let dialog = app
            .world_mut()
            .spawn((Node::default(), ComponentStyleOwner(scope)))
            .id();
        crate::widgets::advanced::dialog::materialize(app.world_mut(), dialog, &[]);
        app.world_mut().entity_mut(parent).add_child(dialog);
        app.world_mut()
            .entity_mut(dialog)
            .insert(BackdropFilter(vec![BackgroundEffect::Blur(8)]));

        assert!(crate::open_dialog(app.world_mut(), dialog));
        sync_dialog_backdrops(app.world_mut());
        assert!(app.world().get::<LiveUiEffects>(camera).is_some());
        assert!(app.world().get::<DetachedDialog>(dialog).is_some());
        assert!(app.world().get::<UiTargetCamera>(dialog).is_some());

        assert!(crate::close_dialog(
            app.world_mut(),
            dialog,
            crate::DialogResult::Cancelled,
        ));
        sync_dialog_backdrops(app.world_mut());
        assert!(app.world().get::<LiveUiEffects>(camera).is_none());
        assert!(app.world().get::<DetachedDialog>(dialog).is_none());
        assert_eq!(app.world().get::<ChildOf>(dialog).unwrap().parent(), parent);
    }

    #[test]
    fn regular_element_keeps_its_layout_slot_and_is_restored() {
        let mut app = App::new();
        let camera = app.world_mut().spawn((Camera2d, IsDefaultUiCamera)).id();
        app.world_mut().spawn((Window::default(), PrimaryWindow));
        let parent = app.world_mut().spawn(Node::default()).id();
        let element = app
            .world_mut()
            .spawn((
                Node::default(),
                ComputedNode {
                    size: Vec2::new(120.0, 80.0),
                    inverse_scale_factor: 1.0,
                    ..Default::default()
                },
                UiGlobalTransform::from_xy(200.0, 160.0),
                BackdropFilter(vec![
                    BackgroundEffect::Grayscale(50),
                    BackgroundEffect::Contrast(150),
                ]),
            ))
            .id();
        app.world_mut().entity_mut(parent).add_child(element);

        sync_dialog_backdrops(app.world_mut());
        let detached = app
            .world()
            .get::<DetachedBackdrop>(element)
            .unwrap()
            .clone();
        assert_eq!(detached.parent, parent);
        assert_eq!(
            app.world()
                .get::<ChildOf>(detached.placeholder)
                .unwrap()
                .parent(),
            parent
        );
        assert!(app.world().get::<ChildOf>(element).is_none());
        let settings = app.world().get::<LiveUiEffects>(camera).unwrap();
        assert_eq!(settings.control.x, 1.0);
        assert_eq!(settings.filters[0], Vec4::new(0.0, 0.5, 1.5, 0.0));

        app.world_mut()
            .entity_mut(element)
            .remove::<BackdropFilter>();
        sync_dialog_backdrops(app.world_mut());
        assert!(app.world().get::<DetachedBackdrop>(element).is_none());
        assert_eq!(
            app.world().get::<ChildOf>(element).unwrap().parent(),
            parent
        );
        assert!(app.world().get_entity(detached.placeholder).is_err());
        assert!(app.world().get::<LiveUiEffects>(camera).is_none());
    }

    #[test]
    fn detached_element_keeps_ancestor_css_selectors() {
        use crate::{ElementClasses, TiltElement};
        use tilt_ui_core::ElementKind;
        use tilt_ui_css::parse_stylesheet;

        let mut world = World::new();
        world.spawn((Camera2d, IsDefaultUiCamera));
        world.spawn((Window::default(), PrimaryWindow));
        let scope = world.spawn_empty().id();
        let parent = world
            .spawn((
                Node::default(),
                TiltElement {
                    kind: ElementKind::Div,
                },
                ComponentStyleOwner(scope),
                ElementClasses {
                    classes: vec!["scene".into()],
                },
            ))
            .id();
        world.entity_mut(scope).add_child(parent);
        let element = world
            .spawn((
                Node::default(),
                ComputedNode {
                    size: Vec2::new(120.0, 80.0),
                    inverse_scale_factor: 1.0,
                    ..Default::default()
                },
                UiGlobalTransform::from_xy(200.0, 160.0),
                TiltElement {
                    kind: ElementKind::Div,
                },
                ComponentStyleOwner(scope),
                ElementClasses {
                    classes: vec!["panel".into()],
                },
                BackdropFilter(vec![BackgroundEffect::Blur(4)]),
            ))
            .id();
        world.entity_mut(parent).add_child(element);
        sync_dialog_backdrops(&mut world);
        let view = super::super::matcher::SelectorView::build(&mut world, scope);
        let sheet = parse_stylesheet(".scene > .panel { color: #ffffff; }").unwrap();
        assert!(
            super::super::matcher::matching_specificity(
                &view,
                element,
                &sheet.rules()[0].selectors
            )
            .is_some()
        );
        assert_eq!(logical_parent(&world, element), Some(parent));
    }

    #[test]
    fn animations_share_the_backdrop_gpu_pass() {
        use super::super::animated::{AnimatedFilters, AnimatedRuntimePlugin};
        use tilt_ui_css::{AnimatedEffect, AnimatedEffectKind, EffectQuality};

        let mut app = App::new();
        app.insert_resource(bevy::time::Time::<()>::default());
        app.add_plugins((BackdropRuntimePlugin, AnimatedRuntimePlugin));
        let camera = app.world_mut().spawn((Camera2d, IsDefaultUiCamera)).id();
        app.world_mut().spawn((Window::default(), PrimaryWindow));
        app.world_mut().spawn((
            Node::default(),
            ComputedNode {
                size: Vec2::new(100.0, 80.0),
                ..Default::default()
            },
            UiGlobalTransform::from_xy(200.0, 160.0),
            AnimatedFilters {
                effects: vec![AnimatedEffect {
                    kind: AnimatedEffectKind::Noise,
                    strength: 50,
                    speed: 100,
                }],
                quality: EffectQuality::Low,
            },
        ));
        app.update();
        let settings = app.world().get::<LiveUiEffects>(camera).unwrap();
        assert_eq!(settings.control.x, 0.0);
        assert_eq!(settings.control.y, 1.0);
    }

    #[test]
    fn root_backdrop_uses_overlay_camera_without_changing_layout() {
        let mut world = World::new();
        world.spawn((Camera2d, IsDefaultUiCamera));
        world.spawn((Window::default(), PrimaryWindow));
        let root = world
            .spawn((
                Node::default(),
                ComputedNode {
                    size: Vec2::new(120.0, 80.0),
                    ..Default::default()
                },
                UiGlobalTransform::from_xy(200.0, 160.0),
                BackdropFilter(vec![BackgroundEffect::Invert(30)]),
            ))
            .id();
        sync_dialog_backdrops(&mut world);
        assert!(world.get::<RootBackdropTarget>(root).is_some());
        assert!(world.get::<UiTargetCamera>(root).is_some());
        assert!(world.get::<DetachedBackdrop>(root).is_none());

        world.entity_mut(root).remove::<BackdropFilter>();
        sync_dialog_backdrops(&mut world);
        assert!(world.get::<RootBackdropTarget>(root).is_none());
        assert!(world.get::<UiTargetCamera>(root).is_none());
    }
}
