#import bevy_ui::ui_vertex_output::UiVertexOutput

struct Parameter { value: vec4<f32> };
@group(1) @binding(0) var<uniform> geometry: Parameter;
@group(1) @binding(1) var<uniform> widths: Parameter;
@group(1) @binding(2) var<uniform> radii: Parameter;
@group(1) @binding(3) var<uniform> styles: Parameter;
@group(1) @binding(4) var<uniform> top_color: Parameter;
@group(1) @binding(5) var<uniform> right_color: Parameter;
@group(1) @binding(6) var<uniform> bottom_color: Parameter;
@group(1) @binding(7) var<uniform> left_color: Parameter;

fn rounded_mask(p: vec2<f32>, origin: vec2<f32>, size: vec2<f32>, corners: vec4<f32>) -> f32 {
    if any(p < origin) || any(p > origin + size) { return 0.0; }
    let local = p - origin;
    let radius = select(
        select(corners.x, corners.y, local.x > size.x * 0.5),
        select(corners.w, corners.z, local.x > size.x * 0.5),
        local.y > size.y * 0.5
    );
    let r = clamp(radius, 0.0, min(size.x, size.y) * 0.5);
    if r < 0.5 { return 1.0; }
    let corner_distance = length(max(max(vec2<f32>(r)-local, local-(size-vec2<f32>(r))), vec2<f32>(0.0)));
    return 1.0 - smoothstep(r-0.7, r+0.7, corner_distance);
}

// Stable, texture-free variation: the pattern does not change while scrolling.
fn hash(value: f32) -> f32 {
    let seed = fract(value * 0.1031);
    return fract((seed + 33.33) * (seed + 33.33) * seed);
}

fn pattern(kind: f32, axis: f32, inset: f32, thick: f32, strength: f32, side: f32) -> f32 {
    if kind < 0.5 { return 0.0; }
    if kind < 1.5 { return 1.0; }
    let unit = max(thick, 1.5);
    if kind < 2.5 {
        let period = max(unit * 3.1, 5.0);
        let a = abs(fract(axis / period) * period - period * 0.5);
        return 1.0 - smoothstep(unit * 0.43, unit * 0.53, length(vec2<f32>(a, inset-thick*0.5)));
    }
    if kind < 3.5 {
        let period = max(unit * 5.0, 8.0);
        return 1.0 - smoothstep(unit*2.8, unit*3.0, fract(axis/period)*period);
    }
    if kind < 4.5 {
        let period = max(unit * 8.0, 12.0);
        let phase = fract(axis/period)*period;
        let dash = 1.0 - smoothstep(unit*2.8, unit*3.0, phase);
        let dot = 1.0 - smoothstep(unit*0.38, unit*0.5, length(vec2<f32>(phase-unit*5.3, inset-thick*0.5)));
        return max(dash, dot);
    }
    if kind < 5.5 {
        // A brittle rim with missing chips, small diagonal fractures and stray splinters.
        let span = max(unit*4.0, 15.0);
        let cell = floor(axis/span);
        let phase = fract(axis/span)*span;
        let wobble = sin(axis*0.37 + side*2.7)*0.55 + sin(axis*1.13 + side)*0.24;
        let body = 1.0 - smoothstep(thick*0.67+wobble-0.7, thick*0.67+wobble+0.7, inset);
        let chip = hash(cell + side*37.0 + 4.1);
        let chipped_segment = select(1.0, smoothstep(span*0.24, span*0.38, phase), chip > 0.74);
        let crack_at = span*(0.2 + 0.6*hash(cell + side*19.0 + 8.3));
        let crack = smoothstep(0.7, 1.9, abs(phase-crack_at-(inset/thick-0.5)*2.2));
        let splinter = (1.0-smoothstep(0.3, 0.9, abs(inset-thick*0.78-wobble*0.35)))
            * select(0.0, 0.38, chip > 0.52);
        return max(body*chipped_segment*crack, splinter*crack);
    }
    // Dry, weathered paint: faded patches, directional grain and missing flakes.
    let weather = 0.5 + 0.28*sin(axis*0.085+side*3.1) + 0.22*sin(axis*0.027+side*6.7);
    let grain = 0.5 + 0.5*sin(inset*7.2+axis*0.075+sin(axis*0.21+side));
    let edge_wear = 0.11*sin(axis*0.32+side*4.0) + 0.07*sin(axis*0.91+side);
    let painted_depth = thick*(0.89 + strength*edge_wear);
    let coverage = 1.0 - smoothstep(painted_depth-0.8, painted_depth+0.8, inset);
    let faded_area = clamp(1.0-strength*(0.07+0.30*weather), 0.35, 1.0);
    let bristle = 1.0-strength*(0.28*grain);
    let scratch_cell = floor(axis/max(thick*3.0, 18.0));
    let scratch_depth = thick*(0.24+0.52*hash(scratch_cell+side*47.0+2.6));
    let scratch = 1.0 - smoothstep(0.25, 0.85, abs(inset-scratch_depth));
    let scratch_active = select(0.0, 1.0, hash(scratch_cell+side*29.0+6.4) > 0.59);
    let flake_span = max(thick*3.8, 25.0);
    let flake_cell = floor((axis+side*11.0)/flake_span);
    let flake_phase = fract((axis+side*11.0)/flake_span)*flake_span;
    let flake_center = flake_span*(0.25+0.5*hash(flake_cell+side*13.0+1.9));
    let flake_depth = thick*(0.32+0.36*hash(flake_cell+side*23.0+5.1));
    let flake_radius = vec2<f32>(flake_span*(0.10+0.08*hash(flake_cell+7.4)), thick*0.38);
    let flake_shape = length(vec2<f32>((flake_phase-flake_center)/flake_radius.x, (inset-flake_depth)/flake_radius.y));
    let jagged_flake = 1.0-smoothstep(0.76, 1.04, flake_shape+0.11*sin(axis*1.7+inset*2.4));
    let flake_active = select(0.0, 1.0, hash(flake_cell+side*41.0+8.7) > 0.39);
    let peeling = 1.0-min(strength*1.16, 1.0)*jagged_flake*flake_active;
    let fleck = hash(floor(axis*1.3)+floor(inset*2.8)*83.0+side*17.0);
    let speckle = select(1.0, 0.38, fleck > 0.89);
    return coverage*faded_area*bristle*peeling*speckle*(1.0-strength*0.72*scratch*scratch_active);
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let size = geometry.value.xy;
    if any(size <= vec2<f32>(0.0)) { return vec4<f32>(0.0); }
    let p = in.uv * size;
    let w = max(widths.value, vec4<f32>(0.0));
    let outer = rounded_mask(p, vec2<f32>(0.0), size, radii.value);
    if outer <= 0.0 { return vec4<f32>(0.0); }
    let inset = vec2<f32>(w.w, w.x);
    let inner_size = max(size - vec2<f32>(w.w+w.y, w.x+w.z), vec2<f32>(0.0));
    let inner_radii = max(radii.value - vec4<f32>(max(w.x,w.w), max(w.x,w.y), max(w.z,w.y), max(w.z,w.w)), vec4<f32>(0.0));
    let ring = outer * (1.0-rounded_mask(p, inset, inner_size, inner_radii));
    if ring <= 0.0 { return vec4<f32>(0.0); }
    let distances = vec4<f32>(p.y/max(w.x,0.1), (size.x-p.x)/max(w.y,0.1), (size.y-p.y)/max(w.z,0.1), p.x/max(w.w,0.1));
    var side = 0;
    var nearest = distances.x;
    if distances.y < nearest { nearest = distances.y; side = 1; }
    if distances.z < nearest { nearest = distances.z; side = 2; }
    if distances.w < nearest { side = 3; }
    var kind = styles.value.x;
    var axis = p.x;
    var depth = p.y;
    var thickness = w.x;
    var tint = top_color.value;
    if side == 1 { kind = styles.value.y; axis = p.y; depth = size.x-p.x; thickness = w.y; tint = right_color.value; }
    if side == 2 { kind = styles.value.z; axis = p.x; depth = size.y-p.y; thickness = w.z; tint = bottom_color.value; }
    if side == 3 { kind = styles.value.w; axis = p.y; depth = p.x; thickness = w.w; tint = left_color.value; }
    if kind > 5.5 {
        let oxidation = 0.5 + 0.5*sin(axis*0.13+f32(side)*2.1)*sin(depth*0.9+f32(side));
        tint = vec4<f32>(tint.rgb*(1.0-geometry.value.z*(0.12+0.25*oxidation)), tint.a);
    }
    tint.a *= ring * pattern(kind, axis, depth, max(thickness, 0.1), geometry.value.z, f32(side));
    return tint;
}
