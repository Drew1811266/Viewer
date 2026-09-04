struct Magnifier {
    origin: vec4<f32>,
    axis_x: vec4<f32>,
    axis_y: vec4<f32>,
    viewport_physical: vec4<f32>,
    clip: vec4<f32>,
    style: vec4<f32>,
};

@group(0) @binding(0) var<uniform> magnifier: Magnifier;
@group(1) @binding(0) var glyph_atlas: texture_2d<f32>;
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
    let clip_offset = vec2<f32>(
        input.screen_offset_px.x * 2.0 / magnifier.viewport_physical.x,
        -input.screen_offset_px.y * 2.0 / magnifier.viewport_physical.y,
    );
    output.position = vec4<f32>(center + clip_offset, 0.0, 1.0);
    output.uv = input.uv;
    output.color = input.color;
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    if !inside_clip(input.position.xy) {
        discard;
    }
    let coverage = textureSample(glyph_atlas, glyph_sampler, input.uv).r;
    return vec4<f32>(input.color.rgb * coverage, input.color.a * coverage);
}
