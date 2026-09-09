struct Camera {
    origin: vec4<f32>,
    axis_x: vec4<f32>,
    axis_y: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var image_sampler: sampler;
@group(1) @binding(0) var image_texture: texture_2d<f32>;

struct VertexInput {
    @location(0) source_position: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    let clip_position = camera.origin.xy
        + input.source_position.x * camera.axis_x.xy
        + input.source_position.y * camera.axis_y.xy;
    output.position = vec4<f32>(clip_position, 0.0, 1.0);
    output.uv = input.uv;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(image_texture, image_sampler, input.uv);
    // Match the render pass's linear 0.95 background exactly. Each LOD
    // replaces coverage; source-over across LODs would apply alpha twice.
    return vec4<f32>(color.rgb + vec3<f32>(0.95) * (1.0 - color.a), 1.0);
}
