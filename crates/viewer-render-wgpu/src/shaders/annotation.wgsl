struct Camera {
    origin: vec4<f32>,
    axis_x: vec4<f32>,
    axis_y: vec4<f32>,
    viewport_physical: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: Camera;

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
    return camera.origin.xy
        + source.x * camera.axis_x.xy
        + source.y * camera.axis_y.xy;
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
            * vec2<f32>(camera.viewport_physical.x * 0.5, -camera.viewport_physical.y * 0.5);
        if input.kind > 1.5 {
            pixel_delta = -pixel_delta;
        }
        let length = max(length(pixel_delta), 0.0001);
        segment_length_px = length;
        let tangent = pixel_delta / length;
        let normal = vec2<f32>(-tangent.y, tangent.x);
        offset_px = normal * input.screen_offset_px.x + tangent * input.screen_offset_px.y;
    }
    let clip_offset = vec2<f32>(
        offset_px.x * 2.0 / camera.viewport_physical.x,
        -offset_px.y * 2.0 / camera.viewport_physical.y,
    );
    output.position = vec4<f32>(center + clip_offset, 0.0, 1.0);
    output.color = input.color;
    output.dashed = input.dashed;
    output.segment_distance_px = input.segment_factor * segment_length_px;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    if input.dashed > 0.5 && (input.segment_distance_px % 10.0) > 6.0 {
        discard;
    }
    return input.color;
}
