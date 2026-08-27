# Viewer Image Annotation Review Workbench Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **Status:** Active

**Goal:** 把 Viewer 的图片大图预览升级为可直接创建、编辑、恢复并发布局部自然语言意见的评审工作台，同时保持历史 `viewer.review/1` 数据与既有非评审预览行为可验证兼容。

**Architecture:** 先在 Domain 建立与显示无关的归一化 Anchor，再以 Application 用例掌管固定范围、首条意见原子建 Draft、版本升级和发布编排；Infrastructure 负责 v1/v2 编解码、目录评审包和崩溃恢复，macOS Platform 只负责受限的标注 PNG 渲染。前端把通用图片视口抽成无 Review 依赖的 `ImagePreviewSurface`，评审状态机、画布工具、就地编辑器和意见栏通过窄接口组合，Tauri DTO 是唯一跨进程边界。

**Tech Stack:** Rust workspace、Tokio、Serde/serde_json、blake3、objc2 ImageIO/CoreGraphics、Tauri 2、React 19、TypeScript、Vitest/Testing Library、Node test runner、现有 Viewer 视觉验收工具链。

**Spec:** `docs/superpowers/specs/2026-08-26-viewer-image-annotation-review-workbench-design.md`

## Execution Checkpoint — 2026-08-26

Tasks 1–13 are implemented, tested and committed through `76e5d0c`. Task 14 remains
**in progress**: functional gates and nine real-App acceptance recipes are implemented;
initial references and complete paired evidence are established. Eight Important findings
from the whole-branch review are being corrected; post-fix visual verification, product-document
updates and the final clean-tree gate remain pending. This is not an implementation-complete claim.

Update: the user accepted the displayed RVW-17 workbench as the first visual baseline.
`pnpm verify:clean` passed on `2d68d3d` with a clean worktree. See the
[baseline confirmation](../../reviews/2026-08-26-image-review-visual-baseline.md).
The user explicitly permitted local Playwright capture. All18 full-size reference/product pairs
at clean `8009656` passed initial-baseline inspection after an acceptance-readiness correction.
The fixes must be compared against those frozen references; Step 5, Step 6, Step 8 and the whole
plan remain incomplete until their final gates pass.

See [the development checkpoint](../../progress/2026-08-26-image-annotation-review-workbench-checkpoint.md)
for commit mapping, verified scope, capture limitations and the next decision. Do not rerun
product discovery or expand the approved scope when resuming.

## Global Constraints

- 第一条意见必须是“非空自然语言 + 有效 Anchor”；保存失败不得留下空 Draft，也不得丢失前端临时输入。
- 自动开始只能使用进入大图时由后端解析并冻结的 `ReviewScopeProposal`；前端不得提交文件路径、资产摘要或自行构造 `AssetVersion`。
- 一个项目同一时刻最多一个可写人工 Draft；Draft 成员名单创建后保持固定，范围外图片只能只读预览。
- `FeedbackAnchor` 是定位权威；完成包中的 PNG 只是只读衍生物，原始图片不得被修改、覆盖或移动。
- 本轮只提供图片整图、矩形和画笔 Anchor 的 UI；协议保留 `videoPoint`、`videoRange`，但不增加视频标注界面。
- v2 必须是 v1 的严格超集：历史 v1 Completed Round 继续可读，未使用 v2 能力的 v1 Draft 可原样保存和完成；第一次 v2 写入必须原子升级。
- v1 Completed Round 文件必须保持逐字节不变；迁移只允许生成 v2 索引元数据，不重写历史 Round。
- v2 完成记录固定为 `.viewer/reviews/rounds/<round-id>/round.json` 和 `.viewer/reviews/rounds/<round-id>/artifacts/<asset-version-id>-annotation.png`。
- 所有持久化相对位置必须经过安全路径校验；发布时必须重新限制文件类型、大小并核对摘要，任何临时路径都不得进入 Tauri DTO 或开放协议。
- 发布顺序固定为：写临时评审包、同步包内文件、同步临时目录、原子重命名、同步 `rounds` 父目录、原子替换索引、同步评审根目录、最后清理 Draft。
- 浏览曝光、停留时长、滚动位置和是否进入大图不参与 Outcome；完成时无 Feedback 且可评审的固定素材仍按默认通过。
- 复用现有 Viewer token、按钮、图标与响应式规则；不新增手绘 SVG、图标依赖或独立设计系统。
- 代码签名、Apple 公证、正式安装包、公开发布和发售不属于本轮计划、验收、阻塞或收尾事项。

## Dependency Order and File Ownership

```text
Domain anchors
  -> v1 characterization split
  -> v2 protocol + catalog records
  -> repository bundle transaction
  -> artifact renderer + completion publication
  -> session use cases
  -> Tauri/UI bridge
  -> reusable preview surface
  -> frontend annotation state
  -> workbench components
  -> workspace/grid integration
  -> external reader + end-to-end compatibility
  -> visual acceptance + product documentation + clean-tree gate
```

- `viewer-domain/review/feedback.rs` owns only valid Anchor values and Feedback invariants.
- `viewer-application/review.rs` owns persistence-facing record/version contracts; it does not encode JSON or touch the filesystem.
- `viewer-application/review_artifact.rs` owns rendering/publication port types; it is the only Application module allowed to mention internal `PathBuf` artifact leases.
- `viewer-application/review_session.rs` owns review lifecycle and atomic mutations; it does not render pixels or choose repository paths.
- `viewer-infrastructure/review/protocol/` owns deterministic v1/v2 JSON codecs and schemas’ runtime interpretation.
- `viewer-infrastructure/review/catalog.rs` owns v1 index migration and record lookup; `bundle.rs` owns v2 directory publication and recovery.
- `viewer-platform-macos/image/review_annotation.rs` owns oriented PNG rasterization; it does not know Review Draft lifecycle or catalog structure.
- `src-tauri/dto/review.rs` is the only Rust-to-TypeScript shape boundary; paths and publication internals are absent.
- `ui/src/components/imagePreview/ImagePreviewSurface.tsx` owns viewport geometry and image interaction, with no import from `app/review` or `api/viewer`.
- `ui/src/app/review/` owns client state and commands; `ui/src/components/review/` owns presentation and pointer/keyboard event routing.

---

### Task 1: Add Validated Image Anchor Value Objects

**Files:**
- Modify: `crates/viewer-domain/src/review/feedback.rs`
- Modify: `crates/viewer-domain/src/review/round.rs`
- Modify: `crates/viewer-domain/src/review/mod.rs`
- Modify: `crates/viewer-domain/tests/review_round.rs`

**Interfaces:**
- Consumes: existing `Feedback`, `FeedbackTarget`, `ReviewValueError`, `NormalizedRect`.
- Produces: `NormalizedPoint::new(x, y)`, `ImageStroke::new(points)`, `FeedbackAnchor::{Asset, ImageRect, ImageStroke, VideoPoint, VideoRange}`, `MAX_IMAGE_STROKE_POINTS = 2_048`, and `MAX_IMAGE_STROKE_POINTS_PER_ROUND = 200_000`.

- [x] **Step 1: Write failing Domain tests for rectangle, stroke, and aggregate limits**

```rust
#[test]
fn image_anchors_accept_normalized_rectangles_and_strokes() {
    let rect = NormalizedRect::new(0.1, 0.2, 0.3, 0.4).unwrap();
    let stroke = ImageStroke::new(vec![
        NormalizedPoint::new(0.1, 0.2).unwrap(),
        NormalizedPoint::new(0.4, 0.6).unwrap(),
    ]).unwrap();

    assert_eq!(FeedbackAnchor::ImageRect(rect).kind_name(), "imageRect");
    assert_eq!(FeedbackAnchor::ImageStroke(stroke).kind_name(), "imageStroke");
}

#[test]
fn image_strokes_reject_invalid_coordinates_and_unbounded_payloads() {
    assert_eq!(NormalizedPoint::new(f64::NAN, 0.2), Err(ReviewValueError::InvalidNumber));
    assert_eq!(NormalizedPoint::new(1.1, 0.2), Err(ReviewValueError::InvalidNumber));
    assert_eq!(
        ImageStroke::new(vec![NormalizedPoint::new(0.2, 0.2).unwrap(); 2]),
        Err(ReviewValueError::InvalidNumber),
    );
    assert_eq!(
        ImageStroke::new(vec![
            NormalizedPoint::new(0.2, 0.1).unwrap(),
            NormalizedPoint::new(0.2, 0.8).unwrap(),
        ]),
        Err(ReviewValueError::InvalidNumber),
    );
    assert_eq!(
        ImageStroke::new(
            (0..=MAX_IMAGE_STROKE_POINTS)
                .map(|index| NormalizedPoint::new(index as f64 / MAX_IMAGE_STROKE_POINTS as f64, 0.5).unwrap())
                .collect(),
        ),
        Err(ReviewValueError::LimitExceeded),
    );
}

#[test]
fn local_image_anchors_require_confirmed_image_dimensions() {
    let mut unknown_size = image_asset(1);
    unknown_size.media = ReviewMedia::Image { width: None, height: None };
    let unknown_id = unknown_size.id;
    let mut image_draft = review_draft(vec![unknown_size]);
    assert_eq!(
        image_draft.upsert_feedback(feedback(
            1,
            unknown_id,
            "标出领口",
            FeedbackAnchor::ImageRect(NormalizedRect::new(0.1, 0.1, 0.2, 0.2).unwrap()),
        )),
        Err(ReviewRoundError::AnchorUnavailable),
    );

    let video = video_asset(2, Some(1_000));
    let video_id = video.id;
    let mut video_draft = review_draft(vec![video]);
    assert_eq!(
        video_draft.upsert_feedback(feedback(
            2,
            video_id,
            "错误媒体",
            FeedbackAnchor::ImageStroke(ImageStroke::new(vec![
                NormalizedPoint::new(0.1, 0.1).unwrap(),
                NormalizedPoint::new(0.2, 0.3).unwrap(),
            ]).unwrap()),
        )),
        Err(ReviewRoundError::AnchorMediaMismatch),
    );
}
```

- [x] **Step 2: Run the focused Domain tests and confirm the new types are missing**

Run: `cargo test --locked -p viewer-domain --test review_round image_anchor -- --nocapture`

Expected: FAIL because `NormalizedPoint`, `ImageStroke`, and the new variants are not defined.

- [x] **Step 3: Implement immutable normalized geometry and rename the internal rectangle variant**

```rust
pub const MAX_IMAGE_STROKE_POINTS: usize = 2_048;
pub const MAX_IMAGE_STROKE_POINTS_PER_ROUND: usize = 200_000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormalizedPoint { x: f64, y: f64 }

impl NormalizedPoint {
    pub fn new(x: f64, y: f64) -> Result<Self, ReviewValueError> {
        (x.is_finite() && y.is_finite() && (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y))
            .then_some(Self { x, y })
            .ok_or(ReviewValueError::InvalidNumber)
    }
    pub fn x(&self) -> f64 { self.x }
    pub fn y(&self) -> f64 { self.y }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImageStroke { points: Vec<NormalizedPoint> }

fn normalized_bounds(points: &[NormalizedPoint]) -> (f64, f64, f64, f64) {
    points.iter().fold(
        (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY),
        |(min_x, max_x, min_y, max_y), point| {
            (
                min_x.min(point.x()),
                max_x.max(point.x()),
                min_y.min(point.y()),
                max_y.max(point.y()),
            )
        },
    )
}

impl ImageStroke {
    pub fn new(points: Vec<NormalizedPoint>) -> Result<Self, ReviewValueError> {
        if points.len() > MAX_IMAGE_STROKE_POINTS { return Err(ReviewValueError::LimitExceeded); }
        let (min_x, max_x, min_y, max_y) = normalized_bounds(&points);
        if points.len() < 2 || max_x <= min_x || max_y <= min_y {
            return Err(ReviewValueError::InvalidNumber);
        }
        Ok(Self { points })
    }
    pub fn points(&self) -> &[NormalizedPoint] { &self.points }
}

pub enum FeedbackAnchor {
    Asset,
    ImageRect(NormalizedRect),
    ImageStroke(ImageStroke),
    VideoPoint { position_us: u64 },
    VideoRange { start_us: u64, end_us: u64 },
}
```

In `ReviewDraft::upsert_feedback`, validate `ImageRect` and `ImageStroke` only against `ReviewMedia::Image { width: Some(width), height: Some(height) }` with nonzero dimensions. Count all `ImageStroke` points after the candidate replacement and reject totals above `MAX_IMAGE_STROKE_POINTS_PER_ROUND`; this prevents many individually valid strokes from bypassing the round bound.

- [x] **Step 4: Run the complete Domain review suite**

Run: `cargo test --locked -p viewer-domain review -- --nocapture && cargo test --locked -p viewer-domain --test review_round`

Expected: PASS; existing `Asset` and video Anchor tests remain green after replacing internal `ImageRegion` references with `ImageRect`.

- [x] **Step 5: Commit the Domain contract**

```bash
git add crates/viewer-domain/src/review/feedback.rs crates/viewer-domain/src/review/round.rs crates/viewer-domain/src/review/mod.rs crates/viewer-domain/tests/review_round.rs
git commit -m "feat(review): add validated image annotation anchors"
```

---

### Task 2: Split and Characterize the v1 Protocol Without Changing Bytes

**Files:**
- Delete: `crates/viewer-infrastructure/src/review/protocol.rs`
- Create: `crates/viewer-infrastructure/src/review/protocol/mod.rs`
- Create: `crates/viewer-infrastructure/src/review/protocol/common.rs`
- Create: `crates/viewer-infrastructure/src/review/protocol/v1.rs`
- Modify: `crates/viewer-infrastructure/src/review/mod.rs`
- Read: `tests/fixtures/review-protocol/review-draft-v1.valid.json`
- Create: `crates/viewer-infrastructure/src/review/protocol/tests.rs`
- Modify: `tests/review_protocol_contract.rs`

**Interfaces:**
- Consumes: Task 1 Anchor types; existing v1 fixtures and public codec exports.
- Produces: behavior-identical `v1::{encode_draft, decode_draft, encode_completed, decode_completed, encode_catalog, decode_catalog}` and shared bounded JSON/hash helpers under `protocol::common`.

- [x] **Step 1: Add byte-level characterization tests before moving code**

```rust
#[test]
fn v1_round_fixture_round_trips_byte_for_byte() {
    let fixture = include_bytes!("../../../../../tests/fixtures/review-protocol/review-round-v1.valid.json");
    let decoded = v1::decode_completed(fixture).unwrap();
    assert_eq!(v1::encode_completed(&decoded).unwrap(), fixture);
}

#[test]
fn v1_image_region_maps_to_internal_image_rect() {
    let fixture = include_bytes!("../../../../../tests/fixtures/review-protocol/review-draft-v1.valid.json");
    let decoded = v1::decode_draft(fixture).unwrap();
    assert!(decoded.feedback.iter().flat_map(|item| &item.targets).any(|target| {
        matches!(target.anchor, FeedbackAnchor::ImageRect(_))
    }));
    assert!(String::from_utf8(v1::encode_draft(&decoded).unwrap()).unwrap().contains("\"imageRegion\""));
}
```

- [x] **Step 2: Run the characterization tests before the split**

Run: `cargo test --locked -p viewer-infrastructure review::protocol::tests::v1_ -- --nocapture && cargo test --locked -p viewer-infrastructure --test review_protocol_contract`

Expected: PASS after routing the existing test module to `v1`; record the fixture digests printed by the existing contract test and do not edit fixture whitespace.

- [x] **Step 3: Move v1 DTOs and codecs into focused modules**

```rust
// protocol/mod.rs
mod common;
mod tests;
pub mod v1;

pub const REVIEW_PROTOCOL_V1: &str = "viewer.review/1";
pub use common::{MAX_REVIEW_DOCUMENT_BYTES, MAX_REVIEW_INDEX_BYTES, ReviewProtocolError};

pub fn detect_review_protocol(bytes: &[u8]) -> Result<&'static str, ReviewProtocolError> {
    common::detect_protocol(bytes, &[REVIEW_PROTOCOL_V1])
}
```

Keep every v1 stored field name, array order, canonical newline and validation branch unchanged. The only semantic adapter is `StoredAnchorV1::ImageRegion <-> FeedbackAnchor::ImageRect`.

- [x] **Step 4: Run fixture, repository, and public-export regressions**

Run: `cargo test --locked -p viewer-infrastructure review::protocol -- --nocapture && cargo test --locked -p viewer-infrastructure --test review_protocol_contract --test review_repository`

Expected: PASS with byte-identical v1 output and no change to public consumers.

- [x] **Step 5: Commit the behavior-preserving split**

```bash
git add crates/viewer-infrastructure/src/review/protocol crates/viewer-infrastructure/src/review/mod.rs tests/review_protocol_contract.rs
git commit -m "refactor(review): isolate v1 protocol codec"
```

---

### Task 3: Add the `viewer.review/2` Schemas and Deterministic Codec

**Files:**
- Create: `crates/viewer-infrastructure/src/review/protocol/v2.rs`
- Modify: `crates/viewer-infrastructure/src/review/protocol/mod.rs`
- Modify: `crates/viewer-infrastructure/src/review/protocol/tests.rs`
- Modify: `crates/viewer-application/src/review.rs`
- Modify: `crates/viewer-application/src/lib.rs`
- Create: `docs/protocol/viewer-review-draft-v2.schema.json`
- Create: `docs/protocol/viewer-review-index-v2.schema.json`
- Create: `docs/protocol/viewer-review-round-v2.schema.json`
- Create: `tests/fixtures/review-protocol/review-draft-v2.valid.json`
- Create: `tests/fixtures/review-protocol/review-index-v2.valid.json`
- Create: `tests/fixtures/review-protocol/review-round-v2.valid.json`
- Create: `scripts/review-protocol/schema-contract.test.mjs`
- Modify: `package.json`
- Modify: `tests/review_protocol_contract.rs`

**Interfaces:**
- Consumes: Task 2 `common` helpers and `v1` codec; Task 1 Anchor values.
- Produces: `REVIEW_PROTOCOL_V2`, `DecodedReview<T> { version: ReviewProtocolVersion, value: T }`, v2 codecs, and strict union dispatch for v1/v2.

- [x] **Step 1: Write failing codec and schema contract tests**

```rust
#[test]
fn v2_round_preserves_rect_stroke_and_reserved_video_anchors() {
    let bytes = include_bytes!("../../../../../tests/fixtures/review-protocol/review-round-v2.valid.json");
    let decoded = decode_completed_versioned(bytes).unwrap();
    assert_eq!(decoded.version, ReviewProtocolVersion::V2);
    assert!(decoded.value.feedback.iter().flat_map(|item| &item.targets).any(|target| {
        matches!(target.anchor, FeedbackAnchor::ImageStroke(_))
    }));
    assert_eq!(encode_completed_v2(&decoded.value).unwrap(), bytes);
}

#[test]
fn protocol_dispatch_rejects_unknown_versions_instead_of_guessing() {
    let bytes = br#"{"protocolVersion":"viewer.review/99"}"#;
    assert_eq!(decode_completed_versioned(bytes), Err(ReviewProtocolError::UnsupportedVersion));
}
```

Add a Node schema-declaration test that parses all three schemas and asserts their closed-object, coordinate, point-count, location and digest constraints. Rust codec negative fixtures exercise a stroke with 2,049 points, non-finite JSON numbers, an escaping artifact location, an incomplete ordinal mapping, and an invalid 64-character lowercase digest.

```js
test('v2 schemas declare closed bounded anchor and artifact contracts', async () => {
  const round = JSON.parse(await readFile('docs/protocol/viewer-review-round-v2.schema.json', 'utf8'))
  const anchor = round.$defs.anchor
  const stroke = anchor.oneOf.find((entry) => entry.properties.kind.const === 'imageStroke')
  assert.equal(round.additionalProperties, false)
  assert.equal(stroke.properties.points.minItems, 2)
  assert.equal(stroke.properties.points.maxItems, 2048)
  assert.equal(round.$defs.digest.pattern, '^[0-9a-f]{64}$')
  assert.equal(round.$defs.artifact.additionalProperties, false)
})
```

- [x] **Step 2: Run the new contract tests and confirm v2 is unavailable**

Run: `cargo test --locked -p viewer-infrastructure review::protocol::tests::v2_ -- --nocapture && node --test scripts/review-protocol/schema-contract.test.mjs`

Expected: FAIL because `REVIEW_PROTOCOL_V2`, the v2 DTOs, fixtures, and schemas do not exist.

- [x] **Step 3: Implement the v2 stored Anchor union and versioned dispatch**

```rust
pub const REVIEW_PROTOCOL_V2: &str = "viewer.review/2";

// viewer-application/src/review.rs
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewProtocolVersion { V1, V2 }

pub struct DecodedReview<T> {
    pub version: ReviewProtocolVersion,
    pub value: T,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind")]
enum StoredAnchorV2 {
    #[serde(rename = "asset")]
    Asset,
    #[serde(rename = "imageRect")]
    ImageRect { x: f64, y: f64, width: f64, height: f64 },
    #[serde(rename = "imageStroke")]
    ImageStroke { points: Vec<StoredPointV2> },
    #[serde(rename = "videoPoint")]
    VideoPoint { position_us: u64 },
    #[serde(rename = "videoRange")]
    VideoRange { start_us: u64, end_us: u64 },
}
```

Define `ReviewProtocolVersion { V1, V2 }` once in `viewer_application::review` and export it from `viewer-application/src/lib.rs`; Infrastructure codecs import that single type. Use camelCase field names in JSON (`protocolVersion`, `positionUs`, `startUs`, `endUs`) through explicit Serde renames. `decode_*_versioned` must inspect only the root `protocolVersion` string, dispatch to exactly one decoder, and apply Domain constructors to every coordinate.

- [x] **Step 4: Implement schemas with explicit limits and closed objects**

Set `additionalProperties: false` on every object; set stroke `minItems: 2`, `maxItems: 2048`; set all coordinates to `minimum: 0`, `maximum: 1`; set digest pattern to `^[0-9a-f]{64}$`; restrict index locations to `rounds/...` paths relative to `.viewer/reviews/`; and restrict Round artifact locations to `artifacts/...` paths relative to that Round bundle. Both location forms reject `..`, a leading slash and backslash. Each artifact record includes `assetVersionId`, `relativePath`, `blake3`, `mediaType: image/png`, nonzero `width`/`height`, and an `annotations` array of unique `{ ordinal, feedbackId }` pairs.

Set the root script to `"test:review-protocol": "node --test scripts/review-protocol/read-latest.test.mjs scripts/review-protocol/schema-contract.test.mjs"` so the existing v1 reader and the v2 schema declaration contract remain one gate.

- [x] **Step 5: Run Rust and schema contract suites**

Run: `cargo test --locked -p viewer-infrastructure review::protocol -- --nocapture && cargo test --locked -p viewer-infrastructure --test review_protocol_contract && node --test scripts/review-protocol/schema-contract.test.mjs && pnpm test:review-protocol`

Expected: PASS for v1 byte fixtures and all v2 positive/negative fixtures.

- [x] **Step 6: Commit the versioned protocol**

```bash
git add crates/viewer-infrastructure/src/review/protocol crates/viewer-infrastructure/src/review/mod.rs crates/viewer-application/src/review.rs crates/viewer-application/src/lib.rs docs/protocol/viewer-review-*-v2.schema.json tests/fixtures/review-protocol/review-*-v2.valid.json tests/review_protocol_contract.rs scripts/review-protocol/schema-contract.test.mjs package.json
git commit -m "feat(review): define viewer review protocol v2"
```

---

### Task 4: Introduce Versioned Drafts and v2 Catalog Records

**Files:**
- Modify: `crates/viewer-application/src/review.rs`
- Modify: `crates/viewer-application/src/lib.rs`
- Modify: `crates/viewer-application/src/review_session.rs`
- Modify: `crates/viewer-application/tests/review_session_start.rs`
- Modify: `crates/viewer-application/tests/review_session_mutation.rs`
- Modify: `crates/viewer-application/tests/review_session_completion.rs`
- Create: `crates/viewer-infrastructure/src/review/catalog.rs`
- Modify: `crates/viewer-infrastructure/src/review/protocol/v1.rs`
- Modify: `crates/viewer-infrastructure/src/review/protocol/v2.rs`
- Modify: `crates/viewer-infrastructure/src/review/protocol/tests.rs`
- Modify: `crates/viewer-infrastructure/src/review/repository.rs`
- Modify: `crates/viewer-infrastructure/src/review/mod.rs`
- Modify: `crates/viewer-infrastructure/src/review/provider.rs`
- Test: `crates/viewer-application/tests/review_catalog.rs`
- Modify: `tests/review_repository.rs`

**Interfaces:**
- Consumes: Task 3 `ReviewProtocolVersion` and versioned codecs.
- Produces: `PersistedReviewDraft`, `ReviewRoundRecord`, `ReviewRecordLocation`, and repository load/save methods that preserve the decoded version.

- [x] **Step 1: Write failing Application contract tests for versioned records**

```rust
#[test]
fn catalog_resolves_round_records_without_exposing_absolute_paths() {
    let record = ReviewRoundRecord {
        review_round_id: ReviewRoundId::from_u128(7),
        protocol_version: ReviewProtocolVersion::V2,
        location: ReviewRecordLocation::new("rounds/00000000-0000-0000-0000-000000000007/round.json").unwrap(),
        blake3: [0x2a; 32],
    };
    assert!(!record.location.as_str().starts_with('/'));
    assert!(ReviewRecordLocation::new("../outside.json").is_err());
}
```

- [x] **Step 2: Run the focused tests and verify the record types are missing**

Run: `cargo test --locked -p viewer-application --test review_catalog`

Expected: FAIL because the versioned persistence types are undefined.

- [x] **Step 3: Define persistence-neutral record types and update the port**

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct PersistedReviewDraft {
    pub protocol_version: ReviewProtocolVersion,
    pub draft: ReviewDraft,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewRepositoryInspection {
    pub catalog: ReviewCatalog,
    pub active_draft: Option<PersistedReviewDraft>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewRoundRecord {
    pub review_round_id: ReviewRoundId,
    pub protocol_version: ReviewProtocolVersion,
    pub location: ReviewRecordLocation,
    pub blake3: [u8; 32],
}

pub trait ReviewRepositoryPort: Send + Sync {
    fn load_active_draft(&self) -> Result<Option<PersistedReviewDraft>, ReviewRepositoryError>;
    fn load_draft(&self, stream_id: ReviewStreamId, round_id: ReviewRoundId)
        -> Result<Option<PersistedReviewDraft>, ReviewRepositoryError>;
    fn save_draft(&self, draft: &PersistedReviewDraft) -> Result<(), ReviewRepositoryError>;
    fn load_completed(&self, stream_id: ReviewStreamId, round_id: ReviewRoundId)
        -> Result<Option<ReviewSnapshot>, ReviewRepositoryError>;
    fn publish(&self, snapshot: &ReviewSnapshot) -> Result<(), ReviewRepositoryError>;
}
```

`ReviewRecordLocation::new` accepts only normalized forward-slash relative components and rejects empty components, `.`, `..`, NUL, backslash, drive prefixes and absolute paths. `ReviewStreamHead` stores ordered `completed_rounds: Vec<ReviewRoundRecord>` and exposes `completed_round_ids()` as a derived iterator for existing callers. Update `ReviewSessionService`, all fake repository implementations, catalog fixtures and both protocol codecs in this task so the commit compiles and all v1 projections remain behavior-compatible.

- [x] **Step 4: Write failing v1-index migration tests**

```rust
#[test]
fn reading_v1_catalog_projects_v2_records_without_rewriting_history() {
    let project = PersistentProject::new();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/review-protocol/project/.viewer/reviews");
    fs::create_dir_all(project.reviews().join("rounds")).unwrap();
    let mut index: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.join("index.json")).unwrap()).unwrap();
    index["streams"].as_array_mut().unwrap().remove(0);
    fs::write(
        project.reviews().join("index.json"),
        format!("{}\n", serde_json::to_string_pretty(&index).unwrap()),
    ).unwrap();
    let legacy_round = project.reviews()
        .join("rounds/00000000-0000-4000-8000-000000000202.json");
    fs::copy(
        fixture.join("rounds/00000000-0000-4000-8000-000000000202.json"),
        &legacy_round,
    ).unwrap();
    let index_path = project.reviews().join("index.json");
    let index_before = fs::read(&index_path).unwrap();
    let round_before = fs::read(&legacy_round).unwrap();

    let repository = open_writable(&project).unwrap();
    let catalog = repository.load_catalog().unwrap();
    let record = &catalog.streams[0].completed_rounds[0];

    assert_eq!(record.protocol_version, ReviewProtocolVersion::V1);
    assert_eq!(record.blake3, *blake3::hash(&round_before).as_bytes());
    assert_eq!(fs::read(index_path).unwrap(), index_before);
    assert_eq!(fs::read(legacy_round).unwrap(), round_before);
}
```

Also test that one missing, malformed, identity-mismatched, symlinked, or digest-changing historical Round makes migration return `RecoveryRequired` and leaves the v1 index untouched.

- [x] **Step 5: Implement atomic v1-index migration and version-preserving draft I/O**

Read every v1 completed filename from the v1 index, validate it with the v1 decoder and expected project/stream/round identity, compute BLAKE3 from its exact bytes, and project `ReviewRoundRecord` values in memory. Neither read-only nor writer open rewrites the v1 index. Expose `prepare_v2_catalog_migration(catalog_document) -> Result<ReviewCatalog, ReviewRepositoryError>` for Task 5; only the first successful v2 publication encodes and atomically replaces the on-disk catalog. A v1 Draft save or v1 completion leaves the catalog at v1. Keep the existing explicit start use case on v1 during this compatibility-only task; Task 5 switches new Drafts to v2 in the same commit that makes v2 publication durable.

- [x] **Step 6: Run catalog, protocol, and repository regressions**

Run: `cargo test --locked -p viewer-application --test review_catalog && cargo test --locked -p viewer-infrastructure review::catalog -- --nocapture && cargo test --locked -p viewer-infrastructure --test review_repository`

Expected: PASS; v1 Round fixture digests and bytes remain unchanged.

- [x] **Step 7: Commit versioned repository metadata**

```bash
git add crates/viewer-application/src/review.rs crates/viewer-application/src/lib.rs crates/viewer-application/src/review_session.rs crates/viewer-application/tests/review_catalog.rs crates/viewer-application/tests/review_session_start.rs crates/viewer-application/tests/review_session_mutation.rs crates/viewer-application/tests/review_session_completion.rs crates/viewer-infrastructure/src/review/catalog.rs crates/viewer-infrastructure/src/review/protocol crates/viewer-infrastructure/src/review/repository.rs crates/viewer-infrastructure/src/review/mod.rs crates/viewer-infrastructure/src/review/provider.rs tests/review_protocol_contract.rs tests/review_repository.rs
git commit -m "feat(review): add versioned catalog records"
```

---

### Task 5: Publish Crash-Consistent v2 Round Bundles

**Files:**
- Create: `crates/viewer-application/src/review_artifact.rs`
- Modify: `crates/viewer-application/src/lib.rs`
- Modify: `crates/viewer-application/src/review_session.rs`
- Modify: `crates/viewer-application/tests/review_session_start.rs`
- Modify: `crates/viewer-application/tests/review_session_mutation.rs`
- Modify: `crates/viewer-application/tests/review_session_completion.rs`
- Create: `crates/viewer-infrastructure/src/review/bundle.rs`
- Modify: `crates/viewer-infrastructure/src/review/repository.rs`
- Modify: `crates/viewer-infrastructure/src/review/atomic.rs`
- Modify: `crates/viewer-infrastructure/src/review/mod.rs`
- Modify: `tests/review_repository.rs`

**Interfaces:**
- Consumes: Task 4 `ReviewProtocolVersion`, `ReviewRoundRecord`, and updated repository port.
- Produces: `ReviewRenderedArtifact`, `ReviewArtifactAnnotation`, `ReviewPublication`, bounded artifact validation, atomic directory commit, and orphan-bundle recovery.

- [x] **Step 1: Write failure-injection tests for every durable boundary**

```rust
#[test]
fn v2_publish_recovers_after_bundle_rename_before_catalog_replace() {
    let project = PersistentProject::new();
    let publication = v2_publication(project.project_id);
    let round_id = publication.snapshot.review_round_id;
    let repository = open_writable_with_faults(
        &project,
        fail_once(ReviewRepositoryFaultPoint::AfterBundleDurableBeforeIndex),
    ).unwrap();
    assert_eq!(repository.publish(&publication), Err(ReviewRepositoryError::Unavailable));
    drop(repository);

    let reopened = open_writable(&project).unwrap();
    let catalog = reopened.load_catalog().unwrap();
    assert!(catalog.streams[0].completed_round_ids().contains(&round_id));
    assert!(project.reviews().join(format!("rounds/{round_id}/round.json")).is_file());
}

#[test]
fn v2_publish_rejects_artifact_digest_or_path_substitution() {
    let project = PersistentProject::new();
    let mut publication = v2_publication(project.project_id);
    publication.artifacts[0].blake3 = [0; 32];
    assert_eq!(open_writable(&project).unwrap().publish(&publication), Err(ReviewRepositoryError::InvalidData));
}
```

Add this exact test helper beside the existing `PersistentProject`, `review_draft`, `completed_round`, `fail_once` and `open_writable_with_faults` helpers:

```rust
fn v2_publication(project_id: ProjectId) -> ReviewPublication {
    let mut draft = review_draft(
        project_id,
        ReviewStreamId::from_u128(21),
        ReviewRoundId::from_u128(22),
    );
    let asset_version_id = draft.assets[0].id;
    let feedback_id = FeedbackId::from_u128(23);
    draft.upsert_feedback(Feedback::new(
        feedback_id,
        "右手结构需要修正".into(),
        1_100,
        vec![FeedbackTarget {
            asset_version_id,
            anchor: FeedbackAnchor::ImageRect(
                NormalizedRect::new(0.2, 0.3, 0.4, 0.2).unwrap(),
            ),
        }],
    ).unwrap()).unwrap();
    let artifact_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/images/alpha.png");
    let artifact_bytes = fs::read(&artifact_path).unwrap();
    ReviewPublication {
        protocol_version: ReviewProtocolVersion::V2,
        snapshot: draft.complete(2_000).unwrap(),
        artifacts: vec![ReviewRenderedArtifact {
            asset_version_id,
            temporary_path: artifact_path,
            media_type: "image/png".into(),
            width: 640,
            height: 480,
            size_bytes: artifact_bytes.len() as u64,
            blake3: *blake3::hash(&artifact_bytes).as_bytes(),
            annotations: vec![ReviewArtifactAnnotation { ordinal: 1, feedback_id }],
        }],
    }
}
```

Cover failures after each artifact file sync, after `round.json` sync, after temp-directory sync, after rename, before index replace and after index replace. Assert every reopen yields exactly one of: clean uncommitted Draft, recoverable complete bundle, or indexed Completed Round—never a catalog entry pointing to missing data.

- [x] **Step 2: Run repository tests and confirm bundle publication is absent**

Run: `cargo test --locked -p viewer-infrastructure review::bundle -- --nocapture && cargo test --locked -p viewer-infrastructure --test review_repository`

Expected: FAIL because v2 bundle types, fault points, and directory commit are missing.

- [x] **Step 3: Define internal artifact and publication contracts**

```rust
pub const MAX_REVIEW_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_REVIEW_ARTIFACT_PIXELS: u64 = 16_777_216;
pub const MAX_REVIEW_BUNDLE_BYTES: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewArtifactAnnotation {
    pub ordinal: u32,
    pub feedback_id: FeedbackId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewRenderedArtifact {
    pub asset_version_id: AssetVersionId,
    pub temporary_path: PathBuf,
    pub media_type: String,
    pub width: u32,
    pub height: u32,
    pub size_bytes: u64,
    pub blake3: [u8; 32],
    pub annotations: Vec<ReviewArtifactAnnotation>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewPublication {
    pub protocol_version: ReviewProtocolVersion,
    pub snapshot: ReviewSnapshot,
    pub artifacts: Vec<ReviewRenderedArtifact>,
}

pub trait ReviewRepositoryPort: Send + Sync {
    fn load_catalog(&self) -> Result<ReviewCatalog, ReviewRepositoryError>;
    fn load_active_draft(&self) -> Result<Option<PersistedReviewDraft>, ReviewRepositoryError>;
    fn load_draft(&self, stream_id: ReviewStreamId, round_id: ReviewRoundId)
        -> Result<Option<PersistedReviewDraft>, ReviewRepositoryError>;
    fn save_draft(&self, draft: &PersistedReviewDraft) -> Result<(), ReviewRepositoryError>;
    fn delete_draft(&self, stream_id: ReviewStreamId, round_id: ReviewRoundId)
        -> Result<(), ReviewRepositoryError>;
    fn load_completed(&self, stream_id: ReviewStreamId, round_id: ReviewRoundId)
        -> Result<Option<ReviewSnapshot>, ReviewRepositoryError>;
    fn publish(&self, publication: &ReviewPublication) -> Result<(), ReviewRepositoryError>;
}
```

`ReviewRenderedArtifact` is Application-internal and must never derive `Serialize`; Tauri and protocol DTO modules must not import it. Reject a single artifact above 64 MiB or 16,777,216 pixels, a publication above 4 GiB, more artifact records than fixed assets, duplicate Asset Version IDs, non-PNG media, zero dimensions, incomplete ordinal mappings, and any mapping that references a Feedback outside that asset. At this task boundary, switch newly created explicit-start Drafts to v2 and update `ReviewSessionService` plus its three fake repositories so completion publishes `ReviewPublication { protocol_version, snapshot, artifacts: vec![] }`; Task 7 replaces the empty v2 local-annotation path with real rendering before any UI can create such Anchors.

- [x] **Step 4: Implement the exact bundle transaction**

Before creating a bundle, call `prepare_v2_catalog_migration` so every historical v1 Round is valid and its exact v2 record is ready while the v1 index remains untouched. Then create `.viewer/reviews/rounds/.<round-id>.tmp-<nonce>/`, copy each regular non-symlink PNG through a bounded reader while recomputing BLAKE3, write the deterministic v2 `round.json` with each artifact’s Asset Version ID, `artifacts/...` location, digest, width, height and complete `(ordinal, feedbackId)` mapping, sync every file, sync the temp directory, rename it to `<round-id>/`, and sync `rounds/`. Append the new record to the prepared v2 catalog, atomically replace `index.json`, sync `.viewer/reviews/`, and remove the Draft. This index replacement is the one and only v1-to-v2 catalog migration write.

If `<round-id>/` exists, validate its manifest, all artifact digests and full identity before treating it as a recoverable orphan. A writer reopen removes only transaction-owned `.<round-id>.tmp-<nonce>` directories after verifying their exact name, containment and absence from the catalog; their Draft remains active. Read-only opens never expose those directories as Completed data. Any other unknown entry, symlink, extra artifact, digest mismatch or conflicting Round becomes `RecoveryRequired`.

- [x] **Step 5: Keep v1 publication on its existing byte path**

For `ReviewProtocolVersion::V1`, call the characterized single-file create-once path and v1 encoder. Assert `ReviewPublication.artifacts` is empty. This branch is the compatibility path for a resumed v1 Draft that never receives a v2 Anchor.

- [x] **Step 6: Run bundle, repository, and protocol suites**

Run: `cargo test --locked -p viewer-infrastructure review::bundle -- --nocapture && cargo test --locked -p viewer-infrastructure --test review_repository --test review_protocol_contract`

Expected: PASS across all injected crash points and both protocol versions.

- [x] **Step 7: Commit transactional bundle publication**

```bash
git add crates/viewer-application/src/review_artifact.rs crates/viewer-application/src/lib.rs crates/viewer-application/src/review_session.rs crates/viewer-application/tests/review_session_start.rs crates/viewer-application/tests/review_session_mutation.rs crates/viewer-application/tests/review_session_completion.rs crates/viewer-infrastructure/src/review/bundle.rs crates/viewer-infrastructure/src/review/repository.rs crates/viewer-infrastructure/src/review/atomic.rs crates/viewer-infrastructure/src/review/mod.rs tests/review_repository.rs
git commit -m "feat(review): publish crash-consistent review bundles"
```

---

### Task 6: Render Bounded Read-Only Annotation PNGs on macOS

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/viewer-platform-macos/Cargo.toml`
- Create: `crates/viewer-platform-macos/src/image/review_annotation.rs`
- Modify: `crates/viewer-platform-macos/src/image/mod.rs`
- Modify: `crates/viewer-platform-macos/src/lib.rs`
- Modify: `crates/viewer-infrastructure/src/review/assets.rs`
- Modify: `crates/viewer-infrastructure/src/session_cache.rs`
- Modify: `crates/viewer-application/src/review_assets.rs`
- Modify: `crates/viewer-application/src/review_artifact.rs`
- Modify: `crates/viewer-application/tests/review_session_start.rs`
- Modify: `crates/viewer-application/tests/review_session_mutation.rs`
- Modify: `crates/viewer-application/tests/review_session_completion.rs`
- Test: `crates/viewer-infrastructure/tests/review_assets.rs`

**Interfaces:**
- Consumes: existing ImageIO orientation pipeline; Task 1 Anchor values; Task 5 `ReviewRenderedArtifact`.
- Produces: `ReviewArtifactPort::render`, `ReviewArtifactRenderRequest`, `NumberedImageAnnotation`, and `MacReviewArtifactRenderer`.

- [x] **Step 1: Write failing renderer tests with deterministic pixel and bound assertions**

```rust
#[tokio::test]
async fn renderer_orients_source_and_draws_numbered_rect_and_stroke() {
    let cache = tempfile::tempdir().unwrap();
    let renderer = MacReviewArtifactRenderer::new(cache.path()).unwrap();
    let source_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/images/rotated-6.jpg");
    let request = |rect_x| ReviewArtifactRenderRequest {
        source_path: source_path.clone(),
        expected_asset: review_asset_for_source(&source_path),
        cancellation: ReviewTaskCancellation::default(),
        annotations: vec![
            NumberedImageAnnotation {
                ordinal: 1,
                feedback_id: FeedbackId::from_u128(11),
                anchor: FeedbackAnchor::ImageRect(
                    NormalizedRect::new(rect_x, 0.2, 0.2, 0.3).unwrap(),
                ),
            },
            NumberedImageAnnotation {
                ordinal: 2,
                feedback_id: FeedbackId::from_u128(12),
                anchor: FeedbackAnchor::ImageStroke(ImageStroke::new(vec![
                    NormalizedPoint::new(0.2, 0.2).unwrap(),
                    NormalizedPoint::new(0.8, 0.7).unwrap(),
                ]).unwrap()),
            },
        ],
    };
    let first = renderer.render(request(0.1)).await.unwrap();
    let first_bytes = fs::read(&first.temporary_path).unwrap();
    let second = renderer.render(request(0.6)).await.unwrap();

    assert!(first.width < first.height);
    assert!(first.width.max(first.height) <= REVIEW_ANNOTATION_MAX_EDGE);
    assert_eq!(*blake3::hash(&first_bytes).as_bytes(), first.blake3);
    assert_eq!(&first_bytes[..8], b"\x89PNG\r\n\x1a\n");
    assert_ne!(first.blake3, second.blake3);
}

fn review_asset_for_source(path: &Path) -> AssetVersion {
    let bytes = fs::read(path).unwrap();
    let metadata = fs::metadata(path).unwrap();
    let modified_ns = metadata.modified().unwrap()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as i128;
    AssetVersion {
        id: AssetVersionId::from_u128(4),
        source_entity_id: None,
        relative_path: RelativePath::parse("rotated-6.jpg").unwrap(),
        evidence: AssetEvidence {
            size_bytes: bytes.len() as u64,
            modified_ns,
            blake3: Some(*blake3::hash(&bytes).as_bytes()),
        },
        media: ReviewMedia::Image { width: Some(600), height: Some(800) },
        producer_asset_id: None,
        parent_asset_version_id: None,
    }
}
```

Add rejection tests for non-image input, a changed source, output above 64 MiB, a symlinked source, a pre-canceled request, cancellation before encode, unsupported Anchor kind, and more than the Domain aggregate point bound.

- [x] **Step 2: Run the focused macOS tests and verify the renderer is missing**

Run: `cargo test --locked -p viewer-platform-macos review_annotation -- --nocapture && cargo test --locked -p viewer-infrastructure --test review_assets`

Expected: FAIL because the port, renderer, and prepared source path are absent.

- [x] **Step 3: Expose a protected source path only inside Application**

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedReviewAsset {
    pub entity_id: EntityId,
    pub asset: AssetVersion,
    pub failure: Option<ReviewabilityFailure>,
    pub change_revision: u64,
    pub source_path: PathBuf,
}

#[async_trait]
pub trait ReviewArtifactPort: Send + Sync {
    async fn render(
        &self,
        request: ReviewArtifactRenderRequest,
    ) -> Result<ReviewRenderedArtifact, ReviewArtifactError>;
}

pub const REVIEW_ANNOTATION_MAX_EDGE: u32 = 4_096;

#[derive(Clone, Debug, PartialEq)]
pub struct NumberedImageAnnotation {
    pub ordinal: u32,
    pub feedback_id: FeedbackId,
    pub anchor: FeedbackAnchor,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewArtifactRenderRequest {
    pub source_path: PathBuf,
    pub expected_asset: AssetVersion,
    pub cancellation: ReviewTaskCancellation,
    pub annotations: Vec<NumberedImageAnnotation>,
}
```

Populate `source_path` only after `IndexedReviewAssetCatalog` applies existing project containment, regular-file and change-evidence checks. Add `SessionCache::review_artifact_root() -> PathBuf`, create its `review-artifacts/` directory with the session root, and cover containment/stale-session cleanup in `session_cache` tests. Add `source_path` to every `PreparedReviewAsset` constructor in the three Application review-session test fixtures using each fixture’s protected temporary project root. Verify with a repository policy test that no DTO or protocol module imports `PreparedReviewAsset::source_path`.

- [x] **Step 4: Enable only existing CoreGraphics modules and implement the renderer**

Change the existing `objc2-core-graphics` workspace features to `std`, `CGImage`, `CGBitmapContext`, `CGColorSpace`, `CGContext`, and `CGPath`; enable the existing `objc2-app-kit` features `NSFont` and `NSStringDrawing` so numbered markers use a system font; add no new crate. Open the source once as a regular non-symlink file, verify its handle’s size/mtime/BLAKE3 and oriented dimensions against `expected_asset` before and after decode, apply EXIF orientation, cap the longest edge at `REVIEW_ANNOTATION_MAX_EDGE = 4096` and total pixels at 16,777,216, draw fixed Viewer review-color rectangles/strokes and numbered markers in a CoreGraphics bitmap context, encode PNG through the existing ImageIO encoder, then return measured dimensions, size, BLAKE3 and the exact `(ordinal, feedback_id)` mapping copied from the render request. Check `ReviewTaskCancellation` before decode, before drawing, before encoding and before returning the lease; a canceled or evidence-mismatched render removes its temporary file.

- [x] **Step 5: Run renderer, image pipeline, and dependency policy tests**

Run: `cargo test --locked -p viewer-platform-macos image -- --nocapture && cargo test --locked -p viewer-infrastructure --test review_assets && pnpm test:policy`

Expected: PASS; existing thumbnail/preview orientation tests remain green and dependency policy reports only feature expansion on the approved crate.

- [x] **Step 6: Commit the rendering adapter**

```bash
git add Cargo.toml Cargo.lock crates/viewer-platform-macos crates/viewer-infrastructure/src/review/assets.rs crates/viewer-infrastructure/src/session_cache.rs crates/viewer-infrastructure/tests/review_assets.rs crates/viewer-application/src/review_assets.rs crates/viewer-application/src/review_artifact.rs crates/viewer-application/tests/review_session_start.rs crates/viewer-application/tests/review_session_mutation.rs crates/viewer-application/tests/review_session_completion.rs
git commit -m "feat(review): render bounded annotation previews"
```

---

### Task 7: Add Atomic First-Feedback and Anchored Session Mutations

**Files:**
- Modify: `crates/viewer-application/src/review_session.rs`
- Modify: `crates/viewer-application/src/lib.rs`
- Modify: `crates/viewer-application/tests/review_session_start.rs`
- Modify: `crates/viewer-application/tests/review_session_mutation.rs`
- Modify: `crates/viewer-application/tests/review_session_completion.rs`
- Modify: `src-tauri/src/state/session.rs`
- Modify: `src-tauri/examples/review_loop_harness.rs`

**Interfaces:**
- Consumes: Task 4 versioned Drafts; Task 5 publications; Task 6 rendering port.
- Produces: `ReviewFeedbackTargetInput`, `StartReviewWithFeedback`, `UpdateReviewFeedbackText`, `ReplaceReviewFeedbackAnchor`, session-scoped deletion restore, target-rich snapshots, and annotation-aware completion.

- [x] **Step 1: Write failing tests for atomic first save**

```rust
#[tokio::test]
async fn first_valid_feedback_creates_one_v2_draft_write_with_fixed_scope() {
    let fixture = Fixture::new();
    let proposal = fixture.service.preview_start(selection()).await.unwrap();
    let snapshot = fixture.service.start_with_feedback(
        StartReviewWithFeedback {
            proposal_id: proposal.id,
            text: "右手结构需要修正".into(),
            targets: vec![ReviewFeedbackTargetInput {
                entity_id: EntityId::from_u128(1),
                anchor: FeedbackAnchor::ImageRect(
                    NormalizedRect::new(0.62, 0.35, 0.18, 0.22).unwrap(),
                ),
            }],
        },
        fixture.progress(),
    ).await.unwrap();

    assert_eq!(fixture.repositories.state.lock().unwrap().save_attempts, 1);
    assert_eq!(snapshot.members.len(), 2);
    assert_eq!(snapshot.feedback.len(), 1);
    assert_eq!(
        fixture.repositories.state.lock().unwrap().draft.as_ref().unwrap().protocol_version,
        ReviewProtocolVersion::V2,
    );
}

#[tokio::test]
async fn failed_first_save_leaves_no_empty_draft_and_keeps_session_idle() {
    let fixture = Fixture::new();
    fixture.repositories.state.lock().unwrap().fail_save = true;
    let proposal = fixture.service.preview_start(selection()).await.unwrap();
    let command = StartReviewWithFeedback {
        proposal_id: proposal.id,
        text: "修正右手".into(),
        targets: vec![ReviewFeedbackTargetInput {
            entity_id: EntityId::from_u128(1),
            anchor: FeedbackAnchor::ImageRect(NormalizedRect::new(0.6, 0.3, 0.2, 0.2).unwrap()),
        }],
    };
    assert_eq!(fixture.service.start_with_feedback(command, fixture.progress()).await, Err(ReviewSessionError::SaveFailed));
    assert!(fixture.repositories.state.lock().unwrap().draft.is_none());
    assert_eq!(fixture.service.snapshot().await.phase, ReviewSessionPhase::Idle);
}
```

- [x] **Step 2: Write failing tests for fixed membership, anchor edits, v1 upgrade, and restore**

Test these exact cases: target outside the proposal/Draft is rejected; blank text or invalid geometry performs zero writes; rectangle update preserves `FeedbackId` and `created_at_ms`; brush redraw replaces the full stroke; deletion exposes one `restorable_feedback_id`; restore reinstates the server-held Feedback; any later successful mutation clears that restore slot; a resumed v1 Draft remains v1 for text-only asset updates and upgrades to v2 atomically on its first image rect/stroke write.

- [x] **Step 3: Run the session tests and confirm the new commands fail to compile**

Run: `cargo test --locked -p viewer-application --test review_session_start --test review_session_mutation --test review_session_completion`

Expected: FAIL because target inputs, combined start and restore semantics are undefined.

- [x] **Step 4: Define the exact command and snapshot shapes**

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct ReviewFeedbackTargetInput {
    pub entity_id: EntityId,
    pub anchor: FeedbackAnchor,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StartReviewWithFeedback {
    pub proposal_id: ReviewProposalId,
    pub text: String,
    pub targets: Vec<ReviewFeedbackTargetInput>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AddReviewFeedback {
    pub guard: ReviewMutationGuard,
    pub text: String,
    pub targets: Vec<ReviewFeedbackTargetInput>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UpdateReviewFeedback {
    pub guard: ReviewMutationGuard,
    pub feedback_id: FeedbackId,
    pub text: String,
    pub targets: Vec<ReviewFeedbackTargetInput>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UpdateReviewFeedbackText {
    pub guard: ReviewMutationGuard,
    pub feedback_id: FeedbackId,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReplaceReviewFeedbackAnchor {
    pub guard: ReviewMutationGuard,
    pub feedback_id: FeedbackId,
    pub target: ReviewFeedbackTargetInput,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewFeedbackTargetSnapshot {
    pub asset_version_id: AssetVersionId,
    pub entity_id: Option<EntityId>,
    pub anchor: FeedbackAnchor,
}
```

Add `restorable_feedback_id: Option<FeedbackId>` to `ReviewSessionSnapshot`. Replace `asset_targets` with `feedback_targets` that resolves each Entity ID from immutable bindings and preserves the supplied Anchor.

- [x] **Step 5: Implement start-with-feedback as a single persistence mutation**

Reuse scope re-resolution and asset preparation from `perform_start`, construct the Draft and first Feedback in memory, validate all invariants, set protocol v2, and call `save_draft` once. Install active session state only after the save succeeds. Keep the existing explicit start entry as a v2 whole-asset flow for batch feedback compatibility. Inject `Arc<dyn ReviewArtifactPort>` into `ReviewSessionService::new`; Application tests use a bounded `FakeReviewArtifactPort`, `DesktopRuntime::prepare_review_services` constructs `MacReviewArtifactRenderer` under `cache.review_artifact_root()`, and the manual harness supplies the same platform adapter for its real-image path.

- [x] **Step 6: Implement version upgrades and server-held delete restore**

`ActiveReviewSession` stores `protocol_version` and `last_deleted_feedback: Option<Feedback>`. `mutate_active` clones the Draft and version, saves the versioned clone, then swaps state and increments revision. `start_with_feedback` is always v2; adding/replacing a local Anchor upgrades a resumed v1 clone before the same atomic save; asset-only add/update, text-only update, delete and restore preserve the existing version. This distinction lets a text-only edit of historical v1 `imageRegion` remain v1 while any geometry replacement is an explicit v2 edit. Delete stores the removed Feedback only after save success; `restore_deleted_feedback(guard, feedback_id)` reinserts that exact object and clears the slot only after save success.

- [x] **Step 7: Render artifacts only after final revalidation and before publish**

For each image with at least one local Anchor, sort its local Feedback targets by `(created_at_ms, feedback_id)`, assign stable one-based ordinals, pass the same `(ordinal, feedback_id, anchor)` projection and completion cancellation token to the renderer, then pass all results in `ReviewPublication`. Render sequentially to keep one decoded high-resolution bitmap resident at a time; check cancellation between assets and delete already-rendered leases on cancel/error. Whole-image Feedback remains in JSON/list but receives no image marker. If rendering or publication fails, return to Active with the Draft intact; only successful `publish` transitions to CompletedReadOnly.

- [x] **Step 8: Run all Application review tests**

Run: `cargo test --locked -p viewer-application --test review_session_start --test review_session_mutation --test review_session_completion --test review_catalog`

Expected: PASS for first-save atomicity, fixed scope, anchor mutation, v1 upgrade, undo and artifact orchestration.

- [x] **Step 9: Commit the use-case layer**

```bash
git add crates/viewer-application/src/review_session.rs crates/viewer-application/src/lib.rs crates/viewer-application/tests/review_session_start.rs crates/viewer-application/tests/review_session_mutation.rs crates/viewer-application/tests/review_session_completion.rs src-tauri/src/state/session.rs src-tauri/examples/review_loop_harness.rs
git commit -m "feat(review): add atomic image annotation use cases"
```

---

### Task 8: Expose Anchors Through a Strict Tauri and TypeScript Bridge

**Files:**
- Modify: `src-tauri/src/dto/review.rs`
- Modify: `src-tauri/src/state/review.rs`
- Modify: `src-tauri/src/commands/review.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/tests/review_commands.rs`
- Modify: `ui/src/api/types.ts`
- Modify: `ui/src/api/viewer.ts`
- Modify: `ui/src/api/viewer.test.ts`
- Modify: `ui/src/app/workspace/ports.ts`

**Interfaces:**
- Consumes: Task 7 commands/snapshots.
- Produces: camelCase Tauri requests, snake_case Anchor discriminants, `reviewStartWithFeedback`, `reviewUpdateFeedbackText`, `reviewReplaceFeedbackAnchor`, `reviewRestoreDeletedFeedback`, and target-rich `ReviewFeedbackSnapshot` types.

- [x] **Step 1: Write failing DTO boundary tests**

```rust
#[test]
fn add_feedback_dto_parses_a_bounded_image_stroke() {
    let dto: ReviewAddFeedbackRequestDto = serde_json::from_value(json!({
        "sessionId": SESSION_ID,
        "generation": 7,
        "reviewRoundId": ROUND_ID,
        "expectedRevision": 3,
        "text": "重画袖口边缘",
        "targets": [{
            "entityId": ENTITY_ID,
            "anchor": { "kind": "image_stroke", "points": [{"x": 0.2, "y": 0.3}, {"x": 0.7, "y": 0.6}] }
        }]
    })).unwrap();
    assert!(matches!(dto.try_into_parts().unwrap().1.targets[0].anchor, FeedbackAnchor::ImageStroke(_)));
}
```

Add rejection tests for unknown fields/kinds, empty targets, duplicate targets, 2,049 points, invalid coordinates, blank text, stale generation, and any request containing `path`, `sourcePath`, `artifactPath`, digest or `AssetVersion` evidence.

- [x] **Step 2: Write failing TypeScript adapter tests**

```ts
it('sends anchor geometry without filesystem evidence', async () => {
  await bridge.reviewStartWithFeedback({
    sessionId: SESSION_ID,
    generation: 7,
    proposalId: 11,
    text: '修正衣领边缘',
    targets: [{ entityId: ENTITY_ID, anchor: { kind: 'image_rect', x: 0.2, y: 0.1, width: 0.3, height: 0.2 } }],
  })
  expect(invoke).toHaveBeenCalledWith('review_start_with_feedback', expect.objectContaining({ request: expect.not.objectContaining({ path: expect.anything() }) }))
})
```

- [x] **Step 3: Run the bridge tests and verify missing shapes/commands**

Run: `cargo test --locked -p viewer-desktop --test review_commands && pnpm --dir ui exec vitest run src/api/viewer.test.ts`

Expected: FAIL because the new Anchor union and command names do not exist.

- [x] **Step 4: Implement strict DTOs and command registration**

```rust
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum ReviewAnchorDto {
    Asset,
    ImageRect { x: f64, y: f64, width: f64, height: f64 },
    ImageStroke { points: Vec<ReviewPointDto> },
    VideoPoint { position_us: u64 },
    VideoRange { start_us: u64, end_us: u64 },
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewFeedbackTargetRequestDto {
    pub entity_id: String,
    pub anchor: ReviewAnchorDto,
}
```

Register `review_start_with_feedback`, `review_update_feedback_text`, `review_replace_feedback_anchor` and `review_restore_deleted_feedback`; use the same error categories, generation checks, progress events and mutation guards as existing review commands. `review_update_feedback_text` accepts no targets, and `review_replace_feedback_anchor` accepts exactly one target belonging to the existing Feedback’s single local-image target.

- [x] **Step 5: Mirror the exact discriminated union in TypeScript**

```ts
export type ReviewAnchor =
  | { kind: 'asset' }
  | { kind: 'image_rect'; x: number; y: number; width: number; height: number }
  | { kind: 'image_stroke'; points: ReadonlyArray<{ x: number; y: number }> }
  | { kind: 'video_point'; positionUs: number }
  | { kind: 'video_range'; startUs: number; endUs: number }

export interface ReviewFeedbackTargetInput {
  entityId: string
  anchor: ReviewAnchor
}

export interface ReviewUpdateFeedbackTextRequest extends ReviewMutationRequest {
  feedbackId: string
  text: string
}

export interface ReviewReplaceFeedbackAnchorRequest extends ReviewMutationRequest {
  feedbackId: string
  target: ReviewFeedbackTargetInput
}
```

Keep the same union for snapshots, adding `assetVersionId` and nullable `entityId`; do not introduce a separate frontend protocol model.

- [x] **Step 6: Run DTO, command, adapter, and architecture boundary tests**

Run: `cargo test --locked -p viewer-desktop --test review_commands && pnpm --dir ui exec vitest run src/api/viewer.test.ts && pnpm architecture:boundaries`

Expected: PASS and no filesystem path appears in serialized review command snapshots.

- [x] **Step 7: Commit the bridge**

```bash
git add src-tauri/src/dto/review.rs src-tauri/src/state/review.rs src-tauri/src/commands/review.rs src-tauri/src/lib.rs src-tauri/tests/review_commands.rs ui/src/api/types.ts ui/src/api/viewer.ts ui/src/api/viewer.test.ts ui/src/app/workspace/ports.ts
git commit -m "feat(review): expose image anchors through tauri"
```

---

### Task 9: Extract a Review-Neutral Image Preview Surface

**Files:**
- Create: `ui/src/components/imagePreview/ImagePreviewSurface.tsx`
- Create: `ui/src/components/imagePreview/ImagePreviewSurface.test.tsx`
- Modify: `ui/src/components/ImagePreview.tsx`
- Modify: `ui/src/components/ImagePreview.test.tsx`
- Modify: `ui/src/components/imagePreview/imageGeometry.ts`
- Modify: `ui/src/components/imagePreview/imageGeometry.test.ts`
- Modify: `ui/src/styles/app.css`

**Interfaces:**
- Consumes: existing `ImagePreview` image loading, zoom, pan, rotation, magnifier, navigation and `imageGeometry` functions.
- Produces: `ImagePreviewSurface`, `ImagePreviewProjection`, `ImagePreviewSurfaceSlots`; keeps `ImagePreview` as the ordinary-preview wrapper.

- [x] **Step 1: Add characterization tests for ordinary preview behavior**

```tsx
it('keeps ordinary preview navigation and closes through 返回网格', async () => {
  render(<ImagePreview {...ordinaryProps} />)
  await user.click(screen.getByRole('button', { name: '下一张' }))
  expect(ordinaryProps.onNavigate).toHaveBeenCalledWith(1)
  await user.click(screen.getByRole('button', { name: '返回网格' }))
  expect(ordinaryProps.onClose).toHaveBeenCalledTimes(1)
})

it('surface has no review dependency and reports stable projection', () => {
  const source = { width: 6720, height: 4480 }
  const stage = { left: 120, top: 80, width: 960, height: 640 }
  expect(stageToNormalized({ x: 600, y: 400 }, source, stage, 0, 1)).toEqual({ x: 0.5, y: 0.5 })
})

const SOURCE_SIZE = { width: 6720, height: 4480 }
const STAGE_RECT = { left: 120, top: 80, width: 960, height: 640 }

it.each([0, 1, 2, 3])('round-trips canonical coordinates through rotation %i', (quarterTurns) => {
  const normalized = { x: 0.23, y: 0.71 }
  const stage = normalizedToStage(normalized, SOURCE_SIZE, STAGE_RECT, quarterTurns, 4)
  expect(stageToNormalized(stage, SOURCE_SIZE, STAGE_RECT, quarterTurns, 4)).toEqual(normalized)
})
```

Keep all existing wheel, trackpad, fit, rotation, magnifier, load-race and keyboard tests as mandatory characterizations.

- [x] **Step 2: Run the ordinary preview suite before extraction**

Run: `pnpm --dir ui exec vitest run src/components/ImagePreview.test.tsx src/components/imagePreview/imageGeometry.test.ts`

Expected: the newly renamed action test FAILS on the old “完成” label; all existing characterizations PASS.

- [x] **Step 3: Define the composable surface contract**

```ts
export interface ImagePreviewProjection {
  sourceSize: { width: number; height: number }
  stageRect: { left: number; top: number; width: number; height: number }
  stageToNormalized(point: { x: number; y: number }): { x: number; y: number } | null
  normalizedToStage(point: { x: number; y: number }): { x: number; y: number }
}

export interface ImagePreviewSurfaceSlots {
  toolbarLeading?: ReactNode
  toolbarActions?: ReactNode
  stageOverlay?: (projection: ImagePreviewProjection) => ReactNode
  sidePanel?: ReactNode
}

export interface ImagePreviewSurfaceProps {
  file: BrowserFile
  files: BrowserFile[]
  magnifier: MagnifierPreferences
  pointerClientPoint: MutableRefObject<Point | null>
  unavailableEntityIds?: ReadonlySet<string>
  requestImage(
    file: BrowserFile,
    representation: ImageRepresentationRequest,
    signal?: AbortSignal,
  ): Promise<ImageRepresentation>
  onNavigate(file: BrowserFile): void
  onDimensions?(entityId: string, width: number, height: number): void
  slots?: ImagePreviewSurfaceSlots
}
```

`ImagePreviewSurface` owns the current loader and viewport reducer. It accepts content/action slots but imports no Review type, hook, bridge, coordinator or CSS module.

`stageToNormalized` and `normalizedToStage` take the canonical oriented source size, current quarter-turn rotation and current zoom; add round-trip cases for 50%, 100%, 200% and 400%. Return `null` whenever source dimensions, rendered stage bounds or the inverse transform are unavailable so annotation save can be disabled instead of approximated.

- [x] **Step 4: Move viewport/rendering code and keep the wrapper thin**

`ImagePreview` builds the existing title, zoom controls, rotate/magnifier actions, navigation counter and the renamed `返回网格` action around `ImagePreviewSurface`. Preserve DOM focus restoration and all ordinary preview callbacks.

- [x] **Step 5: Add a static architecture test for the dependency direction**

```ts
it('keeps ImagePreviewSurface review-neutral', () => {
  const source = readFileSync('src/components/imagePreview/ImagePreviewSurface.tsx', 'utf8')
  expect(source).not.toMatch(/app\/review|api\/viewer|components\/review/)
})
```

- [x] **Step 6: Run preview tests and production TypeScript build**

Run: `pnpm --dir ui exec vitest run src/components/ImagePreview.test.tsx src/components/imagePreview/ImagePreviewSurface.test.tsx src/components/imagePreview/imageGeometry.test.ts && pnpm --dir ui build`

Expected: PASS; ordinary preview retains every prior interaction with only the approved close-label change.

- [x] **Step 7: Commit the preview seam**

```bash
git add ui/src/components/ImagePreview.tsx ui/src/components/ImagePreview.test.tsx ui/src/components/imagePreview/ImagePreviewSurface.tsx ui/src/components/imagePreview/ImagePreviewSurface.test.tsx ui/src/components/imagePreview/imageGeometry.ts ui/src/components/imagePreview/imageGeometry.test.ts ui/src/styles/app.css
git commit -m "refactor(ui): extract reusable image preview surface"
```

---

### Task 10: Build the Frontend Annotation Geometry and Editing State Machine

**Files:**
- Create: `ui/src/app/review/annotationGeometry.ts`
- Create: `ui/src/app/review/annotationGeometry.test.ts`
- Create: `ui/src/app/review/annotationModel.ts`
- Create: `ui/src/app/review/annotationModel.test.ts`
- Create: `ui/src/app/review/useImageReviewWorkbench.ts`
- Create: `ui/src/app/review/useImageReviewWorkbench.test.tsx`
- Modify: `ui/src/app/review/reviewModel.ts`
- Modify: `ui/src/app/review/useReviewSessionCoordinator.ts`

**Interfaces:**
- Consumes: Task 8 bridge types and Task 9 projection contract.
- Produces: `AnnotationTool`, `AnnotationEditorState`, deterministic stroke simplification, dirty-leave guard, and `ImageReviewWorkbenchController`.

- [x] **Step 1: Write failing pure geometry tests**

```ts
it('simplifies a stroke deterministically and preserves endpoints', () => {
  const input = Array.from({ length: 5000 }, (_, index) => ({ x: index / 4999, y: 0.5 + Math.sin(index / 40) * 0.01 }))
  const first = simplifyNormalizedStroke(input, { sourceWidth: 6720, sourceHeight: 4480, maxPoints: 2048 })
  const second = simplifyNormalizedStroke(input, { sourceWidth: 6720, sourceHeight: 4480, maxPoints: 2048 })
  expect(first).toEqual(second)
  expect(first[0]).toEqual(input[0])
  expect(first.at(-1)).toEqual(input.at(-1))
  expect(first.length).toBeLessThanOrEqual(2048)
})

it('normalizes a dragged rectangle in every direction and clamps to the image', () => {
  expect(rectFromDrag({ x: 0.8, y: 0.7 }, { x: -0.1, y: 1.1 })).toEqual({ x: 0, y: 0.7, width: 0.8, height: 0.3 })
})
```

- [x] **Step 2: Write failing reducer tests for transient versus saved state**

Test exact transitions: Browse -> Brush/Rectangle; Space temporarily pans and restores prior tool on keyup; empty pointer gesture cancels without Feedback; valid mark opens inline editor; blank Command+Enter stays unsaved; nonblank save enters `saving`; save failure returns `editing` with text/anchor intact; save success selects the server Feedback; Escape cancels an unsaved mark or returns to Browse; navigation/close/finish are blocked whenever `dirty === true`.

- [x] **Step 3: Run pure and hook tests and confirm the model is absent**

Run: `pnpm --dir ui exec vitest run src/app/review/annotationGeometry.test.ts src/app/review/annotationModel.test.ts src/app/review/useImageReviewWorkbench.test.tsx`

Expected: FAIL because the geometry and editor state machine are undefined.

- [x] **Step 4: Implement deterministic geometry functions**

Use radial-distance filtering followed by Douglas–Peucker with source-pixel tolerance `max(1.5, max(sourceWidth, sourceHeight) / 4096)` converted independently to normalized x/y. Clamp every output coordinate to `[0, 1]`, preserve endpoints, discard fewer than two distinct points, and apply a deterministic evenly spaced fallback only if simplification still exceeds 2,048 points.

- [x] **Step 5: Implement an explicit editor reducer**

```ts
export type AnnotationTool = 'browse' | 'brush' | 'rectangle'

export type AnnotationEditorState =
  | { status: 'idle'; tool: AnnotationTool; selectedFeedbackId: string | null }
  | { status: 'drawing'; tool: 'brush' | 'rectangle'; draftAnchor: ReviewAnchor }
  | { status: 'editing'; tool: AnnotationTool; draftAnchor: ReviewAnchor; text: string; sourceFeedbackId: string | null }
  | { status: 'saving'; tool: AnnotationTool; draftAnchor: ReviewAnchor; text: string; sourceFeedbackId: string | null }
  | { status: 'save_error'; tool: AnnotationTool; draftAnchor: ReviewAnchor; text: string; sourceFeedbackId: string | null; message: string }

export function hasUnsavedAnnotation(state: AnnotationEditorState): boolean {
  return state.status === 'drawing' || state.status === 'editing' || state.status === 'saving' || state.status === 'save_error'
}
```

- [x] **Step 6: Implement the workbench hook without optimistic persistence**

```ts
export type ReviewLeaveIntent =
  | { kind: 'navigate'; offset: -1 | 1 }
  | { kind: 'return_grid' }
  | { kind: 'close_project' }
  | { kind: 'finish_review' }
  | { kind: 'abandon_review' }

export interface SavedImageFeedback {
  feedbackId: string
  text: string
  createdAtMs: number
  ordinal: number | null
  anchor: ReviewAnchor
}

export interface ImageReviewWorkbenchController {
  tool: AnnotationTool
  editor: AnnotationEditorState
  feedback: ReadonlyArray<SavedImageFeedback>
  selectedFeedbackId: string | null
  railOpen: boolean
  readOnlyReason: 'outside_scope' | 'write_unavailable' | 'recovery_required' | null
  restorableFeedbackId: string | null
  setTool(tool: AnnotationTool): void
  beginAnnotation(anchor: ReviewAnchor): void
  updateDraftAnchor(anchor: ReviewAnchor): void
  updateDraftText(text: string): void
  saveDraft(): Promise<void>
  cancelDraft(): void
  selectFeedback(feedbackId: string | null): void
  updateFeedbackText(feedbackId: string, text: string): Promise<void>
  replaceFeedbackAnchor(feedbackId: string, anchor: ReviewAnchor): Promise<void>
  deleteFeedback(feedbackId: string): Promise<void>
  restoreDeletedFeedback(feedbackId: string): Promise<void>
  setRailOpen(open: boolean): void
  requestLeave(intent: ReviewLeaveIntent): Promise<'proceeded' | 'blocked'>
  discardUnsavedAndProceed(): Promise<void>
}
```

On preview entry with no active Draft, call `reviewPreviewStart` once and keep its proposal ID with the captured scope. On first save call `reviewStartWithFeedback`; on later saves use guarded add, `reviewUpdateFeedbackText`, or `reviewReplaceFeedbackAnchor` according to the user intent. Replace client state only from the returned `ReviewSessionSnapshot`. Keep unsaved editor state on command errors, calculate current-image Feedback by target `entityId`, and expose `requestLeave(intent)` that either performs the intent or opens the discard guard.

- [x] **Step 7: Run model, coordinator, and API adapter tests**

Run: `pnpm --dir ui exec vitest run src/app/review/annotationGeometry.test.ts src/app/review/annotationModel.test.ts src/app/review/useImageReviewWorkbench.test.tsx src/app/review/useReviewSessionCoordinator.test.tsx src/api/viewer.test.ts`

Expected: PASS for geometry, state transitions, immutable proposal usage and server-authoritative snapshots.

- [x] **Step 8: Commit the frontend state core**

```bash
git add ui/src/app/review/annotationGeometry.ts ui/src/app/review/annotationGeometry.test.ts ui/src/app/review/annotationModel.ts ui/src/app/review/annotationModel.test.ts ui/src/app/review/useImageReviewWorkbench.ts ui/src/app/review/useImageReviewWorkbench.test.tsx ui/src/app/review/reviewModel.ts ui/src/app/review/useReviewSessionCoordinator.ts
git commit -m "feat(review): add image annotation editor state"
```

---

### Task 11: Build the Image Review Workbench Components

**Files:**
- Create: `ui/src/components/review/ImageReviewWorkspace.tsx`
- Create: `ui/src/components/review/ImageReviewWorkspace.test.tsx`
- Create: `ui/src/components/review/AnnotationCanvas.tsx`
- Create: `ui/src/components/review/AnnotationCanvas.test.tsx`
- Create: `ui/src/components/review/AnnotationToolbar.tsx`
- Create: `ui/src/components/review/InlineFeedbackEditor.tsx`
- Create: `ui/src/components/review/ReviewFeedbackRail.tsx`
- Create: `ui/src/components/review/ReviewFeedbackRail.test.tsx`
- Modify: `ui/src/styles/review.css`

**Interfaces:**
- Consumes: Task 9 surface slots/projection and Task 10 `ImageReviewWorkbenchController`.
- Produces: `ImageReviewWorkspaceProps`, numbered stage overlay, Browse/Brush/Rectangle toolbar, inline editor, collapsible feedback rail, and accessible editing controls.

```ts
export interface ImageReviewWorkspaceProps extends Omit<ImagePreviewSurfaceProps, 'slots'> {
  controller: ImageReviewWorkbenchController
  onReturnGrid(): void
  onFinishReview(): void
}
```

- [x] **Step 1: Write failing interaction tests for the primary four-issue flow**

```tsx
it('shows four independent numbered comments without permanent text bubbles', () => {
  const controller = controllerFixture([
    savedRect('feedback-1', 1, '衣领边缘需要更平整', 0.1, 0.1, 0.2, 0.15),
    savedStroke('feedback-2', 2, '右袖阴影断裂', [{ x: 0.2, y: 0.2 }, { x: 0.35, y: 0.45 }]),
    savedRect('feedback-3', 3, '左袖长度不一致', 0.6, 0.2, 0.18, 0.3),
    savedStroke('feedback-4', 4, '裤脚接缝需要修正', [{ x: 0.42, y: 0.7 }, { x: 0.58, y: 0.82 }]),
  ])
  render(
    <ImageReviewWorkspace
      {...surfaceFixture()}
      controller={controller}
      onReturnGrid={vi.fn()}
      onFinishReview={vi.fn()}
    />,
  )

  expect(screen.getAllByTestId('annotation-marker')).toHaveLength(4)
  expect(screen.getAllByRole('listitem', { name: /意见/ })).toHaveLength(4)
  expect(screen.queryAllByTestId('permanent-text-bubble')).toHaveLength(0)
})
```

Define `surfaceFixture`, `savedRect`, `savedStroke`, and `controllerFixture` in the test file with concrete `BrowserFile`, `ImagePreviewSurfaceProps`, `SavedImageFeedback`, and `ImageReviewWorkbenchController` return types. `surfaceFixture` uses one `640×480` JPEG and a resolved `ImageRepresentation`; `controllerFixture` supplies every method from Task 10 as a `vi.fn()` and uses `idle` editor state. Mock `ImagePreviewSurface` only in this component test so it calls `slots.stageOverlay(TEST_PROJECTION)` and renders `slots.sidePanel`; integration tests in Task 12 use the real surface.

```ts
const REVIEW_FILE: BrowserFile = {
  entityId: 'image-1',
  relativePath: 'batch/image-1.jpg',
  name: 'image-1.jpg',
  kind: 'jpeg',
  size: 1024,
  modifiedNs: '1',
  marker: { reviewState: null, favorite: false },
  imageMetadata: { width: 640, height: 480 },
  imageUrl: null,
  videoMetadata: null,
}

function surfaceFixture(): Omit<ImagePreviewSurfaceProps, 'slots'> {
  return {
    file: REVIEW_FILE,
    files: [REVIEW_FILE],
    magnifier: { shape: 'circle', magnification: 2, area: 'small' },
    pointerClientPoint: { current: null },
    requestImage: vi.fn(async () => ({
      cacheKey: 'image-1-fit',
      url: 'viewer-image://localhost/session/image-1-fit',
      width: 640,
      height: 480,
      backend: 'image_io',
    })),
    onNavigate: vi.fn(),
  }
}

function savedRect(
  feedbackId: string, ordinal: number, text: string,
  x: number, y: number, width: number, height: number,
): SavedImageFeedback {
  return { feedbackId, ordinal, text, createdAtMs: ordinal, anchor: { kind: 'image_rect', x, y, width, height } }
}

function savedStroke(
  feedbackId: string, ordinal: number, text: string,
  points: ReadonlyArray<{ x: number; y: number }>,
): SavedImageFeedback {
  return { feedbackId, ordinal, text, createdAtMs: ordinal, anchor: { kind: 'image_stroke', points } }
}

function controllerFixture(feedback: ReadonlyArray<SavedImageFeedback>): ImageReviewWorkbenchController {
  return {
    tool: 'browse',
    editor: { status: 'idle', tool: 'browse', selectedFeedbackId: null },
    feedback,
    selectedFeedbackId: null,
    railOpen: true,
    readOnlyReason: null,
    restorableFeedbackId: null,
    setTool: vi.fn(),
    beginAnnotation: vi.fn(),
    updateDraftAnchor: vi.fn(),
    updateDraftText: vi.fn(),
    saveDraft: vi.fn(async () => undefined),
    cancelDraft: vi.fn(),
    selectFeedback: vi.fn(),
    updateFeedbackText: vi.fn(async () => undefined),
    replaceFeedbackAnchor: vi.fn(async () => undefined),
    deleteFeedback: vi.fn(async () => undefined),
    restoreDeletedFeedback: vi.fn(async () => undefined),
    setRailOpen: vi.fn(),
    requestLeave: vi.fn(async () => 'proceeded' as const),
    discardUnsavedAndProceed: vi.fn(async () => undefined),
  }
}
```

- [x] **Step 2: Write failing edit, selection, keyboard, and accessibility tests**

Cover marker/rail bidirectional selection; rectangle move/resize and arrow-key position/size micro-adjustment; brush redraw preserving ID/text; whole-image comment; delete and session restore; collapsed rail; `V` Browse, `B` Brush, `R` Rectangle; Space pan; Escape semantics; Command+Enter save; suppression of tool shortcuts while an input owns focus; focus return after save/cancel; numbered markers with accessible labels; `aria-pressed` tools; `aria-live` save errors; no pointer-only action.

- [x] **Step 3: Run component tests and confirm the workbench is missing**

Run: `pnpm --dir ui exec vitest run src/components/review/ImageReviewWorkspace.test.tsx src/components/review/AnnotationCanvas.test.tsx src/components/review/ReviewFeedbackRail.test.tsx`

Expected: FAIL because the workbench components do not exist.

- [x] **Step 4: Implement the toolbar and stage overlay with existing assets**

Use existing Viewer icon components (`move` for Browse, `pencil` for Brush, `maximize` for Rectangle, and `panel-right` for the rail) with visible Chinese labels; do not add SVG files or an icon package. Render saved rectangle/stroke geometry with a device-pixel-ratio-aware HTML Canvas and render numbered markers/resize handles as positioned semantic DOM controls; do not create inline SVG assets. Pointer events are active only for the current tool and never intercept ordinary zoom controls. Register `V`, `B`, and `R` only when no text field owns focus; Space uses keydown/keyup temporary pan ownership.

- [x] **Step 5: Implement inline editing and numbered feedback rail**

Open a compact editor adjacent to the draft mark, flip it inside viewport bounds, and provide `保存`/`取消`. After save, remove the text bubble and retain only the numbered marker. `整图意见` opens an Asset-Anchor editor inside the rail without changing the current pointer tool and never creates a fake canvas marker. The rail lists number, natural-language text, selection state, edit, delete and restore; it can collapse to a narrow tab without changing the image viewport’s fit calculation incorrectly. At the existing `@container viewer-shell (max-width: 700px)` boundary, the rail becomes a closable overlay drawer and defaults closed; at larger widths it is a docked sibling that causes one measured viewport refit.

- [x] **Step 6: Implement saved geometry editing rules**

Rectangle selection exposes move and four corner handles; pointer release produces one complete Anchor update. Arrow keys move the rectangle by one oriented-source pixel and `Shift + Arrow` changes the focused edge by ten oriented-source pixels, clamped to the image, so geometry editing has a keyboard path. Brush edit enters redraw mode and swaps the entire path only after the replacement plus existing text saves successfully. During either save, keep the old server geometry visible and show the candidate as transient; on failure restore the editor with the candidate intact.

- [x] **Step 7: Run component, accessibility, and CSS policy tests**

Run: `pnpm --dir ui exec vitest run src/components/review/ImageReviewWorkspace.test.tsx src/components/review/AnnotationCanvas.test.tsx src/components/review/ReviewFeedbackRail.test.tsx src/components/ModalSheet.test.tsx src/components/ui/ViewerIcon.test.tsx && pnpm test:policy`

Expected: PASS with keyboard parity, focus assertions, no new raw color outside existing review tokens, and no handcrafted icon asset.

- [x] **Step 8: Commit the workbench UI**

```bash
git add ui/src/components/review/ImageReviewWorkspace.tsx ui/src/components/review/ImageReviewWorkspace.test.tsx ui/src/components/review/AnnotationCanvas.tsx ui/src/components/review/AnnotationCanvas.test.tsx ui/src/components/review/AnnotationToolbar.tsx ui/src/components/review/InlineFeedbackEditor.tsx ui/src/components/review/ReviewFeedbackRail.tsx ui/src/components/review/ReviewFeedbackRail.test.tsx ui/src/styles/review.css
git commit -m "feat(ui): build image annotation review workbench"
```

---

### Task 12: Integrate Fixed-Scope Review Into Preview, Grid, and Completion

**Files:**
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/app/workspace/WorkspaceProjectView.tsx`
- Create: `ui/src/app/workspace/WorkspaceProjectView.test.tsx`
- Modify: `ui/src/components/ContentBrowser.tsx`
- Modify: `ui/src/components/ContentBrowser.test.tsx`
- Modify: `ui/src/components/contentBrowser/ImageCell.tsx`
- Modify: `ui/src/components/contentBrowser/VideoCard.tsx`
- Modify: `ui/src/components/contentBrowser/VideoCard.test.tsx`
- Modify: `ui/src/components/contentBrowser/VideoSection.tsx`
- Modify: `ui/src/components/contentBrowser/VideoSection.test.tsx`
- Modify: `ui/src/components/FolderOverview.tsx`
- Modify: `ui/src/components/FolderFilmstripRow.tsx`
- Modify: `ui/src/components/SearchResults.tsx`
- Modify: `ui/src/components/review/ReviewContextBar.tsx`
- Modify: `ui/src/components/review/ReviewCompletionDialog.tsx`
- Modify: `ui/src/components/review/ReviewWorkspaceLayer.tsx`
- Modify: `ui/src/styles/review.css`

**Interfaces:**
- Consumes: Task 10 controller and Task 11 workbench.
- Produces: automatic workbench selection, feedback-count badges, outside-scope read-only state, unified completion entry and unsaved-navigation guard across the workspace.

- [x] **Step 1: Write failing integration tests for preview selection and fixed scope**

```tsx
it('opens the review workbench for an in-scope image and read-only preview outside the active draft', async () => {
  const { rerender } = renderWorkspace({ activeDraftMembers: [IMAGE_A], preview: IMAGE_A })
  expect(screen.getByRole('toolbar', { name: '图片评审工具' })).toBeVisible()

  rerender(renderWorkspaceProps({ activeDraftMembers: [IMAGE_A], preview: IMAGE_B }))
  expect(screen.queryByRole('toolbar', { name: '图片评审工具' })).not.toBeInTheDocument()
  expect(screen.getByText('这张图片不在当前评审范围内')).toBeVisible()
  expect(screen.getByRole('button', { name: '返回本轮素材' })).toBeVisible()
  expect(screen.getByRole('button', { name: '完成当前评审' })).toBeVisible()
  expect(screen.getByRole('button', { name: '放弃当前草稿后重新开始' })).toBeVisible()
})
```

- [x] **Step 2: Write failing grid and leave-guard tests**

Test one `返工 · N 条` badge per affected image or existing whole-asset video Feedback in normal grid/video section, folder overview/filmstrip and search results; no badge for zero feedback; no pass control; `返回网格`, next/previous image, project close and `完成本轮评审` all invoke the same discard guard when a local mark/text is dirty; batch-grid visibility never writes review state or pass outcomes.

- [x] **Step 3: Run integration tests and confirm the missing wiring**

Run: `pnpm --dir ui exec vitest run src/app/workspace/WorkspaceProjectView.test.tsx src/components/ContentBrowser.test.tsx src/App.test.tsx`

Expected: FAIL because the workspace still selects ordinary preview and cells lack feedback counts.

- [x] **Step 4: Add a narrow review presentation boundary to the workspace**

```ts
export interface WorkspaceReviewPresentation {
  coordinator: ReviewSessionCoordinator
  capturedScope: ReviewScopeRequest | null
  feedbackCountByEntityId: ReadonlyMap<string, number>
}

export interface WorkspaceProjectViewProps {
  // existing fields remain unchanged
  review: WorkspaceReviewPresentation
}
```

Build the count map once from target-rich snapshots. Do not pass the full Draft into every cell; leaf components receive only `feedbackCount?: number`.

- [x] **Step 5: Route preview states explicitly**

No active Draft + reviewable image uses `ImageReviewWorkspace` with a captured proposal; active Draft + member image uses the same workbench; active Draft + nonmember image uses ordinary `ImagePreview` plus a read-only notice and the three confirmed actions; video and unsupported files retain current preview paths. No navigation route may append an entity to active members.

- [x] **Step 6: Unify return and completion actions**

Rename the large-preview close action to `返回网格`. Put `完成本轮评审` in both `ReviewContextBar` and the workbench toolbar; both call the same coordinator completion-summary command and open the same `ReviewCompletionDialog`. The dialog shows total, revise, Feedback, unreviewable and default-pass counts plus every natural-language opinion; it never checks view telemetry.

- [x] **Step 7: Integrate leave guards and scope-exit choices**

One workspace-level blocker receives intents `navigate`, `return_grid`, `close_project`, `finish_review`, and `abandon_review`. It proceeds immediately when clean; otherwise it offers `继续编辑` and `放弃未保存内容`. Abandoning the whole Draft remains a separate confirmation and is the only path that enables a new captured scope.

- [x] **Step 8: Run workspace, review, search, folder, and app tests**

Run: `pnpm --dir ui exec vitest run src/App.test.tsx src/app/workspace/WorkspaceProjectView.test.tsx src/components/ContentBrowser.test.tsx src/components/contentBrowser/VideoCard.test.tsx src/components/contentBrowser/VideoSection.test.tsx src/components/FolderOverview.test.tsx src/components/FolderFilmstripRow.test.tsx src/components/SearchResults.test.tsx src/components/review`

Expected: PASS for all preview routes, badges, completion entry points, fixed scope and dirty-state blocking.

- [x] **Step 9: Commit workspace integration**

```bash
git add ui/src/App.tsx ui/src/app/workspace/WorkspaceProjectView.tsx ui/src/app/workspace/WorkspaceProjectView.test.tsx ui/src/components/ContentBrowser.tsx ui/src/components/ContentBrowser.test.tsx ui/src/components/contentBrowser/ImageCell.tsx ui/src/components/contentBrowser/VideoCard.tsx ui/src/components/contentBrowser/VideoCard.test.tsx ui/src/components/contentBrowser/VideoSection.tsx ui/src/components/contentBrowser/VideoSection.test.tsx ui/src/components/FolderOverview.tsx ui/src/components/FolderFilmstripRow.tsx ui/src/components/SearchResults.tsx ui/src/components/review/ReviewContextBar.tsx ui/src/components/review/ReviewCompletionDialog.tsx ui/src/components/review/ReviewWorkspaceLayer.tsx ui/src/styles/review.css
git commit -m "feat(review): integrate annotations into workspace flow"
```

---

### Task 13: Upgrade the Agent Reader and Cross-Version End-to-End Contract

**Files:**
- Modify: `scripts/review-protocol/read-latest.mjs`
- Modify: `scripts/review-protocol/read-latest.test.mjs`
- Modify: `src-tauri/examples/review_loop_harness.rs`
- Modify: `scripts/review-protocol/manual-round-e2e.mjs`
- Modify: `scripts/review-protocol/manual-round-e2e.test.mjs`
- Modify: `docs/protocol/README.md`
- Create: `tests/fixtures/review-protocol/project-v2-mixed/.viewer/project.json`
- Create: `tests/fixtures/review-protocol/project-v2-mixed/.viewer/reviews/index.json`
- Create: `tests/fixtures/review-protocol/project-v2-mixed/.viewer/reviews/rounds/00000000-0000-4000-8000-000000000202.json`
- Create: `tests/fixtures/review-protocol/project-v2-mixed/.viewer/reviews/rounds/00000000-0000-4000-8000-000000000204/round.json`
- Create: `tests/fixtures/review-protocol/project-v2-mixed/.viewer/reviews/rounds/00000000-0000-4000-8000-000000000204/artifacts/00000000-0000-4000-8000-000000000301-annotation.png`

**Interfaces:**
- Consumes: v2 catalog/round/artifact formats from Tasks 3–5.
- Produces: one safe reader contract for v1 file Rounds and v2 bundle Rounds, with verified artifact metadata and unchanged natural-language text.

- [x] **Step 1: Write failing reader tests for mixed history**

```js
test('reads mixed v1 and v2 history and verifies every indexed digest', async () => {
  const result = await readLatestCompletedReview({ projectRoot: mixedFixtureProject })
  assert.equal(result.protocolVersion, 'viewer.review/2')
  assert.equal(result.feedback[0].text, '右手结构需要修正')
  assert.equal(result.feedback[0].targets[0].anchor.kind, 'imageStroke')
  assert.match(result.artifacts[0].relativePath, /^artifacts\/[0-9a-f-]+-annotation\.png$/)
  assert.match(result.artifacts[0].blake3, /^[0-9a-f]{64}$/)
})
```

Define `mixedFixtureProject` with `fileURLToPath(new URL('../../tests/fixtures/review-protocol/project-v2-mixed', import.meta.url))`. Build the committed mixed fixture from the existing valid v1 Round and a v2 bundle whose PNG is an exact copy of `tests/fixtures/images/alpha.png`; calculate and store the real BLAKE3 and 640×480 dimensions. Tests that mutate it must first use the existing `disposableProject` copy pattern and must never edit the committed fixture.

Add negative cases for index digest mismatch, manifest identity mismatch, escaping location, symlink bundle/artifact, absent artifact, extra undeclared artifact, oversized JSON/PNG and an unknown protocol.

- [x] **Step 2: Run reader and manual-loop tests and confirm v2 fails**

Run: `pnpm test:review-protocol && pnpm test:review-loop`

Expected: FAIL on v2 bundle discovery while all v1-only cases remain green.

- [x] **Step 3: Implement safe version-aware resolution**

Resolve catalog locations component-by-component beneath the project root, reject symlinks, read bounded bytes, verify catalog digest before JSON parsing, dispatch by `protocol`, and for v2 verify every declared artifact’s relative location, size, BLAKE3, media type and PNG signature. Return relative paths only; never return absolute source or cache paths.

- [x] **Step 4: Extend the manual harness with one four-annotation Round**

The harness creates a fixed image scope, atomically saves the first rectangle feedback, adds a stroke and two more local comments, drops the service/repository, reopens the same project and resumes the exact Draft, verifies all four Anchors, then completes the review and invokes the JavaScript reader. Assert four natural-language opinions, stable one-based marker order, correct default-pass count for unannotated members, one artifact per annotated asset, and the original image digest unchanged before/after.

- [x] **Step 5: Run the protocol and loop suites twice**

Run: `pnpm test:review-protocol && pnpm test:review-loop && pnpm test:review-protocol && pnpm test:review-loop`

Expected: both consecutive runs PASS, proving fixtures are not mutated and recovery is idempotent.

- [x] **Step 6: Commit Agent interoperability**

```bash
git add scripts/review-protocol/read-latest.mjs scripts/review-protocol/read-latest.test.mjs src-tauri/examples/review_loop_harness.rs scripts/review-protocol/manual-round-e2e.mjs scripts/review-protocol/manual-round-e2e.test.mjs tests/fixtures/review-protocol/project-v2-mixed docs/protocol/README.md
git commit -m "feat(review): read v2 annotation review bundles"
```

---

### Task 14: Add Visual Acceptance, Update Product Truth, and Run the Full Gate

**Files:**
- Modify: `ui/src/acceptance/acceptanceStateCatalog.json`
- Modify: `ui/src/acceptance/acceptanceStateCatalog.ts`
- Modify: `ui/src/acceptance/acceptanceStateCatalog.test.ts`
- Modify: `ui/src/acceptance/scenes/reviewScenes.tsx`
- Modify: `ui/src/acceptance/scenes/reviewScenes.test.tsx`
- Modify: `ui/src/acceptance/AcceptanceApp.tsx`
- Modify: `scripts/viewer-visual-acceptance.mjs`
- Modify: `scripts/viewer-visual-acceptance.test.mjs`
- Modify: `docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md`
- Modify: `docs/PRODUCT_SPEC.md`
- Modify: `docs/product/USER_GUIDE.md`
- Modify: `docs/product/FEATURE_REFERENCE.md`
- Modify: `docs/product/SHORTCUTS.md`
- Modify: `docs/product/DATA_PRIVACY.md`
- Modify: `docs/product/TROUBLESHOOTING.md`
- Modify: `docs/README.md`
- Modify: `CHANGELOG.md`
- Modify: `docs/superpowers/specs/2026-08-26-viewer-image-annotation-review-workbench-design.md`
- Modify: `docs/superpowers/plans/2026-08-26-viewer-image-annotation-review-workbench.md`

**Interfaces:**
- Consumes: the complete workbench and protocol implementation.
- Produces: deterministic visual states, verified development evidence, current product documentation and final plan/spec status.

- [x] **Step 1: Add failing acceptance catalog and scene tests**

Add these exact states:

```text
RVW-16 review-auto-start-first-annotation
RVW-17 review-image-four-annotations
RVW-18 review-annotation-save-error
RVW-19 review-rectangle-edit
RVW-20 review-brush-redraw
RVW-21 review-outside-scope-read-only
RVW-22 review-unsaved-leave-guard
RVW-23 review-grid-feedback-badges
RVW-24 review-workbench-zoom-200
```

Tests must assert each ID has a scene, a readiness predicate tied to semantic UI state, no pending network work, and correct zoom metadata for `RVW-24`.

- [x] **Step 2: Run acceptance unit tests and confirm the states are absent**

Run: `pnpm --dir ui exec vitest run src/acceptance/acceptanceStateCatalog.test.ts src/acceptance/scenes/reviewScenes.test.tsx && pnpm test:visual-acceptance`

Expected: FAIL because RVW-16 through RVW-24 are not registered.

- [x] **Step 3: Implement deterministic scenes and capture rules**

Reuse real workbench components and the acceptance bridge; seed stable IDs, image dimensions and four Chinese natural-language opinions. Update both `AcceptanceApp.tsx` and `viewer-visual-acceptance.mjs` so RVW-15 and RVW-24 use zoom 2 while preserving the existing A11Y-05 zoom-2 contract. Do not add screenshot-only substitute markup.

- [x] **Step 4: Run functional, acceptance-unit, and production build gates**

Run:

```bash
cargo test --locked -p viewer-domain --test review_round
cargo test --locked -p viewer-application --test review_session_start --test review_session_mutation --test review_session_completion --test review_catalog
cargo test --locked -p viewer-infrastructure --test review_protocol_contract --test review_repository
cargo test --locked -p viewer-infrastructure --test review_assets
cargo test --locked -p viewer-platform-macos image -- --nocapture
cargo test --locked -p viewer-desktop --test review_commands
pnpm test:review-protocol
pnpm test:review-loop
pnpm --dir ui test
pnpm --dir ui build
pnpm test:visual-acceptance
```

Expected: every command exits 0.

- [ ] **Step 5: Capture and inspect the nine approved visual states**

Run:

```bash
pnpm accept:visual --id RVW-16 --id RVW-17 --id RVW-18 --id RVW-19 --id RVW-20 --id RVW-21 --id RVW-22 --id RVW-23 --id RVW-24 --output-root target/viewer-visual-acceptance/image-annotation-review-workbench
```

Expected: both default viewports pass runtime assertions and produce `product.png`, `reference.png`, `combined.png`, and `manifest.json`. Inspect every combined image for clipping, overlay drift, rail/image overlap, editor overflow, focus visibility and 200% zoom readability; record the evidence paths and verdicts in the acceptance ledger.

- [ ] **Step 6: Update product truth only after implementation evidence passes**

Document the exact image annotation workflow, tool semantics, shortcuts, fixed-scope/outside-scope behavior, default-pass rule, local `.viewer/reviews` storage, read-only PNG derivation, original-file immutability, recovery messages and v1/v2 protocol contract. Change the design status to `Implemented` and this plan status to `Complete` only after Steps 4–5 pass.

Every product-facing document must repeat the durable boundary: Viewer is in early development; signing, Apple notarization, formal installers, public release and sale are not normal post-feature tasks and are not required to close this work.

- [x] **Step 7: Run architecture and policy checks**

Run:

```bash
pnpm architecture:boundaries
pnpm architecture:trends
pnpm test:policy
```

Expected: all architecture, trend and policy gates exit 0 before the documentation/evidence commit.

- [ ] **Step 8: Commit implementation evidence and current documentation**

```bash
git add ui/src/acceptance scripts/viewer-visual-acceptance.mjs scripts/viewer-visual-acceptance.test.mjs docs/reviews/2026-08-02-viewer-atlas-product-migration-ledger.md docs/PRODUCT_SPEC.md docs/product docs/README.md docs/superpowers/specs/2026-08-26-viewer-image-annotation-review-workbench-design.md docs/superpowers/plans/2026-08-26-viewer-image-annotation-review-workbench.md CHANGELOG.md
git commit -m "docs: record image annotation review workbench"
pnpm verify:clean
```

Expected: commit succeeds and the final clean-tree gate exits 0.

## Final Spec-Coverage Gate

Before changing this plan to `Complete`, the executor must be able to point to passing evidence for all of the following:

- automatic first-feedback Draft creation is one repository write and never persists an empty Draft;
- whole-image, rectangle and stroke comments coexist and retain natural-language text;
- numbered marker, inline editor and collapsible rail behavior is keyboard and pointer accessible;
- rectangle edit, full brush redraw, delete and session restore preserve server identity rules;
- ordinary preview still loads, zooms, pans, rotates, magnifies and navigates through its review-neutral surface;
- range membership is frozen, outside-scope annotation is read-only, and only explicit abandonment permits a new scope;
- no browsing telemetry affects completion or default pass;
- v1 fixtures round-trip byte-for-byte and mixed v1/v2 history reads safely;
- v2 catalog records include protocol, safe relative manifest location and verified BLAKE3;
- bundle crash injection recovers at every durable boundary without an index-to-missing-data state;
- generated PNGs are bounded, oriented, digest-verified and leave the original asset byte-identical;
- no absolute/cache/source path crosses Tauri or protocol boundaries;
- all focused, full, visual, architecture, policy, documentation and clean-tree gates pass;
- no signing, notarization, installer, public-release or sales work has been introduced.
