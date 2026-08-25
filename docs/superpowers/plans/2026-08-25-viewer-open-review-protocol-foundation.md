# Viewer Open Review Protocol Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> Status: Active

**Goal:** Build the Agent-independent domain and durable project-file protocol foundation for Viewer review streams, drafts, immutable completed rounds, optional production manifests, and deterministic external reading without adding a user-facing review workflow yet.

**Architecture:** Add a pure Review Domain below an application-owned repository contract, then implement the contract as a focused `.viewer/reviews/` JSON adapter in `viewer-infrastructure`. Keep JSON DTOs, filesystem operations, advisory locking, schema compatibility, and reference-reader concerns outside the Domain; no Tauri command, React state, or concrete Agent integration enters this phase.

**Tech Stack:** Rust 1.97 / edition 2024, existing `serde`, `serde_json`, `uuid`, `blake3`, `libc`, `thiserror`, `tempfile`, Node.js 24 ESM, Node built-in test runner, JSON Schema 2020-12, pnpm 10.

**Spec:** `docs/superpowers/specs/2026-08-25-viewer-ai-material-review-workflow-design.md`

## Global Constraints

- Implement only phase 1, “开放评审协议基础”. Do not add review UI, Tauri commands/events/DTOs, project-opening behavior, CLI handoff into Viewer, version-comparison UI, image-region editing, video-time editing, MCP, local API, or a concrete Agent adapter.
- Preserve every current 0.1.6 user-visible behavior and existing command/event/capability contract.
- Keep `viewer-domain` free of filesystem, JSON-schema, Tauri, React, Agent, and platform dependencies.
- Keep `viewer-application` dependent only on `viewer-domain`; repository implementations remain in `viewer-infrastructure`.
- Do not change `.viewer/metadata.sqlite`, its migrations, marker semantics, operation journal, or `project.json` schema. Review records live only below `.viewer/reviews/`.
- Use project-relative `RelativePath` values for every material reference. Never serialize an absolute project or cache path.
- The canonical protocol identifiers are `viewer.production/1` and `viewer.review/1`.
- Completed rounds are append-only. Drafts never update a Review Stream head, and a Project never has one ambiguous global latest round.
- User feedback text is preserved byte-for-byte after UTF-8 decoding; derived structure may locate it but may not replace it.
- Use these deterministic limits: 50,000 assets per round, 10,000 feedback items per round, 10,000 targets per feedback item, 65,536 UTF-8 bytes per feedback text, 64 MiB per draft/completed/production JSON file, 16 MiB per index file, 10,000 Review Streams per index, and 10,000 completed rounds per Stream.
- Add no npm package and no new third-party Rust crate. Reuse the workspace dependencies already reviewed in `Cargo.toml`.
- Use advisory OS locking for one writer per project. Do not recover a lock by age or delete another live writer’s lock file.
- Every write is durable before publication: file sync first, then directory entry publication, then directory sync. A failed index update must leave the previous Stream head authoritative.
- Core and protocol tests must not install, log in to, or invoke Codex, Claude, OpenCode, or any network service.
- Code signing, Apple notarization, formal installers, public release, and sale are outside this phase and are not completion requirements.
- Execute this plan in an isolated worktree created at execution time. Every task ends in an independently testable commit and a clean task worktree.

---

## File Structure

### Review Domain

- Modify: `crates/viewer-domain/src/lib.rs` — declare review identifiers and export the review module.
- Create: `crates/viewer-domain/src/review/mod.rs` — shared limits, production scope, exports, and error surface.
- Create: `crates/viewer-domain/src/review/asset.rs` — Asset Version, media bounds, portable content evidence, and production identifiers.
- Create: `crates/viewer-domain/src/review/feedback.rs` — natural-language Feedback, multi-target anchors, and coordinate/time validation.
- Create: `crates/viewer-domain/src/review/round.rs` — Review Draft state, completion, Outcomes, unreviewable failures, and immutable Snapshot.

### Application Contracts

- Modify: `crates/viewer-application/src/lib.rs` — export review application contracts.
- Create: `crates/viewer-application/src/review.rs` — production-manifest input model, Review Catalog, Stream resolution, and `ReviewRepositoryPort`.

### Protocol and Repository Adapter

- Modify: `crates/viewer-infrastructure/src/lib.rs` — export the review adapter.
- Create: `crates/viewer-infrastructure/src/review/mod.rs` — public adapter surface.
- Create: `crates/viewer-infrastructure/src/review/protocol.rs` — strict v1 DTOs, limits, validation, and Domain/Application mapping.
- Create: `crates/viewer-infrastructure/src/review/atomic.rs` — durable replace and create-once primitives restricted to owned review paths.
- Create: `crates/viewer-infrastructure/src/review/lease.rs` — macOS advisory writer lease held for repository lifetime.
- Create: `crates/viewer-infrastructure/src/review/repository.rs` — directory validation, Draft persistence, immutable Round publication, per-Stream index updates, and orphan recovery.
- Modify: `crates/viewer-infrastructure/Cargo.toml` — register the two integration-test targets; add no dependency.
- Create: `tests/review_protocol_contract.rs` — Rust codec/fixture/limit/compatibility tests.
- Create: `tests/review_repository.rs` — filesystem, locking, publication, failure-injection, and recovery tests.

### Public Protocol Artifacts

- Create: `docs/protocol/viewer-production-v1.schema.json` — optional producer-owned input schema.
- Create: `docs/protocol/viewer-review-index-v1.schema.json` — per-Stream discovery schema.
- Create: `docs/protocol/viewer-review-draft-v1.schema.json` — recoverable non-consumable Draft schema.
- Create: `docs/protocol/viewer-review-round-v1.schema.json` — immutable Completed Snapshot schema.
- Create: `docs/protocol/README.md` — protocol ownership, discovery, outcome, compatibility, and reader instructions.
- Create: `tests/fixtures/review-protocol/viewer-production-v1.valid.json` — canonical valid production input.
- Create: `tests/fixtures/review-protocol/review-index-v1.valid.json` — canonical two-Stream index proving there is no project-global latest result.
- Create: `tests/fixtures/review-protocol/review-draft-v1.valid.json` — canonical Draft fixture.
- Create: `tests/fixtures/review-protocol/review-round-v1.valid.json` — canonical Completed fixture.
- Create: `tests/fixtures/review-protocol/project/.viewer/reviews/index.json` — reference-reader project index.
- Create: `tests/fixtures/review-protocol/project/.viewer/reviews/rounds/00000000-0000-4000-8000-000000000202.json` — reference-reader completed round.
- Create: `tests/fixtures/review-protocol/project/.viewer/reviews/drafts/00000000-0000-4000-8000-000000000203.json` — Draft that the reader must ignore.

### Agent-Independent Reference Reader and Gates

- Create: `scripts/review-protocol/read-latest.mjs` — read and validate one target Stream’s latest Completed Snapshot.
- Create: `scripts/review-protocol/read-latest.test.mjs` — selector, ambiguity, Draft exclusion, symlink, size, and mismatch tests.
- Modify: `package.json` — add `test:review-protocol` and include it in `quality`.
- Modify: `scripts/repository-policy.test.mjs` — freeze the protocol gate in the deterministic verification path.
- Modify: `docs/README.md` — index the protocol documentation and this plan with Active status.

---

### Task 1: Define review assets, identifiers, Feedback, and Anchors

**Files:**

- Modify: `crates/viewer-domain/src/lib.rs`
- Create: `crates/viewer-domain/src/review/mod.rs`
- Create: `crates/viewer-domain/src/review/asset.rs`
- Create: `crates/viewer-domain/src/review/feedback.rs`

**Interfaces:**

- Consumes: existing `ProjectId`, `RelativePath`, UUID ID convention, image dimensions in pixels, and video durations in microseconds.
- Produces:

```rust
pub const MAX_ASSETS_PER_ROUND: usize = 50_000;
pub const MAX_FEEDBACK_ITEMS_PER_ROUND: usize = 10_000;
pub const MAX_TARGETS_PER_FEEDBACK: usize = 10_000;
pub const MAX_FEEDBACK_TEXT_BYTES: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewValueError {
    Empty,
    InvalidFormat,
    InvalidNumber,
    LimitExceeded,
    DuplicateTarget,
}

pub struct AssetVersionId(Uuid);
pub struct ReviewStreamId(Uuid);
pub struct ReviewRoundId(Uuid);
pub struct FeedbackId(Uuid);

pub struct ProductionId(String);
impl ProductionId {
    pub fn parse(value: &str) -> Result<Self, ReviewValueError>;
    pub fn as_str(&self) -> &str;
}

pub struct ProductionScope {
    pub task_id: ProductionId,
    pub batch_id: ProductionId,
}

pub struct AssetEvidence {
    pub size_bytes: u64,
    pub modified_ns: i128,
    pub blake3: Option<[u8; 32]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewAssetKind {
    Image,
    Video,
}

pub enum ReviewMedia {
    Image { width: u32, height: u32 },
    Video {
        duration_us: Option<u64>,
        display_width: Option<u32>,
        display_height: Option<u32>,
    },
}

pub struct AssetVersion {
    pub id: AssetVersionId,
    pub relative_path: RelativePath,
    pub evidence: AssetEvidence,
    pub media: ReviewMedia,
    pub producer_asset_id: Option<ProductionId>,
    pub parent_asset_version_id: Option<AssetVersionId>,
}

pub struct NormalizedRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}
impl NormalizedRect {
    pub fn new(x: f64, y: f64, width: f64, height: f64)
        -> Result<Self, ReviewValueError>;
    pub fn x(&self) -> f64;
    pub fn y(&self) -> f64;
    pub fn width(&self) -> f64;
    pub fn height(&self) -> f64;
}

pub enum FeedbackAnchor {
    Asset,
    ImageRegion(NormalizedRect),
    VideoPoint { position_us: u64 },
    VideoRange { start_us: u64, end_us: u64 },
}

pub struct FeedbackTarget {
    pub asset_version_id: AssetVersionId,
    pub anchor: FeedbackAnchor,
}

pub struct Feedback {
    pub id: FeedbackId,
    pub text: String,
    pub created_at_ms: i64,
    pub targets: Vec<FeedbackTarget>,
}

impl Feedback {
    pub fn new(
        id: FeedbackId,
        text: String,
        created_at_ms: i64,
        targets: Vec<FeedbackTarget>,
    ) -> Result<Self, ReviewValueError>;
}
```

- `ProductionId` accepts 1–128 ASCII bytes matching `[A-Za-z0-9][A-Za-z0-9._:-]*` and preserves exact spelling.
- `NormalizedRect` requires finite values, positive width/height, nonnegative origin, and `x + width <= 1`, `y + height <= 1`.
- `Feedback::new` rejects negative timestamps, empty/all-whitespace text, text over 65,536 UTF-8 bytes, empty targets, more than 10,000 targets, and duplicate `(asset_version_id, anchor)` targets.
- `ReviewDraft::new` later validates each Asset Version: image dimensions are nonzero; video display dimensions are either both absent or both nonzero; known video duration is nonzero; an Asset cannot name itself as its parent version.

- [ ] **Step 1: Write failing value and anchor tests**

Place unit tests beside the new types. Include these exact cases:

```rust
#[test]
fn production_ids_are_bounded_portable_and_exact() {
    assert_eq!(ProductionId::parse("task:2026-08-25.alpha").unwrap().as_str(), "task:2026-08-25.alpha");
    for invalid in ["", " contains-space", "任务一", "../task", "a/b"] {
        assert!(ProductionId::parse(invalid).is_err(), "accepted {invalid:?}");
    }
    assert!(ProductionId::parse(&"a".repeat(129)).is_err());
}

#[test]
fn normalized_regions_never_escape_the_oriented_image() {
    assert!(NormalizedRect::new(0.1, 0.2, 0.3, 0.4).is_ok());
    for rect in [
        (-0.1, 0.0, 0.2, 0.2),
        (0.0, 0.0, 0.0, 0.2),
        (0.8, 0.0, 0.3, 0.2),
        (0.0, f64::NAN, 0.2, 0.2),
    ] {
        assert!(NormalizedRect::new(rect.0, rect.1, rect.2, rect.3).is_err());
    }
}

#[test]
fn feedback_keeps_natural_language_and_rejects_duplicate_targets() {
    let asset = AssetVersionId::from_u128(1);
    let text = "人物手部需要修正，整体光线保持不变。";
    let feedback = Feedback::new(
        FeedbackId::from_u128(2),
        text.to_owned(),
        10,
        vec![FeedbackTarget { asset_version_id: asset, anchor: FeedbackAnchor::Asset }],
    ).unwrap();
    assert_eq!(feedback.text, text);
    assert!(Feedback::new(
        FeedbackId::from_u128(3),
        text.to_owned(),
        10,
        vec![
            FeedbackTarget { asset_version_id: asset, anchor: FeedbackAnchor::Asset },
            FeedbackTarget { asset_version_id: asset, anchor: FeedbackAnchor::Asset },
        ],
    ).is_err());
}
```

- [ ] **Step 2: Run the focused Domain tests and verify failure**

```bash
cargo test --locked -p viewer-domain review::
```

Expected: FAIL because `viewer_domain::review` and the four review ID types do not exist.

- [ ] **Step 3: Add UUID review IDs and focused value modules**

Add these `id_type!` declarations beside the existing IDs in `lib.rs`:

```rust
id_type!(AssetVersionId);
id_type!(ReviewStreamId);
id_type!(ReviewRoundId);
id_type!(FeedbackId);
```

Implement the interfaces above in `review/asset.rs` and `review/feedback.rs`. Use private fields plus validated constructors for `ProductionId` and `NormalizedRect`; do not expose a deserialization path that bypasses validation. Keep `ReviewMedia` limited to `Image` and `Video`, so Markdown, text, directories, and other files cannot enter a Review Round accidentally.

- [ ] **Step 4: Run Domain tests and formatting**

```bash
cargo test --locked -p viewer-domain review::
cargo fmt --check
```

Expected: PASS; existing Domain tests remain green.

- [ ] **Step 5: Commit the review value model**

```bash
git add crates/viewer-domain/src/lib.rs crates/viewer-domain/src/review
git commit -m "feat: define review asset and feedback values"
```

---

### Task 2: Implement Review Round completion and immutable Outcomes

**Files:**

- Create: `crates/viewer-domain/src/review/round.rs`
- Modify: `crates/viewer-domain/src/review/mod.rs`

**Interfaces:**

- Consumes: Task 1’s `AssetVersion`, `Feedback`, IDs, Anchor validation, and limits.
- Produces:

```rust
pub enum ReviewabilityFailure {
    Unsupported,
    Damaged,
    Unreadable,
    PermissionDenied,
    Missing,
    DecodeFailed,
}

pub struct UnreviewableAsset {
    pub asset_version_id: AssetVersionId,
    pub failure: ReviewabilityFailure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewOutcomeKind { Pass, Revise, Unreviewable }

pub struct ReviewOutcome {
    pub asset_version_id: AssetVersionId,
    pub kind: ReviewOutcomeKind,
    pub feedback_ids: Vec<FeedbackId>,
    pub failure: Option<ReviewabilityFailure>,
}

pub struct ReviewDraft {
    pub project_id: ProjectId,
    pub review_stream_id: ReviewStreamId,
    pub review_round_id: ReviewRoundId,
    pub production: Option<ProductionScope>,
    pub previous_completed_round_id: Option<ReviewRoundId>,
    pub created_at_ms: i64,
    pub assets: Vec<AssetVersion>,
    pub feedback: Vec<Feedback>,
    pub unreviewable: Vec<UnreviewableAsset>,
}

impl ReviewDraft {
    pub fn new(
        project_id: ProjectId,
        review_stream_id: ReviewStreamId,
        review_round_id: ReviewRoundId,
        production: Option<ProductionScope>,
        previous_completed_round_id: Option<ReviewRoundId>,
        created_at_ms: i64,
        assets: Vec<AssetVersion>,
    ) -> Result<Self, ReviewRoundError>;
    pub fn upsert_feedback(&mut self, feedback: Feedback) -> Result<(), ReviewRoundError>;
    pub fn remove_feedback(&mut self, feedback_id: FeedbackId) -> bool;
    pub fn mark_unreviewable(
        &mut self,
        asset_version_id: AssetVersionId,
        failure: ReviewabilityFailure,
    ) -> Result<(), ReviewRoundError>;
    pub fn complete(self, completed_at_ms: i64) -> Result<ReviewSnapshot, ReviewRoundError>;
}

pub struct ReviewSnapshot {
    pub project_id: ProjectId,
    pub review_stream_id: ReviewStreamId,
    pub review_round_id: ReviewRoundId,
    pub production: Option<ProductionScope>,
    pub previous_completed_round_id: Option<ReviewRoundId>,
    pub created_at_ms: i64,
    pub completed_at_ms: i64,
    pub assets: Vec<AssetVersion>,
    pub feedback: Vec<Feedback>,
    pub outcomes: Vec<ReviewOutcome>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewRoundError {
    EmptyAssets,
    LimitExceeded,
    InvalidTimestamp,
    DuplicateAssetId,
    DuplicateAssetPath,
    DuplicateFeedbackId,
    UnknownAsset,
    AnchorMediaMismatch,
    AnchorOutOfBounds,
}
```

- Completion precedence is exact: an asset with one or more valid Feedback targets is `Revise`; otherwise an explicitly unreviewable asset is `Unreviewable`; otherwise it is `Pass`.
- Outcomes follow the frozen asset order. `feedback_ids` follow Feedback insertion order and contain only IDs targeting that asset.
- A `Revise` outcome has at least one `feedback_id` and no failure; an `Unreviewable` outcome has no Feedback IDs and exactly one failure; a `Pass` outcome has neither.
- `upsert_feedback` appends a new Feedback ID, but replaces an existing ID in its original position after fully validating the replacement. A rejected replacement leaves the Draft byte-for-byte equivalent at the Domain-value level.
- `ReviewSnapshot` exposes no mutating methods. A correction requires constructing another Draft whose `previous_completed_round_id` identifies the current Stream head.

- [ ] **Step 1: Write failing aggregate tests**

Cover the state invariants with one readable fixture helper and these assertions:

```rust
#[test]
fn completion_derives_revise_unreviewable_and_default_pass_in_frozen_order() {
    let [revise, unavailable, pass] = review_assets();
    let mut draft = review_draft(vec![revise.clone(), unavailable.clone(), pass.clone()]);
    draft.upsert_feedback(feedback_for(revise.id, "修正手部")).unwrap();
    draft.mark_unreviewable(unavailable.id, ReviewabilityFailure::DecodeFailed).unwrap();

    let snapshot = draft.complete(20).unwrap();
    assert_eq!(
        snapshot.outcomes.iter().map(|item| item.kind).collect::<Vec<_>>(),
        vec![ReviewOutcomeKind::Revise, ReviewOutcomeKind::Unreviewable, ReviewOutcomeKind::Pass],
    );
}

#[test]
fn draft_rejects_unknown_targets_media_mismatches_and_late_completion_time() {
    let image = image_asset(1);
    let mut draft = review_draft(vec![image]);
    assert!(draft.upsert_feedback(video_point_feedback(AssetVersionId::from_u128(99), 1)).is_err());
    assert!(draft.upsert_feedback(video_point_feedback(image.id, 1)).is_err());
    assert!(draft.complete(0).is_err());
}

#[test]
fn completion_consumes_the_draft_and_snapshot_has_no_mutation_api() {
    let snapshot = review_draft(vec![image_asset(1)]).complete(20).unwrap();
    assert_eq!(snapshot.outcomes[0].kind, ReviewOutcomeKind::Pass);
}
```

Also test duplicate Asset Version IDs/paths, empty or over-limit asset lists, duplicate Feedback IDs, video times beyond known duration, image anchors on video, and the rule that Feedback takes precedence over an unreviewable marker.

For Anchor bounds, `videoPoint.position_us <= duration_us` and `videoRange.start_us < videoRange.end_us <= duration_us` when duration is known. When duration is unknown, accept any point and any strictly increasing range representable by `u64`; a later media probe may reject or migrate it before UI editing is introduced.

- [ ] **Step 2: Run the focused tests and verify failure**

```bash
cargo test --locked -p viewer-domain review::round::tests
```

Expected: FAIL because `ReviewDraft`, `ReviewSnapshot`, and Outcome derivation do not exist.

- [ ] **Step 3: Implement the aggregate without persistence knowledge**

Use `HashMap`/`HashSet` only for validation and lookup; retain the input `Vec` order for assets and Feedback. Validate Anchor/media compatibility when Feedback is inserted into the Draft, because the Draft owns the authoritative Asset Version set. Do not import `std::fs`, `serde_json`, or repository types.

- [ ] **Step 4: Run Domain and workspace boundary tests**

```bash
cargo test --locked -p viewer-domain
pnpm architecture:boundaries
```

Expected: PASS; `viewer-domain` gains no forbidden dependency.

- [ ] **Step 5: Commit the Review Round state machine**

```bash
git add crates/viewer-domain/src/review
git commit -m "feat: add immutable review round completion"
```

---

### Task 3: Define application-owned production input and repository contracts

**Files:**

- Create: `crates/viewer-application/src/review.rs`
- Modify: `crates/viewer-application/src/lib.rs`

**Interfaces:**

- Consumes: Task 2’s `ReviewDraft`, `ReviewSnapshot`, `ProductionScope`, `ReviewStreamId`, `ReviewRoundId`, `AssetVersionId`, `ReviewMedia`, and `RelativePath`.
- Produces:

```rust
pub const MAX_REVIEW_STREAMS: usize = 10_000;
pub const MAX_COMPLETED_ROUNDS_PER_STREAM: usize = 10_000;
pub const MAX_PRODUCTION_CONTEXT_ENTRIES: usize = 64;
pub const MAX_PRODUCTION_CONTEXT_KEY_BYTES: usize = 128;
pub const MAX_PRODUCTION_CONTEXT_VALUE_BYTES: usize = 4_096;

pub struct ProductionAsset {
    pub producer_asset_id: ProductionId,
    pub relative_path: RelativePath,
    pub kind: ReviewAssetKind,
    pub parent_producer_asset_id: Option<ProductionId>,
    pub generation: Option<u32>,
}

pub struct ProductionManifest {
    pub production: ProductionScope,
    pub context: BTreeMap<String, String>,
    pub assets: Vec<ProductionAsset>,
}

pub struct ReviewStreamHead {
    pub review_stream_id: ReviewStreamId,
    pub production: Option<ProductionScope>,
    pub completed_round_ids: Vec<ReviewRoundId>,
    pub latest_completed_round_id: Option<ReviewRoundId>,
}

pub struct ReviewCatalog {
    pub project_id: ProjectId,
    pub streams: Vec<ReviewStreamHead>,
}

pub enum ReviewStreamLocator {
    Id(ReviewStreamId),
    Production(ProductionScope),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewCatalogError {
    NotFound,
    Ambiguous,
}

impl ReviewCatalog {
    pub fn resolve_stream(
        &self,
        locator: Option<&ReviewStreamLocator>,
    ) -> Result<&ReviewStreamHead, ReviewCatalogError>;
}

pub trait ReviewRepositoryPort: Send + Sync {
    fn load_catalog(&self) -> Result<ReviewCatalog, ReviewRepositoryError>;
    fn load_draft(
        &self,
        stream_id: ReviewStreamId,
        round_id: ReviewRoundId,
    ) -> Result<Option<ReviewDraft>, ReviewRepositoryError>;
    fn save_draft(&self, draft: &ReviewDraft) -> Result<(), ReviewRepositoryError>;
    fn load_completed(
        &self,
        stream_id: ReviewStreamId,
        round_id: ReviewRoundId,
    ) -> Result<Option<ReviewSnapshot>, ReviewRepositoryError>;
    fn publish(&self, snapshot: &ReviewSnapshot) -> Result<(), ReviewRepositoryError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewRepositoryError {
    ReadOnly,
    Busy,
    NotFound,
    Conflict,
    RecoveryRequired,
    UnsupportedVersion,
    InvalidData,
    LimitExceeded,
    Unavailable,
}
```

- `ReviewAssetKind` is a two-value `Image | Video` enum exported by Task 1.
- `resolve_stream(None)` succeeds only when the catalog has exactly one Stream. Zero matches return `NotFound`; multiple matches return `Ambiguous` rather than choosing the last-written Stream.
- Production context accepts at most 64 entries, keys 1–128 portable ASCII bytes, and values at most 4,096 UTF-8 bytes. These checks live in the protocol adapter; the application type remains serialization-independent.

- [ ] **Step 1: Write failing Stream-resolution tests**

```rust
#[test]
fn catalog_never_uses_a_project_global_latest_round() {
    let first = stream_head(1, "task-a", "batch-a");
    let second = stream_head(2, "task-b", "batch-b");
    let catalog = ReviewCatalog { project_id: ProjectId::from_u128(9), streams: vec![first, second] };

    assert!(matches!(catalog.resolve_stream(None), Err(ReviewCatalogError::Ambiguous)));
    assert_eq!(
        catalog.resolve_stream(Some(&ReviewStreamLocator::Id(ReviewStreamId::from_u128(2)))).unwrap().review_stream_id,
        ReviewStreamId::from_u128(2),
    );
}

#[test]
fn production_locator_requires_one_exact_task_and_batch_match() {
    let catalog = two_stream_catalog();
    let locator = ReviewStreamLocator::Production(ProductionScope {
        task_id: ProductionId::parse("task-b").unwrap(),
        batch_id: ProductionId::parse("batch-b").unwrap(),
    });
    assert_eq!(catalog.resolve_stream(Some(&locator)).unwrap().review_stream_id, ReviewStreamId::from_u128(2));
}
```

- [ ] **Step 2: Run application tests and verify failure**

```bash
cargo test --locked -p viewer-application review::
```

Expected: FAIL because the review application module and repository port do not exist.

- [ ] **Step 3: Implement contracts and deterministic catalog resolution**

Keep repository errors detail-free and safe for later desktop mapping. Do not add an in-memory global repository or application singleton. The repository trait carries Domain values, not JSON DTOs or paths.

- [ ] **Step 4: Verify the application layer and dependency direction**

```bash
cargo test --locked -p viewer-application review::
cargo clippy --locked -p viewer-application --all-targets -- -D warnings
pnpm architecture:boundaries
```

Expected: PASS; `viewer-application` still depends only on `viewer-domain` among Viewer crates.

- [ ] **Step 5: Commit the application contracts**

```bash
git add crates/viewer-application/src/lib.rs crates/viewer-application/src/review.rs
git commit -m "feat: define review repository contracts"
```

---

### Task 4: Freeze v1 schemas, fixtures, and strict Rust codecs

**Files:**

- Create: `docs/protocol/viewer-production-v1.schema.json`
- Create: `docs/protocol/viewer-review-index-v1.schema.json`
- Create: `docs/protocol/viewer-review-draft-v1.schema.json`
- Create: `docs/protocol/viewer-review-round-v1.schema.json`
- Create: `tests/fixtures/review-protocol/viewer-production-v1.valid.json`
- Create: `tests/fixtures/review-protocol/review-index-v1.valid.json`
- Create: `tests/fixtures/review-protocol/review-draft-v1.valid.json`
- Create: `tests/fixtures/review-protocol/review-round-v1.valid.json`
- Create: `crates/viewer-infrastructure/src/review/mod.rs`
- Create: `crates/viewer-infrastructure/src/review/protocol.rs`
- Create: `tests/review_protocol_contract.rs`
- Modify: `crates/viewer-infrastructure/src/lib.rs`
- Modify: `crates/viewer-infrastructure/Cargo.toml`

**Interfaces:**

- Consumes: Tasks 1–3 Domain/Application types.
- Produces:

```rust
pub const PRODUCTION_PROTOCOL_V1: &str = "viewer.production/1";
pub const REVIEW_PROTOCOL_V1: &str = "viewer.review/1";
pub const MAX_REVIEW_DOCUMENT_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_REVIEW_INDEX_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewProtocolError {
    UnsupportedVersion,
    InvalidData,
    LimitExceeded,
}

pub fn decode_production_manifest(bytes: &[u8])
    -> Result<ProductionManifest, ReviewProtocolError>;
pub fn encode_draft(draft: &ReviewDraft)
    -> Result<Vec<u8>, ReviewProtocolError>;
pub fn decode_draft(bytes: &[u8])
    -> Result<ReviewDraft, ReviewProtocolError>;
pub fn encode_completed(snapshot: &ReviewSnapshot)
    -> Result<Vec<u8>, ReviewProtocolError>;
pub fn decode_completed(bytes: &[u8])
    -> Result<ReviewSnapshot, ReviewProtocolError>;
pub fn encode_catalog(catalog: &ReviewCatalog)
    -> Result<Vec<u8>, ReviewProtocolError>;
pub fn decode_catalog(bytes: &[u8])
    -> Result<ReviewCatalog, ReviewProtocolError>;
```

- All JSON uses camelCase, `deny_unknown_fields`, a terminal newline, UUID strings for Viewer IDs, decimal strings for `modifiedNs`, lowercase 64-character hexadecimal for BLAKE3, and project-relative forward-slash paths.
- Draft/Completed use `status: "draft" | "completed"`; Completed includes Outcomes, Draft includes `unreviewable`, and Draft never includes Outcomes.
- Anchor objects are tagged by `kind`: `asset`, `imageRegion`, `videoPoint`, `videoRange`.
- Schema roots use JSON Schema 2020-12, `additionalProperties: false`, the exact protocol constant, and the same numeric/string bounds as Rust.
- Catalog decoding rejects duplicate Stream IDs or duplicate non-null Production scopes, duplicate Round IDs inside a Stream, a nonempty history whose head is not its final ID, and an empty history with a non-null head. Multiple manually created Streams may all have no Production scope. Draft/Completed decoding rejects duplicate Asset, Feedback, Target, Unreviewable, or Outcome identities through the Domain constructors.
- Completed decoding requires exactly one Outcome for every frozen Asset and no Outcome for an unknown Asset. Outcome payload combinations follow Task 2 exactly; a Draft cannot serialize or decode a `pass` conclusion.

- [ ] **Step 1: Write failing fixture contract tests**

Register `review_protocol_contract` in `viewer-infrastructure/Cargo.toml`, then create tests that expect exact fixture round trips:

```rust
#[test]
fn canonical_v1_documents_decode_and_reencode_without_semantic_drift() {
    let production = include_bytes!("fixtures/review-protocol/viewer-production-v1.valid.json");
    let draft = include_bytes!("fixtures/review-protocol/review-draft-v1.valid.json");
    let completed = include_bytes!("fixtures/review-protocol/review-round-v1.valid.json");
    let index = include_bytes!("fixtures/review-protocol/review-index-v1.valid.json");

    assert_json_eq(encode_draft(&decode_draft(draft).unwrap()).unwrap(), draft);
    assert_json_eq(encode_completed(&decode_completed(completed).unwrap()).unwrap(), completed);
    assert_json_eq(encode_catalog(&decode_catalog(index).unwrap()).unwrap(), index);
    assert_eq!(decode_production_manifest(production).unwrap().assets.len(), 2);
}

#[test]
fn protocol_rejects_unknown_versions_fields_absolute_paths_and_oversize_values() {
    assert!(matches!(decode_completed(&mutate_protocol("viewer.review/2")), Err(ReviewProtocolError::UnsupportedVersion)));
    assert!(matches!(decode_completed(&add_unknown_field()), Err(ReviewProtocolError::InvalidData)));
    assert!(matches!(decode_completed(&replace_path("/Users/private/item.png")), Err(ReviewProtocolError::InvalidData)));
    assert!(matches!(decode_completed(&vec![b' '; 64 * 1024 * 1024 + 1]), Err(ReviewProtocolError::LimitExceeded)));
}
```

Use `env!("CARGO_MANIFEST_DIR")` to resolve repository-root fixtures; do not copy fixture bodies into Rust strings.

- [ ] **Step 2: Run the protocol test and verify failure**

```bash
cargo test --locked -p viewer-infrastructure --test review_protocol_contract
```

Expected: FAIL because schemas, fixtures, codecs, and the review infrastructure module do not exist.

- [ ] **Step 3: Add exact schemas and canonical fixtures**

The canonical index fixture must contain two Streams and no project-global `latestCompletedRoundId`:

```json
{
  "protocolVersion": "viewer.review/1",
  "projectId": "00000000-0000-4000-8000-000000000001",
  "streams": [
    {
      "reviewStreamId": "00000000-0000-4000-8000-000000000101",
      "taskId": "task-a",
      "batchId": "batch-a",
      "completedRoundIds": ["00000000-0000-4000-8000-000000000201"],
      "latestCompletedRoundId": "00000000-0000-4000-8000-000000000201"
    },
    {
      "reviewStreamId": "00000000-0000-4000-8000-000000000102",
      "taskId": "task-b",
      "batchId": "batch-b",
      "completedRoundIds": ["00000000-0000-4000-8000-000000000202"],
      "latestCompletedRoundId": "00000000-0000-4000-8000-000000000202"
    }
  ]
}
```

The Completed fixture must exercise one natural-language multi-target Feedback, one `Unreviewable` outcome, one default `Pass`, and `previousCompletedRoundId`. The Draft fixture contains no `outcomes` field.

- [ ] **Step 4: Implement private DTOs and checked Domain mapping**

Keep every `Stored*V1` type private to `protocol.rs`. Deserialize into DTOs, validate counts/IDs/paths/digests/timestamps, then call Domain constructors so JSON cannot bypass Domain invariants. Encode from Domain/Application values using the same DTOs and pretty JSON plus one newline. Convert every parse/serde/domain failure to the stable `ReviewProtocolError` categories without embedding absolute paths or raw input.

- [ ] **Step 5: Run codec, Domain, and schema syntax checks**

```bash
cargo test --locked -p viewer-infrastructure --test review_protocol_contract
cargo test --locked -p viewer-domain -p viewer-application
node -e "for (const p of process.argv.slice(1)) JSON.parse(require('node:fs').readFileSync(p, 'utf8'))" docs/protocol/*.schema.json tests/fixtures/review-protocol/*.json
cargo fmt --check
```

Expected: all commands PASS and all schema/fixture files parse as JSON.

- [ ] **Step 6: Commit the v1 protocol contract**

```bash
git add crates/viewer-infrastructure/Cargo.toml crates/viewer-infrastructure/src/lib.rs crates/viewer-infrastructure/src/review/mod.rs crates/viewer-infrastructure/src/review/protocol.rs docs/protocol tests/fixtures/review-protocol tests/review_protocol_contract.rs
git commit -m "feat: freeze Viewer review protocol v1"
```

---

### Task 5: Add one-writer repository opening and durable Draft persistence

**Files:**

- Create: `crates/viewer-infrastructure/src/review/atomic.rs`
- Create: `crates/viewer-infrastructure/src/review/lease.rs`
- Create: `crates/viewer-infrastructure/src/review/repository.rs`
- Create: `tests/review_repository.rs`
- Modify: `crates/viewer-infrastructure/src/review/mod.rs`
- Modify: `crates/viewer-infrastructure/Cargo.toml`

**Interfaces:**

- Consumes: Task 3’s `ReviewRepositoryPort`; Task 4’s Draft/Catalog codecs.
- Produces:

```rust
pub enum ReviewRepositoryAccess { ReadOnly, ReadWrite }

pub struct ProjectReviewRepository {
    project_root: PathBuf,
    reviews_root: PathBuf,
    project_id: ProjectId,
    access: ReviewRepositoryAccess,
    lease: Option<ProjectReviewLease>,
}

impl ProjectReviewRepository {
    pub fn open(
        project_root: &Path,
        project_id: ProjectId,
        access: ReviewRepositoryAccess,
    ) -> Result<Self, ReviewRepositoryError>;
}
```

- Writable open creates only `.viewer/reviews`, `drafts`, `rounds`, `index.json`, and `write.lock`; it requires an existing safe `.viewer` directory and never changes `project.json` or SQLite.
- Read-only open creates nothing. If `.viewer/reviews` is absent, it returns an empty catalog for the supplied Project ID.
- `ProjectReviewLease` holds an open `write.lock` file and a nonblocking `flock(LOCK_EX | LOCK_NB)` for repository lifetime. Lock release comes from closing the file descriptor, not deleting by timestamp.
- `atomic_replace` uses a unique same-directory file, `create_new`, `write_all`, `sync_all`, atomic `rename`, and parent-directory `sync_all`; it cleans only its owned temporary path after failure.

- [ ] **Step 1: Write failing repository-open and Draft tests**

Register `review_repository` in `viewer-infrastructure/Cargo.toml`. Cover:

```rust
#[test]
fn writable_repository_creates_only_review_paths_and_round_trips_a_draft() {
    let fixture = persistent_project();
    let repository = ProjectReviewRepository::open(
        fixture.root(), fixture.project_id(), ReviewRepositoryAccess::ReadWrite,
    ).unwrap();
    let draft = review_draft(fixture.project_id(), ReviewStreamId::from_u128(1), ReviewRoundId::from_u128(2));
    repository.save_draft(&draft).unwrap();
    assert_eq!(repository.load_draft(draft.review_stream_id, draft.review_round_id).unwrap(), Some(draft));
    assert!(fixture.root().join(".viewer/metadata.sqlite").is_file());
    assert!(fixture.root().join(".viewer/reviews/drafts/00000000-0000-0000-0000-000000000002.json").is_file());
}

#[test]
fn readonly_absent_repository_creates_nothing_and_rejects_writes() {
    let fixture = persistent_project();
    let repository = ProjectReviewRepository::open(
        fixture.root(), fixture.project_id(), ReviewRepositoryAccess::ReadOnly,
    ).unwrap();
    assert!(!fixture.root().join(".viewer/reviews").exists());
    assert_eq!(repository.save_draft(&review_draft_for(&fixture)), Err(ReviewRepositoryError::ReadOnly));
}

#[test]
fn second_writer_is_busy_until_the_first_lease_drops() {
    let fixture = persistent_project();
    let first = writable_repository(&fixture);
    assert!(matches!(writable_repository_result(&fixture), Err(ReviewRepositoryError::Busy)));
    drop(first);
    assert!(writable_repository_result(&fixture).is_ok());
}
```

Also reject a symlinked `.viewer`, `reviews`, `drafts`, `rounds`, `index.json`, or `write.lock`; verify a Draft Project ID mismatch is rejected before writing; and verify an existing `viewer.review/2` index returns `UnsupportedVersion` without changing its bytes or creating Draft/Round files.

- [ ] **Step 2: Run repository tests and verify failure**

```bash
cargo test --locked -p viewer-infrastructure --test review_repository
```

Expected: FAIL because repository, atomic writer, and lease modules do not exist.

- [ ] **Step 3: Implement exact-case directory validation and advisory locking**

Locate `.viewer` by exact case, reject duplicate case-insensitive variants, and validate every owned directory/file with `symlink_metadata`. Open the lock and temporary files with `O_NOFOLLOW` through `std::os::unix::fs::OpenOptionsExt`. Use the existing `libc` dependency for `flock`; map `EWOULDBLOCK` to `Busy` and all other lock failures to `Unavailable`. Keep the locked file handle inside `ProjectReviewRepository`. Sync each newly created directory and its parent before treating writable open as successful.

- [ ] **Step 4: Implement Draft save/load with the final port signatures**

Add inherent `load_catalog`, `load_draft`, and `save_draft` methods using the exact `ReviewRepositoryPort` signatures; defer the trait implementation until Task 6 supplies `load_completed` and `publish`, so this commit contains no placeholder port behavior. Draft path is `drafts/{review_round_id}.json`. Before saving, require Draft `project_id` to equal repository Project ID and reject a final Completed file with the same Round ID. Load uses the 64 MiB bound before allocating and verifies filename, Stream ID, Round ID, and Project ID against decoded content.

- [ ] **Step 5: Run repository, portable metadata, and security regression tests**

```bash
cargo test --locked -p viewer-infrastructure --test review_repository
cargo test --locked -p viewer-infrastructure --test m2_portable_metadata
cargo test --locked -p viewer-desktop --test security_boundaries
cargo clippy --locked -p viewer-infrastructure --all-targets -- -D warnings
```

Expected: PASS; existing `.viewer` portable metadata behavior remains unchanged.

- [ ] **Step 6: Commit Draft persistence and writer lease**

```bash
git add crates/viewer-infrastructure/Cargo.toml crates/viewer-infrastructure/src/review tests/review_repository.rs
git commit -m "feat: persist review drafts under one writer lease"
```

---

### Task 6: Publish immutable rounds and recover only linear orphan history

**Files:**

- Modify: `crates/viewer-infrastructure/src/review/atomic.rs`
- Modify: `crates/viewer-infrastructure/src/review/repository.rs`
- Modify: `crates/viewer-infrastructure/src/review/mod.rs`
- Modify: `tests/review_repository.rs`

**Interfaces:**

- Consumes: Task 5’s repository/lease and Task 4’s Completed/Catalog codecs.
- Produces:

```rust
pub enum ReviewRepositoryFaultPoint {
    AfterRoundDurableBeforeIndex,
    BeforeIndexReplace,
}

pub trait ReviewRepositoryFaultInjector: Send + Sync {
    fn check(&self, point: ReviewRepositoryFaultPoint) -> Result<(), ReviewRepositoryError>;
}

pub struct NoReviewRepositoryFaults;

impl ProjectReviewRepository {
    pub fn open_with_faults(
        project_root: &Path,
        project_id: ProjectId,
        access: ReviewRepositoryAccess,
        faults: Arc<dyn ReviewRepositoryFaultInjector>,
    ) -> Result<Self, ReviewRepositoryError>;
}
```

Task 6 adds a private `faults: Arc<dyn ReviewRepositoryFaultInjector>` field to `ProjectReviewRepository`; `open` supplies `NoReviewRepositoryFaults`, and `open_with_faults` is test-only infrastructure selected explicitly by the caller.

- `publish` requires `snapshot.previous_completed_round_id == stream.latest_completed_round_id`; both `None` is valid only for the first Completed Round in a Stream.
- `load_completed` returns `Some` only when the requested Round ID appears in that Stream’s indexed `completed_round_ids`; durable but unindexed orphan files are not published results.
- Completed path is `rounds/{review_round_id}.json` and is create-once. `atomic_create_once` writes/syncs a unique temporary, creates the final name with a same-directory hard link that fails if the final exists, syncs the directory, removes the temporary, and syncs again.
- Index publication happens only after the Round is durable. The Stream’s `completedRoundIds` append in history order and the head becomes the new Round ID.
- Writable open scans unindexed Completed files. It recovers a unique linear chain whose `previous_completed_round_id` matches the current Stream head. A fork, Project mismatch, Stream mismatch, invalid file, or unconnectable orphan returns `RecoveryRequired` without changing the index.

- [ ] **Step 1: Add failing publication and recovery tests**

```rust
#[test]
fn publishing_updates_only_the_target_stream_and_never_overwrites_a_round() {
    let fixture = persistent_project();
    let repository = writable_repository(&fixture);
    let first = completed_round(fixture.project_id(), 1, 11, None);
    let other = completed_round(fixture.project_id(), 2, 21, None);
    repository.publish(&first).unwrap();
    repository.publish(&other).unwrap();

    let catalog = repository.load_catalog().unwrap();
    assert_eq!(stream(&catalog, 1).latest_completed_round_id, Some(first.review_round_id));
    assert_eq!(stream(&catalog, 2).latest_completed_round_id, Some(other.review_round_id));
    assert_eq!(repository.publish(&first), Err(ReviewRepositoryError::Conflict));
}

#[test]
fn failed_index_publication_keeps_old_head_then_recovers_one_linear_orphan() {
    let fixture = persistent_project();
    let faults = fail_once(ReviewRepositoryFaultPoint::AfterRoundDurableBeforeIndex);
    let repository = writable_repository_with_faults(&fixture, faults);
    let round = completed_round(fixture.project_id(), 1, 11, None);
    assert_eq!(repository.publish(&round), Err(ReviewRepositoryError::Unavailable));
    drop(repository);

    let recovered = writable_repository(&fixture);
    assert_eq!(
        stream(&recovered.load_catalog().unwrap(), 1).latest_completed_round_id,
        Some(round.review_round_id),
    );
}

#[test]
fn orphan_forks_require_recovery_instead_of_guessing() {
    let fixture = project_with_two_orphans_from_the_same_head();
    assert!(matches!(writable_repository_result(&fixture), Err(ReviewRepositoryError::RecoveryRequired)));
    assert_eq!(read_index_bytes(&fixture), fixture.original_index_bytes());
}
```

Also test stale `previous_completed_round_id`, duplicate Round ID, failed index replacement, read-only orphan detection without mutation, Round/filename mismatch, no Draft deletion until successful publication, and the exact stale-Draft behavior after a post-publication cleanup failure.

- [ ] **Step 2: Run focused repository tests and verify failure**

```bash
cargo test --locked -p viewer-infrastructure --test review_repository publish
cargo test --locked -p viewer-infrastructure --test review_repository orphan
```

Expected: FAIL because immutable publication, per-Stream compare-and-set, injection, and orphan recovery do not exist.

- [ ] **Step 3: Implement durable create-once and Stream compare-and-set**

Never call `fs::write` for protocol publication. Complete the Round file first, inject `AfterRoundDurableBeforeIndex`, create a new catalog value in memory, inject `BeforeIndexReplace`, then atomically replace `index.json`. Remove the corresponding Draft only after index publication succeeds. If Draft removal fails after the Completed file and index are durable, `publish` still returns `Ok(())`; `load_draft` treats a Draft with the same Round ID as an indexed Completed Round as absent, and the next writable `open` attempts to remove that owned stale Draft while holding the writer lease. A repeated cleanup failure leaves the stale Draft ignored and does not make the published Round unavailable. Implement `ReviewRepositoryPort` for `ProjectReviewRepository` only after all five port methods have their real behavior; earlier tasks must not provide placeholder behavior for methods they do not yet own.

- [ ] **Step 4: Implement deterministic orphan recovery**

Group valid unindexed files by Review Stream. Starting from each index head, repeatedly accept exactly one orphan whose `previous_completed_round_id` equals the head. Stop successfully when none remain. If more than one matches a head, or any remaining orphan cannot connect, return `RecoveryRequired` and leave `index.json` byte-identical. Publish one recovered catalog replacement only after the whole recovery plan validates.

- [ ] **Step 5: Run fault, concurrency, and full repository tests**

```bash
cargo test --locked -p viewer-infrastructure --test review_repository
cargo test --locked -p viewer-infrastructure --test review_protocol_contract
cargo test --locked -p viewer-infrastructure
cargo fmt --check
```

Expected: PASS with no overwrite, ambiguous recovery, or global-latest behavior.

- [ ] **Step 6: Commit immutable publication and recovery**

```bash
git add crates/viewer-infrastructure/src/review tests/review_repository.rs
git commit -m "feat: publish immutable review stream history"
```

---

### Task 7: Ship the Agent-independent reference reader and mandatory protocol gate

**Files:**

- Create: `scripts/review-protocol/read-latest.mjs`
- Create: `scripts/review-protocol/read-latest.test.mjs`
- Create: `tests/fixtures/review-protocol/project/.viewer/reviews/index.json`
- Create: `tests/fixtures/review-protocol/project/.viewer/reviews/rounds/00000000-0000-4000-8000-000000000202.json`
- Create: `tests/fixtures/review-protocol/project/.viewer/reviews/drafts/00000000-0000-4000-8000-000000000203.json`
- Create: `docs/protocol/README.md`
- Modify: `package.json`
- Modify: `scripts/repository-policy.test.mjs`
- Modify: `docs/README.md`

**Interfaces:**

- Consumes: Task 4’s public schemas/fixtures and Task 6’s per-Stream index contract.
- Produces:

```js
export async function readLatestCompletedReview({
  projectRoot,
  reviewStreamId,
  taskId,
  batchId,
}) // => validated Completed round object

export async function listReviewStreams({ projectRoot })
// => [{ reviewStreamId, taskId, batchId, latestCompletedRoundId }]
```

- CLI forms:

```bash
node scripts/review-protocol/read-latest.mjs --project /absolute/project --stream 00000000-0000-4000-8000-000000000102
node scripts/review-protocol/read-latest.mjs --project /absolute/project --task task-b --batch batch-b
node scripts/review-protocol/read-latest.mjs --project /absolute/project --list
```

- The reader is read-only and returns Completed JSON on stdout. Errors use stderr and exit `1`; it never launches Viewer or an Agent.
- It accepts one exact Stream selector. With no selector, exactly one Stream is required; multiple Streams return a stable ambiguity error and `--list` guidance.
- It reads only `index.json` plus the indexed Round filename, rejects symlinks and size overflow before parsing, verifies protocol/Project/Stream/Round identity, and never enumerates `drafts/` as instructions.

- [ ] **Step 1: Write failing reader and policy tests**

```js
test('reads the selected Stream head and ignores a newer Draft', async () => {
  const round = await readLatestCompletedReview({
    projectRoot: fixtureProject,
    reviewStreamId: '00000000-0000-4000-8000-000000000102',
  })
  assert.equal(round.status, 'completed')
  assert.equal(round.reviewRoundId, '00000000-0000-4000-8000-000000000202')
  assert.notEqual(round.reviewRoundId, '00000000-0000-4000-8000-000000000203')
})

test('refuses to guess when multiple Streams exist', async () => {
  await assert.rejects(
    readLatestCompletedReview({ projectRoot: fixtureProject }),
    /multiple review streams; provide --stream or --task and --batch/i,
  )
})

test('rejects an indexed round whose Stream identity does not match', async () => {
  const project = await mutatedProject({ roundReviewStreamId: '00000000-0000-4000-8000-000000000999' })
  await assert.rejects(readLatestCompletedReview({ projectRoot: project, reviewStreamId: STREAM_B }), /stream identity/i)
})
```

Add symlinked index/round, 16 MiB index, 64 MiB round, unsupported version, missing head, task/batch ambiguity, and CLI exit-code cases. In `repository-policy.test.mjs`, first assert the intended script:

```js
assert.equal(
  packageJson.scripts['test:review-protocol'],
  'node --test scripts/review-protocol/read-latest.test.mjs',
)
assert.match(packageJson.scripts.quality, /pnpm test:review-protocol/)
```

- [ ] **Step 2: Run the reader and policy tests and verify failure**

```bash
node --test scripts/review-protocol/read-latest.test.mjs
pnpm test:policy
```

Expected: FAIL because the reader, fixture project, package command, and policy assertion do not exist.

- [ ] **Step 3: Implement the bounded read-only reference reader**

Use only Node built-ins. Resolve the user-supplied Project root once, then access fixed `.viewer/reviews` children. Use `lstat` to reject symlinks, `stat.size` before `readFile`, UUID syntax before composing a Round filename, and explicit object/array/string/number guards after `JSON.parse`. Never use a recursive directory scan to discover a result.

- [ ] **Step 4: Document the protocol without claiming current product support**

`docs/protocol/README.md` must state:

- Status is Active protocol design/implementation evidence, not a 0.1.6 user feature.
- Producers own `viewer-production.json`; Viewer owns `.viewer/reviews/`.
- Drafts are recoverable Viewer data but never valid Agent instructions.
- Completed outcomes are `pass`, `revise`, and `unreviewable`; natural-language Feedback is authoritative.
- Readers select one Review Stream and do not use a project-global latest result.
- Major-version mismatch is read-only/unsupported; no downgrade write is allowed.
- The reference reader is Agent-independent and performs no network access.

Add one Active row for `protocol/README.md` to `docs/README.md`. The implementation-plan row is created when this plan is accepted; verify that it remains Active instead of adding a duplicate.

- [ ] **Step 5: Bind the protocol contract into normal verification**

Add:

```json
"test:review-protocol": "node --test scripts/review-protocol/read-latest.test.mjs"
```

Insert `pnpm test:review-protocol` in `quality` immediately after `pnpm test:policy`. Do not add it to security or create a release gate.

- [ ] **Step 6: Run focused cross-language and policy verification**

```bash
pnpm test:review-protocol
pnpm test:policy
cargo test --locked -p viewer-infrastructure --test review_protocol_contract --test review_repository
node scripts/review-protocol/read-latest.mjs --project tests/fixtures/review-protocol/project --stream 00000000-0000-4000-8000-000000000102
git diff --check
```

Expected: all tests PASS; the CLI emits the canonical Completed round for Stream B and no Draft.

- [ ] **Step 7: Run the complete clean verification gate**

```bash
pnpm verify:clean
```

Expected: PASS with no untracked/generated repository drift. Existing intentional skips that require bundled native video acceptance remain skips, not failures.

- [ ] **Step 8: Commit the reader, gate, and protocol documentation**

```bash
git add package.json scripts/repository-policy.test.mjs scripts/review-protocol docs/protocol/README.md docs/README.md tests/fixtures/review-protocol/project
git commit -m "feat: add Agent-independent review protocol reader"
```

---

## Completion Checklist

- [ ] `viewer-domain` owns Review vocabulary and invariants without filesystem/protocol dependencies.
- [ ] `viewer-application` owns the production input model, Stream resolution, and repository port.
- [ ] `viewer-infrastructure` is the only crate that serializes protocol DTOs or writes `.viewer/reviews/`.
- [ ] Production, index, Draft, and Completed JSON schemas have canonical fixtures and strict codecs.
- [ ] There is no project-global latest completed round; every head belongs to one Review Stream.
- [ ] Drafts cannot update a Stream head, and Completed Round files cannot be overwritten.
- [ ] One advisory writer lease protects each Project without timeout-based lock deletion.
- [ ] Interrupted publication leaves the previous index authoritative and recovers only an unambiguous linear orphan chain.
- [ ] The reference reader selects an exact Stream, ignores Drafts, rejects unsafe paths/symlinks/oversize files, and needs no specific Agent.
- [ ] Existing `.viewer` SQLite/project metadata contracts and all 0.1.6 product behavior remain unchanged.
- [ ] `pnpm verify:clean` passes from the final task commit.

## Explicitly Deferred to Later Phases

- Starting/ending a Review Round from the current UI;
- Capturing user Feedback from single or multiple selections;
- Mapping current scan/index entities into Asset Versions;
- Mapping image/video failures into `unreviewable` during a real session;
- Reading a production manifest during project open;
- Automatically or manually creating Revision Relations from returned assets;
- Comparing original and revised assets;
- Editing `imageRegion`, `videoPoint`, or `videoRange` Anchors;
- Viewer-opening CLI/URL handoff, MCP, local API, and concrete Agent plugins.
