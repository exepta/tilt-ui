//! Opt-in, in-window release probe. It uses Bevy pointer messages for the
//! ColorPicker so the normal widget input path is measured without OS access.

use std::time::Instant;

use bevy::{
    app::AppExit,
    camera::NormalizedRenderTarget,
    input::{mouse::MouseScrollUnit, touch::TouchPhase},
    picking::{
        PickingSystems,
        backend::HitData,
        events::{Drag, Pointer, Press, Scroll},
        hover::HoverMap,
        pointer::{Location, PointerButton, PointerId},
    },
    prelude::*,
    ui::{ComputedNode, ComputedUiRenderTargetInfo, ScrollPosition, UiGlobalTransform, UiScale},
};
use tilt_ui::{
    ColorPickerState, ControlPart, ControlPartKind, ElementKind, TiltElement, set_color_picker_open,
};

const WARMUP: usize = 90;
const SAMPLE_FRAMES: usize = 120;
const END: usize = WARMUP + SAMPLE_FRAMES * 5;

#[derive(Resource, Default)]
struct Probe {
    frame: usize,
    last: Option<Instant>,
    samples: [Vec<(usize, f64)>; 5],
    first_cpu_ms: [f64; 5],
    first_step_ms: [f64; 5],
    first_wheel_ms: [f64; 5],
    color_changes: usize,
    color_events: usize,
    last_color_event_frame: Option<usize>,
    max_color_lag_frames: usize,
    wheel_events: usize,
    body_moves: usize,
    last_body_y: Option<f32>,
    last_color: Option<tilt_ui::tilt_ui_css::CssColor>,
}

pub(super) fn install(app: &mut App) {
    if std::env::var("TILT_UI_PERF_PROBE").as_deref() == Ok("1") {
        app.init_resource::<Probe>()
            .add_systems(First, step)
            .add_systems(PreUpdate, wheel_events.after(PickingSystems::Hover))
            .add_systems(Last, frame_cpu);
    }
}

fn step(world: &mut World) {
    let mut probe = world.remove_resource::<Probe>().unwrap();
    let now = Instant::now();
    if let Some(last) = probe.last {
        let previous = probe.frame.saturating_sub(1);
        if (WARMUP..END).contains(&previous) {
            let phase = (previous - WARMUP) / SAMPLE_FRAMES;
            probe.samples[phase].push((
                (previous - WARMUP) % SAMPLE_FRAMES,
                now.duration_since(last).as_secs_f64() * 1000.0,
            ));
        }
    }
    probe.last = Some(now);
    observe_color(world, &mut probe);

    if probe.frame == END {
        for (phase, (name, samples)) in [
            "idle",
            "body scroll first",
            "ColorPicker drag first",
            "body scroll repeat",
            "ColorPicker drag repeat",
        ]
        .into_iter()
        .zip(&probe.samples)
        .enumerate()
        {
            let mut sorted = samples.clone();
            sorted.sort_by(|left, right| left.1.total_cmp(&right.1));
            let at = |percent: usize| sorted[sorted.len() * percent / 100].1;
            let spikes = sorted
                .iter()
                .filter(|(_, time)| *time > 33.3)
                .copied()
                .collect::<Vec<_>>();
            info!(
                "perf probe {name}: frames={}, p50={:.2} ms, p95={:.2} ms, max={:.2} ms, >33.3 ms={spikes:?}, first CPU={:.2} ms, step={:.2} ms, wheel={:.2} ms",
                sorted.len(),
                at(50),
                at(95),
                sorted.last().map_or(0.0, |(_, time)| *time),
                probe.first_cpu_ms[phase],
                probe.first_step_ms[phase],
                probe.first_wheel_ms[phase],
            );
        }
        info!(
            "perf probe body: injected wheel events={}, position changes={}",
            probe.wheel_events, probe.body_moves
        );
        info!(
            "perf probe ColorPicker: injected events={}, value changes={}, max response={} frame(s)",
            probe.color_events, probe.color_changes, probe.max_color_lag_frames
        );
        world.write_message(AppExit::Success);
        world.insert_resource(probe);
        return;
    }

    let phase = probe.frame.saturating_sub(WARMUP) / SAMPLE_FRAMES;
    if probe.frame >= WARMUP && matches!(phase, 1 | 3) {
        let position = world
            .query::<(&TiltElement, &ScrollPosition)>()
            .iter(world)
            .find(|(element, _)| element.kind == ElementKind::Body)
            .map(|(_, scroll)| scroll.0.y);
        if position.is_some_and(|position| probe.last_body_y.is_some_and(|old| old != position)) {
            probe.body_moves += 1;
        }
        probe.last_body_y = position;
    }
    if probe.frame >= WARMUP && matches!(phase, 2 | 4) {
        let frame = (probe.frame - WARMUP) % SAMPLE_FRAMES;
        drag_color(world, &mut probe, frame);
    }
    if probe.frame >= WARMUP && (probe.frame - WARMUP) % SAMPLE_FRAMES == 0 {
        probe.first_step_ms[phase] = now.elapsed().as_secs_f64() * 1000.0;
    }
    probe.frame += 1;
    world.insert_resource(probe);
}

fn observe_color(world: &mut World, probe: &mut Probe) {
    let previous_frame = probe.frame.saturating_sub(1);
    if previous_frame < WARMUP {
        return;
    }
    let phase = (previous_frame - WARMUP) / SAMPLE_FRAMES;
    if !matches!(phase, 2 | 4) {
        return;
    }
    let value = world
        .query::<&ColorPickerState>()
        .iter(world)
        .next()
        .map(|state| state.value);
    if let Some(value) = value {
        if probe.last_color.is_some_and(|previous| previous != value) {
            probe.color_changes += 1;
            if let Some(sent) = probe.last_color_event_frame {
                probe.max_color_lag_frames = probe.max_color_lag_frames.max(probe.frame - sent);
            }
        }
        probe.last_color = Some(value);
    }
}

fn frame_cpu(mut probe: ResMut<Probe>) {
    let frame = probe.frame.saturating_sub(1);
    if (WARMUP..END).contains(&frame) && (frame - WARMUP) % SAMPLE_FRAMES == 0 {
        let phase = (frame - WARMUP) / SAMPLE_FRAMES;
        if let Some(started) = probe.last {
            probe.first_cpu_ms[phase] = started.elapsed().as_secs_f64() * 1000.0;
        }
    }
}

fn wheel_events(world: &mut World) {
    let started = Instant::now();
    let frame = world.resource::<Probe>().frame.saturating_sub(1);
    let body = world
        .query::<(Entity, &TiltElement, &ComputedNode, &ScrollPosition)>()
        .iter(world)
        .find(|(_, element, node, _)| {
            element.kind == ElementKind::Body && node.content_size().y > node.size().y
        })
        .map(|(entity, _, _, _)| entity);
    let Some(body) = body else {
        return;
    };
    let hit = HitData::new(body, 0.0, None, None);
    world
        .resource_mut::<HoverMap>()
        .entry(PointerId::Mouse)
        .or_default()
        .insert(body, hit.clone());
    let phase = frame.saturating_sub(WARMUP) / SAMPLE_FRAMES;
    if frame < WARMUP || !matches!(phase, 1 | 3) {
        return;
    }
    let direction = if ((frame - WARMUP) % SAMPLE_FRAMES) / 24 % 2 == 0 {
        -1.0
    } else {
        1.0
    };
    for _ in 0..3 {
        world
            .resource_mut::<Messages<Pointer<Scroll>>>()
            .write(Pointer::new(
                PointerId::Mouse,
                Location {
                    target: NormalizedRenderTarget::None {
                        width: 1440,
                        height: 1060,
                    },
                    position: Vec2::new(4.0, 4.0),
                },
                Scroll {
                    unit: MouseScrollUnit::Line,
                    x: 0.0,
                    y: direction,
                    hit: hit.clone(),
                    phase: TouchPhase::Moved,
                },
                body,
            ));
    }
    world.resource_mut::<Probe>().wheel_events += 3;
    if (frame - WARMUP) % SAMPLE_FRAMES == 0 {
        world.resource_mut::<Probe>().first_wheel_ms[phase] =
            started.elapsed().as_secs_f64() * 1000.0;
    }
}

fn drag_color(world: &mut World, probe: &mut Probe, frame: usize) {
    let owner = world
        .query::<(Entity, &ColorPickerState)>()
        .iter(world)
        .next()
        .map(|(entity, state)| (entity, state.value));
    let Some((owner, value)) = owner else {
        return;
    };
    if frame == 0 {
        probe.last_color = Some(value);
        set_color_picker_open(world, owner, true);
    }
    if frame < 5 {
        return;
    }
    let canvas = world
        .query::<(Entity, &ControlPart)>()
        .iter(world)
        .find(|(_, part)| part.owner == owner && part.kind == ControlPartKind::ColorCanvas)
        .map(|(entity, _)| entity);
    let Some(canvas) = canvas else {
        return;
    };
    let (Some(node), Some(transform)) = (
        world.get::<ComputedNode>(canvas),
        world.get::<UiGlobalTransform>(canvas),
    ) else {
        return;
    };
    let size = node.size();
    if size.x <= 0.0 || size.y <= 0.0 {
        return;
    }
    let x = 0.1 + (frame % 60) as f32 / 75.0;
    let y = 0.2 + (frame / 60) as f32 * 0.2;
    let ui_position =
        transform.affine().translation - size * 0.5 + Vec2::new(x * size.x, y * size.y);
    let scale = world
        .get::<ComputedUiRenderTargetInfo>(canvas)
        .map_or(1.0, |target| target.scale_factor());
    let ui_scale = world.get_resource::<UiScale>().map_or(1.0, |scale| scale.0);
    let position = ui_position * ui_scale / scale;
    let location = Location {
        target: NormalizedRenderTarget::None {
            width: 1440,
            height: 1060,
        },
        position,
    };
    if frame == 5 {
        world
            .resource_mut::<Messages<Pointer<Press>>>()
            .write(Pointer::new(
                PointerId::Mouse,
                location,
                Press {
                    button: PointerButton::Primary,
                    hit: HitData::new(canvas, 0.0, None, None),
                    count: 1,
                },
                canvas,
            ));
    } else {
        world
            .resource_mut::<Messages<Pointer<Drag>>>()
            .write(Pointer::new(
                PointerId::Mouse,
                location,
                Drag {
                    button: PointerButton::Primary,
                    distance: Vec2::ZERO,
                    delta: Vec2::ZERO,
                },
                canvas,
            ));
    }
    probe.color_events += 1;
    probe.last_color_event_frame = Some(probe.frame);
}
