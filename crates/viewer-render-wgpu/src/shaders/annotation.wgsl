struct Camera {
    origin: vec4<f32>,
    axis_x: vec4<f32>,
    axis_y: vec4<f32>,
    viewport_physical: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera: Camera;

// Logical-pixel width of the analytic antialiasing ramp around every solid
// shape edge. Must match EDGE_FEATHER_PX in annotation_mesh.rs, which outsets
// the rasterized geometry by the same amount. 0.5 logical px equals one
// physical pixel at 2x Retina scale.
const AA_FEATHER: f32 = 0.5;

struct VertexInput {
    @location(0) source_position: vec2<f32>,
    @location(1) neighbor_position: vec2<f32>,
    @location(2) screen_offset_px: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) kind: f32,
    @location(5) dashed: f32,
    @location(6) segment_factor: f32,
    @location(7) edge_px: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) @interpolate(flat) dashed: f32,
    @location(2) segment_distance_px: f32,
    @location(3) @interpolate(flat) kind: f32,
    // x: nominal solid half extent in logical px (edge_px).
    // yz: shape-space coordinate in logical px — for segments y is the signed
    // perpendicular distance from the centerline; for screen circles/squares
    // yz is the offset from the shape center. Arrowheads carry zeros (no
    // analytic ramp yet; they stay hard-edged).
    @location(4) edge: vec3<f32>,
};

fn source_to_clip(source: vec2<f32>) -> vec2<f32> {
    return camera.origin.xy
        + source.x * camera.axis_x.xy
        + source.y * camera.axis_y.xy;
}

fn cross2d(a: vec2<f32>, b: vec2<f32>) -> f32 {
    return a.x * b.y - a.y * b.x;
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
        // Rebuild a single global start→end frame per segment: the mesh emits
        // the end vertex pair with source=end, neighbor=start, so un-flip the
        // tangent for that pair (segment_factor 1.0). Without this, the
        // pair's normal flips and screen_offset_px stops being linear against
        // the real geometry, which the fragment AA ramp reads directly.
        // Arrowheads keep their own frame flip (tangent toward the head).
        if input.kind > 1.5 || (input.kind < 0.5 && input.segment_factor > 0.5) {
            pixel_delta = -pixel_delta;
        }
        let length = max(length(pixel_delta), 0.0001);
        segment_length_px = length;
        let tangent = pixel_delta / length;
        let normal = vec2<f32>(-tangent.y, tangent.x);
        offset_px = normal * input.screen_offset_px.x + tangent * input.screen_offset_px.y;
    }
    offset_px *= max(camera.viewport_physical.z, 1.0);
    let clip_offset = vec2<f32>(
        offset_px.x * 2.0 / camera.viewport_physical.x,
        -offset_px.y * 2.0 / camera.viewport_physical.y,
    );
    output.position = vec4<f32>(center + clip_offset, 0.0, 1.0);
    output.color = input.color;
    output.dashed = input.dashed;
    output.segment_distance_px = input.segment_factor * segment_length_px;
    output.kind = input.kind;
    if input.kind < 0.5 {
        // Segment quads offset perpendicular to the stroke: the interpolated
        // screen_offset_px.x is the signed distance from the centerline.
        output.edge = vec3<f32>(input.edge_px, input.screen_offset_px.x, 0.0);
    } else if input.kind < 1.75 {
        // Screen circles and squares keep their raw center-relative offset.
        output.edge = vec3<f32>(input.edge_px, input.screen_offset_px);
    } else {
        // Arrowheads carry the raw (pre-rotation) offset so the fragment
        // shader can evaluate the triangle SDF in the mesh's local frame.
        output.edge = vec3<f32>(0.0, input.screen_offset_px);
    }
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    if input.dashed > 0.5 && (input.segment_distance_px % 10.0) > 6.0 {
        discard;
    }
    var coverage = 1.0;
    if input.kind < 0.5 {
        // Stroke edge: full coverage at the nominal half width, ramping to
        // zero across the rasterized feather band.
        let d = abs(input.edge.y);
        coverage = clamp((input.edge.x + AA_FEATHER - d) / (2.0 * AA_FEATHER), 0.0, 1.0);
    } else if input.kind < 1.5 {
        // Screen circle: Euclidean distance from the center.
        let d = length(input.edge.yz);
        coverage = clamp((input.edge.x + AA_FEATHER - d) / (2.0 * AA_FEATHER), 0.0, 1.0);
    } else if input.kind < 1.75 {
        // Screen square: Chebyshev distance from the center.
        let d = max(abs(input.edge.y), abs(input.edge.z));
        coverage = clamp((input.edge.x + AA_FEATHER - d) / (2.0 * AA_FEATHER), 0.0, 1.0);
    } else {
        // Arrowhead triangle SDF, evaluated in the mesh's local frame
        // (apex at the head point, base 14px back, 7px half width). Rotation
        // preserves distances, so the local frame is sufficient. Tips round
        // slightly because the perpendicular line distance underestimates the
        // true distance near vertices — visually a subtle rounding.
        let p = input.edge.yz;
        let p0 = vec2<f32>(0.0, 0.0);
        let p1 = vec2<f32>(7.0, -14.0);
        let p2 = vec2<f32>(-7.0, -14.0);
        let e0 = cross2d(p1 - p0, p - p0) / length(p1 - p0);
        let e1 = cross2d(p2 - p1, p - p1) / length(p2 - p1);
        let e2 = cross2d(p0 - p2, p - p2) / length(p0 - p2);
        let max_e = max(e0, max(e1, e2));
        let min_e = min(e0, min(e1, e2));
        let inside = max_e <= 0.0 || min_e >= 0.0;
        let d = min(abs(e0), min(abs(e1), abs(e2)));
        let signed_d = select(d, -d, inside);
        coverage = clamp(0.5 - signed_d / (2.0 * AA_FEATHER), 0.0, 1.0);
    }
    // Colors arrive opaque/premultiplied; scale both terms so the premultiplied
    // blend sees a coverage-weighted fragment.
    let alpha = input.color.a * coverage;
    return vec4<f32>(input.color.rgb * coverage, alpha);
}
