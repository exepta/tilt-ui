#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var backdrop: texture_2d<f32>;
@group(0) @binding(1) var backdrop_sampler: sampler;

struct Settings {
    control: vec4<f32>,
    bounds: vec4<f32>,
    rects: array<vec4<f32>, 8>,
    boxes: array<vec4<f32>, 8>,
    radii: array<vec4<f32>, 8>,
    filters: array<vec4<f32>, 8>,
    animated_rects: array<vec4<f32>, 8>,
    animated_boxes: array<vec4<f32>, 8>,
    animated_radii: array<vec4<f32>, 8>,
    animated_specs: array<vec4<f32>, 8>,
};
@group(0) @binding(2) var<uniform> settings: Settings;

// Integer hashing keeps the animated patterns stable even after a long uptime.
fn effect_hash(seed: u32) -> f32 {
    let state = seed * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return f32((word >> 22u) ^ word) / 4294967296.0;
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(backdrop));
    let uv = in.uv;
    var color = textureSample(backdrop, backdrop_sampler, uv);
    let bounds = settings.bounds;
    if (uv.x < bounds.x || uv.x > bounds.z || uv.y < bounds.y || uv.y > bounds.w) { return color; }
    for (var i = 0u; i < 8u; i++) {
        if (i >= u32(settings.control.x)) { break; }
        let rect = settings.rects[i];
        if (uv.x < rect.x || uv.x > rect.z || uv.y < rect.y || uv.y > rect.w) { continue; }
        let bounds = settings.boxes[i];
        let corners = settings.radii[i];
        let pixel = uv * size;
        let minimum = bounds.xy * size;
        let maximum = bounds.zw * size;
        if (corners.x > 0.0 && pixel.x < minimum.x + corners.x && pixel.y < minimum.y + corners.x
            && distance(pixel, minimum + vec2<f32>(corners.x)) > corners.x) { continue; }
        if (corners.y > 0.0 && pixel.x > maximum.x - corners.y && pixel.y < minimum.y + corners.y
            && distance(pixel, vec2<f32>(maximum.x - corners.y, minimum.y + corners.y)) > corners.y) { continue; }
        if (corners.z > 0.0 && pixel.x > maximum.x - corners.z && pixel.y > maximum.y - corners.z
            && distance(pixel, maximum - vec2<f32>(corners.z)) > corners.z) { continue; }
        if (corners.w > 0.0 && pixel.x < minimum.x + corners.w && pixel.y > maximum.y - corners.w
            && distance(pixel, vec2<f32>(minimum.x + corners.w, maximum.y - corners.w)) > corners.w) { continue; }
        let treatment = settings.filters[i];
        if (treatment.x > 0.0) {
            let offset = vec2<f32>(treatment.x * 0.5) / size;
            var blurred = color * 0.25;
            blurred += textureSample(backdrop, backdrop_sampler, uv + vec2<f32>(offset.x, 0.0)) * 0.125;
            blurred += textureSample(backdrop, backdrop_sampler, uv - vec2<f32>(offset.x, 0.0)) * 0.125;
            blurred += textureSample(backdrop, backdrop_sampler, uv + vec2<f32>(0.0, offset.y)) * 0.125;
            blurred += textureSample(backdrop, backdrop_sampler, uv - vec2<f32>(0.0, offset.y)) * 0.125;
            blurred += textureSample(backdrop, backdrop_sampler, uv + offset) * 0.0625;
            blurred += textureSample(backdrop, backdrop_sampler, uv - offset) * 0.0625;
            blurred += textureSample(backdrop, backdrop_sampler, uv + vec2<f32>(offset.x, -offset.y)) * 0.0625;
            blurred += textureSample(backdrop, backdrop_sampler, uv + vec2<f32>(-offset.x, offset.y)) * 0.0625;
            color = blurred;
        }
        let gray = dot(color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
        color = vec4<f32>(mix(color.rgb, vec3<f32>(gray), treatment.y), color.a);
        color = vec4<f32>(clamp((color.rgb - vec3<f32>(0.5)) * treatment.z + vec3<f32>(0.5),
            vec3<f32>(0.0), vec3<f32>(1.0)), color.a);
        color = vec4<f32>(mix(color.rgb, vec3<f32>(1.0) - color.rgb, treatment.w), color.a);
    }
    for (var i = 0u; i < 8u; i++) {
        if (i >= u32(settings.control.y)) { break; }
        let rect = settings.animated_rects[i];
        if (uv.x < rect.x || uv.x > rect.z || uv.y < rect.y || uv.y > rect.w) { continue; }
        let box_rect = settings.animated_boxes[i];
        let corners = settings.animated_radii[i];
        let pixel = uv * size;
        let minimum = box_rect.xy * size;
        let maximum = box_rect.zw * size;
        if (corners.x > 0.0 && pixel.x < minimum.x + corners.x && pixel.y < minimum.y + corners.x
            && distance(pixel, minimum + vec2<f32>(corners.x)) > corners.x) { continue; }
        if (corners.y > 0.0 && pixel.x > maximum.x - corners.y && pixel.y < minimum.y + corners.y
            && distance(pixel, vec2<f32>(maximum.x - corners.y, minimum.y + corners.y)) > corners.y) { continue; }
        if (corners.z > 0.0 && pixel.x > maximum.x - corners.z && pixel.y > maximum.y - corners.z
            && distance(pixel, maximum - vec2<f32>(corners.z)) > corners.z) { continue; }
        if (corners.w > 0.0 && pixel.x < minimum.x + corners.w && pixel.y > maximum.y - corners.w
            && distance(pixel, vec2<f32>(minimum.x + corners.w, maximum.y - corners.w)) > corners.w) { continue; }
        let spec = settings.animated_specs[i];
        let strength = spec.y;
        let tick = settings.control.z * spec.z;
        let local = (uv - rect.xy) / max(rect.zw - rect.xy, vec2<f32>(0.0001));
        if (spec.x < 1.5) {
            // Fine, neutral film grain. A two-pixel grain on low quality avoids shimmer.
            let grain_size = select(2.0, 1.0, spec.w > 0.5);
            let cell = vec2<u32>(floor(pixel / grain_size));
            let frame = u32(floor(tick * 18.0));
            let grain = effect_hash(cell.x * 1973u + cell.y * 9277u + frame * 26699u) - 0.5;
            let luminance = dot(color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
            let toned = mix(color.rgb, vec3<f32>(luminance), strength * 0.08);
            color = vec4<f32>(clamp(toned + vec3<f32>(grain * strength * 0.22),
                vec3<f32>(0.0), vec3<f32>(1.0)), color.a);
        } else if (spec.x < 2.5) {
            // Broken signal: only a few horizontal slices jump on each time step.
            let slice = u32(floor(local.y * 26.0));
            let frame = u32(floor(tick * 10.0));
            let seed = slice * 131u + frame * 3167u;
            let lost = step(1.0 - strength * 0.42, effect_hash(seed));
            var signal = color.rgb;
            if (lost > 0.5) {
                let shift = (effect_hash(seed + 977u) * 2.0 - 1.0) * strength
                    * (rect.z - rect.x) * 0.075;
                let safe_uv = clamp(uv + vec2<f32>(shift, 0.0), rect.xy + 1.0 / size,
                    rect.zw - 1.0 / size);
                let center = textureSample(backdrop, backdrop_sampler, safe_uv);
                signal = center.rgb;
                if (spec.w > 0.5) {
                    let split = vec2<f32>(strength * 3.0 / size.x, 0.0);
                    let red = textureSample(backdrop, backdrop_sampler,
                        clamp(safe_uv + split, rect.xy, rect.zw));
                    let blue = textureSample(backdrop, backdrop_sampler,
                        clamp(safe_uv - split, rect.xy, rect.zw));
                    signal = vec3<f32>(red.r, center.g, blue.b);
                }
                let dropout = step(0.86, effect_hash(seed + 1823u)) * 0.42;
                signal *= 1.0 - dropout * strength;
            }
            let scanline = 0.94 + 0.06 * cos(pixel.y * 3.14159265);
            color = vec4<f32>(clamp(signal * mix(1.0, scanline, strength),
                vec3<f32>(0.0), vec3<f32>(1.0)), color.a);
        } else if (spec.x < 3.5) {
            let sepia = vec3<f32>(dot(color.rgb, vec3<f32>(0.393, 0.769, 0.189)),
                dot(color.rgb, vec3<f32>(0.349, 0.686, 0.168)),
                dot(color.rgb, vec3<f32>(0.272, 0.534, 0.131)));
            let frame = u32(floor(tick * 5.0));
            let scratch = step(0.992, effect_hash(u32(floor(local.x * 240.0)) + frame * 109u));
            let flicker = effect_hash(u32(floor(tick * 13.0)) + 37u) * 0.09;
            // Unevenly exposed frames have small, short-lived dark film holes.
            let cell = vec2<u32>(floor(local * vec2<f32>(11.0, 7.0)));
            let spot_seed = cell.x * 131u + cell.y * 977u + u32(floor(tick * 2.0)) * 8191u;
            let center = vec2<f32>(effect_hash(spot_seed + 17u), effect_hash(spot_seed + 53u));
            let spot_uv = fract(local * vec2<f32>(11.0, 7.0)) - center;
            let radius = mix(0.1, 0.23, effect_hash(spot_seed + 89u));
            let hole = step(0.93, effect_hash(spot_seed))
                * (1.0 - smoothstep(radius * 0.55, radius, length(spot_uv)));
            color = vec4<f32>(mix(color.rgb, clamp(sepia, vec3<f32>(0.0), vec3<f32>(1.0)), strength * 0.75), color.a);
            color = vec4<f32>(clamp(color.rgb * (1.0 - hole * strength * 0.6)
                + (scratch * 0.3 - flicker) * strength, vec3<f32>(0.0), vec3<f32>(1.0)), color.a);
        } else if (spec.x < 4.5) {
            let edge = pow(1.0 - min(local.x, 1.0 - local.x) * 2.0, 5.0);
            let pulse = 0.75 + 0.25 * sin(tick * 2.0);
            color = vec4<f32>(clamp(color.rgb + vec3<f32>(0.45, 0.12, 0.7) * edge * strength * pulse,
                vec3<f32>(0.0), vec3<f32>(1.0)), color.a);
        } else if (spec.x < 5.5) {
            let radius = select(5.0, 9.0, spec.w > 0.5);
            let delta = vec2<f32>(radius) / size;
            var glow = textureSample(backdrop, backdrop_sampler, uv + vec2<f32>(delta.x, 0.0)).rgb;
            glow += textureSample(backdrop, backdrop_sampler, uv - vec2<f32>(delta.x, 0.0)).rgb;
            var samples = 2.0;
            if (spec.w > 0.5) {
                glow += textureSample(backdrop, backdrop_sampler, uv + vec2<f32>(0.0, delta.y)).rgb;
                glow += textureSample(backdrop, backdrop_sampler, uv - vec2<f32>(0.0, delta.y)).rgb;
                samples = 4.0;
            }
            if (spec.w > 1.5) {
                glow += textureSample(backdrop, backdrop_sampler, uv + delta).rgb;
                glow += textureSample(backdrop, backdrop_sampler, uv - delta).rgb;
                glow += textureSample(backdrop, backdrop_sampler, uv + vec2<f32>(delta.x, -delta.y)).rgb;
                glow += textureSample(backdrop, backdrop_sampler, uv + vec2<f32>(-delta.x, delta.y)).rgb;
                samples = 8.0;
            }
            glow /= samples;
            let bright = max(max(glow.r, glow.g), glow.b);
            let edge_distance = min((uv - rect.xy) * size, (rect.zw - uv) * size);
            let edge_fade = smoothstep(0.0, radius * 1.5,
                min(edge_distance.x, edge_distance.y));
            color = vec4<f32>(clamp(color.rgb + glow * max(bright - 0.42, 0.0) * strength
                * (1.7 + 0.15 * sin(tick)) * edge_fade,
                vec3<f32>(0.0), vec3<f32>(1.0)), color.a);
        } else if (spec.x < 6.5) {
            // Sparse rain on glass: asymmetric drops with a short, thin wet trail.
            let extent = max((rect.zw - rect.xy) * size, vec2<f32>(1.0));
            let column = u32(floor(local.x * 8.0));
            let lane = effect_hash(column * 541u + 19u);
            let velocity = 0.10 + effect_hash(column * 733u + 43u) * 0.18;
            let travel = tick * velocity + sin(tick * (0.35 + lane * 0.2) + lane * 6.2831853) * 0.08;
            let drop_x = local.x * extent.x - (f32(column) + 0.2 + lane * 0.6) * extent.x / 8.0;
            let phase = fract(local.y * 1.8 - travel - lane);
            let drop_y = select(phase, phase - 1.0, phase > 0.5) * extent.y / 1.8;
            let radius = 4.5 + effect_hash(column * 997u + 71u) * 3.0;
            let height = 12.0 + effect_hash(column * 499u + 83u) * 7.0;
            let tapered_width = radius * clamp(0.72 + drop_y / height * 0.48, 0.27, 1.2);
            let shape = length(vec2<f32>(drop_x / tapered_width, drop_y / height));
            let pearl = 1.0 - smoothstep(0.78, 1.02, shape);
            if (pearl > 0.0) {
                let bend = vec2<f32>(drop_x / radius * 2.4, drop_y / height * 1.8)
                    * strength / size;
                let sampled = textureSample(backdrop, backdrop_sampler,
                    clamp(uv + bend, rect.xy + 1.0 / size, rect.zw - 1.0 / size));
                let glint = 1.0 - smoothstep(0.05, 0.42,
                    length(vec2<f32>((drop_x + radius * 0.32) / radius,
                        (drop_y + height * 0.23) / height)));
                let edge = smoothstep(0.52, 0.88, shape)
                    * (1.0 - smoothstep(0.88, 1.04, shape));
                let bright_edge = edge * max(-drop_x / radius, 0.0);
                let dark_edge = edge * max(drop_x / radius, 0.0);
                color = vec4<f32>(clamp(mix(color.rgb, sampled.rgb, pearl * strength * 0.82)
                    + vec3<f32>(0.34, 0.39, 0.42) * glint * pearl * strength
                    + vec3<f32>(0.19, 0.24, 0.27) * bright_edge * strength
                    - vec3<f32>(0.1) * dark_edge * strength,
                    vec3<f32>(0.0), vec3<f32>(1.0)), color.a);
            }
            let trail = (1.0 - smoothstep(0.5, max(radius * 0.23, 0.8), abs(drop_x)))
                * smoothstep(-height * 2.4, -height * 0.9, drop_y)
                * (1.0 - smoothstep(-height * 0.9, -height * 0.6, drop_y));
            color = vec4<f32>(clamp(color.rgb + vec3<f32>(0.12, 0.17, 0.2) * trail * strength,
                vec3<f32>(0.0), vec3<f32>(1.0)), color.a);
        } else {
            // A radial traveling wave, measured in pixels to preserve circular ripples.
            let extent = max((rect.zw - rect.xy) * size, vec2<f32>(1.0));
            let from_center = (local - vec2<f32>(0.5)) * extent;
            let distance_px = length(from_center);
            let half_diagonal = max(length(extent) * 0.5, 1.0);
            let phase = distance_px / half_diagonal * 18.0 - tick * 6.2831853 * 0.45;
            let offset = from_center / max(distance_px, 1.0) * sin(phase) * strength * 6.0;
            color = textureSample(backdrop, backdrop_sampler,
                clamp(uv + offset / size, rect.xy + 1.0 / size, rect.zw - 1.0 / size));
        }
    }
    return color;
}
