# Review Save Pipeline Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a normal review save durably acknowledge in a short local transaction while evidence generation and v3 publication complete in the background, without exposing partial or stale-current data to an external Agent.

**Architecture:** Add an authoring control plane to the existing `.viewer/metadata.sqlite`, with a durable command log, logical snapshots, dual heads, and an outbox. Keep `.viewer/reviews` as the verified v3 publication plane; a restartable materializer moves authoring revisions into it, and the native reader fails closed with `publication_pending` until both heads match. The desktop/UI consume an authoring receipt plus a deterministic patch instead of waiting for evidence work and a full view reload.

**Tech Stack:** Rust 1.85 / edition 2024, Tokio, rusqlite 0.40.1, BLAKE3, Tauri, TypeScript, React, Vitest, existing macOS image renderer.

**Spec:** `docs/superpowers/specs/2026-09-01-review-save-pipeline-design.md`

## Global Constraints

- This is an architecture and performance refactor; it adds no product capability and does not change the meaning of any existing review action.
- Ready external data remains `viewer.review/3`; existing v3 state, history, archive, usage, evidence, and index formats remain unchanged.
- A successful foreground acknowledgement means command identity, logical state, transition, head, and outbox row are durable in one local transaction.
- A current Agent read succeeds only when one read transaction observes `authoring_head == published_head`; otherwise it returns `publication_pending` with no current feedback payload.
- Exact already-published history remains readable while a newer current revision is pending.
- Foreground save performs no image decode, PNG encode, evidence scan, whole-project source hash, public JSON publication, or full workspace reload.
- Normal-save targets on a supported local macOS filesystem are p50 <= 50 ms, p95 <= 150 ms, and p99 <= 300 ms for at most 64 KiB of new command data.
- SQLite uses `foreign_keys=ON`, `synchronous=FULL`, bounded busy timeout, and WAL where supported; rollback journal is a correctness-preserving diagnosed fallback.
- No new third-party runtime dependency and no network service.
- No signing, notarization, formal installer, store listing, public release, or sales tasks belong to this plan.
- Load `superpowers:test-driven-development` and read its complete `writing-good-tests.md` before changing tests; use RED -> GREEN -> REFACTOR for every task.

## File and ownership map

| Area | Responsibility after the refactor |
| --- | --- |
| `crates/viewer-application/src/review_workspace/authoring.rs` | Authoring heads, receipts, logical records, patches, status, and store ports |
| `crates/viewer-application/src/review_workspace/materialization.rs` | One claimed job: reopen exact sources, materialize, publish, finalize or classify failure |
| `crates/viewer-application/src/review_workspace/diagnostics.rs` | Dependency-free stage timing events |
| `crates/viewer-infrastructure/src/review/authoring/*` | SQLite schema adapter, canonical codec, command CAS/dedup, outbox, leases, cache, and bootstrap |
| `crates/viewer-infrastructure/src/review/continuous/publication.rs` | Convert one immutable authoring target into the existing verified v3 repository commit |
| `crates/viewer-infrastructure/src/review/continuous/reader/*` | Dual-head current-read gate; history remains exact-selector based |
| `src-tauri/src/state/review_workspace/materialization.rs` | Per-project worker lifecycle, bounded scheduling, wakeups, and status polling bridge |
| `src-tauri/src/dto/review_workspace/*` | Internal desktop wire contracts for patch/status/receipt |
| `ui/src/app/review/*` | Patch reducer and four save/publication states without a full reload |
| `ui/src/components/review/*` | Existing nonblocking user-facing status copy |

The implementation order deliberately keeps the new path behind an internal gate until recovery, reader gating, and UI patching are all present. Every intermediate commit keeps the old synchronous path as the active path and keeps existing v3 projects readable.
The control plane, materializer, reader gate, and UI acknowledgement are not independent product
subsystems: splitting them into separately activated projects would create a period in which acknowledged
authoring state could be stale or unreadable to the Agent. They therefore remain one plan with gated,
independently testable commits.

## Spec coverage matrix

| Approved requirement | Implemented and proven in |
| --- | --- |
| Foreground durability, CAS, command idempotency, outbox atomicity | Tasks 2, 3, 5 |
| Existing-v3 byte-preserving activation | Task 4 |
| Restartable materializer and atomic publication order | Task 6 |
| Equal-head native current-reader gate and pending history behavior | Task 7 |
| Exact source reopening, prewarm, action key, dirty evidence, bounded encodes | Task 8 |
| Archive/restore/migration barriers and safe compaction | Task 9 |
| Desktop worker lifetime and wire boundaries | Task 10 |
| In-place UI patching and four publication states | Task 11 |
| Failure matrix, real corpus, percentile budgets, complete regressions | Task 12 |
| No new product behavior, network/runtime dependency, or release engineering | Global constraints and Task 12 removal/verification gate |

---

### Task 1: Freeze the Current Latency Baseline and Add Stage Timings

**Files:**
- Create: `crates/viewer-application/src/review_workspace/diagnostics.rs`
- Modify: `crates/viewer-application/src/review_workspace/mod.rs`
- Modify: `crates/viewer-application/src/review_workspace/service.rs`
- Modify: `crates/viewer-application/tests/support/continuous_review.rs`
- Create: `crates/viewer-application/tests/review_save_diagnostics.rs`
- Create: `src-tauri/examples/support/continuous_review_fixture.rs`
- Modify: `src-tauri/examples/continuous_review_harness.rs`
- Create: `src-tauri/examples/review_save_latency.rs`

**Interfaces:**
- Produces: `ReviewSaveStage`, `ReviewSaveMeasurement`, and `ReviewSaveObserverPort::record(&self, ReviewSaveMeasurement)`.
- Produces: `ContinuousReviewService::with_save_observer(Arc<dyn ReviewSaveObserverPort>) -> Self`.
- No UI or wire behavior changes.

- [ ] **Step 1: Write the failing stage-order test**

```rust
#[tokio::test]
async fn synchronous_baseline_reports_expensive_work_before_reply() {
    let fixture = Fixture::new();
    let observer = Arc::new(RecordingSaveObserver::default());
    let service = fixture.service().with_save_observer(observer.clone());
    let first = service.prepare_assets(
        &[EntityId::from_u128(10)],
        ReviewTaskCancellation::default(),
    ).await.unwrap().remove(0);

    let envelope = service.prepare(
        ReviewCommandId::from_u128(70),
        None,
        save(first.id, "收紧左袖口"),
    ).await.unwrap();
    service.apply(envelope).await.unwrap();

    assert_eq!(observer.stages(), vec![
        ReviewSaveStage::EvidenceMaterialization,
        ReviewSaveStage::PublicV3Publish,
        ReviewSaveStage::WorkspaceRefresh,
    ]);
}
```

- [ ] **Step 2: Run the targeted test and verify RED**

Run: `cargo test --locked -p viewer-application --test review_save_diagnostics synchronous_baseline_reports_expensive_work_before_reply -- --exact`

Expected: compilation fails because `ReviewSaveObserverPort` and `with_save_observer` do not exist.

- [ ] **Step 3: Add the dependency-free observer and place exact spans**

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewSaveStage {
    AuthoringCommit,
    EvidenceMaterialization,
    PublicV3Publish,
    WorkspaceRefresh,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewSaveMeasurement {
    pub stage: ReviewSaveStage,
    pub elapsed_us: u64,
}

pub trait ReviewSaveObserverPort: Send + Sync {
    fn record(&self, measurement: ReviewSaveMeasurement);
}

#[derive(Default)]
pub struct NoReviewSaveObserver;
impl ReviewSaveObserverPort for NoReviewSaveObserver {
    fn record(&self, _: ReviewSaveMeasurement) {}
}
```

Wrap only the existing evidence call, repository commit call, and `refresh_after_commit` call with `std::time::Instant`; saturate `as_micros()` to `u64::MAX`. Do not log file paths, feedback text, or evidence bytes.

- [ ] **Step 4: Add a repeatable baseline harness**

Extract the existing example's deterministic project builder into
`src-tauri/examples/support/continuous_review_fixture.rs`, import it from both examples, and leave the
existing harness output unchanged. The latency example must print one JSON line per scenario with fields
`scenario`, `samples`, `p50Us`, `p95Us`, and `p99Us`. Fixture setup occurs before timing. Run 30 asset
saves plus 30 region saves; do not assert the new budgets yet because this task records the old path.

Run: `cargo run --locked --release -p viewer-desktop --example review_save_latency -- --samples 30`

Expected: two valid JSON lines and nonzero percentiles; the current region path remains visibly above the final p95 target.

- [ ] **Step 5: Run the focused regression suite**

Run: `cargo test --locked -p viewer-application --test review_save_diagnostics --test continuous_review_service`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/viewer-application/src/review_workspace crates/viewer-application/tests src-tauri/examples
git commit -m "test: baseline review save stages"
```

---

### Task 2: Define Authoring, Patch, Publication, and Materialization Contracts

**Files:**
- Create: `crates/viewer-application/src/review_workspace/authoring.rs`
- Create: `crates/viewer-application/src/review_workspace/materialization.rs`
- Create: `crates/viewer-application/src/review_workspace/patch.rs`
- Modify: `crates/viewer-application/src/review_workspace/mod.rs`
- Modify: `crates/viewer-application/src/review_workspace/model.rs`
- Create: `crates/viewer-application/tests/review_workspace_patch.rs`

**Interfaces:**
- Produces the exact types below; later tasks must use these names rather than parallel DTO-only models.

```rust
pub type ReviewAuthoringSequence = u64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewAuthoringHead {
    pub sequence: ReviewAuthoringSequence,
    pub snapshot_id: ReviewSnapshotId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewHeads {
    pub authoring: Option<ReviewAuthoringHead>,
    pub published: Option<ReviewAuthoringHead>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewBarrierKind { None, Archive, Restore, Migration }

#[derive(Clone, Debug, PartialEq)]
pub struct StoredAuthoringState {
    pub head: ReviewAuthoringHead,
    pub production: Option<ProductionScope>,
    pub state: ContinuousReviewState,
    pub command_id: ReviewCommandId,
    pub payload_digest: [u8; 32],
    pub generated: GeneratedReviewIds,
    pub changes: Vec<ReviewChange>,
    pub archives: Vec<ArchiveCheckpoint>,
    pub adopted_usage: Vec<ReviewUsageDeclaration>,
    pub barrier: ReviewBarrierKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewAuthoringReceipt {
    pub command_id: ReviewCommandId,
    pub payload_digest: [u8; 32],
    pub head: ReviewAuthoringHead,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewMaterializationFailure {
    SourceChanged, SourceMissing, SourceUnreadable, RenderFailed,
    Integrity, LimitExceeded, Io,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewPublicationStatus {
    Ready,
    Pending { pending_revisions: u32 },
    Blocked { code: ReviewMaterializationFailure },
}
```

- Produces `ReviewWorkspaceCurrent`, replacing the UI-facing assumption that every logical head already has a v3 digest:

```rust
pub struct ReviewWorkspaceCurrent {
    pub authoring: StoredAuthoringState,
    pub published_ref: Option<SnapshotRef>,
    pub evidence: Vec<ReviewEvidenceBinding>,
}
```

Change `ReviewWorkspaceView.current` from `Option<StoredContinuousSnapshot>` to
`Option<ReviewWorkspaceCurrent>`. All authoring guards read
`current.authoring.head.snapshot_id`; `published_ref` and `evidence` describe only the last verified
v3 publication and must never be relabeled with the newer authoring snapshot ID.

- Produces these exact authoring result and patch types:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthoringCommandLookup {
    Found(ReviewAuthoringReceipt),
    Absent,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewAuthoringCommitRequest {
    pub expected_snapshot_id: Option<ReviewSnapshotId>,
    pub next: StoredAuthoringState,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewWorkspacePatch {
    pub basis_snapshot_id: Option<ReviewSnapshotId>,
    pub head: ReviewAuthoringHead,
    pub upsert_assets: Vec<AssetVersion>,
    pub remove_asset_version_ids: Vec<AssetVersionId>,
    pub upsert_feedback: Vec<VersionedFeedback>,
    pub remove_feedback_ids: Vec<FeedbackId>,
    pub projection: CurrentReviewProjection,
    pub history_selectors: Option<Vec<HistorySelector>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewAuthoringApplyResult {
    pub receipt: ReviewAuthoringReceipt,
    pub patch: ReviewWorkspacePatch,
    pub publication: ReviewPublicationStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ReviewPatchError {
    #[error("review patch basis does not match the current authoring head")]
    StaleBasis,
    #[error("review patch contains conflicting identities")]
    InvalidPatch,
}
```

- Produces these exact materialization interfaces:

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct ClaimedReviewMaterialization {
    pub stream_id: ReviewStreamId,
    pub target: StoredAuthoringState,
    pub lease_epoch: u64,
    pub attempt_count: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedReviewPublication {
    pub target: ReviewAuthoringHead,
    pub request: ReviewCommitRequest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewPublicationReceipt {
    pub target: ReviewAuthoringHead,
    pub snapshot: SnapshotRef,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewMaterializationOutcome {
    Idle,
    Published { receipt: ReviewPublicationReceipt },
    Retrying { target: ReviewAuthoringHead, attempt_count: u32 },
    Blocked { target: ReviewAuthoringHead, code: ReviewMaterializationFailure },
}

pub trait ReviewMaterializationQueuePort: Send + Sync {
    fn next(&self, now_ms: i64)
        -> Result<Option<ClaimedReviewMaterialization>, ReviewCommitError>;
    fn retry(&self, claim: &ClaimedReviewMaterialization, code: ReviewMaterializationFailure,
        next_attempt_at_ms: i64) -> Result<(), ReviewCommitError>;
    fn block(&self, claim: &ClaimedReviewMaterialization,
        code: ReviewMaterializationFailure) -> Result<(), ReviewCommitError>;
    fn mark_published(&self, claim: &ClaimedReviewMaterialization,
        receipt: ReviewPublicationReceipt) -> Result<(), ReviewCommitError>;
    fn status(&self, stream: ReviewStreamId)
        -> Result<ReviewPublicationStatus, ReviewCommitError>;
    fn requeue_expired(&self, now_ms: i64) -> Result<u32, ReviewCommitError>;
}

#[async_trait]
pub trait ReviewPublicationPort: Send + Sync {
    async fn materialize(&self, target: StoredAuthoringState,
        cancellation: ReviewTaskCancellation)
        -> Result<PreparedReviewPublication, ReviewWorkspaceError>;
    async fn publish(&self, prepared: PreparedReviewPublication)
        -> Result<ReviewPublicationReceipt, ReviewWorkspaceError>;
    async fn verify_publication(&self, target: ReviewAuthoringHead)
        -> Result<Option<ReviewPublicationReceipt>, ReviewWorkspaceError>;
}
```

- [ ] **Step 1: Write failing patch-equivalence tests**

```rust
#[test]
fn patch_reconstructs_the_same_logical_view_as_a_full_projection() {
    let before = workspace_current_with_feedback("原意见");
    let after = workspace_current_with_feedback("新意见");
    let patch = ReviewWorkspacePatch::between(Some(&before), &after, None);
    assert_eq!(patch.apply(Some(before)).unwrap(), after);
}

#[test]
fn patch_rejects_a_different_basis() {
    let before = workspace_current_with_feedback("原意见");
    let after = workspace_current_with_feedback("新意见");
    let mut wrong = before.clone();
    wrong.authoring.head.snapshot_id = ReviewSnapshotId::from_u128(999);
    let patch = ReviewWorkspacePatch::between(Some(&before), &after, None);
    assert_eq!(patch.apply(Some(wrong)), Err(ReviewPatchError::StaleBasis));
}
```

- [ ] **Step 2: Run the targeted test and verify RED**

Run: `cargo test --locked -p viewer-application --test review_workspace_patch`

Expected: compilation fails because `ReviewWorkspacePatch`, `ReviewWorkspaceCurrent`, and `ReviewPatchError` do not exist.

- [ ] **Step 3: Implement the pure contracts and patch reducer**

`ReviewWorkspacePatch::between` must compare stable IDs, emit whole changed `AssetVersion` and `VersionedFeedback` values, sort removals by ID, and set `history_selectors` only when the caller supplies a barrier replacement. `apply` must first compare `basis_snapshot_id` with the current authoring head, then apply removals before upserts, reject duplicate IDs, set the new head/projection, and preserve the last-known published reference/evidence until an explicit full workspace refresh replaces them.

The store commit signature must keep pure transition work inside the bounded SQLite transaction:

```rust
pub trait ContinuousReviewAuthoringStorePort: Send + Sync {
    fn load_heads(&self, stream: ReviewStreamId) -> Result<ReviewHeads, ReviewCommitError>;
    fn load_current(&self, stream: ReviewStreamId)
        -> Result<Option<StoredAuthoringState>, ReviewCommitError>;
    fn find_command(&self, stream: ReviewStreamId, command: ReviewCommandId)
        -> Result<AuthoringCommandLookup, ReviewCommitError>;
    fn commit(
        &self,
        stream: ReviewStreamId,
        command: ReviewCommandId,
        payload_digest: [u8; 32],
        prepare: &mut dyn FnMut(Option<&StoredAuthoringState>)
            -> Result<ReviewAuthoringCommitRequest, ReviewWorkspaceError>,
    ) -> Result<ReviewAuthoringReceipt, ReviewWorkspaceError>;
}
```

- [ ] **Step 4: Run contract tests**

Run: `cargo test --locked -p viewer-application --test review_workspace_patch`

Expected: PASS, including add, redraw, text edit, withdraw, archive, restore, and stale-basis cases.

- [ ] **Step 5: Run application tests and commit**

Run: `cargo test --locked -p viewer-application`

```bash
git add crates/viewer-application/src/review_workspace crates/viewer-application/tests/review_workspace_patch.rs
git commit -m "refactor: define review authoring contracts"
```

---

### Task 3: Add Portable Schema v4 and the SQLite Authoring Store

**Files:**
- Create: `crates/viewer-infrastructure/migrations/portable/0004_review_authoring.sql`
- Modify: `crates/viewer-infrastructure/src/portable/schema.rs`
- Create: `crates/viewer-infrastructure/src/review/authoring/mod.rs`
- Create: `crates/viewer-infrastructure/src/review/authoring/codec.rs`
- Create: `crates/viewer-infrastructure/src/review/authoring/store.rs`
- Create: `crates/viewer-infrastructure/src/review/authoring/queue.rs`
- Modify: `crates/viewer-infrastructure/src/review/mod.rs`
- Modify: `crates/viewer-infrastructure/src/review/provider.rs`
- Create: `crates/viewer-infrastructure/tests/continuous_review_authoring_store.rs`
- Modify: `tests/m2_portable_metadata.rs`
- Modify: `crates/viewer-infrastructure/src/operation/journal.rs`

**Interfaces:**
- Consumes all Task 2 authoring/store types.
- Produces `SqliteContinuousReviewAuthoringStore::open(project_root, project_id, access)` implementing `ContinuousReviewAuthoringStorePort` and `ReviewMaterializationQueuePort`.
- Produces `ProjectReviewRepositoryProvider::authoring_reader()` and `authoring_writer()`.
- Canonical bytes are versioned `viewer.review.authoring/1`, bounded to 16 MiB per logical snapshot and 64 KiB per command-transition payload.

- [ ] **Step 1: Write failing real-database tests for CAS, dedup, and same-transaction outbox**

```rust
#[test]
fn commit_advances_head_and_enqueues_exactly_once() {
    let fixture = AuthoringFixture::new();
    let receipt = fixture.commit(7, None, "第一条").unwrap();
    let duplicate = fixture.commit(7, None, "第一条").unwrap();

    assert_eq!(duplicate, receipt);
    assert_eq!(fixture.store.load_heads(fixture.stream).unwrap().authoring, Some(receipt.head));
    assert_eq!(fixture.queued_targets(), vec![receipt.head.sequence]);
}

#[test]
fn same_command_with_different_digest_is_a_conflict() {
    let fixture = AuthoringFixture::new();
    fixture.commit(7, None, "第一条").unwrap();
    assert!(matches!(
        fixture.commit(7, None, "不同负载"),
        Err(ReviewWorkspaceError::Repository(ReviewCommitError::CommandConflict))
    ));
}
```

- [ ] **Step 2: Run the store test and verify RED**

Run: `cargo test --locked -p viewer-infrastructure --test continuous_review_authoring_store`

Expected: compilation fails because the schema migration and store module do not exist.

- [ ] **Step 3: Add schema v4 with database-enforced invariants**

The migration creates the four tables from the spec. Use canonical UUID text columns with `length(...)=36`; nonnegative integer sequences; foreign keys from snapshots/jobs to streams; unique `(stream_id, snapshot_id)` plus globally unique `command_id`; status checks for `queued|running|retryable|blocked`; barrier checks for `none|archive|restore|migration`; and stable error-code checks using the existing `[a-z0-9_]`, 1..64 convention. Insert schema migration version 4 at the end of the SQL transaction.

Update `LATEST_PORTABLE_SCHEMA_VERSION` to 4. Writable `configure` tries `journal_mode=WAL`, verifies the returned mode, keeps `synchronous=FULL`, and falls back to `journal_mode=DELETE` only when WAL is unavailable. Expose the selected mode as `PortablePersistenceMode::{Wal, Rollback}` to diagnostics. Update the existing journal-mode assertion from hard-coded `delete` to the selected supported mode.

- [ ] **Step 4: Implement canonical codec and bounded transaction**

The codec explicitly maps every field of `StoredAuthoringState` to a serde-owned `AuthoringRecordV1`; it must not serialize Rust enum debug strings or `usize`. Decode must enforce protocol, project/stream identity, sequence/head equality, ID uniqueness, state limits, transition limits, and the same Domain validation used by v3.

`commit` must use `TransactionBehavior::Immediate` and this order: command lookup, current row load, callback, basis validation, insert snapshot, update stream authoring fields, insert queued job, commit. It must perform no filesystem or renderer call while the transaction exists.

- [ ] **Step 5: Add crash-safe migration and WAL tests**

```rust
#[test]
fn v3_database_migrates_to_v4_without_changing_operation_rows() {
    let fixture = portable_v3_fixture();
    let before = fixture.operation_rows();
    let connection = open_database(fixture.path(), true).unwrap();
    assert_eq!(schema_versions(&connection), vec![1, 2, 3, 4]);
    assert_eq!(fixture.operation_rows(), before);
}
```

Run: `cargo test --locked -p viewer-infrastructure --test continuous_review_authoring_store --test m2_portable_metadata`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/viewer-infrastructure/migrations/portable/0004_review_authoring.sql crates/viewer-infrastructure/src/portable crates/viewer-infrastructure/src/review crates/viewer-infrastructure/tests/continuous_review_authoring_store.rs tests/m2_portable_metadata.rs crates/viewer-infrastructure/src/operation/journal.rs
git commit -m "feat: add durable review authoring store"
```

---

### Task 4: Bootstrap Existing v3 Projects Without Rewriting Published Data

**Files:**
- Create: `crates/viewer-infrastructure/src/review/authoring/bootstrap.rs`
- Modify: `crates/viewer-infrastructure/src/review/authoring/mod.rs`
- Modify: `crates/viewer-infrastructure/src/review/provider.rs`
- Modify: `src-tauri/src/state/review_workspace/session.rs`
- Create: `crates/viewer-infrastructure/tests/continuous_review_authoring_bootstrap.rs`

**Interfaces:**
- Produces `ProjectReviewRepositoryProvider::bootstrap_authoring(stream_id) -> Result<ReviewHeads, ReviewCommitError>`.
- Bootstrap acquires the existing review writer lease, verifies the current v3 record with the current repository, inserts exactly one logical authoring snapshot, and sets equal heads in one SQLite transaction.
- A project with legacy v1/v2 data still returns `MigrationRequired`; bootstrap never interprets it.

- [ ] **Step 1: Write failing byte-preservation and idempotency tests**

```rust
#[test]
fn bootstrap_preserves_every_existing_v3_byte() {
    let fixture = verified_v3_project();
    let before = fixture.review_tree_digests();
    let first = fixture.provider.bootstrap_authoring(fixture.stream).unwrap();
    let second = fixture.provider.bootstrap_authoring(fixture.stream).unwrap();

    assert_eq!(first, second);
    assert_eq!(first.authoring, first.published);
    assert_eq!(fixture.review_tree_digests(), before);
    assert_eq!(fixture.materialization_job_count(), 0);
}
```

- [ ] **Step 2: Run and verify RED**

Run: `cargo test --locked -p viewer-infrastructure --test continuous_review_authoring_bootstrap`

Expected: compilation fails because `bootstrap_authoring` does not exist.

- [ ] **Step 3: Implement verified bootstrap**

Decode the exact current v3 state through `ContinuousReviewRepository::load_current`; convert it to `StoredAuthoringState` with sequence 1, the same `snapshot_id`, command ID, digest, changes, archive/usage semantics, and existing evidence references kept only as the published projection. Insert with `published_seq=authoring_seq=1`. A no-current v3 stream inserts the stream control row with both heads null. Do not write an outbox row.

- [ ] **Step 4: Cover legacy, empty, read-only, and interrupted bootstrap**

Run: `cargo test --locked -p viewer-infrastructure --test continuous_review_authoring_bootstrap --test continuous_review_migration`

Expected: PASS; existing v3 bytes and history digests are unchanged, legacy still requires explicit migration, and retry after an injected SQLite failure is idempotent.

- [ ] **Step 5: Commit**

```bash
git add crates/viewer-infrastructure/src/review/authoring crates/viewer-infrastructure/src/review/provider.rs crates/viewer-infrastructure/tests/continuous_review_authoring_bootstrap.rs src-tauri/src/state/review_workspace/session.rs
git commit -m "feat: bootstrap review authoring heads"
```

---

### Task 5: Commit Logical Review Commands and Return Deterministic Patches

**Files:**
- Create: `crates/viewer-application/src/review_workspace/authoring_service.rs`
- Modify: `crates/viewer-application/src/review_workspace/service.rs`
- Modify: `crates/viewer-application/src/review_workspace/transition.rs`
- Modify: `crates/viewer-application/src/review_workspace/projection.rs`
- Modify: `crates/viewer-application/src/review_workspace/model.rs`
- Modify: `crates/viewer-application/src/review_workspace/ports.rs`
- Modify: `crates/viewer-application/tests/support/continuous_review.rs`
- Create: `crates/viewer-application/tests/continuous_review_authoring_service.rs`

**Interfaces:**
- Produces `ReviewAuthoringApplyResult { receipt, patch, publication }`.
- Produces `ContinuousReviewService::apply_authoring_with_cancellation(envelope, cancellation) -> Result<ReviewAuthoringApplyResult, ReviewWorkspaceError>`.
- Adds this restart-safe asset capability; it must verify the persisted version rather than substitute the current path:

```rust
async fn reopen_exact(
    &self,
    assets: &[AssetVersion],
    cancellation: ReviewTaskCancellation,
) -> Result<Vec<PreparedReviewAsset>, ReviewAssetError>;
```
- Keeps existing `apply` active through Task 9; this task does not switch desktop behavior.

- [ ] **Step 1: Write a failing foreground-boundary test**

```rust
#[tokio::test]
async fn authoring_apply_does_not_touch_evidence_or_reload_the_view() {
    let fixture = Fixture::new();
    fixture.evidence.fail_if_called();
    fixture.repository.fail_if_opened();
    let service = fixture.authoring_service();
    let asset = service.prepare_assets(
        &[EntityId::from_u128(10)], ReviewTaskCancellation::default()
    ).await.unwrap().remove(0);
    let envelope = service.prepare(
        ReviewCommandId::from_u128(80), None, save(asset.id, "收紧袖口")
    ).await.unwrap();

    let result = service.apply_authoring_with_cancellation(
        envelope, ReviewTaskCancellation::default()
    ).await.unwrap();

    assert!(matches!(result.publication, ReviewPublicationStatus::Pending { pending_revisions: 1 }));
    assert_eq!(result.patch.upsert_feedback.len(), 1);
}
```

- [ ] **Step 2: Run and verify RED**

Run: `cargo test --locked -p viewer-application --test continuous_review_authoring_service authoring_apply_does_not_touch_evidence_or_reload_the_view -- --exact`

Expected: compilation fails because `apply_authoring_with_cancellation` does not exist.

- [ ] **Step 3: Move pure transition into the authoring-store callback**

The callback must check expected authoring snapshot ID, call the current `transition::prepare`, derive `ReviewBarrierKind` from the command, and return the complete `ReviewAuthoringCommitRequest`. After commit, build the patch from the transaction's before/after states and return immediately. Duplicate command + same digest returns the original receipt and reconstructs the same patch from its parent/target records. A different digest returns `CommandConflict`.

Recovery behavior changes only at the durable boundary: before commit, retain the existing editor recovery record; after a known authoring receipt, clear authoring recovery even if publication is pending. Publication failure is job state, not an unsaved draft.

- [ ] **Step 4: Add equivalence tests for every command family**

Use a table over `SaveFeedback`, text edit, redraw, `Withdraw`, `Archive`, `Restore`, `ConfirmSource`, `ConfirmApplicability`, `AdoptUsage`, `ContinueHistorical`, and `Migrate`. For each, apply the patch to the before view and compare logical current/projection/history selectors with `view()` from the authoring store. Migration may set `requires_full_refresh=true`, but its patch must still preserve exact logical head and receipt.

Run: `cargo test --locked -p viewer-application --test continuous_review_authoring_service --test review_workspace_patch`

Expected: PASS.

- [ ] **Step 5: Prove foreground call exclusions with spies**

Add spies that panic on evidence capture, v3 writer open, source rehash, and full `view()` from inside `apply_authoring_with_cancellation`. Assert all remain unused for asset, region, and text-only saves.

Run: `cargo test --locked -p viewer-application --test continuous_review_authoring_service foreground_`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/viewer-application/src/review_workspace crates/viewer-application/tests
git commit -m "refactor: separate review authoring commit"
```

---

### Task 6: Implement Durable Materialization, Publication, and Restart Reconciliation

**Files:**
- Modify: `crates/viewer-application/src/review_workspace/materialization.rs`
- Create: `crates/viewer-infrastructure/src/review/continuous/publication.rs`
- Create: `crates/viewer-infrastructure/src/review/authoring/reconcile.rs`
- Modify: `crates/viewer-infrastructure/src/review/authoring/queue.rs`
- Modify: `crates/viewer-infrastructure/src/review/continuous/evidence.rs`
- Modify: `crates/viewer-infrastructure/src/review/continuous/commit.rs`
- Modify: `crates/viewer-infrastructure/src/review/provider.rs`
- Create: `crates/viewer-application/tests/review_materialization_service.rs`
- Create: `crates/viewer-infrastructure/tests/continuous_review_materialization.rs`

**Interfaces:**
- `ClaimedReviewMaterialization { stream_id, target, lease_epoch, attempt_count }`.
- `ReviewMaterializationQueuePort::{next, retry, block, mark_published, status, requeue_expired}`.
- `ReviewPublicationPort::{materialize, publish, verify_publication}`.
- `ReviewMaterializationService::run_one(cancellation) -> Result<ReviewMaterializationOutcome, ReviewWorkspaceError>`.
- Publication uses the exact existing v3 writer order: immutable evidence -> immutable state/archive/usage -> atomic index -> database `published_head`.

- [ ] **Step 1: Write the failing dual-head lifecycle test**

```rust
#[tokio::test]
async fn one_job_materializes_exact_target_and_advances_published_head_last() {
    let fixture = MaterializationFixture::pending_revision();
    assert_ne!(fixture.heads().authoring, fixture.heads().published);

    let outcome = fixture.service.run_one(ReviewTaskCancellation::default()).await.unwrap();

    assert!(matches!(outcome, ReviewMaterializationOutcome::Published { .. }));
    assert_eq!(fixture.heads().authoring, fixture.heads().published);
    fixture.assert_v3_current_matches_authoring();
}
```

- [ ] **Step 2: Run and verify RED**

Run: `cargo test --locked -p viewer-application --test review_materialization_service`

Expected: compilation fails because materialization service/claim types are incomplete.

- [ ] **Step 3: Implement restartable lease and failure classification**

`next` claims one eligible job in `BEGIN IMMEDIATE`, increments `lease_epoch`, changes status to `running`, and returns the immutable target. `retry` accepts only the matching epoch, increments attempt count, and uses bounded delays 100 ms, 500 ms, 2 s, 10 s, then 30 s. `SourceChanged`, `SourceMissing`, `SourceUnreadable`, `Integrity`, and `LimitExceeded` become `blocked`; transient renderer/IO failures retry up to five attempts, then block with their stable code. Arbitrary OS error text is never persisted.

- [ ] **Step 4: Adapt the existing v3 repository into `ReviewPublicationPort`**

Reuse current evidence and commit validation. `materialize` reopens only exact persisted assets and returns `PreparedReviewPublication`; `publish` installs/syncs immutable evidence, state, archive, and usage records, then atomically replaces/syncs `index.json`. Only after `verify_publication(target)` rereads and verifies that exact v3 snapshot may `mark_published` advance the database head.

- [ ] **Step 5: Add fault tests at every publication boundary**

Use existing fault injection plus new points `AfterEvidenceSync`, `AfterStateSync`, `AfterIndexPublish`, and `BeforePublishedHead`. Restart a fresh store/service after each injected failure. Assert heads differ until the target verifies; after-index recovery verifies the already-complete target and advances the head without regenerating evidence.

Run: `cargo test --locked -p viewer-infrastructure --test continuous_review_materialization --test continuous_review_recovery`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/viewer-application/src/review_workspace/materialization.rs crates/viewer-application/tests/review_materialization_service.rs crates/viewer-infrastructure/src/review crates/viewer-infrastructure/tests
git commit -m "feat: materialize review publications in background"
```

---

### Task 7: Fail Closed in the Native Reader While Publication Is Pending

**Files:**
- Modify: `crates/viewer-infrastructure/src/review/protocol/v3/read_result.rs`
- Modify: `crates/viewer-infrastructure/src/review/continuous/reader/mod.rs`
- Modify: `crates/viewer-infrastructure/src/review/continuous/reader/project.rs`
- Modify: `crates/viewer-infrastructure/src/review/continuous/reader/current.rs`
- Modify: `crates/viewer-infrastructure/src/review/continuous/reader/history.rs`
- Modify: `crates/viewer-infrastructure/tests/continuous_review_reader.rs`
- Modify: `tests/review_protocol_v3_contract.rs`
- Modify: `docs/protocol/README.md`

**Interfaces:**
- Adds `ReadErrorCode::PublicationPending`, serialized exactly as `publication_pending`.
- Adds read-only `PinnedReviewHeads` backed by a SQLite deferred read transaction; it remains alive until current v3 verification and result encoding finish.
- No control row means existing v3/legacy behavior, preserving compatibility.

- [ ] **Step 1: Write failing current-vs-history tests**

```rust
#[test]
fn current_read_returns_no_payload_when_authoring_is_ahead() {
    let fixture = published_then_authored_project();
    let result = fixture.read_current();
    assert_eq!(result["error"]["code"], "publication_pending");
    assert!(result.pointer("/result/feedback").is_none());
}

#[test]
fn exact_published_history_remains_readable_while_current_is_pending() {
    let fixture = published_then_authored_project();
    let result = fixture.read_history(fixture.published_history_selector());
    assert_eq!(result["result"]["status"], "ok");
    assert_eq!(result["result"]["role"], "history");
}
```

- [ ] **Step 2: Run and verify RED**

Run: `cargo test --locked -p viewer-infrastructure --test continuous_review_reader current_read_returns_no_payload_when_authoring_is_ahead -- --exact`

Expected: current reader incorrectly returns the older published payload or compilation lacks `PublicationPending`.

- [ ] **Step 3: Implement the pinned-head gate**

Open `.viewer/metadata.sqlite` read-only, begin a deferred transaction, and select both heads for the resolved stream. For `Current` and `List`, compare heads before reading the v3 index. Different heads produce a bounded `Failure` with code `PublicationPending` and message `review publication is still being generated`; no `CurrentReadResult` is created. Equal heads pin the authoring snapshot ID and require the v3 current snapshot ID to match before returning success. History reads bypass equality but retain current identity/reachability verification.

- [ ] **Step 4: Add race and compatibility tests**

Pause a reader after pinning equal heads, commit a newer authoring revision, then let the read finish: it may return the complete old invocation snapshot. A new invocation must return `publication_pending`. Also test no metadata DB, no authoring row, empty stream, equal heads, unknown stream, malformed head row, and existing v3 fixtures.

Run: `cargo test --locked -p viewer-infrastructure --test continuous_review_reader --test review_protocol_v3_contract`

Expected: PASS.

- [ ] **Step 5: Document the typed retry contract and commit**

Document that Agent integrations retry `publication_pending` with bounded backoff and must not fall back to cached “latest” data. History selectors are still exact and readable.

```bash
git add crates/viewer-infrastructure/src/review crates/viewer-infrastructure/tests tests/review_protocol_v3_contract.rs docs/protocol/README.md
git commit -m "feat: gate agent reads on published review head"
```

---

### Task 8: Add Evidence Prewarming, Action-Key Cache, and Dirty-Set Materialization

**Files:**
- Create: `crates/viewer-application/src/review_workspace/evidence_action.rs`
- Modify: `crates/viewer-application/src/review_workspace/preview.rs`
- Modify: `crates/viewer-application/src/review_workspace/materialization.rs`
- Create: `crates/viewer-infrastructure/src/review/authoring/evidence_cache.rs`
- Modify: `crates/viewer-infrastructure/src/review/authoring/mod.rs`
- Modify: `crates/viewer-infrastructure/src/review/continuous/evidence.rs`
- Modify: `crates/viewer-platform-macos/src/image/review_evidence.rs`
- Create: `crates/viewer-application/tests/review_evidence_action.rs`
- Create: `crates/viewer-infrastructure/tests/continuous_review_dirty_evidence.rs`
- Create: `crates/viewer-platform-macos/tests/review_evidence_renderer.rs`

**Interfaces:**
- Produces `ReviewEvidenceActionKey([u8; 32])` and `ReviewEvidenceActionPolicy { renderer_version: u32, output_policy_version: u32 }`.
- Produces `dirty_evidence_assets(previous: Option<&StoredAuthoringState>, target: &StoredAuthoringState, policy: ReviewEvidenceActionPolicy) -> Result<Vec<AssetVersionId>, ReviewWorkspaceError>`.
- Extends preview preparation with cancellable `prewarm_base_evidence`, never changing review state.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CachedReviewEvidenceAction {
    pub action_key: ReviewEvidenceActionKey,
    pub base: EvidenceRef,
    pub annotated: Option<EvidenceRef>,
    pub renderer_version: u32,
    pub output_policy_version: u32,
}

pub trait ReviewEvidenceActionCachePort: Send + Sync {
    fn load_verified(&self, key: ReviewEvidenceActionKey)
        -> Result<Option<CachedReviewEvidenceAction>, ReviewCommitError>;
    fn store_verified(&self, value: &CachedReviewEvidenceAction)
        -> Result<(), ReviewCommitError>;
    fn remove(&self, key: ReviewEvidenceActionKey) -> Result<(), ReviewCommitError>;
}
```

- [ ] **Step 1: Write failing action-key tests**

```rust
#[test]
fn feedback_text_does_not_change_evidence_action_key() {
    let a = scene("原文字", rectangle(0.1, 0.2, 0.3, 0.4));
    let b = scene("新文字", rectangle(0.1, 0.2, 0.3, 0.4));
    assert_eq!(evidence_action_key(&a, policy()), evidence_action_key(&b, policy()));
}

#[test]
fn source_geometry_renderer_and_output_policy_each_invalidate_the_key() {
    let base = scene("文字", rectangle(0.1, 0.2, 0.3, 0.4));
    assert_ne!(key(&base), key(&base.with_source_digest([2; 32])));
    assert_ne!(key(&base), key(&base.with_rect(rectangle(0.2, 0.2, 0.3, 0.4))));
    assert_ne!(key(&base), key_with_versions(&base, 2, 1));
    assert_ne!(key(&base), key_with_versions(&base, 1, 2));
}
```

- [ ] **Step 2: Run and verify RED**

Run: `cargo test --locked -p viewer-application --test review_evidence_action`

Expected: compilation fails because the action-key module does not exist.

- [ ] **Step 3: Implement canonical scene hashing and dirty selection**

Hash the exact domain separator `viewer.review.evidence-action/1`, source digest, ordered target/revision IDs, normalized geometry encoded as canonical finite decimal/fixed integer values, marker ordinals, drawing policy, renderer/output versions, and upright dimensions. Exclude feedback text. Reject missing source digest for cache lookup. Dirty selection compares keys against the last published logical target and carries unchanged evidence bindings forward without opening their PNG files.

- [ ] **Step 4: Implement verified cache and single-pass object creation**

On cache lookup, verify referenced object kind, byte count, digest, width/height, renderer version, and output-policy version. Missing/corrupt objects are misses and the row is removed. Extend the renderer result to return its digest and byte count computed while writing the private temporary PNG. Installation uses atomic create-once plus file and parent sync; it does not reread the new file to hash it.

- [ ] **Step 5: Add preview prewarming and bounded scheduling**

After `prepare_asset_previews` authorizes an exact image version, schedule a cancellable clean-base capture keyed by `AssetVersionId`. Use one active materialization job per project and a process-wide `tokio::sync::Semaphore` with two image-encode permits. Preview rendering retains priority by acquiring its own existing interactive task slot before background materialization.

- [ ] **Step 6: Prove unchanged evidence is O(dirty assets)**

Create 1,000 verified cache rows and instrument filesystem opens. A text-only edit and an edit on one asset must not read 999 unchanged PNGs. A corrupt cache object must be re-rendered or block as integrity failure, never publish success from the row alone.

Run: `cargo test --locked -p viewer-application --test review_evidence_action && cargo test --locked -p viewer-infrastructure --test continuous_review_dirty_evidence && cargo test --locked -p viewer-platform-macos --test review_evidence_renderer`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/viewer-application/src/review_workspace crates/viewer-application/tests crates/viewer-infrastructure/src/review crates/viewer-infrastructure/tests crates/viewer-platform-macos
git commit -m "perf: materialize only dirty review evidence"
```

---

### Task 9: Add Barrier Flushes and Safe Queue Compaction

**Files:**
- Create: `crates/viewer-application/src/review_workspace/barrier.rs`
- Modify: `crates/viewer-application/src/review_workspace/authoring_service.rs`
- Modify: `crates/viewer-application/src/review_workspace/materialization.rs`
- Modify: `crates/viewer-infrastructure/src/review/authoring/queue.rs`
- Modify: `crates/viewer-infrastructure/src/review/continuous/publication.rs`
- Create: `crates/viewer-application/tests/review_publication_barriers.rs`
- Create: `crates/viewer-infrastructure/tests/continuous_review_compaction.rs`

**Interfaces:**
- Produces `ReviewMaterializationService::flush_through(stream, head, cancellation) -> Result<(), ReviewWorkspaceError>`.
- Produces pure `fold_public_changes(published, targets) -> Result<Vec<ReviewChange>, ReviewWorkspaceError>`.
- Normal queued jobs may compact only within one interval bounded by `Archive`, `Restore`, or `Migration` revisions.

- [ ] **Step 1: Write failing barrier and folding tests**

```rust
#[test]
fn compaction_stops_before_and_after_every_barrier() {
    let jobs = jobs([None, None, Archive, None, Restore, None, Migration, None]);
    assert_eq!(compact_targets(&jobs), vec![2, 3, 4, 5, 6, 7, 8]);
}

#[test]
fn add_then_edit_folds_to_one_final_addition() {
    let changes = fold_public_changes(&published_empty(), &[added_v1(), edited_v2()]).unwrap();
    assert_eq!(changes, vec![final_added_v2()]);
}
```

- [ ] **Step 2: Run and verify RED**

Run: `cargo test --locked -p viewer-infrastructure --test continuous_review_compaction`

Expected: compilation fails because compaction/folding functions do not exist.

- [ ] **Step 3: Implement barrier flush semantics**

Before `Archive`, `Restore`, or `Migration` transition preparation, flush through the selected authoring basis and verify equal heads. If a job is blocked, map its stable failure to the existing source/evidence error and do not insert the barrier command. After a public basis exists, commit the barrier authoring revision normally and enqueue it. Never infer a historical basis from unpublished logical state.

- [ ] **Step 4: Implement deterministic compaction**

Claim the newest normal target within the first unblocked segment after `published_head`; mark skipped render jobs satisfied only after the chosen target publishes. Fold transitions from the public basis: add+edit -> final `Added`, edit+edit -> one `Edited` from public key, add+withdraw -> no target delta, published withdraw -> typed `Withdrawn`, and availability changes retain their typed cause. Never delete command/snapshot rows and never cross a barrier.

- [ ] **Step 5: Run archive/restore/migration regressions**

Run: `cargo test --locked -p viewer-application --test review_publication_barriers && cargo test --locked -p viewer-infrastructure --test continuous_review_compaction --test continuous_review_archive_integrity --test continuous_review_migration`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/viewer-application/src/review_workspace crates/viewer-application/tests crates/viewer-infrastructure/src/review crates/viewer-infrastructure/tests
git commit -m "feat: preserve review barriers during compaction"
```

---

### Task 10: Run the Worker in the Desktop Session and Expose Patch/Status Wire Types

**Files:**
- Create: `src-tauri/src/state/review_workspace/materialization.rs`
- Modify: `src-tauri/src/state/review_workspace.rs`
- Modify: `src-tauri/src/state/review_workspace/session.rs`
- Modify: `src-tauri/src/state/review_workspace/tasks.rs`
- Modify: `src-tauri/src/commands/review_workspace.rs`
- Modify: `src-tauri/src/dto/review_workspace.rs`
- Modify: `src-tauri/src/dto/review_workspace/view.rs`
- Modify: `src-tauri/src/dto/review_workspace/requests.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/tests/review_workspace_commands.rs`
- Create: `src-tauri/tests/review_materialization_lifecycle.rs`

**Interfaces:**
- `apply_review_command` returns `ReviewAuthoringApplyResultDto`, not a complete `ReviewWorkspaceViewDto`.
- Adds `get_review_publication_status(request) -> ReviewPublicationStatusDto` for scoped, nonblocking reconciliation.
- A session owns one worker wake channel and cancellation token. Close/reopen cancels, awaits, drops leases, and a new session requeues expired work.
- The internal rollout gate is `ReviewSavePipeline::{SynchronousV3, AuthoringOutbox}` in `ReviewWorkspaceConfig`; only one path may write a stream.

- [ ] **Step 1: Write failing command contract and close/restart tests**

```rust
#[tokio::test]
async fn apply_returns_before_slow_materialization_and_status_converges() {
    let fixture = DesktopFixture::with_blocked_renderer();
    let reply = fixture.apply_feedback().await.unwrap();
    assert!(matches!(reply.publication, ReviewPublicationStatus::Pending { .. }));
    assert!(!fixture.renderer_finished());

    fixture.release_renderer();
    fixture.wait_until_ready().await;
    assert_eq!(fixture.status().await, ReviewPublicationStatus::Ready);
}
```

- [ ] **Step 2: Run and verify RED**

Run: `cargo test --locked -p viewer-desktop --test review_materialization_lifecycle`

Expected: compilation fails because the worker/status command and authoring DTO do not exist.

- [ ] **Step 3: Implement worker ownership and wakeups**

Initialization bootstraps authoring, calls `requeue_expired`, starts one loop, and stores its join handle in `ReviewWorkspaceSession`. A successful authoring commit sends a coalescing wake signal after the transaction. The loop processes until `next()` is empty, yields between jobs, and sleeps until wake or the earliest retry deadline. Session close cancels and awaits the worker before project resources are released.

- [ ] **Step 4: Add exact DTO encoders**

Serialize `ReviewAuthoringReceipt`, `ReviewWorkspacePatch`, `ReviewPublicationStatus`, `ReviewMaterializationFailure`, `ReviewAuthoringHead`, and the new `ReviewWorkspaceCurrent` with existing bounded `ReviewWire`. Add TypeScript contract fixtures now so later UI work consumes stable camelCase values. Error DTO mapping must distinguish unsaved authoring failures from saved-but-blocked publication status.

- [ ] **Step 5: Run desktop lifecycle and command suites**

Run: `cargo test --locked -p viewer-desktop --test review_workspace_commands --test review_materialization_lifecycle`

Expected: PASS under both internal pipeline modes; authoring mode returns while the renderer is held.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src src-tauri/tests
git commit -m "feat: run review materializer in project session"
```

---

### Task 11: Apply Review Patches in Place and Present Publication Status

**Files:**
- Modify: `ui/src/api/reviewWorkspaceTypes.ts`
- Modify: `ui/src/api/reviewWorkspaceViewTypes.ts`
- Modify: `ui/src/api/viewer.ts`
- Modify: `ui/src/api/reviewWorkspaceTypes.test.ts`
- Create: `ui/src/app/review/reviewWorkspacePatch.ts`
- Modify: `ui/src/app/review/continuousReviewModel.ts`
- Modify: `ui/src/app/review/continuousReviewSession.ts`
- Modify: `ui/src/app/review/useContinuousReviewCoordinator.ts`
- Modify: `ui/src/app/review/imageReviewWorkbenchAdapter.ts`
- Modify: `ui/src/app/review/continuousReviewTestFixtures.ts`
- Modify: `ui/src/app/review/useContinuousReviewCoordinator.test.tsx`
- Modify: `ui/src/app/review/useContinuousReviewCoordinator.lifecycle.test.tsx`
- Modify: `ui/src/components/review/ReviewFeedbackRail.tsx`
- Modify: `ui/src/components/review/ImageReviewWorkbench.integration.test.tsx`

**Interfaces:**
- `ContinuousReviewState` becomes exactly:

```ts
export type ContinuousReviewState =
  | { kind: 'loading' }
  | { kind: 'ready' }
  | { kind: 'saving_authoring' }
  | { kind: 'saved_pending_publication'; pendingRevisions: number }
  | { kind: 'publication_blocked'; code: ReviewMaterializationFailure }
  | { kind: 'save_failed'; error: ReviewWorkspaceError }
  | { kind: 'recovery_required'; error: ReviewWorkspaceError | null }
  | { kind: 'migration_required' }
  | { kind: 'unavailable'; error: ReviewWorkspaceError }
```

- `applyReviewWorkspacePatch(view, patch) -> ReviewWorkspaceView` mirrors the Rust reducer and throws `ReviewPatchMismatch` only for stale/corrupt basis.
- While pending, coordinator polls `getReviewPublicationStatus` at 250 ms, 500 ms, 1 s, then 2 s; polling stops on ready, blocked, session replacement, or unmount.

- [ ] **Step 1: Write failing no-reload interaction test**

```tsx
it('keeps canvas transform and marker while publication completes', async () => {
  const port = pendingPublicationPort()
  const work = renderImageReviewWorkbench({ port })
  work.zoomTo(180)
  work.panTo({ x: 120, y: -40 })
  work.drawRectangle()
  await work.saveText('logo有错误')

  expect(work.canvasTransform()).toEqual({ zoom: 1.8, x: 120, y: -40 })
  expect(work.savedMarkers()).toHaveLength(1)
  expect(screen.getByRole('status')).toHaveTextContent('已保存，Agent 数据生成中')
  expect(port.getWorkspace).toHaveBeenCalledTimes(1)

  port.resolveStatus({ kind: 'ready' })
  expect(await screen.findByRole('status')).toHaveTextContent('已保存，可供外部读取')
  expect(port.getWorkspace).toHaveBeenCalledTimes(1)
})
```

- [ ] **Step 2: Run and verify RED**

Run: `pnpm --dir ui test -- ImageReviewWorkbench.integration.test.tsx -t "keeps canvas transform and marker while publication completes"`

Expected: current client expects `reply.view`, replaces the full snapshot, or lacks pending status.

- [ ] **Step 3: Implement TypeScript patch reducer and authoring save flow**

After `applyCommand`, validate and apply `reply.patch`, close the editor, retain the saved marker, clear only the submitted geometry, set the authoring head as the next expected snapshot ID, and enter pending/ready/blocked from `reply.publication`. Do not call `refresh()` on a normal successful save. A patch-basis mismatch enters reconciliation and performs one full refresh; uncertain/unknown commit outcomes retain current recovery behavior.

- [ ] **Step 4: Implement bounded status polling and exact copy**

Map states to: `saving_authoring` -> `正在保存意见，请稍候。`; pending -> `已保存，Agent 数据生成中`; ready -> `已保存，可供外部读取`; blocked -> `评审已保存，Agent 数据生成失败`. Materialization failure must not reopen the editor or remove the saved marker. Existing retry/source confirmation actions remain the only recovery actions shown for their stable error code.

- [ ] **Step 5: Run coordinator and integration tests**

Run: `pnpm --dir ui test -- reviewWorkspaceTypes.test.ts useContinuousReviewCoordinator.test.tsx useContinuousReviewCoordinator.lifecycle.test.tsx ImageReviewWorkbench.integration.test.tsx`

Expected: PASS, including duplicate save, further typing during authoring commit, pending status, blocked status, session replacement, unmount, patch mismatch refresh, and no full-screen loading after save.

- [ ] **Step 6: Commit**

```bash
git add ui/src/api ui/src/app/review ui/src/components/review
git commit -m "perf: apply saved review patches without reload"
```

---

### Task 12: Activate the New Path, Exercise Faults and Performance, Then Remove the Fallback

**Files:**
- Modify: `src-tauri/src/state/review_workspace/session.rs`
- Modify: `src-tauri/examples/review_save_latency.rs`
- Modify: `crates/viewer-infrastructure/tests/continuous_review_recovery.rs`
- Modify: `crates/viewer-infrastructure/tests/continuous_review_costs.rs`
- Create: `src-tauri/tests/review_save_performance.rs`
- Create: `docs/reviews/2026-09-01-review-save-pipeline-verification.md`
- Modify: `docs/superpowers/specs/2026-09-01-review-save-pipeline-design.md`
- Modify: `docs/PRODUCT_SPEC.md`
- Modify: `docs/product/README.md`

**Interfaces:**
- Final build has one active authoring/outbox path; remove `ReviewSavePipeline::SynchronousV3`, old synchronous `ContinuousReviewService::apply` wiring, and DTO code that returns a full view from normal save.
- Keep full `view()` for activation, explicit refresh, stale rebase, uncertain outcome, and migration only.

- [ ] **Step 1: Add final foreground performance assertions**

```rust
#[test]
#[ignore = "release-mode local filesystem performance gate"]
fn normal_authoring_save_meets_latency_budgets() {
    let samples = measure_authoring_saves(1_000, 64 * 1024);
    assert!(percentile(&samples, 50) <= Duration::from_millis(50));
    assert!(percentile(&samples, 95) <= Duration::from_millis(150));
    assert!(percentile(&samples, 99) <= Duration::from_millis(300));
}
```

The harness must cover asset-only, uncached first region, cached additional region, text-only edit, 30 sequential saves with a deliberately slow materializer, and 1/10/100/1,000 unchanged evidence objects. It includes SQLite sync and real desktop DTO serialization, but excludes compilation and fixture construction.

- [ ] **Step 2: Run the release-mode performance gate and verify GREEN**

Run: `cargo test --locked --release -p viewer-desktop --test review_save_performance -- --ignored --nocapture`

Expected: p50 <= 50 ms, p95 <= 150 ms, p99 <= 300 ms; foreground renderer/source-hash/full-view spy counts are zero; latency does not grow with unchanged evidence count.

- [ ] **Step 3: Run real-corpus measurements**

Run: `cargo run --locked --release -p viewer-desktop --example review_save_latency -- --project "/Users/abc/Downloads/测试图" --samples 30`

Expected: the harness prints separate authoring and materialization percentiles. Copy the JSON output and machine/filesystem description into `docs/reviews/2026-09-01-review-save-pipeline-verification.md`; do not turn materialization time into a foreground failure.

- [ ] **Step 4: Run the full failure matrix**

For each spec row, restart from a real temporary project and assert the native current reader returns exactly one complete verified current result, `publication_pending`, or one stable integrity/source error. Also run source replacement before and after publication; native cold restart with pending work; exact history during pending; retry/redraw/edit/delete; archive/history/restore; and legacy migration.

Run: `cargo test --locked -p viewer-infrastructure --test continuous_review_recovery --test continuous_review_materialization --test continuous_review_reader --test continuous_review_archive_integrity --test continuous_review_migration`

Expected: PASS.

- [ ] **Step 5: Switch the internal default and remove the old write path**

Set authoring/outbox as the sole path, delete the temporary pipeline enum and synchronous-save response adapter, and keep old v3 reading/bootstrap compatibility. Update the design status to `Implemented and verified`, and explicitly retain the product-stage note that signing, notarization, formal installers, store listing, public release, and sales are out of scope during early development.

- [ ] **Step 6: Run every regression gate**

Run: `cargo fmt --all -- --check`

Run: `cargo clippy --locked --workspace --all-targets -- -D warnings`

Run: `cargo test --locked --workspace`

Run: `pnpm test:review-protocol`

Run: `pnpm test:review-loop`

Run: `pnpm quality`

Expected: every command exits 0. `git status --short` lists only the Task 12 implementation,
verification record, and documentation changes; no generated benchmark corpus, SQLite files, WAL files,
evidence objects, logs, or unrelated user changes are staged.

- [ ] **Step 7: Commit the activation and verification record**

```bash
git add src-tauri crates docs
git commit -m "perf: activate asynchronous review publication"
```

Run: `pnpm verify:clean`

Expected: exit 0 with an empty worktree. Record its success in the task handoff. Do not create an
installer, sign, notarize, publish, or start release work.
