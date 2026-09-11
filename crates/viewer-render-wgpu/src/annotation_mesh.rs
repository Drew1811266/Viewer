use std::{collections::BTreeMap, error::Error, f64::consts::TAU, fmt, ops::Range};

use viewer_render_core::{
    AnnotationGeometry, AnnotationHandle, AnnotationNode, AnnotationStyle, HANDLE_RADIUS_PX,
    NormalizedPoint, NormalizedRect, ORDINAL_RADIUS_PX, SceneRevision, SceneSnapshot,
    TransformSnapshot, annotation_handles, annotation_ordinal_position,
};

const ELLIPSE_SEGMENTS: usize = 48;
const CIRCLE_SEGMENTS: usize = 16;
const BADGE_RADIUS_PX: f32 = ORDINAL_RADIUS_PX as f32;
const BADGE_RING_WIDTH_PX: f32 = 2.5;
const POINT_RADIUS_PX: f32 = 7.0;
/// Logical-pixel outset added around every solid shape so the fragment shader
/// has room for its analytic antialiasing ramp. Must match `AA_FEATHER` in
/// `shaders/annotation.wgsl`. 0.5 logical px equals 1 physical px at 2x
/// Retina scale, which is the minimum ramp width that survives 1x displays.
const EDGE_FEATHER_PX: f32 = 0.5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VertexKind {
    Segment,
    ScreenOffset,
    ScreenSquare,
    ArrowHead,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnnotationVertex {
    pub source_position: [f32; 2],
    pub neighbor_position: [f32; 2],
    pub screen_offset_px: [f32; 2],
    pub color: [f32; 4],
    pub kind: VertexKind,
    pub dashed: bool,
    pub segment_factor: f32,
    /// Nominal solid half extent in logical px (half stroke width for
    /// segments, shape radius for screen circles/squares). The rasterized
    /// geometry is outset by [`EDGE_FEATHER_PX`]; the fragment shader ramps
    /// coverage from full at this extent out to zero at the rasterized edge.
    pub edge_px: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OrdinalLabel {
    pub anchor: NormalizedPoint,
    pub text: String,
    pub geometry: AnnotationGeometry,
    pub badge_vertices: Range<usize>,
    pub badge_indices: Range<usize>,
    /// Tint for the ordinal digits drawn over the badge disc. The badge itself
    /// is a white disc with a colored ring, so digits share the ring color.
    pub color: [f32; 4],
}

impl OrdinalLabel {
    pub fn layout_anchor(&self, transform: &TransformSnapshot) -> Option<NormalizedPoint> {
        annotation_ordinal_position(&self.geometry, transform)
            .map(|position| transform.view_to_image_unclamped(position))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnnotationMesh {
    revision: SceneRevision,
    vertices: Vec<AnnotationVertex>,
    indices: Vec<u32>,
    ordinal_labels: Vec<OrdinalLabel>,
    selection_handle_count: usize,
    geometry_index_ranges: Vec<Range<u32>>,
}

impl AnnotationMesh {
    pub fn geometry_index_ranges(&self) -> &[Range<u32>] {
        &self.geometry_index_ranges
    }
    pub const fn revision(&self) -> SceneRevision {
        self.revision
    }

    pub fn vertices(&self) -> &[AnnotationVertex] {
        &self.vertices
    }

    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    pub fn ordinal_labels(&self) -> &[OrdinalLabel] {
        &self.ordinal_labels
    }

    pub const fn selection_handle_count(&self) -> usize {
        self.selection_handle_count
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnnotationMeshFragment {
    vertices: Vec<AnnotationVertex>,
    indices: Vec<u32>,
}

impl AnnotationMeshFragment {
    pub fn vertices(&self) -> &[AnnotationVertex] {
        &self.vertices
    }

    pub fn indices(&self) -> &[u32] {
        &self.indices
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnnotationMeshBuilder {
    ellipse_segments: usize,
    circle_segments: usize,
}

impl Default for AnnotationMeshBuilder {
    fn default() -> Self {
        Self {
            ellipse_segments: ELLIPSE_SEGMENTS,
            circle_segments: CIRCLE_SEGMENTS,
        }
    }
}

impl AnnotationMeshBuilder {
    pub fn build(&self, scene: &SceneSnapshot) -> Result<AnnotationMesh, MeshError> {
        let mut mesh = AnnotationMesh {
            revision: scene.revision(),
            vertices: Vec::new(),
            indices: Vec::new(),
            ordinal_labels: Vec::new(),
            selection_handle_count: 0,
            geometry_index_ranges: Vec::new(),
        };
        // Selected controls visually precede overlapping unselected geometry,
        // matching the selected-handle priority in native hit testing.
        for selected in [false, true] {
            for node in scene
                .annotations()
                .iter()
                .filter(|node| node.visible && node.selected == selected)
            {
                self.append_node(&mut mesh, node, true)?;
            }
        }
        if let Some(draft) = scene.draft().filter(|node| node.visible) {
            self.append_node(&mut mesh, draft, false)?;
        }
        Ok(mesh)
    }

    pub fn tessellate_geometry(
        &self,
        geometry: &AnnotationGeometry,
        style: AnnotationStyle,
    ) -> Result<AnnotationMeshFragment, MeshError> {
        let mut fragment = AnnotationMeshFragment::default();
        match geometry {
            AnnotationGeometry::Point { position } => {
                push_screen_circle(
                    &mut fragment,
                    *position,
                    POINT_RADIUS_PX,
                    style.color,
                    self.circle_segments,
                )?;
            }
            AnnotationGeometry::Arrow { tail, head } => {
                // The shaft's head cap stays flush with the arrowhead apex.
                push_segment_with_caps(&mut fragment, *tail, *head, style, true, false)?;
                push_arrow_head(&mut fragment, *tail, *head, style.color)?;
            }
            AnnotationGeometry::Rectangle { rect } => {
                let corners = rect_corners(*rect);
                for index in 0..4 {
                    push_segment(
                        &mut fragment,
                        corners[index],
                        corners[(index + 1) % 4],
                        style,
                    )?;
                }
            }
            AnnotationGeometry::Ellipse { rect } => {
                if self.ellipse_segments < 3 {
                    return Err(MeshError::InvalidTessellation);
                }
                let center_x = rect.x + rect.width / 2.0;
                let center_y = rect.y + rect.height / 2.0;
                let radius_x = rect.width / 2.0;
                let radius_y = rect.height / 2.0;
                let points = (0..self.ellipse_segments)
                    .map(|index| {
                        let angle = TAU * index as f64 / self.ellipse_segments as f64;
                        NormalizedPoint {
                            x: center_x + radius_x * angle.cos(),
                            y: center_y + radius_y * angle.sin(),
                        }
                    })
                    .collect::<Vec<_>>();
                for index in 0..points.len() {
                    push_segment(
                        &mut fragment,
                        points[index],
                        points[(index + 1) % points.len()],
                        style,
                    )?;
                }
            }
            AnnotationGeometry::Stroke { points } => {
                if points.len() < 2 {
                    return Err(MeshError::DegenerateSegment);
                }
                for pair in points.windows(2) {
                    if pair[0] != pair[1] {
                        push_segment(&mut fragment, pair[0], pair[1], style)?;
                    }
                }
                if fragment.indices.is_empty() {
                    return Err(MeshError::DegenerateSegment);
                }
            }
        }
        Ok(fragment)
    }

    fn append_node(
        &self,
        mesh: &mut AnnotationMesh,
        node: &AnnotationNode,
        include_badge: bool,
    ) -> Result<(), MeshError> {
        let fragment = self.tessellate_geometry(&node.geometry, node.style)?;
        let geometry_start = mesh.indices.len() as u32;
        append_fragment(mesh, fragment)?;
        mesh.geometry_index_ranges
            .push(geometry_start..mesh.indices.len() as u32);

        if node.selected {
            for (_, handle) in annotation_handles(&node.geometry)
                .into_iter()
                .filter(|(kind, _)| *kind != AnnotationHandle::Point)
            {
                let mut fragment = AnnotationMeshFragment::default();
                if matches!(node.geometry, AnnotationGeometry::Arrow { .. }) {
                    push_screen_circle(
                        &mut fragment,
                        handle,
                        HANDLE_RADIUS_PX as f32,
                        node.style.color,
                        self.circle_segments,
                    )?;
                    push_screen_circle(
                        &mut fragment,
                        handle,
                        (HANDLE_RADIUS_PX - 2.0) as f32,
                        [1.0; 4],
                        self.circle_segments,
                    )?;
                } else {
                    push_screen_square(
                        &mut fragment,
                        handle,
                        HANDLE_RADIUS_PX as f32,
                        node.style.color,
                    )?;
                    push_screen_square(
                        &mut fragment,
                        handle,
                        (HANDLE_RADIUS_PX - 2.0) as f32,
                        [1.0; 4],
                    )?;
                }
                append_fragment(mesh, fragment)?;
                mesh.selection_handle_count += 1;
            }
        }

        if include_badge {
            let anchor = geometry_anchor(&node.geometry);
            let badge_start = mesh.vertices.len();
            let badge_index_start = mesh.indices.len();
            let mut badge = AnnotationMeshFragment::default();
            // White disc with a colored ring: the ring is the full-radius disc
            // in the node color, overpainted by a smaller white disc. Both
            // edges get the shader's analytic AA ramp, so the ring stays crisp
            // on any backdrop.
            push_screen_circle(
                &mut badge,
                anchor,
                BADGE_RADIUS_PX,
                node.style.color,
                self.circle_segments,
            )?;
            push_screen_circle(
                &mut badge,
                anchor,
                BADGE_RADIUS_PX - BADGE_RING_WIDTH_PX,
                [1.0; 4],
                self.circle_segments,
            )?;
            append_fragment(mesh, badge)?;
            mesh.ordinal_labels.push(OrdinalLabel {
                anchor,
                text: node.ordinal.to_string(),
                geometry: node.geometry.clone(),
                badge_vertices: badge_start..mesh.vertices.len(),
                badge_indices: badge_index_start..mesh.indices.len(),
                color: node.style.color,
            });
        }
        Ok(())
    }
}

fn append_fragment(
    mesh: &mut AnnotationMesh,
    fragment: AnnotationMeshFragment,
) -> Result<(), MeshError> {
    let base = u32::try_from(mesh.vertices.len()).map_err(|_| MeshError::MeshTooLarge)?;
    mesh.vertices.extend(fragment.vertices);
    mesh.indices.extend(
        fragment
            .indices
            .into_iter()
            .map(|index| index.checked_add(base).ok_or(MeshError::MeshTooLarge))
            .collect::<Result<Vec<_>, _>>()?,
    );
    Ok(())
}

fn push_segment(
    fragment: &mut AnnotationMeshFragment,
    start: NormalizedPoint,
    end: NormalizedPoint,
    style: AnnotationStyle,
) -> Result<(), MeshError> {
    push_segment_with_caps(fragment, start, end, style, true, true)
}

/// Emits one stroke quad. Both ends are extended along the segment direction
/// by half a stroke width plus the AA feather so adjacent segments overlap
/// and corners stay solid under the coverage ramp (flat caps would leave a
/// visible notch at every joint). The arrow shaft keeps its head cap flush
/// with the arrowhead apex so the extension cannot poke past the triangle.
fn push_segment_with_caps(
    fragment: &mut AnnotationMeshFragment,
    start: NormalizedPoint,
    end: NormalizedPoint,
    style: AnnotationStyle,
    extend_start: bool,
    extend_end: bool,
) -> Result<(), MeshError> {
    if start == end {
        return Err(MeshError::DegenerateSegment);
    }
    let half_width = style.line_width_px.max(1.0) / 2.0;
    // The quad is outset so the fragment AA ramp has room beyond the nominal
    // edge; `edge_px` carries the un-outset half width for the ramp.
    let side = half_width + EDGE_FEATHER_PX;
    // Offsets are expressed in the global start→end segment frame (the
    // vertex shader un-flips the tangent for the end pair via segment_factor,
    // keeping screen_offset_px linear against the true geometry). The ends
    // extend past their segment endpoints so adjacent segments overlap and
    // corners stay solid under the coverage ramp; the arrow shaft's head end
    // stays flush with the arrowhead apex.
    let along = if extend_start {
        half_width + EDGE_FEATHER_PX
    } else {
        0.0
    };
    let along_end = if extend_end {
        half_width + EDGE_FEATHER_PX
    } else {
        0.0
    };
    let base = u32::try_from(fragment.vertices.len()).map_err(|_| MeshError::MeshTooLarge)?;
    let start = [start.x as f32, start.y as f32];
    let end = [end.x as f32, end.y as f32];
    fragment.vertices.extend([
        segment_vertex(
            start,
            end,
            -side,
            -along,
            style.color,
            style.dashed,
            0.0,
            half_width,
        ),
        segment_vertex(
            start,
            end,
            side,
            -along,
            style.color,
            style.dashed,
            0.0,
            half_width,
        ),
        // Perimeter order matters for the [0,1,2, 0,2,3] triangulation: the
        // end pair lists the +side corner first so the shared diagonal runs
        // start(-side) → end(+side). Pair it with the vertex shader's
        // segment_factor tangent un-flip, which keeps screen_offset_px linear
        // against the real geometry for the fragment AA ramp.
        segment_vertex(
            end,
            start,
            side,
            along_end,
            style.color,
            style.dashed,
            1.0,
            half_width,
        ),
        segment_vertex(
            end,
            start,
            -side,
            along_end,
            style.color,
            style.dashed,
            1.0,
            half_width,
        ),
    ]);
    fragment
        .indices
        .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn segment_vertex(
    source_position: [f32; 2],
    neighbor_position: [f32; 2],
    side_px: f32,
    along_px: f32,
    color: [f32; 4],
    dashed: bool,
    segment_factor: f32,
    edge_px: f32,
) -> AnnotationVertex {
    AnnotationVertex {
        source_position,
        neighbor_position,
        screen_offset_px: [side_px, along_px],
        color,
        kind: VertexKind::Segment,
        dashed,
        segment_factor,
        edge_px,
    }
}

fn push_arrow_head(
    fragment: &mut AnnotationMeshFragment,
    tail: NormalizedPoint,
    head: NormalizedPoint,
    color: [f32; 4],
) -> Result<(), MeshError> {
    if tail == head {
        return Err(MeshError::DegenerateSegment);
    }
    let base = u32::try_from(fragment.vertices.len()).map_err(|_| MeshError::MeshTooLarge)?;
    fragment.vertices.extend([
        arrow_head_vertex(tail, head, [0.0, 0.0], color),
        arrow_head_vertex(tail, head, [7.0, -14.0], color),
        arrow_head_vertex(tail, head, [-7.0, -14.0], color),
    ]);
    fragment.indices.extend([base, base + 1, base + 2]);
    Ok(())
}

fn arrow_head_vertex(
    tail: NormalizedPoint,
    head: NormalizedPoint,
    screen_offset_px: [f32; 2],
    color: [f32; 4],
) -> AnnotationVertex {
    AnnotationVertex {
        source_position: [head.x as f32, head.y as f32],
        neighbor_position: [tail.x as f32, tail.y as f32],
        screen_offset_px,
        color,
        kind: VertexKind::ArrowHead,
        dashed: false,
        segment_factor: 0.0,
        edge_px: 0.0,
    }
}

fn push_screen_circle(
    fragment: &mut AnnotationMeshFragment,
    center: NormalizedPoint,
    radius_px: f32,
    color: [f32; 4],
    segments: usize,
) -> Result<(), MeshError> {
    if segments < 3 {
        return Err(MeshError::InvalidTessellation);
    }
    // Rim outset provides the fragment AA ramp room beyond the nominal edge.
    let rim = radius_px + EDGE_FEATHER_PX;
    for index in 0..segments {
        let start_angle = TAU * index as f64 / segments as f64;
        let end_angle = TAU * (index + 1) as f64 / segments as f64;
        let base = u32::try_from(fragment.vertices.len()).map_err(|_| MeshError::MeshTooLarge)?;
        fragment.vertices.extend([
            fixed_vertex(center, [0.0, 0.0], color, radius_px),
            fixed_vertex(
                center,
                [
                    rim * start_angle.cos() as f32,
                    rim * start_angle.sin() as f32,
                ],
                color,
                radius_px,
            ),
            fixed_vertex(
                center,
                [rim * end_angle.cos() as f32, rim * end_angle.sin() as f32],
                color,
                radius_px,
            ),
        ]);
        fragment.indices.extend([base, base + 1, base + 2]);
    }
    Ok(())
}

fn push_screen_square(
    fragment: &mut AnnotationMeshFragment,
    center: NormalizedPoint,
    radius_px: f32,
    color: [f32; 4],
) -> Result<(), MeshError> {
    let rim = radius_px + EDGE_FEATHER_PX;
    let base = u32::try_from(fragment.vertices.len()).map_err(|_| MeshError::MeshTooLarge)?;
    fragment.vertices.extend([
        screen_square_vertex(center, [-rim, -rim], color, radius_px),
        screen_square_vertex(center, [rim, -rim], color, radius_px),
        screen_square_vertex(center, [rim, rim], color, radius_px),
        screen_square_vertex(center, [-rim, rim], color, radius_px),
    ]);
    fragment
        .indices
        .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    Ok(())
}

fn fixed_vertex(
    center: NormalizedPoint,
    offset: [f32; 2],
    color: [f32; 4],
    edge_px: f32,
) -> AnnotationVertex {
    let source_position = [center.x as f32, center.y as f32];
    AnnotationVertex {
        source_position,
        neighbor_position: source_position,
        screen_offset_px: offset,
        color,
        kind: VertexKind::ScreenOffset,
        dashed: false,
        segment_factor: 0.0,
        edge_px,
    }
}

fn screen_square_vertex(
    center: NormalizedPoint,
    offset: [f32; 2],
    color: [f32; 4],
    edge_px: f32,
) -> AnnotationVertex {
    let mut vertex = fixed_vertex(center, offset, color, edge_px);
    vertex.kind = VertexKind::ScreenSquare;
    vertex
}

fn geometry_anchor(geometry: &AnnotationGeometry) -> NormalizedPoint {
    match geometry {
        AnnotationGeometry::Point { position } => *position,
        AnnotationGeometry::Arrow { head, .. } => *head,
        AnnotationGeometry::Rectangle { rect } | AnnotationGeometry::Ellipse { rect } => {
            NormalizedPoint {
                x: rect.x + rect.width,
                y: rect.y,
            }
        }
        AnnotationGeometry::Stroke { points } => points[points.len() - 1],
    }
}

fn rect_corners(rect: NormalizedRect) -> [NormalizedPoint; 4] {
    [
        NormalizedPoint {
            x: rect.x,
            y: rect.y,
        },
        NormalizedPoint {
            x: rect.x + rect.width,
            y: rect.y,
        },
        NormalizedPoint {
            x: rect.x + rect.width,
            y: rect.y + rect.height,
        },
        NormalizedPoint {
            x: rect.x,
            y: rect.y + rect.height,
        },
    ]
}

#[derive(Clone, Debug, Default)]
pub struct AnnotationMeshCache {
    mesh: Option<AnnotationMesh>,
    rebuild_count: u64,
    builder: AnnotationMeshBuilder,
    authoritative: bool,
}

/// Keeps high-frequency renderer-owned draft geometry physically separate
/// from the retained authoritative scene. Updating a draft therefore cannot
/// invalidate or rebuild hundreds of committed annotations.
#[derive(Clone, Debug, Default)]
pub struct AnnotationMeshLayers {
    authoritative: AnnotationMeshCache,
    draft: AnnotationMeshCache,
}

impl AnnotationMeshLayers {
    pub fn update_authoritative(&mut self, scene: &SceneSnapshot) -> Result<MeshUpdate, MeshError> {
        self.authoritative.update(scene)
    }

    pub fn update_transient_authoritative(
        &mut self,
        scene: &SceneSnapshot,
    ) -> Result<MeshUpdate, MeshError> {
        self.authoritative.update_transient(scene)
    }

    pub fn update_draft(&mut self, scene: &SceneSnapshot) -> Result<MeshUpdate, MeshError> {
        self.draft.update_transient(scene)
    }

    pub const fn authoritative(&self) -> &AnnotationMeshCache {
        &self.authoritative
    }

    pub const fn draft(&self) -> &AnnotationMeshCache {
        &self.draft
    }
}

impl AnnotationMeshCache {
    pub fn update(&mut self, scene: &SceneSnapshot) -> Result<MeshUpdate, MeshError> {
        if self.authoritative
            && self.mesh.as_ref().map(AnnotationMesh::revision) == Some(scene.revision())
        {
            return Ok(MeshUpdate::Reused);
        }
        self.mesh = Some(self.builder.build(scene)?);
        self.rebuild_count = self.rebuild_count.saturating_add(1);
        self.authoritative = true;
        Ok(MeshUpdate::Rebuilt)
    }

    /// Rebuilds a renderer-owned interaction projection without claiming the
    /// authoritative scene revision. A later authoritative snapshot with the
    /// same revision must therefore rebuild instead of reusing this mesh.
    pub fn update_transient(&mut self, scene: &SceneSnapshot) -> Result<MeshUpdate, MeshError> {
        self.mesh = Some(self.builder.build(scene)?);
        self.rebuild_count = self.rebuild_count.saturating_add(1);
        self.authoritative = false;
        Ok(MeshUpdate::Rebuilt)
    }

    pub const fn mesh(&self) -> Option<&AnnotationMesh> {
        self.mesh.as_ref()
    }

    pub const fn rebuild_count(&self) -> u64 {
        self.rebuild_count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeshUpdate {
    Rebuilt,
    Reused,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BufferCapacityPlan {
    pub vertex_capacity: usize,
    pub index_capacity: usize,
}

impl BufferCapacityPlan {
    pub fn ensure(&mut self, vertices: usize, indices: usize) -> bool {
        let next_vertices = capacity_for(vertices);
        let next_indices = capacity_for(indices);
        let changed = next_vertices > self.vertex_capacity || next_indices > self.index_capacity;
        self.vertex_capacity = self.vertex_capacity.max(next_vertices);
        self.index_capacity = self.index_capacity.max(next_indices);
        changed
    }
}

fn capacity_for(required: usize) -> usize {
    if required == 0 {
        0
    } else {
        required.next_power_of_two()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphMetrics {
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    pub size_px: [f32; 2],
    pub bearing_px: [f32; 2],
    pub advance_px: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OrdinalGlyphAtlas {
    width: u32,
    height: u32,
    pixels: viewer_render_core::SharedPixels,
    metrics: BTreeMap<char, GlyphMetrics>,
}

impl OrdinalGlyphAtlas {
    pub fn new(
        width: u32,
        height: u32,
        pixels: Vec<u8>,
        metrics: BTreeMap<char, GlyphMetrics>,
    ) -> Result<Self, GlyphAtlasError> {
        if width == 0 || height == 0 {
            return Err(GlyphAtlasError::EmptyDimensions);
        }
        let expected = usize::try_from(u64::from(width) * u64::from(height))
            .map_err(|_| GlyphAtlasError::SizeOverflow)?;
        if pixels.len() != expected {
            return Err(GlyphAtlasError::PixelLength {
                expected,
                actual: pixels.len(),
            });
        }
        let memory = viewer_render_core::ImageMemoryCoordinator::new(
            viewer_render_core::ImageMemoryPolicy::baseline_8gb(),
        );
        Self::from_shared(
            width,
            height,
            viewer_render_core::SharedPixels::try_copy_from_slice(
                &memory,
                viewer_render_core::AssetGeneration(0),
                &pixels,
            )?,
            metrics,
        )
    }

    pub fn from_shared(
        width: u32,
        height: u32,
        pixels: viewer_render_core::SharedPixels,
        metrics: BTreeMap<char, GlyphMetrics>,
    ) -> Result<Self, GlyphAtlasError> {
        if width == 0 || height == 0 {
            return Err(GlyphAtlasError::EmptyDimensions);
        }
        let expected = usize::try_from(u64::from(width) * u64::from(height))
            .map_err(|_| GlyphAtlasError::SizeOverflow)?;
        if pixels.len() != expected {
            return Err(GlyphAtlasError::PixelLength {
                expected,
                actual: pixels.len(),
            });
        }
        for digit in '0'..='9' {
            if !metrics.contains_key(&digit) {
                return Err(GlyphAtlasError::MissingDigit(digit));
            }
        }
        Ok(Self {
            width,
            height,
            pixels,
            metrics,
        })
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub(crate) fn accounted(
        &self,
        memory: &viewer_render_core::ImageMemoryCoordinator,
    ) -> Result<Self, GlyphAtlasError> {
        if self.pixels.is_accounted_by(memory) {
            return Ok(self.clone());
        }
        let pixels = viewer_render_core::SharedPixels::try_copy_from_slice(
            memory,
            viewer_render_core::AssetGeneration(0),
            &self.pixels,
        )?;
        Self::from_shared(self.width, self.height, pixels, self.metrics.clone())
    }
    pub fn metrics(&self, glyph: char) -> Option<&GlyphMetrics> {
        self.metrics.get(&glyph)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GlyphAtlasError {
    Memory(viewer_render_core::MemoryAdmissionError),
    EmptyDimensions,
    SizeOverflow,
    PixelLength { expected: usize, actual: usize },
    MissingDigit(char),
}

impl fmt::Display for GlyphAtlasError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid ordinal glyph atlas: {self:?}")
    }
}

impl Error for GlyphAtlasError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeshError {
    Memory(viewer_render_core::MemoryAdmissionError),
    DegenerateSegment,
    InvalidTessellation,
    MeshTooLarge,
}

impl fmt::Display for MeshError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "annotation mesh error: {self:?}")
    }
}

impl Error for MeshError {}

impl From<viewer_render_core::MemoryAdmissionError> for MeshError {
    fn from(error: viewer_render_core::MemoryAdmissionError) -> Self {
        Self::Memory(error)
    }
}
impl From<viewer_render_core::MemoryAdmissionError> for GlyphAtlasError {
    fn from(error: viewer_render_core::MemoryAdmissionError) -> Self {
        Self::Memory(error)
    }
}
