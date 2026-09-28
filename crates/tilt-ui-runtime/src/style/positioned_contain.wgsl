#import bevy_ui::ui_vertex_output::UiVertexOutput

struct Geometry {
    value: vec4<f32>,
};
@group(1) @binding(0) var<uniform> geometry: Geometry;
@group(1) @binding(1) var source: texture_2d<f32>;
@group(1) @binding(2) var source_sampler: sampler;
struct Opacity {
    value: vec4<f32>,
};
@group(1) @binding(3) var<uniform> opacity: Opacity;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let uv = (in.uv - geometry.value.xy) / geometry.value.zw;
    let sampled = textureSample(source, source_sampler, clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)));
    let inside = all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0));
    return sampled * opacity.value.x * select(0.0, 1.0, inside);
}
