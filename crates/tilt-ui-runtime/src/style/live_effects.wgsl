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

fn random2(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
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
            let cell = floor(uv * size / select(2.0, 1.0, spec.w > 0.5));
            let grain = random2(cell + floor(tick * 24.0)) - 0.5;
            color = vec4<f32>(clamp(color.rgb + grain * strength * 0.35, vec3<f32>(0.0), vec3<f32>(1.0)), color.a);
        } else if (spec.x < 2.5) {
            var crt = color.rgb;
            if (spec.w > 0.5) {
                let shift = vec2<f32>(strength * 1.8 / size.x, 0.0);
                crt = vec3<f32>(textureSample(backdrop, backdrop_sampler, uv + shift).r,
                    crt.g, textureSample(backdrop, backdrop_sampler, uv - shift).b);
            }
            let scan = cos(pixel.y * 3.14159265) * 0.5 + 0.5;
            let centered = local * 2.0 - vec2<f32>(1.0);
            let vignette = min(dot(centered, centered) * 0.16, 0.32);
            let rolling = 1.0 - smoothstep(0.0, 0.12, abs(fract(local.y - tick * 0.16) - 0.5));
            let flicker = sin(tick * 47.0) * 0.03;
            let dim = 1.0 - strength * (0.36 * scan + vignette + 0.10 * rolling + flicker);
            color = vec4<f32>(clamp(crt * dim + vec3<f32>(0.0, 0.012, 0.025) * strength,
                vec3<f32>(0.0), vec3<f32>(1.0)), color.a);
        } else if (spec.x < 3.5) {
            let sepia = vec3<f32>(dot(color.rgb, vec3<f32>(0.393, 0.769, 0.189)),
                dot(color.rgb, vec3<f32>(0.349, 0.686, 0.168)),
                dot(color.rgb, vec3<f32>(0.272, 0.534, 0.131)));
            let scratch = step(0.997, random2(vec2<f32>(floor(local.x * 240.0), floor(tick * 5.0))));
            let flicker = random2(vec2<f32>(floor(tick * 13.0), 3.0)) * 0.08;
            color = vec4<f32>(mix(color.rgb, clamp(sepia, vec3<f32>(0.0), vec3<f32>(1.0)), strength * 0.75), color.a);
            color = vec4<f32>(clamp(color.rgb + (scratch * 0.25 - flicker) * strength, vec3<f32>(0.0), vec3<f32>(1.0)), color.a);
        } else if (spec.x < 4.5) {
            let edge = pow(1.0 - min(local.x, 1.0 - local.x) * 2.0, 5.0);
            let pulse = 0.75 + 0.25 * sin(tick * 2.0);
            color = vec4<f32>(clamp(color.rgb + vec3<f32>(0.45, 0.12, 0.7) * edge * strength * pulse,
                vec3<f32>(0.0), vec3<f32>(1.0)), color.a);
        } else {
            let radius = select(2.0, 4.0, spec.w > 0.5);
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
            color = vec4<f32>(clamp(color.rgb + glow * max(bright - 0.55, 0.0) * strength * (0.9 + 0.1 * sin(tick)),
                vec3<f32>(0.0), vec3<f32>(1.0)), color.a);
        }
    }
    return color;
}
