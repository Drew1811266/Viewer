struct Magnifier {
    origin: vec4<f32>,
    axis_x: vec4<f32>,
    axis_y: vec4<f32>,
    viewport_physical: vec4<f32>,
    clip: vec4<f32>,
    style: vec4<f32>,
};

@group(0) @binding(0) var<uniform> magnifier: Magnifier;
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

fn source_to_clip(source: vec2<f32>) -> vec2<f32> {
    return magnifier.origin.xy
        + source.x * magnifier.axis_x.xy
        + source.y * magnifier.axis_y.xy;
}

fn inside_clip(position: vec2<f32>) -> bool {
    let delta = abs(position - magnifier.clip.xy);
    if magnifier.clip.w < 0.5 {
        return length(delta) <= magnifier.clip.z;
    }
    let radius = magnifier.style.x;
    let corner = max(delta - vec2<f32>(magnifier.clip.z - radius), vec2<f32>(0.0));
    return delta.x <= magnifier.clip.z
        && delta.y <= magnifier.clip.z
        && length(corner) <= radius;
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(source_to_clip(input.source_position), 0.0, 1.0);
    output.uv = input.uv;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    if !inside_clip(input.position.xy) {
        discard;
    }
    return textureSample(image_texture, image_sampler, input.uv);
}
