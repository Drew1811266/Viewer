struct Camera {
    origin: vec4<f32>,
    axis_x: vec4<f32>,
    axis_y: vec4<f32>,
    viewport_physical: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var glyph_texture: texture_2d<f32>;
@group(1) @binding(1) var glyph_sampler: sampler;

struct VertexInput {
    @location(0) source_position: vec2<f32>,
    @location(1) screen_offset_px: vec2<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    let center = camera.origin.xy
        + input.source_position.x * camera.axis_x.xy
        + input.source_position.y * camera.axis_y.xy;
    let clip_offset = vec2<f32>(
        input.screen_offset_px.x * 2.0 / camera.viewport_physical.x,
        -input.screen_offset_px.y * 2.0 / camera.viewport_physical.y,
    );
    output.position = vec4<f32>(center + clip_offset, 0.0, 1.0);
    output.uv = input.uv;
    output.color = input.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let alpha = textureSample(glyph_texture, glyph_sampler, input.uv).r * input.color.a;
    return vec4<f32>(input.color.rgb * alpha, alpha);
}
