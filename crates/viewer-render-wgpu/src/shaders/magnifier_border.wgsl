struct Magnifier {
    origin: vec4<f32>,
    axis_x: vec4<f32>,
    axis_y: vec4<f32>,
    viewport_physical: vec4<f32>,
    clip: vec4<f32>,
    style: vec4<f32>,
};

@group(0) @binding(0) var<uniform> magnifier: Magnifier;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
};

fn signed_distance(position: vec2<f32>, inset: f32) -> f32 {
    let half_size = magnifier.clip.zw - vec2<f32>(inset);
    let delta = abs(position - magnifier.clip.xy);
    if magnifier.style.z < 0.5 {
        return length(delta) - min(half_size.x, half_size.y);
    }
    let radius = max(magnifier.style.x - inset, 0.0);
    let corner = max(delta - (half_size - vec2<f32>(radius)), vec2<f32>(0.0));
    return length(corner) - radius;
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );
    let physical = magnifier.clip.xy + corners[vertex_index] * magnifier.clip.zw;
    let clip = vec2<f32>(
        physical.x * 2.0 / magnifier.viewport_physical.x - 1.0,
        1.0 - physical.y * 2.0 / magnifier.viewport_physical.y,
    );
    var output: VertexOutput;
    output.position = vec4<f32>(clip, 0.0, 1.0);
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let outer = signed_distance(input.position.xy, 0.0);
    let inner = signed_distance(input.position.xy, magnifier.style.y);
    if outer > 0.0 || inner <= 0.0 {
        discard;
    }
    return vec4<f32>(0.45, 0.49, 0.56, 1.0);
}
