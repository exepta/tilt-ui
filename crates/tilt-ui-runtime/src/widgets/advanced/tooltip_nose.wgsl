#import bevy_ui::ui_vertex_output::UiVertexOutput

struct Parameter { value: vec4<f32> };
@group(1) @binding(0) var<uniform> fill: Parameter;
@group(1) @binding(1) var<uniform> side: Parameter;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    var p = in.uv;
    // Canonical triangle points down. Rotate its coordinates for the other sides.
    if (side.value.x > 2.5) { p = vec2<f32>(1.0 - p.y, p.x); }
    else if (side.value.x > 1.5) { p = vec2<f32>(p.y, 1.0 - p.x); }
    else if (side.value.x > 0.5) { p.y = 1.0 - p.y; }
    let half_width = (1.0 - p.y) * 0.5;
    let coverage = smoothstep(-0.035, 0.035, half_width - abs(p.x - 0.5));
    return vec4<f32>(fill.value.rgb, fill.value.a * coverage);
}
