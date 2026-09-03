# Viewer Unified Image Markup Tools Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add point, arrow, and ellipse review anchors behind one extensible “标记” menu while preserving precise viewport behavior, fast durable saves, strict Agent-readable semantics, and all existing review data.

**Architecture:** Extend `FeedbackAnchor` end to end, introduce a strict `viewer.review/4` wire contract without changing v3 bytes, and promote a review stream to v4 only when its first extended anchor is durably authored. In the UI, separate active-tool selection from editor phase, centralize normalized geometry and hit testing, and render every image anchor through the existing shared annotation scene used by both the stage and composite magnifier.

**Tech Stack:** Rust 2024 workspace, serde/serde_json, SQLite authoring store, macOS Core Graphics evidence renderer, Tauri 2, React 19, TypeScript 6, Canvas 2D, Vitest/Testing Library, Node test runner, JSON Schema.

**Spec:** `docs/superpowers/specs/2026-09-02-viewer-unified-image-markup-tools-design.md`

## Global Constraints

- New tools are exactly point, arrow, and ellipse; keep brush, rectangle, and whole-image feedback.
- Toolbar entry is a fixed “标记” menu ordered 点、箭头、画笔、矩形、椭圆.
- Shortcuts are `P`, `A`, `B`, `R`, `O`, and `V`; Space is temporary pan; Shift constrains ellipse creation to a visual circle.
- Natural-language feedback remains authoritative; geometry and text save atomically as one Feedback.
- Persist image coordinates in normalized oriented-source space; never persist viewport pixels, display size, zoom, pan, marker color, line width, or ordinal placement.
- `imageArrow.head` is the target and `tail` supplies direction/context.
- Keep `viewer.review/3` schemas and encoded bytes unchanged; extended anchors require `viewer.review/4`.
- A stream promotes v3 → v4 once and never downgrades. Opening or merely reading a v3 project performs no write.
- A successful foreground acknowledgement is durable authoring; evidence materialization and Agent publication remain asynchronous.
- Current Agent reads succeed only when `authoring_head == published_head`; never expose drafts or partial publications.
- Normal authoring command payloads at or below 64 KiB retain p50 ≤ 50 ms, p95 ≤ 150 ms, and p99 ≤ 300 ms targets.
- Reuse the existing visual system and Lucide asset convention; add no runtime dependency and no runtime plugin system.
- Viewer remains in early development. Signing, notarization, formal installers, store listing, public release, and sales are outside this plan.

---

## File Structure

### New files

- `crates/viewer-infrastructure/src/review/protocol/continuous_version.rs` — shared v3/v4 protocol discriminator and anchor capability policy.
- `crates/viewer-infrastructure/src/review/protocol/v4/mod.rs` — thin public v4 encode/decode wrappers over the shared continuous-review wire implementation.
- `docs/protocol/viewer-review-state-v4.schema.json` — strict v4 state contract with all eight Anchor variants.
- `docs/protocol/viewer-review-index-v4.schema.json` — strict v4 index contract.
- `docs/protocol/viewer-review-archive-v4.schema.json` — strict v4 archive contract.
- `docs/protocol/viewer-review-read-result-v4.schema.json` — strict v4 native-reader result contract.
- `ui/src/app/review/annotationToolRegistry.ts` — stable tool metadata, ordering, shortcuts, icons, and gesture kinds.
- `ui/src/app/review/annotationToolRegistry.test.ts` — registry uniqueness and shortcut tests.
- `ui/src/app/review/annotationHitTest.ts` — pure screen-tolerance-aware hit testing and deterministic priority.
- `ui/src/app/review/annotationHitTest.test.ts` — overlap, zoom, endpoint, path, rectangle, and ellipse hit tests.
- `ui/src/components/review/AnnotationToolMenu.tsx` — accessible fixed-label menu.
- `ui/src/components/review/AnnotationToolMenu.test.tsx` — menu focus, selection, outside click, and read-only tests.
- `ui/src/assets/icons/lucide/arrow-up-right.svg` — official Lucide arrow asset under the existing bundled Lucide license.

### Principal modified files

- `crates/viewer-domain/src/review/feedback.rs` — normalized arrow value and three new `FeedbackAnchor` variants.
- `crates/viewer-domain/src/review/round.rs` and `crates/viewer-domain/src/review/continuous/model.rs` — media and aggregate validation.
- `crates/viewer-application/src/review_artifact.rs` — evidence request acceptance for all image anchors.
- `crates/viewer-application/src/review_workspace/authoring.rs` and `authoring_service.rs` — sticky publication protocol.
- `src-tauri/src/dto/review.rs` — strict command/view DTO mappings.
- `crates/viewer-infrastructure/src/review/protocol/v3/*` — shared version-aware wire plumbing while preserving v3 rejection rules and bytes.
- `crates/viewer-infrastructure/src/review/authoring/codec.rs` and `bootstrap.rs` — persist/recover the selected publication protocol.
- `crates/viewer-infrastructure/src/review/continuous/{repository,prepare,history,commit}.rs` — detect, read, and publish v3/v4 records.
- `crates/viewer-infrastructure/src/review/continuous/reader/*` — return the protocol version of the exact selected state.
- `crates/viewer-platform-macos/src/image/review_annotation.rs` — deterministic evidence rendering for point, arrow, and ellipse.
- `ui/src/api/types.ts` — three new command/view Anchor variants.
- `ui/src/app/review/{annotationGeometry,annotationModel,useImageReviewWorkbench}.ts` — geometry, orthogonal state, and save/edit behavior.
- `ui/src/components/review/AnnotationToolbar.tsx`, `AnnotationCanvas.tsx`, and `annotationScene.ts` — menu integration, gestures/edit handles, and shared rendering.
- `ui/src/components/ui/ViewerIcon.tsx` and `ui/src/styles/review.css` — registered arrow icon and adaptive menu/handle styling.
- `docs/product/{FEATURE_REFERENCE,SHORTCUTS,USER_GUIDE}.md`, `docs/protocol/README.md`, and `docs/PRODUCT_SPEC.md` — product and integration facts.

---

### Task 1: Add the normalized arrow value object without changing the Anchor union

**Files:**
- Modify: `crates/viewer-domain/src/review/feedback.rs`
- Modify: `crates/viewer-domain/src/review/mod.rs`
- Test: `crates/viewer-domain/tests/review_round.rs`
- Include in commit: `docs/superpowers/specs/2026-09-02-viewer-unified-image-markup-tools-design.md`
- Include in commit: `docs/superpowers/plans/2026-09-02-viewer-unified-image-markup-tools.md`

**Interfaces:**
- Consumes: existing `NormalizedPoint::new(x, y)`.
- Produces: `NormalizedArrow::new(tail, head)`, `tail()`, and `head()` for later Domain and wire tasks.

- [x] **Step 1: Write the failing value-object test**

```rust
#[test]
fn normalized_arrows_preserve_direction_and_reject_zero_length() {
    let tail = NormalizedPoint::new(0.1, 0.2).unwrap();
    let head = NormalizedPoint::new(0.8, 0.7).unwrap();
    let arrow = NormalizedArrow::new(tail, head).unwrap();
    assert_eq!(arrow.tail(), tail);
    assert_eq!(arrow.head(), head);
    assert_eq!(
        NormalizedArrow::new(head, head),
        Err(ReviewValueError::InvalidNumber)
    );
}
```

- [x] **Step 2: Run the test and verify the missing type failure**

Run: `cargo test --locked -p viewer-domain --test review_round normalized_arrows_preserve_direction_and_reject_zero_length`

Expected: FAIL because `NormalizedArrow` is not exported.

- [x] **Step 3: Implement and export the value object**

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormalizedArrow {
    tail: NormalizedPoint,
    head: NormalizedPoint,
}

impl NormalizedArrow {
    pub fn new(
        tail: NormalizedPoint,
        head: NormalizedPoint,
    ) -> Result<Self, ReviewValueError> {
        (tail != head)
            .then_some(Self { tail, head })
            .ok_or(ReviewValueError::InvalidNumber)
    }

    pub fn tail(&self) -> NormalizedPoint { self.tail }
    pub fn head(&self) -> NormalizedPoint { self.head }
}
```

Add `NormalizedArrow` to the existing `pub use feedback::{...}` list in `review/mod.rs`. Do not add new
`FeedbackAnchor` variants in this task, so every existing workspace consumer remains exhaustive.

- [x] **Step 4: Run the focused Domain suite**

Run: `cargo test --locked -p viewer-domain --test review_round`

Expected: PASS.

- [x] **Step 5: Commit the reviewed design, plan, and value object**

```bash
git add docs/superpowers/specs/2026-09-02-viewer-unified-image-markup-tools-design.md \
  docs/superpowers/plans/2026-09-02-viewer-unified-image-markup-tools.md \
  crates/viewer-domain/src/review/feedback.rs \
  crates/viewer-domain/src/review/mod.rs \
  crates/viewer-domain/tests/review_round.rs
git commit -m "design: define unified image markup delivery"
```

### Task 2: Introduce a shared continuous-review version boundary and v4 shell

**Files:**
- Create: `crates/viewer-infrastructure/src/review/protocol/continuous_version.rs`
- Create: `crates/viewer-infrastructure/src/review/protocol/v4/mod.rs`
- Modify: `crates/viewer-infrastructure/src/review/protocol/mod.rs`
- Modify: `crates/viewer-infrastructure/src/review/protocol/v3/mod.rs`
- Modify: `crates/viewer-infrastructure/src/review/protocol/v3/wire.rs`
- Modify: `crates/viewer-infrastructure/src/review/protocol/v3/wire/state.rs`
- Modify: `crates/viewer-infrastructure/src/review/protocol/v3/catalog_wire.rs`
- Modify: `crates/viewer-infrastructure/src/review/protocol/v3/archive_wire.rs`
- Modify: `crates/viewer-infrastructure/src/review/protocol/v3/read_result.rs`
- Test: `crates/viewer-infrastructure/src/review/protocol/v3/tests.rs`
- Test: `crates/viewer-infrastructure/src/review/protocol/v4/mod.rs`

**Interfaces:**
- Consumes: existing `ReviewStateRecord`, `ReviewIndexV3`, `ReviewArchiveRecord`, and `ReviewReadResult`.
- Produces: `ContinuousReviewProtocol::{V3,V4}`, `v4::encode_*_v4`, and `v4::decode_*_v4`.

- [x] **Step 1: Add failing version-isolation tests**

```rust
#[test]
fn v3_and_v4_emit_distinct_protocol_versions_for_the_same_legacy_state() {
    let fixture = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/review-protocol/review-state-v3.valid.json"
    ));
    let record = super::v3::decode_state_v3(fixture).unwrap();
    let v3 = super::v3::encode_state_v3(&record).unwrap();
    let v4 = super::v4::encode_state_v4(&record).unwrap();
    assert_eq!(serde_json::from_slice::<Value>(&v3).unwrap()["protocolVersion"], "viewer.review/3");
    assert_eq!(serde_json::from_slice::<Value>(&v4).unwrap()["protocolVersion"], "viewer.review/4");
    assert!(super::v3::decode_state_v3(&v4).is_err());
    assert!(super::v4::decode_state_v4(&v3).is_err());
}
```

Also freeze deterministic v3 canonical re-encoding before refactoring:

```rust
let before = encode_state_v3(&record).unwrap();
let decoded = decode_state_v3(&before).unwrap();
assert_eq!(encode_state_v3(&decoded).unwrap(), before);
```

- [x] **Step 2: Run the protocol tests and verify v4 is missing**

Run: `cargo test --locked -p viewer-infrastructure review::protocol`

Expected: FAIL because module `v4` and its functions do not exist.

- [x] **Step 3: Add the version enum and parameterize wire construction**

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ContinuousReviewProtocol {
    #[serde(rename = "viewer.review/3")]
    V3,
    #[serde(rename = "viewer.review/4")]
    V4,
}

impl ContinuousReviewProtocol {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V3 => "viewer.review/3",
            Self::V4 => "viewer.review/4",
        }
    }
}
```

Move only the protocol discriminator to this shared file. Keep records and wire adapters in their current
focused modules. Replace hard-coded `Protocol::V3` construction with `State::from_record(record, version)`,
`Index::from_record(record, version)`, and `Archive::from_record(record, version)`. Decoders still call
`decode_document(..., expected.as_str())`, so a v3 decoder cannot accept a v4 document.

Rename the version-neutral record implementations to `ReviewIndexRecord` and `ReviewStreamRecord`, while
keeping `pub type ReviewIndexV3 = ReviewIndexRecord` and `pub type ReviewStreamV3 = ReviewStreamRecord` so
existing callers and tests remain source-compatible. Re-export the same records as `ReviewIndexV4` and
`ReviewStreamV4` from the v4 wrapper.

- [x] **Step 4: Add thin v4 wrappers without duplicating the wire model**

```rust
pub const REVIEW_PROTOCOL_V4: &str = "viewer.review/4";

pub fn encode_state_v4(record: &ReviewStateRecord) -> Result<Vec<u8>, ReviewProtocolError> {
    super::v3::encode_state_for(ContinuousReviewProtocol::V4, record)
}

pub fn decode_state_v4(bytes: &[u8]) -> Result<ReviewStateRecord, ReviewProtocolError> {
    super::v3::decode_state_for(ContinuousReviewProtocol::V4, bytes)
}
```

Provide the same thin pair for index, archive, and read-result records. Keep `encode_usage_v1` and
`decode_usage_v1` in the existing usage protocol because their wire contract does not contain Anchor.

- [x] **Step 5: Run protocol and workspace compilation checks**

Run: `cargo test --locked -p viewer-infrastructure review::protocol && cargo check --locked --workspace --all-targets`

Expected: PASS, and the v3 fixture bytes remain identical.

- [x] **Step 6: Commit the version boundary**

```bash
git add crates/viewer-infrastructure/src/review/protocol
git commit -m "refactor: isolate continuous review protocol versions"
```

### Task 3: Extend the backend Anchor contract and strict v4 schemas

**Files:**
- Modify: `crates/viewer-domain/src/review/feedback.rs`
- Modify: `crates/viewer-domain/src/review/round.rs`
- Modify: `crates/viewer-domain/src/review/continuous/model.rs`
- Modify: `crates/viewer-application/src/review_artifact.rs`
- Modify: `crates/viewer-application/src/review_evidence.rs`
- Modify: `crates/viewer-application/src/review_workspace/evidence.rs`
- Modify: `crates/viewer-application/src/review_workspace/evidence_action.rs`
- Modify: `crates/viewer-application/src/review_workspace/budget.rs`
- Modify: `crates/viewer-infrastructure/src/review/bundle.rs`
- Modify: `crates/viewer-infrastructure/src/review/continuous/command_fields.rs`
- Modify: `crates/viewer-infrastructure/src/review/protocol/v1.rs`
- Modify: `crates/viewer-infrastructure/src/review/protocol/v2.rs`
- Modify: `crates/viewer-infrastructure/src/review/protocol/v3/validate.rs`
- Modify: `crates/viewer-infrastructure/src/review/protocol/v3/wire/feedback_types.rs`
- Modify: `crates/viewer-platform-macos/src/image/review_annotation.rs`
- Modify: `src-tauri/src/dto/review.rs`
- Create: `docs/protocol/viewer-review-state-v4.schema.json`
- Create: `docs/protocol/viewer-review-index-v4.schema.json`
- Create: `docs/protocol/viewer-review-archive-v4.schema.json`
- Create: `docs/protocol/viewer-review-read-result-v4.schema.json`
- Test: `crates/viewer-domain/tests/review_round.rs`
- Test: `crates/viewer-domain/tests/continuous_review_state.rs`
- Test: `src-tauri/tests/review_workspace_dto.rs`
- Test: `scripts/review-protocol/schema-contract.test.mjs`

**Interfaces:**
- Consumes: `NormalizedPoint`, `NormalizedArrow`, `NormalizedRect`, and `ContinuousReviewProtocol`.
- Produces: `FeedbackAnchor::{ImagePoint,ImageArrow,ImageEllipse}` and strict UI/Tauri/wire mappings.

- [x] **Step 1: Write failing Domain, DTO, and Schema tests**

```rust
assert_eq!(
    FeedbackAnchor::ImagePoint(NormalizedPoint::new(0.2, 0.3).unwrap()).kind_name(),
    "imagePoint"
);
assert_eq!(
    FeedbackAnchor::ImageEllipse(NormalizedRect::new(0.1, 0.2, 0.3, 0.4).unwrap()).kind_name(),
    "imageEllipse"
);
```

```javascript
import { readFileSync } from 'node:fs'

const stateV3 = JSON.parse(readFileSync('docs/protocol/viewer-review-state-v3.schema.json', 'utf8'))
const stateV4 = JSON.parse(readFileSync('docs/protocol/viewer-review-state-v4.schema.json', 'utf8'))
const kinds = stateV4.$defs.anchor.oneOf.map(entry => entry.properties.kind.const)
assert.deepEqual(kinds, [
  'asset', 'imagePoint', 'imageArrow', 'imageStroke',
  'imageRect', 'imageEllipse', 'videoPoint', 'videoRange',
])
assert.equal(stateV3.$defs.anchor.oneOf.some(entry => entry.properties.kind.const === 'imagePoint'), false)
```

Add DTO cases that deserialize `image_point`, `image_arrow`, and `image_ellipse`, then round-trip them back
without swapping arrow endpoints.

- [x] **Step 2: Run focused tests and confirm missing variants/schemas**

Run: `cargo test --locked -p viewer-domain --test review_round && cargo test --locked -p viewer-desktop --test review_workspace_dto && node --test scripts/review-protocol/schema-contract.test.mjs`

Expected: FAIL on the new variants and missing v4 schema files.

- [x] **Step 3: Add the three Domain variants and all mathematical validation**

```rust
pub enum FeedbackAnchor {
    Asset,
    ImagePoint(NormalizedPoint),
    ImageArrow(NormalizedArrow),
    ImageRect(NormalizedRect),
    ImageEllipse(NormalizedRect),
    ImageStroke(ImageStroke),
    VideoPoint { position_us: u64 },
    VideoRange { start_us: u64, end_us: u64 },
}
```

Update image-media validation so all five image variants require known, non-zero image dimensions. Preserve
the stroke-point aggregate limit only for `ImageStroke`. Extend command hashing with canonical numbers:

```rust
FeedbackAnchor::ImagePoint(p) => self.field(&(canonical(p.x()), canonical(p.y()))),
FeedbackAnchor::ImageArrow(a) => self.field(&(
    canonical(a.tail().x()), canonical(a.tail().y()),
    canonical(a.head().x()), canonical(a.head().y()),
)),
FeedbackAnchor::ImageEllipse(r) => self.field(&(
    canonical(r.x()), canonical(r.y()), canonical(r.width()), canonical(r.height()),
)),
```

- [x] **Step 4: Add strict DTO and version-gated wire mappings**

Use nested points for arrows at both Tauri and persisted boundaries:

```rust
ImagePoint { x: f64, y: f64 },
ImageArrow { tail: ReviewPointDto, head: ReviewPointDto },
ImageEllipse { x: f64, y: f64, width: f64, height: f64 },
```

The shared wire `Anchor` can deserialize all variants, but `validate::state_for(version, record)` must reject
any extended anchor for `V3` and allow it for `V4`. Test both encoder and decoder rejection; do not rely only
on the JSON Schema.

Keep publication safe between this contract commit and Task 4: make the macOS renderer's exhaustive match
return `ReviewArtifactError::InvalidRequest` for `ImagePoint`, `ImageArrow`, and `ImageEllipse`. No released UI
can create these anchors yet, and an incomplete annotated evidence image can never be published. Task 4
replaces this explicit rejection with deterministic drawing.

Update every exhaustive semantic boundary reported by `rg -l 'FeedbackAnchor::' crates src-tauri`: v1/v2/v3
encoders must reject unsupported extended anchors, evidence/bundle classification must recognize all five image
anchors, and cost accounting must remain bounded (only strokes contribute point-array bytes). Do not weaken old
protocol decoders or silently coerce a new anchor into an old shape.

- [x] **Step 5: Add all four v4 schemas with closed objects**

Each new Anchor branch must use `additionalProperties: false`. The arrow branch is:

```json
{
  "type": "object",
  "additionalProperties": false,
  "required": ["kind", "tail", "head"],
  "properties": {
    "kind": { "const": "imageArrow" },
    "tail": { "$ref": "#/$defs/normalizedPoint" },
    "head": { "$ref": "#/$defs/normalizedPoint" }
  }
}
```

Define `normalizedPoint` with required `x`, `y`, numeric minimum `0`, maximum `1`, and no extra fields.
Retain every existing document/array/string bound from the corresponding v3 schema.

- [x] **Step 6: Run backend contract checks**

Run: `cargo test --locked -p viewer-domain && cargo test --locked -p viewer-desktop --test review_workspace_dto && cargo test --locked -p viewer-infrastructure review::protocol && node --test scripts/review-protocol/schema-contract.test.mjs && cargo check --locked --workspace --all-targets`

Expected: PASS; v3 rejects extended anchors and v4 round-trips all of them.

- [x] **Step 7: Commit the backend Anchor contract**

```bash
git add crates/viewer-domain crates/viewer-application/src/review_artifact.rs \
  crates/viewer-application/src/review_evidence.rs \
  crates/viewer-application/src/review_workspace/budget.rs \
  crates/viewer-application/src/review_workspace/evidence.rs \
  crates/viewer-application/src/review_workspace/evidence_action.rs \
  crates/viewer-infrastructure/src/review/continuous/command_fields.rs \
  crates/viewer-infrastructure/src/review/protocol \
  crates/viewer-platform-macos/src/image/review_annotation.rs \
  src-tauri/src/dto/review.rs src-tauri/tests/review_workspace_dto.rs \
  docs/protocol/viewer-review-*-v4.schema.json scripts/review-protocol/schema-contract.test.mjs
git commit -m "feat: define extended image review anchors"
```

### Task 4: Render all extended anchors into immutable review evidence

**Files:**
- Modify: `crates/viewer-application/src/review_artifact.rs`
- Modify: `crates/viewer-application/src/review_workspace/evidence_action.rs`
- Modify: `crates/viewer-application/tests/review_evidence_action.rs`
- Modify: `crates/viewer-platform-macos/src/image/review_annotation.rs`
- Modify: `crates/viewer-platform-macos/src/image/review_evidence/tests.rs`
- Test: `crates/viewer-platform-macos/src/image/review_annotation.rs`
- Test: `crates/viewer-platform-macos/src/image/review_evidence/tests.rs`

**Interfaces:**
- Consumes: three new `FeedbackAnchor` variants.
- Produces: deterministic Core Graphics point, directed arrow, and ellipse evidence plus ordinal locations.

- [ ] **Step 1: Write failing request and pixel-level rendering tests**

Create a deterministic 600 × 800 orientation-1 source with the existing `asymmetric_source` test helper, then
render point, arrow, ellipse, rectangle, and stroke annotations whose target coordinates match the assertions:

```rust
assert_eq!(rendered.annotations.len(), 5);
assert_eq!(rendered.annotations[0].ordinal, 1);
let source_pixels = decoded_rgba(&source_path, 600, 800);
let rendered_pixels = decoded_rgba(&rendered.temporary_path, 600, 800);
let pixel = |bytes: &[u8], (x, y): (usize, usize)| {
    let offset = (y * 600 + x) * 4;
    [bytes[offset], bytes[offset + 1], bytes[offset + 2]]
};
let normalized_pixel = |x: f64, y: f64| {
    ((x * 599.0).round() as usize, (y * 799.0).round() as usize)
};
let changed_near = |center: (usize, usize), radius: usize| {
    let (cx, cy) = center;
    (cy - radius..=cy + radius).any(|y| {
        (cx - radius..=cx + radius).any(|x| {
            pixel(&rendered_pixels, (x, y)) != pixel(&source_pixels, (x, y))
        })
    })
};
for target in [
    normalized_pixel(0.20, 0.25), // point target
    normalized_pixel(0.75, 0.30), // arrow head
    normalized_pixel(0.30, 0.70), // ellipse top edge
] {
    assert!(changed_near(target, 6));
}
```

Also keep the existing source digest assertion proving the renderer never modifies the original image.

- [ ] **Step 2: Run the macOS renderer tests and verify new anchors fail**

Run: `cargo test --locked -p viewer-platform-macos review_annotation`

Expected: FAIL because `ReviewArtifactRenderRequest::validate` or `draw_annotations` rejects the new anchors.

- [ ] **Step 3: Add focused Core Graphics drawing helpers**

```rust
fn source_point(point: NormalizedPoint, width: u32, height: u32) -> CGPoint;
fn draw_point(context: &CGContext, target: CGPoint, line_width: f64);
fn draw_arrow(context: &CGContext, tail: CGPoint, head: CGPoint, line_width: f64);
fn draw_ellipse(context: &CGContext, rect: NormalizedRect, width: u32, height: u32, line_width: f64);
```

Use `head` as the arrow tip. Compute the two arrowhead segments from the normalized direction vector with a
bounded display-space head length; reject no data here because Domain already rejects equal endpoints. Draw
the ordinal near point/arrow tail/ellipse candidate corner, clamp it to the evidence bounds, and preserve the
existing ordinal-to-Feedback mapping.

Bump `ReviewEvidenceActionPolicy::renderer_version` once in this task. Add a cache-action test proving an
evidence binding generated by the previous renderer version is not reused, while the original immutable
evidence object and digest references remain readable from historical snapshots.

- [ ] **Step 4: Run renderer and evidence integration tests**

Run: `cargo test --locked -p viewer-platform-macos review_annotation && cargo test --locked -p viewer-platform-macos review_evidence`

Expected: PASS.

- [ ] **Step 5: Commit evidence rendering**

```bash
git add crates/viewer-application/src/review_artifact.rs \
  crates/viewer-application/src/review_workspace/evidence_action.rs \
  crates/viewer-application/tests/review_evidence_action.rs \
  crates/viewer-platform-macos/src/image/review_annotation.rs \
  crates/viewer-platform-macos/src/image/review_evidence/tests.rs
git commit -m "feat: render extended review evidence"
```

### Task 5: Make v4 promotion sticky through authoring, publication, and history

**Files:**
- Modify: `crates/viewer-application/src/review_workspace/authoring.rs`
- Modify: `crates/viewer-application/src/review_workspace/authoring_service.rs`
- Modify: `crates/viewer-application/src/review_workspace/ports.rs`
- Modify: `crates/viewer-infrastructure/src/review/authoring/codec.rs`
- Modify: `crates/viewer-infrastructure/src/review/authoring/bootstrap.rs`
- Modify: `crates/viewer-infrastructure/src/review/continuous/repository.rs`
- Modify: `crates/viewer-infrastructure/src/review/continuous/prepare.rs`
- Modify: `crates/viewer-infrastructure/src/review/continuous/history.rs`
- Modify: `crates/viewer-infrastructure/src/review/continuous/commit.rs`
- Modify: `crates/viewer-infrastructure/src/review/continuous/manual_context.rs`
- Test: `crates/viewer-application/tests/continuous_review_authoring_service.rs`
- Test: `crates/viewer-infrastructure/tests/continuous_review_authoring_store.rs`
- Test: `crates/viewer-infrastructure/tests/continuous_review_materialization.rs`
- Test: `crates/viewer-infrastructure/tests/continuous_review_repository.rs`

**Interfaces:**
- Consumes: v3/v4 codecs and extended Anchors.
- Produces: `ReviewPublicationProtocol::{V3,V4}` on authoring state and version-aware repository snapshots.

- [ ] **Step 1: Write failing one-way promotion tests**

In `continuous_review_authoring_service.rs`, extend the existing `save(asset_version_id, text)` helper with a
`save_anchor(asset_version_id, text, anchor)` variant. Start from the existing `Fixture`, save an asset-only
feedback and assert `current.authoring.publication_protocol == V3`; save an `ImagePoint` feedback and assert the
patched current view is V4; withdraw that feedback using its real `TargetVersionKey` and assert the resulting
current view remains V4. This test must exercise `prepare`, `apply_authoring_with_cancellation`, and
`ReviewWorkspacePatch::apply`, rather than testing a detached version helper.

Add a repository test that opening and reading an untouched v3 fixture leaves `index.json` mtime and bytes
unchanged. Add a materialization test that the first point save produces a v4 state and atomically switches
the index to `viewer.review/4`.

- [ ] **Step 2: Run focused authoring/materialization tests**

Run: `cargo test --locked -p viewer-application continuous_review_authoring_service && cargo test --locked -p viewer-infrastructure continuous_review_materialization`

Expected: FAIL because `StoredAuthoringState` has no publication protocol.

- [ ] **Step 3: Add the application-owned publication protocol**

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewPublicationProtocol { V3, V4 }

impl ReviewPublicationProtocol {
    pub fn promote_for(self, state: &ContinuousReviewState) -> Self {
        let requires_v4 = state.feedback.iter().flat_map(|item| &item.targets).any(|target| {
            matches!(
                &target.anchor,
                FeedbackAnchor::ImagePoint(_)
                    | FeedbackAnchor::ImageArrow(_)
                    | FeedbackAnchor::ImageEllipse(_)
            )
        });
        if self == Self::V4 || requires_v4 { Self::V4 } else { Self::V3 }
    }
}
```

Add `publication_protocol` to `StoredAuthoringState` and `StoredContinuousSnapshot`. Bootstrap a decoded v3
published record as V3 and a v4 record as V4. During authoring, compute the new value from the transaction's
current state, not from stale UI state.

- [ ] **Step 4: Persist the protocol in the private authoring record compatibly**

```rust
#[serde(default = "default_v3_publication_protocol")]
publication_protocol: ReviewPublicationProtocol,
```

Encode the field on all new authoring snapshots. Decode old authoring bytes with the V3 default, include the
field in canonical re-encoding checks, and ensure an idempotent command retry returns the original protocol.
No SQLite schema column is needed because the immutable logical snapshot already owns this fact.

- [ ] **Step 5: Make repository views and commits version-aware**

Replace `View.index: ReviewIndexV3` with a versioned record:

```rust
pub struct VersionedReviewIndex {
    pub protocol: ReviewPublicationProtocol,
    pub record: ReviewIndexRecord,
}
```

At the Infrastructure boundary, add total `From` conversions between application-owned
`ReviewPublicationProtocol` and wire-owned `ContinuousReviewProtocol`. Keep wire codecs dependent only on
`ContinuousReviewProtocol`; do not pass application types into protocol modules or compare version strings in
repository/materializer code.

Detect `protocolVersion` with the bounded protocol detector, then dispatch to v3 or v4 decode. `prepare`
encodes state/archive/index using `request.next.publication_protocol`; history reads dispatch on each immutable
document's own protocol. When a v4 state follows a v3 state, validate the parent and identity chain normally.
When a v3 state attempts to follow v4, return `ReviewCommitError::Integrity`.

- [ ] **Step 6: Run authoring, repository, migration, and materialization suites**

Run: `cargo test --locked -p viewer-application continuous_review && cargo test --locked -p viewer-infrastructure continuous_review_authoring && cargo test --locked -p viewer-infrastructure continuous_review_materialization && cargo test --locked -p viewer-infrastructure continuous_review_repository && cargo test --locked -p viewer-infrastructure continuous_review_migration`

Expected: PASS; v3 reads are side-effect free and promotion never downgrades.

- [ ] **Step 7: Commit sticky v4 publication**

```bash
git add crates/viewer-application/src/review_workspace \
  crates/viewer-application/tests/continuous_review_authoring_service.rs \
  crates/viewer-infrastructure/src/review/authoring \
  crates/viewer-infrastructure/src/review/continuous \
  crates/viewer-infrastructure/tests/continuous_review_authoring_store.rs \
  crates/viewer-infrastructure/tests/continuous_review_materialization.rs \
  crates/viewer-infrastructure/tests/continuous_review_repository.rs
git commit -m "feat: publish extended anchors through review v4"
```

### Task 6: Teach the native reader and Agent contract tests to select v3 or v4 exactly

**Files:**
- Modify: `crates/viewer-infrastructure/src/review/continuous/reader/project.rs`
- Modify: `crates/viewer-infrastructure/src/review/continuous/reader/current.rs`
- Modify: `crates/viewer-infrastructure/src/review/continuous/reader/history.rs`
- Modify: `crates/viewer-infrastructure/src/review/protocol/v3/read_result.rs`
- Modify: `crates/viewer-infrastructure/src/review/protocol/v3/history_result.rs`
- Modify: `crates/viewer-infrastructure/src/bin/viewer-review-reader.rs`
- Modify: `scripts/review-protocol/reader-cli.mjs`
- Modify: `scripts/review-protocol/v3-fixtures.mjs`
- Test: `crates/viewer-infrastructure/tests/continuous_review_reader.rs`
- Test: `scripts/review-protocol/native-reader.test.mjs`
- Test: `scripts/review-protocol/read-current.test.mjs`
- Test: `scripts/review-protocol/read-history.test.mjs`

**Interfaces:**
- Consumes: versioned repository records and `authoring_head`/`published_head` gate.
- Produces: v3 results for v3 selections and v4 results for v4 selections, with no fallback.

- [ ] **Step 1: Add failing reader tests for current and mixed history**

```javascript
const current = await readCurrent(v4Project)
assert.equal(current.protocolVersion, 'viewer.review/4')
assert.equal(current.feedback[0].targets[0].anchor.kind, 'imageArrow')
assert.deepEqual(current.feedback[0].targets[0].anchor.head, { x: 0.7, y: 0.25 })

const history = await readHistory(v4Project, { snapshotId: v3SnapshotId })
assert.equal(history.protocolVersion, 'viewer.review/3')
```

Keep the existing pending-publication case and assert it returns `publication_pending` instead of the older
v3 head after a v4 authoring save.

- [ ] **Step 2: Run reader suites and verify protocol mismatch failures**

Run: `cargo test --locked -p viewer-infrastructure continuous_review_reader && pnpm build:review-reader && node --test scripts/review-protocol/native-reader.test.mjs scripts/review-protocol/read-current.test.mjs scripts/review-protocol/read-history.test.mjs`

Expected: FAIL because reader result construction is hard-coded to v3.

- [ ] **Step 3: Thread the exact selected protocol through result construction**

```rust
pub fn encode_read_result(
    protocol: ReviewPublicationProtocol,
    result: &ReviewReadResult,
) -> Result<Vec<u8>, ReviewProtocolError> {
    match protocol {
        ReviewPublicationProtocol::V3 => v3::encode_read_result_v3(result),
        ReviewPublicationProtocol::V4 => v4::encode_read_result_v4(result),
    }
}
```

No-review-state and errors use the index protocol when an index exists; a project with no index continues to
emit `viewer.review/3`, preserving the existing contract. Exact history uses the immutable selected state's
protocol, not the newest index protocol. Preserve all source, evidence, digest, path, size, ambiguity, and
head-equality checks.

- [ ] **Step 4: Run the complete protocol gate**

Run: `pnpm test:review-protocol && pnpm test:review-loop`

Expected: PASS for existing v3 fixtures plus new v4 and mixed-history fixtures.

- [ ] **Step 5: Commit native Agent reading**

```bash
git add crates/viewer-infrastructure/src/review/continuous/reader \
  crates/viewer-infrastructure/src/review/protocol/v3 \
  crates/viewer-infrastructure/src/bin/viewer-review-reader.rs \
  crates/viewer-infrastructure/tests/continuous_review_reader.rs \
  scripts/review-protocol
git commit -m "feat: read review v4 without weakening agent gates"
```

### Task 7: Add TypeScript Anchor types, geometry primitives, hit testing, and tool registry

**Files:**
- Modify: `ui/src/api/types.ts`
- Modify: `ui/src/api/viewer.test.ts`
- Modify: `ui/src/app/review/annotationGeometry.ts`
- Modify: `ui/src/app/review/annotationGeometry.test.ts`
- Create: `ui/src/app/review/annotationHitTest.ts`
- Create: `ui/src/app/review/annotationHitTest.test.ts`
- Create: `ui/src/app/review/annotationToolRegistry.ts`
- Create: `ui/src/app/review/annotationToolRegistry.test.ts`

**Interfaces:**
- Consumes: Tauri `snake_case` Anchor DTO contract.
- Produces: `AnnotationTool`, tool descriptors, pure gesture geometry, and deterministic hit-test results.

- [ ] **Step 1: Write failing geometry and registry tests**

```ts
expect(arrowFromDrag({ x: 0.1, y: 0.2 }, { x: 0.7, y: 0.8 })).toEqual({
  kind: 'image_arrow', tail: { x: 0.1, y: 0.2 }, head: { x: 0.7, y: 0.8 },
})
expect(ellipseFromDrag(
  { x: 0.1, y: 0.2 },
  { x: 0.5, y: 0.7 },
  { constrainCircle: true, sourceWidth: 1200, sourceHeight: 800 },
))
  .toMatchObject({ kind: 'image_ellipse' })
expect(ANNOTATION_TOOLS.map(tool => tool.id)).toEqual(['point', 'arrow', 'brush', 'rectangle', 'ellipse'])
expect(toolForShortcut('o')?.id).toBe('ellipse')
```

Add hit tests proving that a fixed 8 CSS px tolerance selects the same normalized arrow endpoint at 50%,
100%, and 400% zoom, and that overlap priority is handle > selected > ordinal > nearest > later item.

- [ ] **Step 2: Run focused tests and verify missing functions/types**

Run: `pnpm --dir ui exec vitest run src/app/review/annotationGeometry.test.ts src/app/review/annotationHitTest.test.ts src/app/review/annotationToolRegistry.test.ts src/api/viewer.test.ts`

Expected: FAIL because extended Anchor variants and helper modules do not exist.

- [ ] **Step 3: Extend the TypeScript union exactly**

```ts
export interface ReviewPoint { x: number; y: number }

export type ReviewAnchor =
  | { kind: 'asset' }
  | { kind: 'image_point'; x: number; y: number }
  | { kind: 'image_arrow'; tail: ReviewPoint; head: ReviewPoint }
  | { kind: 'image_stroke'; points: ReadonlyArray<ReviewPoint> }
  | { kind: 'image_rect'; x: number; y: number; width: number; height: number }
  | { kind: 'image_ellipse'; x: number; y: number; width: number; height: number }
  | { kind: 'video_point'; positionUs: number }
  | { kind: 'video_range'; startUs: number; endUs: number }
```

Use the same nested `tail`/`head` names as Tauri. Update bridge fixtures to prove no flattening or endpoint swap.
In `annotationGeometry.ts`, define `export type NormalizedPoint = ReviewPoint`; the API layer must not import
from the application layer.

- [ ] **Step 4: Implement pure geometry and hit testing**

Export these exact functions:

```ts
export type ImageAnchor = Extract<ReviewAnchor, { kind: `image_${string}` }>
export type ImagePointAnchor = Extract<ReviewAnchor, { kind: 'image_point' }>
export type ImageArrowAnchor = Extract<ReviewAnchor, { kind: 'image_arrow' }>
export type ImageRectAnchor = Extract<ReviewAnchor, { kind: 'image_rect' }>
export type ImageEllipseAnchor = Extract<ReviewAnchor, { kind: 'image_ellipse' }>
export function pointAnchor(point: NormalizedPoint): ImagePointAnchor | null
export function arrowFromDrag(start: NormalizedPoint, end: NormalizedPoint): ImageArrowAnchor | null
export function ellipseFromDrag(start: NormalizedPoint, end: NormalizedPoint, options: EllipseOptions): ImageEllipseAnchor | null
export function moveAnchor(anchor: ImageAnchor, delta: NormalizedPoint): ImageAnchor
export function resizeBoxAnchor(anchor: ImageRectAnchor | ImageEllipseAnchor, handle: BoxHandle, point: NormalizedPoint): typeof anchor | null
export function hitTestAnnotation(input: AnnotationHitTestInput): AnnotationHit | null
```

Circle constraint must compare oriented source pixels (`dx * sourceWidth`, `dy * sourceHeight`) before
converting the resulting box back to normalized values. Geometry creation clamps to the image; whole-shape
movement preserves size while constraining position.

- [ ] **Step 5: Implement the compile-time tool registry**

```ts
export type MarkupTool = 'point' | 'arrow' | 'brush' | 'rectangle' | 'ellipse'
export type AnnotationTool = 'browse' | MarkupTool

export const ANNOTATION_TOOLS = [
  { id: 'point', label: '点', shortcut: 'P', icon: 'circle-dot', gesture: 'click' },
  { id: 'arrow', label: '箭头', shortcut: 'A', icon: 'arrow-up-right', gesture: 'drag' },
  { id: 'brush', label: '画笔', shortcut: 'B', icon: 'pencil', gesture: 'stroke' },
  { id: 'rectangle', label: '矩形', shortcut: 'R', icon: 'maximize', gesture: 'drag' },
  { id: 'ellipse', label: '椭圆', shortcut: 'O', icon: 'circle', gesture: 'drag' },
] as const satisfies ReadonlyArray<AnnotationToolDescriptor>
```

Tests must enforce unique IDs, shortcuts, and menu positions so future additions cannot silently collide.

- [ ] **Step 6: Run focused UI foundations**

Run: `pnpm --dir ui exec vitest run src/api/viewer.test.ts src/app/review/annotationGeometry.test.ts src/app/review/annotationHitTest.test.ts src/app/review/annotationToolRegistry.test.ts && pnpm --dir ui typecheck`

Expected: PASS.

- [ ] **Step 7: Commit UI foundations**

```bash
git add ui/src/api/types.ts ui/src/api/viewer.test.ts ui/src/app/review/annotationGeometry* \
  ui/src/app/review/annotationHitTest* ui/src/app/review/annotationToolRegistry*
git commit -m "feat: add normalized markup geometry foundations"
```

### Task 8: Separate active tool from editor phase and preserve it after save

**Files:**
- Modify: `ui/src/app/review/annotationModel.ts`
- Modify: `ui/src/app/review/annotationModel.test.ts`
- Modify: `ui/src/app/review/useImageReviewWorkbench.ts`
- Modify: `ui/src/app/review/useImageReviewWorkbench.test.tsx`
- Modify: `ui/src/components/review/ReviewFeedbackRail.tsx`
- Modify: `ui/src/components/review/ReviewFeedbackRail.test.tsx`

**Interfaces:**
- Consumes: `AnnotationTool` and extended `ReviewAnchor`.
- Produces: `AnnotationInteractionState { activeTool, temporarilyPanning, selectedItemId, phase }` and a controller whose `tool` remains stable across saves.

- [ ] **Step 1: Write failing reducer and controller lifecycle tests**

```ts
let state = initialAnnotationState()
state = annotationEditorReducer(state, { type: 'set_tool', tool: 'ellipse' })
state = annotationEditorReducer(state, {
  type: 'begin_drawing',
  anchor: { kind: 'image_ellipse', x: 0.2, y: 0.2, width: 0.3, height: 0.4 },
})
state = annotationEditorReducer(state, { type: 'complete_drawing' })
state = annotationEditorReducer(state, { type: 'set_text', text: '调整脸部轮廓' })
state = annotationEditorReducer(state, { type: 'request_save' })
state = annotationEditorReducer(state, { type: 'save_succeeded', feedbackId: 'feedback-1' })
expect(state.activeTool).toBe('ellipse')
expect(state.phase.status).toBe('idle')
```

Test `Esc` cancels a dirty phase but preserves the selected markup tool on the first press; the next `Esc`
returns to browse. Test Space temporarily pans and returns to the exact prior tool. Re-run the existing dirty
navigation guard with an extended anchor and prove image switching, returning to the grid, and closing the
window each require the established explicit save/discard decision.

- [ ] **Step 2: Run state/controller tests and verify the current reset-to-browse failure**

Run: `pnpm --dir ui exec vitest run src/app/review/annotationModel.test.ts src/app/review/useImageReviewWorkbench.test.tsx`

Expected: FAIL because `idleState()` currently hard-codes `tool: 'browse'`.

- [ ] **Step 3: Replace the coupled union with orthogonal state**

```ts
export interface AnnotationInteractionState {
  activeTool: AnnotationTool
  temporarilyPanning: boolean
  selectedItemId: string | null
  phase: AnnotationEditorPhase
}

export type AnnotationEditorPhase =
  | { status: 'idle' }
  | { status: 'drawing'; draftAnchor: ReviewAnchor; sourceItemId?: string }
  | { status: 'editing'; draftAnchor: ReviewAnchor; text: string; sourceItemId: string | null; operation?: 'text' | 'geometry' }
  | { status: 'saving'; draftAnchor: ReviewAnchor; text: string; sourceItemId: string | null; operation?: 'text' | 'geometry' }
  | { status: 'save_error'; draftAnchor: ReviewAnchor; text: string; sourceItemId: string | null; operation?: 'text' | 'geometry'; message: string }
```

Reducer save/cancel helpers modify `phase` only. `set_tool` refuses to discard a dirty phase. Update the
workbench ownership refs and feedback-rail redraw/adjust actions to consume `phase` explicitly.

- [ ] **Step 4: Extend frontend validation and cloning exhaustively**

`isValidAnnotationAnchor` accepts finite normalized points, different arrow endpoints, positive bounded
ellipse boxes, and all existing variants. `cloneReviewAnchor` deep-clones arrow points and stroke arrays.
Use exhaustive `never` checks so later Anchor additions fail compilation at every semantic boundary.

- [ ] **Step 5: Run workbench and rail tests**

Run: `pnpm --dir ui exec vitest run src/app/review/annotationModel.test.ts src/app/review/useImageReviewWorkbench.test.tsx src/components/review/ReviewFeedbackRail.test.tsx`

Expected: PASS, including retry preserving geometry/text and save preserving the active tool.

- [ ] **Step 6: Commit the state model**

```bash
git add ui/src/app/review/annotationModel* ui/src/app/review/useImageReviewWorkbench* \
  ui/src/components/review/ReviewFeedbackRail*
git commit -m "refactor: separate markup tool and editor lifecycle"
```

### Task 9: Build the accessible fixed “标记” menu and keyboard routing

**Files:**
- Create: `ui/src/components/review/AnnotationToolMenu.tsx`
- Create: `ui/src/components/review/AnnotationToolMenu.test.tsx`
- Create: `ui/src/assets/icons/lucide/arrow-up-right.svg`
- Modify: `ui/src/components/review/AnnotationToolbar.tsx`
- Modify: `ui/src/components/review/ImageReviewWorkspace.test.tsx`
- Modify: `ui/src/components/ui/ViewerIcon.tsx`
- Modify: `ui/src/styles/review.css`

**Interfaces:**
- Consumes: `ANNOTATION_TOOLS`, `toolForShortcut`, controller `tool`, `setTool`, and `readOnlyReason`.
- Produces: one fixed “标记” toolbar button and an accessible single-selection tool menu.

- [ ] **Step 1: Write failing menu and shortcut tests**

```tsx
fireEvent.click(screen.getByRole('button', { name: '标记' }))
expect(screen.getAllByRole('menuitemradio').map(item => item.textContent)).toEqual([
  expect.stringContaining('点'),
  expect.stringContaining('箭头'),
  expect.stringContaining('画笔'),
  expect.stringContaining('矩形'),
  expect.stringContaining('椭圆'),
])
fireEvent.click(screen.getByRole('menuitemradio', { name: /椭圆/ }))
expect(controller.setTool).toHaveBeenCalledWith('ellipse')
```

Test outside click closes without calling `setTool`, reopening checks the active item, Escape returns focus to
the trigger, arrow keys move roving focus, and text inputs, editable elements, modifier chords, and IME
composition suppress `P/A/B/R/O/V`.

- [ ] **Step 2: Run component tests and verify the old separate buttons fail**

Run: `pnpm --dir ui exec vitest run src/components/review/AnnotationToolMenu.test.tsx src/components/review/ImageReviewWorkspace.test.tsx`

Expected: FAIL because no “标记” trigger/menu exists.

- [ ] **Step 3: Add the official icon asset and register it**

Copy the official Lucide `arrow-up-right.svg` asset under the existing Lucide license, then add
`'arrow-up-right'` to `VIEWER_ICON_NAMES`. Do not draw an arrow using text, CSS borders, or inline SVG.

- [ ] **Step 4: Implement the fixed trigger and accessible menu**

Use a wrapper anchored inside the current toolbar:

```tsx
<ViewerButton
  ref={triggerRef}
  leadingIcon="pencil"
  tone="quiet"
  active={controller.tool !== 'browse'}
  aria-haspopup="menu"
  aria-expanded={open}
  onClick={() => setOpen(value => !value)}
>
  标记
</ViewerButton>
<AnnotationToolMenu open={open} activeTool={controller.tool} onSelect={selectTool} />
```

The menu contains visible labels and shortcut hints. Use `menuitemradio` with `aria-checked`, roving `tabIndex`,
and focus restoration. In read-only mode, retain discoverability but disable selection with the existing reason.

- [ ] **Step 5: Add adaptive styling using existing tokens**

Position the menu below the trigger, flip/shift within the viewport, use current surface/border/shadow/radius
tokens, and keep each row at least 32 px high. Preserve the adjacent Browse and Opinion Rail buttons. Remove
the separate Brush and Rectangle top-level buttons.

- [ ] **Step 6: Run menu, workspace, accessibility, and type checks**

Run: `pnpm --dir ui exec vitest run src/components/review/AnnotationToolMenu.test.tsx src/components/review/ImageReviewWorkspace.test.tsx && pnpm --dir ui check`

Expected: PASS.

- [ ] **Step 7: Commit the unified menu**

```bash
git add ui/src/components/review/AnnotationToolMenu* ui/src/components/review/AnnotationToolbar.tsx \
  ui/src/components/review/ImageReviewWorkspace.test.tsx ui/src/components/ui/ViewerIcon.tsx \
  ui/src/assets/icons/lucide/arrow-up-right.svg ui/src/styles/review.css
git commit -m "feat: unify image markup tools in one menu"
```

### Task 10: Implement point, arrow, and ellipse creation and geometry editing

**Files:**
- Modify: `ui/src/components/review/AnnotationCanvas.tsx`
- Modify: `ui/src/components/review/AnnotationCanvas.test.tsx`
- Modify: `ui/src/components/review/ImageReviewWorkbench.integration.test.tsx`
- Modify: `ui/src/components/review/InlineFeedbackEditor.tsx`
- Modify: `ui/src/styles/review.css`

**Interfaces:**
- Consumes: pure geometry helpers, hit testing, extended controller state, and `ImagePreviewProjection`.
- Produces: pointer-captured creation and editing for all five local image Anchor types.

- [ ] **Step 1: Add failing gesture tests for all new tools**

```ts
fireEvent.pointerDown(canvas, { clientX: 74, clientY: 68, pointerId: 1 })
fireEvent.pointerUp(canvas, { clientX: 74, clientY: 68, pointerId: 1 })
expect(review.finishDrawing).toHaveBeenCalledWith({ kind: 'image_point', x: 0.1, y: 0.1 })
```

Add arrow drag direction, 5 px arrow rejection, free ellipse, Shift visual-circle on a non-square image, pointer
cancel, and pointer release outside the Canvas. Add editing tests for point drag, both arrow endpoints, ellipse
move, and four ellipse handles. Saving any geometry edit must call the controller with the existing Feedback ID;
the rail item's ordinal and selection identity must remain unchanged after the returned Patch.

- [ ] **Step 2: Run Canvas tests and verify missing gesture support**

Run: `pnpm --dir ui exec vitest run src/components/review/AnnotationCanvas.test.tsx src/components/review/ImageReviewWorkbench.integration.test.tsx`

Expected: FAIL because drawing is enabled only for brush/rectangle.

- [ ] **Step 3: Route one pointer lifecycle through tool gesture kinds**

```ts
type DrawingGesture =
  | { kind: 'point'; point: NormalizedPoint }
  | { kind: 'arrow'; start: NormalizedPoint; startClient: Point }
  | { kind: 'brush'; points: NormalizedPoint[] }
  | { kind: 'box'; shape: 'rectangle' | 'ellipse'; start: NormalizedPoint; startClient: Point }
```

On pointer down, capture the pointer and store the normalized start plus CSS-pixel start. On move, update the
controller-owned candidate. On release, use CSS-pixel distance to reject arrows below 6 px and boxes below
6 × 6 px; never save a viewport threshold. Point completes from the down/up location without a drag.

- [ ] **Step 4: Add selection and editing handles with deterministic ownership**

Use hit-test results to select/move geometry. Render DOM controls only for the selected item:

- point: one 24 × 24 px draggable target;
- arrow: tail and head controls, each at least 24 × 24 px;
- rectangle/ellipse: four corner controls plus interior move;
- stroke: retain whole-path redraw.

Window-level move/up listeners must be installed and removed by one helper per active edit, with Pointer ID
checks and cleanup on unmount/cancel. Geometry candidates go through `stageFeedbackAnchor`; pointer release
calls `replaceFeedbackAnchor` exactly once.

- [ ] **Step 5: Keep the editor in the screen-space overlay**

Anchor `InlineFeedbackEditor` through the existing candidate-to-screen projection and edge-clamping logic.
It remains above the Canvas and magnifier, is never scaled with the image, and retains clickable Save/Cancel
after zoom or pan.

- [ ] **Step 6: Run Canvas and end-to-end workbench tests**

Run: `pnpm --dir ui exec vitest run src/components/review/AnnotationCanvas.test.tsx src/components/review/ImageReviewWorkbench.integration.test.tsx src/components/review/ImageReviewWorkspace.test.tsx`

Expected: PASS for new and existing brush/rectangle flows.

- [ ] **Step 7: Commit Canvas interaction**

```bash
git add ui/src/components/review/AnnotationCanvas* ui/src/components/review/ImageReviewWorkbench.integration.test.tsx \
  ui/src/components/review/InlineFeedbackEditor.tsx ui/src/styles/review.css
git commit -m "feat: create and edit extended image markup"
```

### Task 11: Extend the shared annotation scene and composite magnifier

**Files:**
- Modify: `ui/src/components/review/annotationScene.ts`
- Modify: `ui/src/components/review/annotationScene.test.ts`
- Modify: `ui/src/components/review/ImageReviewWorkspace.tsx`
- Modify: `ui/src/components/imagePreview/ImageMagnifier.test.tsx`
- Modify: `ui/src/components/review/ImageReviewWorkspace.test.tsx`

**Interfaces:**
- Consumes: every image Anchor and existing `MagnifierOverlayPainter`.
- Produces: one deterministic paint path for stage, transient candidates, magnifier, and ordinals.

- [ ] **Step 1: Add failing scene paint tests**

```ts
expect(paintAnnotationScene(context, sceneWithPointArrowEllipse, PROJECTION, OPTIONS)).toBe(3)
expect(context.arc).toHaveBeenCalled()
expect(context.ellipse).toHaveBeenCalled()
expect(context.lineTo).toHaveBeenCalledWith(expectedArrowHeadX, expectedArrowHeadY)
```

Run the same scene through `createAnnotationMagnifierPainter` and assert `normalizedToLens` is used for the
arrow tail/head, ellipse bounds, and point. Assert control handles and the editor do not appear in the scene.

- [ ] **Step 2: Run scene/magnifier tests and verify missing paint branches**

Run: `pnpm --dir ui exec vitest run src/components/review/annotationScene.test.ts src/components/imagePreview/ImageMagnifier.test.tsx src/components/review/ImageReviewWorkspace.test.tsx`

Expected: FAIL because `isImageAnchor`, `paintAnchor`, and `annotationMarkerPoint` know only rectangle/stroke.

- [ ] **Step 3: Add pure paint branches and ordinal candidates**

```ts
case 'image_point': paintPoint(context, anchor, projection); break
case 'image_arrow': paintArrow(context, anchor, projection); break
case 'image_ellipse': paintEllipse(context, anchor, projection); break
```

Place arrow ordinals near `tail`, never `head`. Generate ordered screen-space ordinal candidates for points
and box corners, then choose the first candidate inside stage bounds that does not overlap the target geometry.
Keep appearance style separate from Anchor data.

- [ ] **Step 4: Verify the compound magnifier rules**

The injected Painter receives saved and transient anchors, draws them after the magnified image, clamps line
width to 2–5 CSS px, and draws fixed-size ordinals. `ImageMagnifier` remains `pointer-events: none`; handles,
menu, editor, and rail remain outside the painter.

- [ ] **Step 5: Run shared-rendering and full UI tests**

Run: `pnpm --dir ui exec vitest run src/components/review/annotationScene.test.ts src/components/imagePreview/ImageMagnifier.test.tsx src/components/review/ImageReviewWorkspace.test.tsx && pnpm test:ui`

Expected: PASS.

- [ ] **Step 6: Commit shared rendering**

```bash
git add ui/src/components/review/annotationScene* ui/src/components/review/ImageReviewWorkspace* \
  ui/src/components/imagePreview/ImageMagnifier.test.tsx
git commit -m "feat: magnify the complete markup scene"
```

### Task 12: Verify fast saves, failure recovery, ordering, and no preview reload

**Files:**
- Modify: `src-tauri/tests/review_save_performance.rs`
- Modify: `src-tauri/tests/review_materialization_lifecycle.rs`
- Modify: `crates/viewer-infrastructure/tests/continuous_review_costs.rs`
- Modify: `crates/viewer-infrastructure/tests/continuous_review_dirty_evidence.rs`
- Modify: `ui/src/app/review/reviewWorkspacePatch.test.ts`
- Modify: `ui/src/app/review/useContinuousReviewCoordinator.operations.test.tsx`
- Modify: `ui/src/components/review/ImageReviewWorkbench.integration.test.tsx`

**Interfaces:**
- Consumes: extended Anchor save commands, authoring Patch, outbox, and v4 materializer.
- Produces: regression proof for foreground latency, last-write ordering, failure retention, and stable preview state.

- [ ] **Step 1: Add failing performance and failure cases for an ellipse/arrow save**

```rust
let harness = Harness::new().await;
let arrow = NormalizedArrow::new(
    NormalizedPoint::new(0.2, 0.3).unwrap(),
    NormalizedPoint::new(0.7, 0.6).unwrap(),
).unwrap();
let (saved, elapsed) = harness.save(
    None,
    None,
    "调整箭头指向处".into(),
    vec![TargetEdit::Add {
        asset_version_id: harness.asset,
        anchor: FeedbackAnchor::ImageArrow(arrow),
    }],
).await;
let heads = harness.store.load_heads(harness.stream).unwrap();
assert!(elapsed <= Duration::from_millis(300));
assert_eq!(heads.authoring.unwrap(), saved.receipt.head);
assert_ne!(heads.authoring, heads.published);
```

Add `stream: ReviewStreamId` to the test-only `Harness`, populated from the existing `real.stream_id`, so the
assertion reads the real AuthoringStore heads rather than reconstructing them from the response.

UI tests capture the rendered image element and viewport state before saving, then assert identity/state remain
stable after the Patch:

```ts
const imageBeforeSave = document.querySelector<HTMLImageElement>('.image-preview-image')
if (imageBeforeSave === null) throw new Error('Expected prepared preview image')
const transformBeforeSave = imageBeforeSave.style.transform
const scaleBeforeSave = document.querySelector('.preview-scale-label')?.textContent

expect(screen.queryByText('正在加载')).not.toBeInTheDocument()
expect(document.querySelector('.image-preview-image')).toBe(imageBeforeSave)
expect(imageBeforeSave.style.transform).toBe(transformBeforeSave)
expect(document.querySelector('.preview-scale-label')?.textContent).toBe(scaleBeforeSave)
expect(screen.getByTestId('annotation-marker')).toBeVisible()
```

- [ ] **Step 2: Run save and lifecycle suites**

Run: `cargo test --locked -p viewer-desktop --test review_save_performance --test review_materialization_lifecycle && cargo test --locked -p viewer-infrastructure --test continuous_review_costs --test continuous_review_dirty_evidence && pnpm --dir ui exec vitest run src/app/review/reviewWorkspacePatch.test.ts src/app/review/useContinuousReviewCoordinator.operations.test.tsx src/components/review/ImageReviewWorkbench.integration.test.tsx`

Expected: FAIL until new anchor cost/action classification and UI assertions are complete.

- [ ] **Step 3: Keep new anchors on the existing incremental path**

Update evidence-action classification so point/arrow/ellipse geometry changes dirty only the affected image
evidence. Do not rebuild unrelated evidence and do not add a second queue. Ensure multiple authoring revisions
for one Feedback cannot publish an older state after a newer state; the materializer may coalesce superseded
non-barrier revisions using existing sequence rules.

- [ ] **Step 4: Verify typed failure behavior**

Inject foreground transaction failure and assert geometry/text remain in `save_error`. Inject render failure and
assert logical state is durable, publication is pending/blocked, current Agent read returns
`publication_pending`, and an exact already-published historical selector still succeeds. Re-run the uncertain
foreground-result path with an extended anchor: retry with the same command ID must recover the original receipt
and must not create a second Feedback or target revision.

- [ ] **Step 5: Run all save and publication tests**

Run: `cargo test --locked -p viewer-desktop --test review_save_performance --test review_materialization_lifecycle && cargo test --locked -p viewer-infrastructure --test continuous_review_costs --test continuous_review_dirty_evidence && pnpm --dir ui exec vitest run src/app/review/reviewWorkspacePatch.test.ts src/app/review/useContinuousReviewCoordinator.operations.test.tsx src/components/review/ImageReviewWorkbench.integration.test.tsx`

Expected: PASS with no full workspace reload on save.

- [ ] **Step 6: Commit save-path verification**

```bash
git add src-tauri/tests/review_save_performance.rs src-tauri/tests/review_materialization_lifecycle.rs \
  crates/viewer-infrastructure/tests/continuous_review_costs.rs \
  crates/viewer-infrastructure/tests/continuous_review_dirty_evidence.rs \
  ui/src/app/review/reviewWorkspacePatch.test.ts \
  ui/src/app/review/useContinuousReviewCoordinator.operations.test.tsx \
  ui/src/components/review/ImageReviewWorkbench.integration.test.tsx
git commit -m "test: protect extended markup save performance"
```

### Task 13: Update acceptance scenes, product facts, and protocol documentation

**Files:**
- Modify: `ui/src/acceptance/scenes/reviewWorkbenchScenes.ts`
- Modify: `ui/src/acceptance/scenes/reviewWorkbenchScenes.test.tsx`
- Modify: `scripts/viewer-visual-acceptance.test.mjs`
- Modify: `docs/product/FEATURE_REFERENCE.md`
- Modify: `docs/product/SHORTCUTS.md`
- Modify: `docs/product/USER_GUIDE.md`
- Modify: `docs/protocol/README.md`
- Modify: `docs/PRODUCT_SPEC.md`
- Create: `docs/quality/2026-09-02-unified-image-markup-validation.md`

**Interfaces:**
- Consumes: completed UI/protocol behavior.
- Produces: canonical product documentation and deterministic visual/native acceptance coverage.

- [ ] **Step 1: Add failing acceptance assertions**

Extend the review-workbench scene with point, arrow, ellipse, rectangle, stroke, an open marker menu, and one
selected editable shape. Assert scene invariants:

```ts
expect(screen.getByRole('button', { name: '标记' })).toBeVisible()
expect(document.querySelectorAll('[data-anchor-kind]')).toHaveLength(5)
expect(document.querySelector('[data-anchor-kind="image_arrow"]')).not.toBeNull()
```

Add visual acceptance IDs for wide and compact widths, menu open, arrow selected, ellipse editor near an edge,
and composite magnifier over overlapping annotations.

- [ ] **Step 2: Run acceptance tests and verify new states are absent**

Run: `pnpm test:visual-acceptance && pnpm build:visual-acceptance`

Expected: FAIL until scenes and baselines describe the new states.

- [ ] **Step 3: Update product and protocol facts**

Document:

- the five tools and exact shortcuts;
- click/drag direction and Shift-circle behavior;
- current-tool persistence after save;
- editable current feedback and immutable archives;
- v3 compatibility, one-way v4 promotion, exact `imageArrow.head` semantics, and Agent publication gating;
- the early-development exclusion of signing, notarization, formal installers, store listing, and release work.

Do not describe automatic Agent notification, automatic success judgment, or unimplemented video annotation.

- [ ] **Step 4: Run product/visual policy checks**

Run: `pnpm test:visual-acceptance && pnpm build:visual-acceptance && pnpm test:policy`

Expected: PASS.

- [ ] **Step 5: Commit documentation and acceptance coverage**

```bash
git add ui/src/acceptance scripts/viewer-visual-acceptance.test.mjs \
  docs/product docs/protocol docs/PRODUCT_SPEC.md \
  docs/quality/2026-09-02-unified-image-markup-validation.md
git commit -m "docs: document unified image markup workflow"
```

### Task 14: Run full quality gates and real-project desktop acceptance

**Files:**
- Modify only if a verified defect is found: files owned by Tasks 1–13
- Finalize: `docs/quality/2026-09-02-unified-image-markup-validation.md`

**Interfaces:**
- Consumes: the complete feature.
- Produces: evidence that the implementation satisfies the project's current development-stage acceptance scope.

- [ ] **Step 1: Run formatting, static checks, and focused suites**

Run: `pnpm --dir ui check && pnpm --dir ui test && pnpm --dir ui build && cargo fmt --check && cargo clippy --locked --workspace --all-targets -- -D warnings && cargo test --locked --workspace`

Expected: PASS.

- [ ] **Step 2: Run architecture, protocol, review-loop, and security gates**

Run: `pnpm architecture:boundaries && pnpm test:review-protocol && pnpm test:review-loop && pnpm security`

Expected: PASS with no new dependency exception and no network/capability expansion.

- [ ] **Step 3: Run the repository-wide verification gate**

Run: `pnpm verify`

Expected: PASS.

- [ ] **Step 4: Start one clean Viewer instance and execute desktop acceptance**

Run: `pnpm start:viewer`

Using the existing development test project in the user's Downloads library, verify in the native Viewer:

1. Open landscape, portrait, and square images at fit, zoomed, panned, and rotated states.
2. Create/save/edit/delete point, arrow, brush, rectangle, and ellipse feedback.
3. Confirm Shift creates a visual circle on non-square images.
4. Confirm the unified menu, shortcuts, Space pan, Esc behavior, and read-only presentation.
5. Confirm Save/Cancel remain clickable after zoom/pan and saving does not show a loading-page flash.
6. Confirm the magnifier enlarges image and annotations together without enlarging handles/editor.
7. Confirm archive/history keeps prior geometry and current feedback remains editable.
8. Run the native reader and verify v4 semantics; verify a v3 fixture still reads as v3.

- [ ] **Step 5: Record measured evidence and any scoped corrections**

Add the exact commands, pass counts, authoring latency distribution, tested image ratios, native observations,
v3/v4 reader samples, and final commit hash to the validation document. If a defect appears, first add the
smallest failing automated test, patch the owning module, rerun its focused suite, then rerun Steps 1–4.

- [ ] **Step 6: Commit the final validation record**

```bash
git add docs/quality/2026-09-02-unified-image-markup-validation.md
git commit -m "test: validate unified image markup tools"
```

- [ ] **Step 7: Confirm repository state**

Run: `git status --short && git log --oneline -14`

Expected: no unintended files; all implementation and validation commits are visible.
