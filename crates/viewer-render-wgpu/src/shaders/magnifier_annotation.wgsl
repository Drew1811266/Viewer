struct Magnifier {
    origin: vec4<f32>,
    axis_x: vec4<f32>,
    axis_y: vec4<f32>,
    viewport_physical: vec4<f32>,
    clip: vec4<f32>,
    style: vec4<f32>,
};

@group(0) @binding(0) var<uniform> magnifier: Magnifier;

struct VertexInput {
    @location(0) source_position: vec2<f32>,
    @location(1) neighbor_position: vec2<f32>,
    @location(2) screen_offset_px: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) kind: f32,
    @location(5) dashed: f32,
    @location(6) segment_factor: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) @interpolate(flat) dashed: f32,
    @location(2) segment_distance_px: f32,
};

fn source_to_clip(source: vec2<f32>) -> vec2<f32> {
    return magnifier.origin.xy
        + source.x * magnifier.axis_x.xy
        + source.y * magnifier.axis_y.xy;
}

fn inside_clip(position: vec2<f32>) -> bool {
    let delta = abs(position - magnifier.clip.xy);
    if magnifier.style.z < 0.5 {
        return length(delta) <= min(magnifier.clip.z, magnifier.clip.w);
    }
    let radius = magnifier.style.x;
    let half_size = magnifier.clip.zw;
    let corner = max(delta - (half_size - vec2<f32>(radius)), vec2<f32>(0.0));
    return delta.x <= half_size.x
        && delta.y <= half_size.y
        && length(corner) <= radius;
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    let center = source_to_clip(input.source_position);
    var offset_px = input.screen_offset_px;
    var segment_length_px = 0.0;
    if input.kind < 0.5 || input.kind > 1.5 {
        let neighbor = source_to_clip(input.neighbor_position);
        var pixel_delta = (neighbor - center)
            * vec2<f32>(magnifier.viewport_physical.x * 0.5, -magnifier.viewport_physical.y * 0.5);
        if input.kind > 1.5 {
            pixel_delta = -pixel_delta;
        }
        let segment_length = max(length(pixel_delta), 0.0001);
        segment_length_px = segment_length;
        let tangent = pixel_delta / segment_length;
        let normal = vec2<f32>(-tangent.y, tangent.x);
        offset_px = normal * input.screen_offset_px.x + tangent * input.screen_offset_px.y;
    }
    let clip_offset = vec2<f32>(
        offset_px.x * 2.0 / magnifier.viewport_physical.x,
        -offset_px.y * 2.0 / magnifier.viewport_physical.y,
    );
    output.position = vec4<f32>(center + clip_offset, 0.0, 1.0);
    output.color = input.color;
    output.dashed = input.dashed;
    output.segment_distance_px = input.segment_factor * segment_length_px;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    if !inside_clip(input.position.xy) {
        discard;
    }
    if input.dashed > 0.5 && (input.segment_distance_px % 10.0) > 6.0 {
        discard;
    }
    return input.color;
}
