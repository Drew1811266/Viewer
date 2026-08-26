# Viewer Exception-Driven Review Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> Status: Ready for execution

**Goal:** Connect Viewer's phase-1 open review protocol to the current project session and workspace so a user can freeze an explicit image/video set, persist natural-language rework Feedback, safely resume the Draft, and explicitly publish one immutable exception-driven Completed Round.

**Architecture:** Keep `ReviewSessionService` in `viewer-application` as the only workflow authority. It owns one project-scoped state machine, delegates filesystem/index facts to narrow ports, and owns repository-writer lifetime; `viewer-infrastructure` implements safe evidence and repository adapters, while Tauri and React remain typed translation/composition layers. The review UI is a contextual layer over the existing browser/preview workspace and never creates a second media-navigation system.

**Tech Stack:** Rust edition 2024 / Rust 1.85 minimum, Tokio, `async-trait`, BLAKE3, existing Viewer Domain and review protocol, Tauri 2, React 19, TypeScript 6, Vitest, Testing Library, Node.js built-in test runner, pnpm 10.

**Spec:** `docs/superpowers/specs/2026-08-25-viewer-exception-driven-review-loop-design.md`

## Global Constraints

- Implement only product phase 2. Do not add Production Manifest execution, Revision Relation, version comparison, image-region editing, video-point/range editing, CLI, MCP, local API, URL Scheme, Agent plugins, automatic generation, or automatic rework.
- Preserve all current browsing, preview, compare, search, keep/pending/reject, favorite, rename, move, copy, trash, undo, settings, and video behavior. Existing marker state must never participate in review Outcome computation.
- Preserve the dependency direction `viewer-domain <- viewer-application <- viewer-infrastructure/Tauri <- React`. Tauri is the production composition root; React and Tauri must not construct Outcomes or write protocol JSON.
- Enforce one Active Draft per Project and at most one manual Stream (`production: None`). Never select a Draft or manual Stream by newest timestamp when data is ambiguous.
- Treat Feedback text as authoritative natural language. Do not classify, rewrite, summarize, or require structured fields. One explicit submit creates one Feedback that may target one or many fixed members.
- Treat grid visibility and full preview as equally valid review contexts. Do not record viewed count, scroll exposure, dwell time, preview openings, or video playback as completion evidence.
- A Draft contains no `pass`. Only explicit completion computes `revise`, `unreviewable`, and default `pass`, in that order.
- `unreviewable` is automatic and restricted to confirmed stable technical failures. Thumbnail/cover failures, unopened video, `Pending`, `Unknown`, engine initialization, and render-surface failures must not silently become `unreviewable` or `pass`.
- Freeze only JPEG, PNG, `UnsupportedImage`, and Video candidates. With a nonempty selection, resolve only submitted Entity IDs; without a selection, resolve the submitted Folder Projection and its aggregate flag. Never offer “all search results”.
- The frontend submits scope descriptors and Entity IDs only. It must never submit paths, sizes, hashes, media bounds, failures, Outcomes, or protocol records.
- Use clone-validate-save-swap for every Draft mutation. Increment the in-memory revision only after an atomic save succeeds, and reject commands with stale Round ID or revision.
- Stream BLAKE3 with bounded concurrency; check no-follow path ownership and stable identity before and after reading; cancellation or failure before the first Draft save must leave no partial Draft.
- Revalidate identity, path, size, modification time, watcher evidence, media bounds, and stable failure before completion. Replacement, movement, deletion, or digest change blocks completion; phase 2 never rebinds automatically.
- Acquire the writer lease before preparation and recheck Draft/Stream invariants under that lease. Drop it after cancellation/failure before persistence, successful abandon, verified completion, or project teardown.
- Never expose absolute paths, raw I/O/parser text, process IDs, lock contents, or protocol internals to React. Expose stable error codes and safe project-relative paths only.
- Keep all protocol limits from phase 1. Add no npm package and no new Rust crate unless a later reviewer proves an existing workspace dependency cannot satisfy the requirement.
- Evolve v1 compatibly for local source identity and unavailable image bounds. Existing v1 Draft/Completed documents with required image dimensions and without source identity must continue decoding; newly encoded reviewable images keep dimensions, confirmed unreviewable images may omit both dimensions, and the Node reference reader must continue working.
- Current development completion ends at source, tests, and development documentation. Code signing, Apple notarization, formal installers, public release, sale, and sales work are explicitly out of scope.
- Execute in a dedicated worktree created at implementation time. Preserve unrelated user changes. Each task ends with its focused tests passing, an architecture review of the diff, and an independently revertible commit.

---

## File Structure

### Domain and protocol compatibility

- Modify: `crates/viewer-domain/src/review/asset.rs` — add optional local source identity and unavailable image bounds.
- Modify: `crates/viewer-domain/src/review/round.rs` — validate paired image bounds and reject region anchors without bounds.
- Modify: `crates/viewer-infrastructure/src/review/protocol.rs` — additive v1 mapping for `sourceEntityId`.
- Modify: `docs/protocol/viewer-review-draft-v1.schema.json` — optional source identity and paired optional image bounds.
- Modify: `docs/protocol/viewer-review-round-v1.schema.json` — optional source identity and paired optional image bounds.
- Modify: `tests/review_protocol_contract.rs` — old/new document compatibility and round-trip tests.
- Modify: `tests/fixtures/review-protocol/review-draft-v1.valid.json` — canonical manual-source identity example.
- Modify: `tests/fixtures/review-protocol/review-round-v1.valid.json` — canonical manual-source identity example.

### Repository lifecycle

- Modify: `crates/viewer-application/src/review.rs` — repository inspection and provider contracts.
- Modify: `crates/viewer-infrastructure/src/review/repository.rs` — zero/one/many Draft discovery and owned deletion.
- Create: `crates/viewer-infrastructure/src/review/provider.rs` — project-scoped read inspection and writer factory.
- Modify: `crates/viewer-infrastructure/src/review/mod.rs` — export provider.
- Modify: `tests/review_repository.rs` — no-side-effect inspection, Draft ambiguity, deletion, and lease lifetime.

### Asset scope and evidence

- Modify: `crates/viewer-application/src/browse.rs` — add a default indexed-node lookup without widening implementor burden.
- Create: `crates/viewer-application/src/review_assets.rs` — scope, candidate, prepared asset, validation, progress, cancellation, and catalog port types.
- Modify: `crates/viewer-application/src/lib.rs` — export asset-review contracts.
- Create: `crates/viewer-infrastructure/src/review/assets.rs` — index-backed scope resolution, safe file evidence, bounded BLAKE3, and stable failure mapping.
- Create: `crates/viewer-infrastructure/src/review/change_ledger.rs` — project-session member-change evidence.
- Modify: `crates/viewer-infrastructure/src/review/mod.rs` — export asset adapter and ledger.
- Modify: `crates/viewer-infrastructure/src/lib.rs` — export review adapter surface.
- Create: `crates/viewer-infrastructure/tests/review_assets.rs` — real-file scope/evidence/change tests.
- Modify: `src-tauri/src/watcher_runtime.rs` — record reconciled review-member changes in the ledger.

### Application workflow

- Create: `crates/viewer-application/src/review_session.rs` — project-scoped state machine, proposals, snapshots, Feedback commands, completion, recovery, and abandon.
- Modify: `crates/viewer-application/src/lib.rs` — export session API.
- Create: `crates/viewer-application/tests/review_session_start.rs` — inspection, scope confirmation, manual Stream, and resume tests.
- Create: `crates/viewer-application/tests/review_session_mutation.rs` — mutation transaction and revision tests.
- Create: `crates/viewer-application/tests/review_session_completion.rs` — summary, revalidation, publish recovery, and abandon tests.

### Desktop adapter

- Create: `src-tauri/src/dto/review.rs` — bounded review requests/responses and safe projections.
- Modify: `src-tauri/src/dto/mod.rs` — export and serialization-contract tests.
- Create: `src-tauri/src/commands/review.rs` — thin review command adapter.
- Modify: `src-tauri/src/commands/mod.rs` — export review commands.
- Create: `src-tauri/src/state/review.rs` — session service construction, validation, cancellation, and teardown helpers.
- Modify: `src-tauri/src/state/mod.rs` — store one review runtime in `DesktopSession`.
- Modify: `src-tauri/src/state/session.rs` — build review after index construction and shut it down before project resources.
- Modify: `src-tauri/src/error.rs` — stable review error mappings.
- Modify: `src-tauri/src/lib.rs` — register commands and progress event contract.
- Create: `src-tauri/tests/review_commands.rs` — DTO/identity/error/teardown contract tests.

### React bridge and coordination

- Modify: `ui/src/api/types.ts` — review request/snapshot/progress types.
- Modify: `ui/src/api/viewer.ts` — sole Tauri invoke/listen implementation.
- Modify: `ui/src/api/viewer.test.ts` — exact command/event payload contracts.
- Modify: `ui/src/app/workspace/ports.ts` — add narrow `ReviewPort`.
- Modify: `ui/src/app/workspace/contracts.test.ts` — freeze review dependency boundary.
- Create: `ui/src/app/review/reviewModel.ts` — pure selection, target, and display projection helpers.
- Create: `ui/src/app/review/reviewModel.test.ts` — pure model tests.
- Create: `ui/src/app/review/useReviewSessionCoordinator.ts` — review-owned UI state and serialized commands.
- Create: `ui/src/app/review/useReviewSessionCoordinator.test.tsx` — lifecycle, stale save, and editor-preservation tests.

### Contextual review UI and acceptance

- Create: `ui/src/components/review/ReviewWorkspaceLayer.tsx` — small workspace composition boundary.
- Create: `ui/src/components/review/ReviewStartDialog.tsx` — scope preview/confirm/cancel.
- Create: `ui/src/components/review/ReviewContextBar.tsx` — fixed counts and review navigation.
- Create: `ui/src/components/review/ReviewInspector.tsx` — target list, editor, saved Feedback editing/deletion.
- Create: `ui/src/components/review/ReviewCompletionDialog.tsx` — completion summary and explicit confirm.
- Create: `ui/src/components/review/ReviewRecoveryNotice.tsx` — resume, busy, read-only, and recovery states.
- Create: `ui/src/components/review/ReviewConflictList.tsx` — safe relative-path conflicts.
- Create: `ui/src/components/review/reviewComponents.test.tsx` — keyboard, focus, ARIA, and behavior tests.
- Modify: `ui/src/app/workspace/WorkspaceProjectView.tsx` — expose composition slots only; do not move review rules here.
- Modify: `ui/src/App.tsx` — compose the coordinator and review layer.
- Modify: `ui/src/App.test.tsx` — existing-feature regression and project-session lifecycle.
- Create: `ui/src/styles/review.css` — review-specific layout and responsive/accessibility states.
- Modify: `ui/src/styles/app.css` — import review styles from the existing stylesheet entry point.
- Modify: `ui/src/acceptance/acceptanceBridge.ts` — deterministic review bridge defaults.
- Modify: `ui/src/acceptance/acceptanceFixtures.ts` — review snapshot fixtures.
- Create: `ui/src/acceptance/scenes/reviewScenes.tsx` — review visual states.
- Create: `ui/src/acceptance/scenes/reviewScenes.test.tsx` — scene contract tests.
- Modify: `ui/src/acceptance/scenes/index.ts` — register scenes.
- Modify: `ui/src/acceptance/acceptanceStateCatalog.json` — review state inventory.
- Modify: `ui/src/acceptance/acceptanceStateCatalog.test.ts` — inventory coverage.

### End-to-end verification and documentation

- Create: `src-tauri/examples/review_loop_harness.rs` — deterministic production-composition harness for temporary projects.
- Create: `scripts/review-protocol/manual-round-e2e.mjs` — drive a real temporary project through the desktop test seam and invoke the existing reader.
- Create: `scripts/review-protocol/manual-round-e2e.test.mjs` — end-to-end Draft/resume/Completed/head assertions.
- Modify: `package.json` — include the new deterministic review-loop gate.
- Modify: `scripts/repository-policy.test.mjs` — freeze command, event, and dependency boundaries.
- Modify: `docs/PRODUCT_SPEC.md` — mark only implemented phase-2 behavior current and retain release exclusions.
- Modify: `docs/protocol/README.md` — document the optional local identity and manual Stream producer.
- Modify: `docs/README.md` — index the implementation plan/status.

---

### Task 1: Make manual source evidence durable without breaking review protocol v1

**Files:**

- Modify: `crates/viewer-domain/src/review/asset.rs`
- Modify: `crates/viewer-domain/src/review/round.rs`
- Modify: `crates/viewer-infrastructure/src/review/protocol.rs`
- Modify: `docs/protocol/viewer-review-draft-v1.schema.json`
- Modify: `docs/protocol/viewer-review-round-v1.schema.json`
- Modify: `tests/review_protocol_contract.rs`
- Modify: `tests/fixtures/review-protocol/review-draft-v1.valid.json`
- Modify: `tests/fixtures/review-protocol/review-round-v1.valid.json`

**Interfaces:**

- Consumes: the scanner's stable `EntityId`, currently derived from platform file identity, and the existing `AssetVersion` codec.
- Produces this additive field:

```rust
pub struct AssetVersion {
    pub id: AssetVersionId,
    pub source_entity_id: Option<EntityId>,
    pub relative_path: RelativePath,
    pub evidence: AssetEvidence,
    pub media: ReviewMedia,
    pub producer_asset_id: Option<ProductionId>,
    pub parent_asset_version_id: Option<AssetVersionId>,
}

pub enum ReviewMedia {
    Image {
        width: Option<u32>,
        height: Option<u32>,
    },
    Video {
        duration_us: Option<u64>,
        display_width: Option<u32>,
        display_height: Option<u32>,
    },
}
```

- `sourceEntityId` is optional in v1 JSON. Viewer-created manual assets write `Some`; existing documents and future producer assets may decode as `None`.
- The value is local validation evidence, not a global producer ID and not an Agent selector. It never replaces BLAKE3 or relative-path checks.
- Existing image media with positive `width`/`height` decodes to `Some`/`Some`. Newly confirmed unreviewable images may omit both fields and decode to `None`/`None`; one present and one absent, or zero, remains invalid.
- `FeedbackAnchor::ImageRegion` requires real `Some`/`Some` bounds. Phase 2 creates only `FeedbackAnchor::Asset`, so missing bounds never authorize a fabricated region coordinate system.

- [x] **Step 1: Add backward-compatibility tests before changing the model**

Add tests that remove `sourceEntityId` from the current fixture, decode it, and assert `source_entity_id == None`; add a second round trip asserting an exact UUID survives Draft and Completed encoding.

```rust
#[test]
fn review_v1_accepts_old_assets_without_source_entity_id() {
    let mut value: serde_json::Value = serde_json::from_slice(DRAFT_FIXTURE).unwrap();
    value["assets"][0].as_object_mut().unwrap().remove("sourceEntityId");
    let draft = decode_draft(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(draft.assets[0].source_entity_id, None);
}

#[test]
fn review_v1_round_trips_optional_local_source_identity() {
    let entity_id = EntityId::from_u128(0x51);
    let mut draft = valid_draft();
    draft.assets[0].source_entity_id = Some(entity_id);
    let decoded = decode_draft(&encode_draft(&draft).unwrap()).unwrap();
    assert_eq!(decoded.assets[0].source_entity_id, Some(entity_id));
}

#[test]
fn review_v1_represents_confirmed_unreviewable_image_without_fake_dimensions() {
    let mut draft = valid_draft();
    draft.assets[0].media = ReviewMedia::Image { width: None, height: None };
    draft.mark_unreviewable(draft.assets[0].id, ReviewabilityFailure::DecodeFailed).unwrap();
    let decoded = decode_draft(&encode_draft(&draft).unwrap()).unwrap();
    assert_eq!(decoded.assets[0].media, ReviewMedia::Image { width: None, height: None });
}
```

- [x] **Step 2: Run the focused test and observe the expected compile/schema failure**

Run: `cargo test --locked -p viewer-infrastructure --test review_protocol_contract review_v1_`

Expected: FAIL because `AssetVersion`/protocol do not expose `source_entity_id` and image bounds are still mandatory integers.

- [x] **Step 3: Add the optional model/DTO/schema field and update every constructor explicitly**

Use `#[serde(default, skip_serializing_if = "Option::is_none")]` on source identity and each image dimension. Update the schema so image `width` and `height` are optional but mutually dependent. Do not use a blanket `..Default::default()` migration for `AssetVersion`; update every constructor so reviewers can distinguish manual `Some(entity_id)` from legacy/producer `None`, and real image dimensions from confirmed unavailable dimensions.

- [x] **Step 4: Verify protocol compatibility and the reference reader**

Run:

```bash
cargo test --locked -p viewer-infrastructure --test review_protocol_contract
pnpm test:review-protocol
```

Expected: all protocol tests pass; the Node reader still reads the exact Completed head and ignores Drafts.

- [x] **Step 5: Commit**

```bash
git add crates/viewer-domain/src/review/asset.rs crates/viewer-domain/src/review/round.rs crates/viewer-infrastructure/src/review/protocol.rs docs/protocol/viewer-review-draft-v1.schema.json docs/protocol/viewer-review-round-v1.schema.json tests/review_protocol_contract.rs tests/fixtures/review-protocol/review-draft-v1.valid.json tests/fixtures/review-protocol/review-round-v1.valid.json
git commit -m "feat: persist manual review evidence"
```

### Task 2: Add no-side-effect inspection and explicit writer lifecycle

**Files:**

- Modify: `crates/viewer-application/src/review.rs`
- Modify: `crates/viewer-infrastructure/src/review/repository.rs`
- Create: `crates/viewer-infrastructure/src/review/provider.rs`
- Modify: `crates/viewer-infrastructure/src/review/mod.rs`
- Modify: `tests/review_repository.rs`

**Interfaces:**

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct ReviewRepositoryInspection {
    pub catalog: ReviewCatalog,
    pub active_draft: Option<ReviewDraft>,
}

pub trait ReviewRepositoryPort: Send + Sync {
    fn load_catalog(&self) -> Result<ReviewCatalog, ReviewRepositoryError>;
    fn load_active_draft(&self) -> Result<Option<ReviewDraft>, ReviewRepositoryError>;
    fn load_draft(&self, stream_id: ReviewStreamId, round_id: ReviewRoundId)
        -> Result<Option<ReviewDraft>, ReviewRepositoryError>;
    fn save_draft(&self, draft: &ReviewDraft) -> Result<(), ReviewRepositoryError>;
    fn delete_draft(&self, stream_id: ReviewStreamId, round_id: ReviewRoundId)
        -> Result<(), ReviewRepositoryError>;
    fn load_completed(&self, stream_id: ReviewStreamId, round_id: ReviewRoundId)
        -> Result<Option<ReviewSnapshot>, ReviewRepositoryError>;
    fn publish(&self, snapshot: &ReviewSnapshot) -> Result<(), ReviewRepositoryError>;
}

pub trait ReviewRepositoryProviderPort: Send + Sync {
    fn inspect(&self) -> Result<ReviewRepositoryInspection, ReviewRepositoryError>;
    fn open_reader(&self) -> Result<Box<dyn ReviewRepositoryPort>, ReviewRepositoryError>;
    fn open_writer(&self) -> Result<Box<dyn ReviewRepositoryPort>, ReviewRepositoryError>;
}
```

- `inspect()` opens read-only, never creates `.viewer/reviews`, and returns `RecoveryRequired` for more than one owned Draft file.
- `open_reader()` has the same no-side-effect property and lets Application load one exact Completed head after it has resolved the unique manual Stream. It never acquires the writer lease.
- `open_writer()` acquires the existing lease and performs existing orphan recovery. Dropping its returned repository is the only normal lease-release mechanism.
- `delete_draft` verifies exact canonical filename, Project/Stream/Round ownership, regular no-follow file type, and writer access before durable deletion. Missing exact Draft returns `NotFound`; it never deletes Completed data.

- [x] **Step 1: Write real-filesystem failure tests**

Cover: absent review directory leaves it absent; zero Draft; one Draft; two Drafts; malformed filename; symlink; wrong Project/Stream; Busy while writer is alive; lease becomes available after drop; deletion failure leaves Draft readable.

```rust
#[test]
fn inspection_does_not_create_reviews_and_refuses_multiple_drafts() {
    let project = project_fixture();
    let provider = ProjectReviewRepositoryProvider::new(project.root(), project.project_id());
    assert_eq!(provider.inspect().unwrap().active_draft, None);
    assert!(!project.root().join(".viewer/reviews").exists());

    project.write_draft(valid_draft(1));
    project.write_draft(valid_draft(2));
    assert_eq!(provider.inspect(), Err(ReviewRepositoryError::RecoveryRequired));
}
```

- [x] **Step 2: Run the focused test and observe missing API failures**

Run: `cargo test --locked -p viewer-infrastructure --test review_repository`

Expected: FAIL to compile until the provider and repository extensions exist.

- [x] **Step 3: Implement discovery/deletion with existing bounded/no-follow helpers**

Keep Draft scanning inside `repository.rs` so the provider does not duplicate path security. Return the full validated Draft only after its filename Round ID matches its contents. Use directory sync after successful removal.

- [x] **Step 4: Verify repository and phase-1 publication behavior**

Run:

```bash
cargo test --locked -p viewer-infrastructure --test review_repository
cargo test --locked -p viewer-infrastructure review
```

Expected: all new lifecycle tests and all previous atomic publication/recovery tests pass.

- [x] **Step 5: Commit**

```bash
git add crates/viewer-application/src/review.rs crates/viewer-infrastructure/src/review/repository.rs crates/viewer-infrastructure/src/review/provider.rs crates/viewer-infrastructure/src/review/mod.rs tests/review_repository.rs
git commit -m "feat: add review repository lifecycle"
```

### Task 3: Resolve review scopes and build safe content evidence

**Files:**

- Modify: `crates/viewer-application/src/browse.rs`
- Create: `crates/viewer-application/src/review_assets.rs`
- Modify: `crates/viewer-application/src/lib.rs`
- Create: `crates/viewer-infrastructure/src/review/assets.rs`
- Create: `crates/viewer-infrastructure/src/review/change_ledger.rs`
- Modify: `crates/viewer-infrastructure/src/review/mod.rs`
- Modify: `crates/viewer-infrastructure/src/lib.rs`
- Create: `crates/viewer-infrastructure/tests/review_assets.rs`
- Modify: `src-tauri/src/watcher_runtime.rs`

**Interfaces:**

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReviewScope {
    Selection { entity_ids: Vec<EntityId> },
    Folder { folder_id: Option<EntityId>, include_descendants: bool },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewScopeResolution {
    pub candidate_entity_ids: Vec<EntityId>,
    pub image_count: u32,
    pub video_count: u32,
    pub excluded_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedReviewAsset {
    pub entity_id: EntityId,
    pub asset: AssetVersion,
    pub failure: Option<ReviewabilityFailure>,
    pub change_revision: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewAssetConflictKind {
    Missing,
    Moved,
    Replaced,
    SizeChanged,
    ContentChanged,
    MediaChanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReviewAssetValidation {
    Current(PreparedReviewAsset),
    Conflict {
        asset_version_id: AssetVersionId,
        relative_path: RelativePath,
        kind: ReviewAssetConflictKind,
    },
    Pending { asset_version_id: AssetVersionId, relative_path: RelativePath },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewTaskProgress { pub completed: u32, pub total: u32 }

#[derive(Clone, Default)]
pub struct ReviewTaskCancellation(Arc<AtomicBool>);

impl ReviewTaskCancellation {
    pub fn cancel(&self) { self.0.store(true, Ordering::Release); }
    pub fn is_cancelled(&self) -> bool { self.0.load(Ordering::Acquire) }
}

pub trait ReviewProgressPort: Send + Sync {
    fn report(&self, progress: ReviewTaskProgress);
}

#[async_trait]
pub trait ReviewAssetCatalogPort: Send + Sync {
    async fn resolve_scope(&self, scope: &ReviewScope)
        -> Result<ReviewScopeResolution, ReviewAssetError>;
    async fn prepare_assets(
        &self,
        entity_ids: &[EntityId],
        cancellation: ReviewTaskCancellation,
        progress: Arc<dyn ReviewProgressPort>,
    ) -> Result<Vec<PreparedReviewAsset>, ReviewAssetError>;
    async fn revalidate_assets(
        &self,
        assets: &[PreparedReviewAsset],
        cancellation: ReviewTaskCancellation,
        progress: Arc<dyn ReviewProgressPort>,
    ) -> Result<Vec<ReviewAssetValidation>, ReviewAssetError>;
    fn release_tracking(&self);
}
```

- Add `BrowseIndexPort::indexed_node` as a default implementation over `all_indexed_nodes`, then override it in `SessionIndex` for an indexed lookup. This avoids forcing every existing fake to change while keeping production lookup efficient.
- Candidate kinds: Jpeg/Png/UnsupportedImage -> Image; Video -> Video. Directories, Markdown, Text, and Other increment `excluded_count`.
- Sort and deduplicate candidates deterministically by relative path; duplicate input Entity IDs do not create duplicate members.
- `ReviewChangeLedger` stores only the currently tracked fixed-member Entity IDs. It supports `replace_members`, `record`, `revision`, and `clear`; each member has a monotonic change revision. `prepare_assets` starts tracking before evidence reads and captures the pre-read revision in `PreparedReviewAsset`; watcher reconciliation increments revisions after resolving Entity IDs through the index. `revalidate_assets` compares current revision with the captured baseline, so a same-size/same-mtime watcher-observed change still forces rehash. Cancellation/failure before Active, verified completion, abandon, and shutdown call `release_tracking`.
- `IndexedReviewAssetCatalog::new` also receives the existing `Arc<dyn ImagePort>` and `Arc<dyn VideoMetadataProbe>`. It uses their probe operations for bounded confirmation when indexed media state is Pending/Failed/transient; it does not render thumbnails, open the playback surface, or create a second media engine.
- `prepare_assets` opens `project_root.join(relative_path)` with no-follow, verifies file identity/size/mtime before and after streaming, uses a fixed semaphore (start with `min(4, available_parallelism)`), and never loads a whole video into memory.
- A source whose no-follow metadata establishes exact identity but whose content open is stably permission-denied/unreadable may produce `blake3: None` plus a stable `unreviewable` failure. If exact identity or owned relative-path authorization cannot be established, preparation fails the whole operation and saves no Draft.
- Stable failure mapping is exact: UnsupportedImage -> `Unsupported`; image probe Unsupported -> `Unsupported`, Corrupt -> `Damaged`, and a proven permission/open error -> `PermissionDenied`/`Unreadable`; successful image probe is reviewable even if an earlier thumbnail/index operation failed. Video Unsupported/Damaged/Unreadable/Missing/DecodeFallbackFailed map to protocol equivalents. Image BudgetExceeded/generic transient I/O and Video EngineInitialization/RenderSurface/ThumbnailUnavailable/Pending remain `Pending`, not `unreviewable`.
- Revalidation is deterministic: missing/path/source-identity/size mismatch -> conflict; same identity/path/size with changed mtime or `change_revision` -> stream BLAKE3 and continue only on exact digest equality; unchanged identity/path/size/mtime/revision -> reuse saved digest; `blake3: None` assets require exact identity/path/size/mtime and unchanged media failure. Media bounds or stable-failure changes are conflicts/pending facts, never silent pass.

- [x] **Step 1: Write scope, hashing, cancellation, and failure-mapping tests**

Include selected mixed kinds, direct folder, aggregate descendants, empty candidate set, duplicate Entity IDs, symlink, replacement during hash, cancellation, grid-renderable image, unopened video, transient video failure, stable video failure, and changed-ledger rehash.

- [x] **Step 2: Run focused tests and confirm the port/adapter is absent**

Run: `cargo test --locked -p viewer-infrastructure --test review_assets`

Expected: FAIL because the review asset adapter and contract do not exist.

- [x] **Step 3: Implement scope resolution first, then evidence preparation**

Use small private functions with independently tested semantics:

```rust
fn candidate_kind(kind: FileKind) -> Option<ReviewAssetKind>;
fn stable_review_failure(node: &IndexedNode) -> FailureAssessment;
fn open_owned_source(root: &Path, relative: &RelativePath) -> Result<File, ReviewAssetError>;
fn hash_blake3_streaming(file: &mut File, cancel: &ReviewTaskCancellation)
    -> Result<[u8; 32], ReviewAssetError>;
```

Do not place Tokio, BLAKE3, or filesystem code in `viewer-application`.

- [x] **Step 4: Verify focused application/infrastructure tests and watcher regression**

Run:

```bash
cargo test --locked -p viewer-application review_assets
cargo test --locked -p viewer-infrastructure --test review_assets
cargo test --locked -p viewer-desktop watcher
```

Expected: all pass; watcher behavior remains unchanged except for recording bounded Entity IDs.

- [x] **Step 5: Commit**

```bash
git add crates/viewer-application/src/browse.rs crates/viewer-application/src/review_assets.rs crates/viewer-application/src/lib.rs crates/viewer-infrastructure/src/review/assets.rs crates/viewer-infrastructure/src/review/change_ledger.rs crates/viewer-infrastructure/src/review/mod.rs crates/viewer-infrastructure/src/lib.rs crates/viewer-infrastructure/tests/review_assets.rs src-tauri/src/watcher_runtime.rs
git commit -m "feat: prepare indexed review assets"
```

### Task 4: Build Draft discovery, scope confirmation, manual Stream, and resume state

**Files:**

- Create: `crates/viewer-application/src/review_session.rs`
- Modify: `crates/viewer-application/src/lib.rs`
- Create: `crates/viewer-application/tests/review_session_start.rs`

**Interfaces:**

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewSessionPhase {
    Idle,
    Preparing,
    Active,
    Completing,
    CompletedReadOnly,
    WriteUnavailable,
    RecoveryRequired,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ReviewProposalId(u64);

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewScopeProposal {
    pub id: ReviewProposalId,
    pub scope: ReviewScope,
    pub resolution: ReviewScopeResolution,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewSessionSnapshot {
    pub phase: ReviewSessionPhase,
    pub resume: Option<ReviewResumeSnapshot>,
    pub review_stream_id: Option<ReviewStreamId>,
    pub review_round_id: Option<ReviewRoundId>,
    pub revision: u64,
    pub members: Vec<ReviewMemberSnapshot>,
    pub feedback: Vec<ReviewFeedbackSnapshot>,
    pub unreviewable: Vec<ReviewUnreviewableSnapshot>,
    pub conflicts: Vec<ReviewConflictSnapshot>,
    pub counts: ReviewSessionCounts,
    pub error: Option<ReviewUserError>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewResumeSnapshot {
    pub review_stream_id: ReviewStreamId,
    pub review_round_id: ReviewRoundId,
    pub created_at_ms: i64,
    pub total: u32,
    pub feedback_items: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewMemberSnapshot {
    pub asset_version_id: AssetVersionId,
    pub entity_id: Option<EntityId>,
    pub relative_path: RelativePath,
    pub display_name: String,
    pub kind: ReviewAssetKind,
    pub feedback_items: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReviewFeedbackSnapshot {
    pub feedback_id: FeedbackId,
    pub text: String,
    pub created_at_ms: i64,
    pub target_entity_ids: Vec<EntityId>,
    pub target_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewUnreviewableSnapshot {
    pub asset_version_id: AssetVersionId,
    pub relative_path: RelativePath,
    pub failure: ReviewabilityFailure,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewConflictSnapshot {
    pub asset_version_id: AssetVersionId,
    pub relative_path: RelativePath,
    pub kind: ReviewConflictKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewConflictKind {
    Missing,
    Moved,
    Replaced,
    SizeChanged,
    ContentChanged,
    MediaChanged,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ReviewSessionCounts {
    pub total: u32,
    pub feedback_items: u32,
    pub revise: u32,
    pub unreviewable: u32,
    pub pass: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewUserError {
    pub code: &'static str,
    pub retryable: bool,
    pub affected_paths: Vec<RelativePath>,
}

pub struct ReviewSessionService {
    project_id: ProjectId,
    catalog: Arc<dyn ReviewAssetCatalogPort>,
    repositories: Arc<dyn ReviewRepositoryProviderPort>,
    clock: Arc<dyn ClockPort>,
    state: tokio::sync::Mutex<ReviewSessionState>,
}
```

Public methods for this task:

```rust
pub async fn inspect(&self) -> ReviewSessionSnapshot;
pub async fn preview_start(&self, scope: ReviewScope)
    -> Result<ReviewScopeProposal, ReviewSessionError>;
pub async fn start(
    &self,
    proposal_id: ReviewProposalId,
    progress: Arc<dyn ReviewProgressPort>,
) -> Result<ReviewSessionSnapshot, ReviewSessionError>;
pub async fn resume(
    &self,
    progress: Arc<dyn ReviewProgressPort>,
) -> Result<ReviewSessionSnapshot, ReviewSessionError>;
pub async fn cancel_task(&self) -> bool;
pub async fn shutdown(&self);
```

- `inspect` uses provider inspection plus an exact read-only load when a manual Stream has a Completed head. One Draft -> `Idle` with `resume: Some(...)`; no Draft plus one manual head -> `CompletedReadOnly`; neither -> `Idle`; Busy/read-only does not damage ordinary project state; multiple Draft/manual Streams -> `RecoveryRequired`. This keeps the accepted phase enum exact while representing the explicit resume prompt as structured projection data rather than a scattered boolean.
- A discovered Draft with `production: Some(...)` belongs to a future production workflow. Phase 2 exposes it as review-only unsupported/recovery state and never edits, abandons, publishes, or reclassifies it as the manual Stream. Completed production Streams remain independently readable and are ignored by manual-Stream selection.
- `CompletedReadOnly` entered during inspection owns no writer and still permits `preview_start` for a later manual Round; the new Draft uses that displayed head as `previous_completed_round_id`.
- `preview_start` resolves and caches an immutable proposal with a service-local monotonic ID. It performs no write and reports image/video/excluded counts.
- `start` opens the writer, rechecks active Draft and manual Stream under the lease, re-resolves the exact proposal Entity IDs, prepares all evidence, creates one Draft, saves once, then becomes `Active` revision 1.
- The first manual Draft creates a new Stream ID but does not index it. Later Drafts reuse the one `production: None` Stream and its exact head. More than one manual Stream is `RecoveryRequired`.
- `resume` opens writer, reloads the exact Draft under lease, revalidates every saved member using source identity/path/hash, reconstructs the AssetVersion-to-Entity binding, and enters `Active` only after validation. Conflicts remain visible and block completion but do not erase Feedback.
- Cancellation before first save returns to `Idle` with the prior `resume` projection (if one existed) and drops writer. `shutdown` cancels preparation, waits for the task boundary, releases asset tracking, and drops writer deterministically.

- [x] **Step 1: Create fakes and write state-transition tests**

Test zero/one/many Drafts, future production Draft refusal, Busy, read-only Draft projection, empty candidates, stale proposal, scope re-resolution changes, first/reused/ambiguous manual Stream, production-Stream isolation, exact latest Completed read-only projection, later Round previous head, save failure, cancellation, resume hash mismatch, and shutdown writer release.

```rust
#[tokio::test]
async fn start_rechecks_unique_draft_after_acquiring_writer() {
    let fixture = SessionFixture::idle();
    let proposal = fixture.service.preview_start(selection(&[1, 2])).await.unwrap();
    fixture.repositories.insert_draft_after_inspection(valid_draft());
    let error = fixture.service.start(proposal.id, fixture.progress()).await.unwrap_err();
    assert_eq!(error.code(), "review_draft_already_active");
    assert!(!fixture.repositories.writer_is_held());
}
```

- [x] **Step 2: Run the application integration test and observe the missing service**

Run: `cargo test --locked -p viewer-application --test review_session_start`

Expected: FAIL to compile before `review_session.rs` exists.

- [x] **Step 3: Implement the state machine with one mutex and explicit transition helpers**

Do not hold the mutex across arbitrary adapter work. Store a task token/state transition under the mutex, perform async work with immutable inputs, then reacquire and commit only if the same task token is current.

```rust
fn manual_stream(catalog: &ReviewCatalog)
    -> Result<Option<&ReviewStreamHead>, ReviewSessionError>;
fn transition_to_preparing(state: &mut ReviewSessionState, task: ReviewTask)
    -> Result<(), ReviewSessionError>;
fn install_active(state: &mut ReviewSessionState, active: ActiveReviewSession);
```

- [x] **Step 4: Verify the entire application crate**

Run: `cargo test --locked -p viewer-application`

Expected: all existing and new application tests pass.

- [x] **Step 5: Commit**

```bash
git add crates/viewer-application/src/review_session.rs crates/viewer-application/src/lib.rs crates/viewer-application/tests/review_session_start.rs
git commit -m "feat: start and resume review sessions"
```

### Task 5: Make Feedback mutations transactional and revision-safe

**Files:**

- Modify: `crates/viewer-application/src/review_session.rs`
- Create: `crates/viewer-application/tests/review_session_mutation.rs`

**Interfaces:**

```rust
pub struct ReviewMutationGuard {
    pub review_round_id: ReviewRoundId,
    pub expected_revision: u64,
}

pub struct AddReviewFeedback {
    pub guard: ReviewMutationGuard,
    pub text: String,
    pub target_entity_ids: Vec<EntityId>,
}

pub struct UpdateReviewFeedback {
    pub guard: ReviewMutationGuard,
    pub feedback_id: FeedbackId,
    pub text: String,
    pub target_entity_ids: Vec<EntityId>,
}

pub struct DeleteReviewFeedback {
    pub guard: ReviewMutationGuard,
    pub feedback_id: FeedbackId,
}

pub async fn add_feedback(&self, command: AddReviewFeedback)
    -> Result<ReviewSessionSnapshot, ReviewSessionError>;
pub async fn update_feedback(&self, command: UpdateReviewFeedback)
    -> Result<ReviewSessionSnapshot, ReviewSessionError>;
pub async fn delete_feedback(&self, command: DeleteReviewFeedback)
    -> Result<ReviewSessionSnapshot, ReviewSessionError>;
```

- Resolve target Entity IDs only through the Active session's fixed binding. Nonmembers, duplicates, empty targets, empty text, and over-limit text fail before repository I/O.
- Freeze targets at submit time and construct only `FeedbackAnchor::Asset`.
- Add creates a new typed Feedback UUID and timestamp. Update keeps the original Feedback ID and original `created_at_ms` unless the Domain later gains a distinct edited timestamp. Delete requires an existing Feedback ID.
- For every command: validate guard -> clone Draft -> apply Domain behavior -> atomic `save_draft` -> swap Draft -> increment revision -> emit snapshot. On save failure, both Draft and revision remain unchanged.
- Serialize mutations through the service mutex. A second command using the first command's old revision returns `StaleRevision` without attempting a save.
- Every successful Draft mutation invalidates any cached completion proposal; a proposal can authorize only the exact saved revision summarized for the user.

- [x] **Step 1: Write mutation transaction tests**

Cover single/multiple targets, frozen targets, multiple Feedback on one asset, edit, delete, nonmember, whitespace, limits, save failure, concurrent same-revision commands, stale Round ID, stale revision, and revision monotonicity.

- [x] **Step 2: Run tests and verify red state**

Run: `cargo test --locked -p viewer-application --test review_session_mutation`

Expected: FAIL because mutation APIs are not implemented.

- [x] **Step 3: Implement one shared clone-save-swap helper**

```rust
fn mutate_active<F>(
    state: &mut ReviewSessionState,
    guard: ReviewMutationGuard,
    repositories: &dyn ReviewRepositoryPort,
    change: F,
) -> Result<ReviewSessionSnapshot, ReviewSessionError>
where
    F: FnOnce(&mut ReviewDraft, &ActiveBindings) -> Result<(), ReviewSessionError>;
```

Do not duplicate save/revision logic among add, update, and delete.

- [x] **Step 4: Verify mutation and start suites together**

Run: `cargo test --locked -p viewer-application --test review_session_start --test review_session_mutation`

Expected: all pass, including save-failure snapshot invariants.

- [x] **Step 5: Commit**

```bash
git add crates/viewer-application/src/review_session.rs crates/viewer-application/tests/review_session_mutation.rs
git commit -m "feat: persist review feedback mutations"
```

### Task 6: Complete, recover, and abandon review rounds safely

**Files:**

- Modify: `crates/viewer-application/src/review_session.rs`
- Create: `crates/viewer-application/tests/review_session_completion.rs`

**Interfaces:**

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewCompletionSummary {
    pub review_round_id: ReviewRoundId,
    pub revision: u64,
    pub total: u32,
    pub revise: u32,
    pub unreviewable: u32,
    pub default_pass: u32,
    pub feedback: Vec<ReviewFeedbackSummary>,
    pub conflicts: Vec<ReviewConflictSnapshot>,
    pub pending: Vec<RelativePath>,
    pub can_complete: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ReviewCompletionProposalId(u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewCompletionProposal {
    pub id: ReviewCompletionProposalId,
    pub summary: ReviewCompletionSummary,
}

pub async fn completion_summary(&self, guard: ReviewMutationGuard)
    -> Result<ReviewCompletionProposal, ReviewSessionError>;
pub async fn complete(
    &self,
    proposal_id: ReviewCompletionProposalId,
    guard: ReviewMutationGuard,
    progress: Arc<dyn ReviewProgressPort>,
) -> Result<ReviewSessionSnapshot, ReviewSessionError>;
pub async fn abandon(&self, guard: ReviewMutationGuard)
    -> Result<ReviewSessionSnapshot, ReviewSessionError>;
```

- Summary runs the same revalidation pipeline as completion and never reports viewed count. If it discovers a changed stable `unreviewable` set, it persists that set with clone-save-swap and returns a service-local proposal containing the post-save revision. `can_complete` is false for conflict or unresolved Pending/Unknown state.
- `complete` requires that exact proposal ID plus the Round/revision returned by the summary; no adapter can bypass the summary use case. It revalidates after user confirmation. If external changes alter conflicts, Pending state, or the stable failure/count projection, it invalidates the proposal, saves any stable fact, returns `review_completion_changed`, and requires a fresh summary/confirmation; it never publishes counts the user did not confirm.
- With an unchanged confirmed projection, `complete` calls `ReviewDraft::complete`, publishes, reloads catalog, and checks exact `(stream_id, round_id)` head before entering `CompletedReadOnly`. Feedback always wins over technical failure when Domain computes Outcome.
- If `publish` returns after a possibly durable Round, drop the old writer, open a new writer so existing orphan recovery runs, then check the exact head. Exact success is success; ambiguity becomes `RecoveryRequired` and never displays a false success.
- Verified completion drops the writer. The immutable snapshot may stay in memory for read-only display.
- `abandon` calls exact `delete_draft`; only success clears Active state and drops writer. It never publishes, indexes, or deletes Completed records.
- Cancellation is accepted during hash/revalidation before `ReviewDraft::complete`; once create-once publication begins it cannot be presented as cancelled.

- [x] **Step 1: Write completion and failure-recovery tests**

Cover outcome precedence, all-default-pass, stable failure, pending failure, each conflict type, changed-but-equal digest, changed digest, summary/completion race, publish success, both phase-1 fault points, exact-head mismatch, abandon success/failure, and writer release.

```rust
#[tokio::test]
async fn feedback_wins_and_remaining_assets_pass_only_after_completion() {
    let fixture = SessionFixture::active_with_feedback_and_failure();
    let before = fixture.service.snapshot().await;
    assert_eq!(before.counts.pass, 0);

    let proposal = fixture.service.completion_summary(before.guard()).await.unwrap();
    let completed = fixture.service
        .complete(proposal.id, proposal.summary.guard(), fixture.progress())
        .await
        .unwrap();
    assert_eq!(completed.counts.revise, 1);
    assert_eq!(completed.counts.unreviewable, 1);
    assert_eq!(completed.counts.pass, completed.counts.total - 2);
}
```

- [x] **Step 2: Run tests and confirm missing completion API**

Run: `cargo test --locked -p viewer-application --test review_session_completion`

Expected: FAIL before completion/recovery/abandon methods exist.

- [x] **Step 3: Implement one validation result reducer and exact-head verifier**

```rust
fn apply_validations(
    draft: &mut ReviewDraft,
    validations: Vec<ReviewAssetValidation>,
) -> Result<ValidationProjection, ReviewSessionError>;

fn exact_head_is_published(
    catalog: &ReviewCatalog,
    stream_id: ReviewStreamId,
    round_id: ReviewRoundId,
) -> bool;
```

Reuse these functions for summary, completion, resume recovery, and publish recovery. Do not let Tauri recompute counts.

- [x] **Step 4: Verify all review application and repository tests**

Run:

```bash
cargo test --locked -p viewer-application review_session
cargo test --locked -p viewer-infrastructure --test review_repository
```

Expected: all pass, including stage-1 fault injection.

- [x] **Step 5: Commit**

```bash
git add crates/viewer-application/src/review_session.rs crates/viewer-application/tests/review_session_completion.rs
git commit -m "feat: complete and abandon review rounds"
```

### Task 7: Compose the service into one Desktop project session

**Files:**

- Create: `src-tauri/src/dto/review.rs`
- Modify: `src-tauri/src/dto/mod.rs`
- Create: `src-tauri/src/commands/review.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Create: `src-tauri/src/state/review.rs`
- Modify: `src-tauri/src/state/mod.rs`
- Modify: `src-tauri/src/state/session.rs`
- Modify: `src-tauri/src/error.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `src-tauri/tests/review_commands.rs`

**Command contract:**

```text
review_status(SessionGenerationRequest) -> ReviewSessionSnapshotDto
review_preview_start(ReviewPreviewStartRequest) -> ReviewScopeProposalDto
review_start(ReviewStartRequest) -> ReviewSessionSnapshotDto
review_resume(SessionGenerationRequest) -> ReviewSessionSnapshotDto
review_add_feedback(ReviewAddFeedbackRequest) -> ReviewSessionSnapshotDto
review_update_feedback(ReviewUpdateFeedbackRequest) -> ReviewSessionSnapshotDto
review_delete_feedback(ReviewDeleteFeedbackRequest) -> ReviewSessionSnapshotDto
review_completion_summary(ReviewGuardRequest) -> ReviewCompletionProposalDto
review_complete(ReviewCompleteRequest) -> ReviewSessionSnapshotDto
review_abandon(ReviewGuardRequest) -> ReviewSessionSnapshotDto
review_cancel_task(SessionGenerationRequest) -> bool
event: viewer://review-progress -> ReviewProgressDto
```

- Every request carries `sessionId` and `generation`; mutation/complete/abandon additionally carry `reviewRoundId` and `expectedRevision`; `ReviewCompleteRequest` also carries the completion proposal ID returned by `review_completion_summary`.
- Request DTOs parse UUID strings exactly, cap vectors before allocation/use, cap Feedback UTF-8 bytes at Domain limits, reject duplicate Entity IDs, and express scope as a tagged union. They never accept a path.
- `DesktopSession` owns `Arc<ReviewSessionService>` and one `Arc<ReviewChangeLedger>`. Construct them after `SessionIndex` exists and before watcher start; pass the ledger to watcher.
- Project open calls `inspect()` but does not fail the project when review is Busy, unsupported, invalid, or needs recovery. Those conditions appear only in the review snapshot.
- Project close/switch/window teardown calls `shutdown().await` before dropping watcher/index/cache so no hashing task reads torn-down session state.
- Progress DTO includes session/generation, task kind, completed, total, and cancellable. It carries no file path.
- Map errors to stable codes such as `review_read_only`, `review_busy`, `review_stale_session`, `review_stale_revision`, `review_completion_changed`, `review_version_conflict`, `review_pending_validation`, `review_recovery_required`, `review_unsupported_version`, `review_invalid_data`, and `review_unavailable`.

- [x] **Step 1: Write DTO and runtime contract tests**

Test exact serde shapes, invalid IDs, over-limit text/targets, session/generation mismatch, stale Round/revision, changed completion summary, safe relative conflict paths, no absolute error leakage, Busy does not block browse, and teardown releases writer/cancels progress.

- [x] **Step 2: Run desktop review test and observe missing modules**

Run: `cargo test --locked -p viewer-desktop --test review_commands`

Expected: FAIL because DTOs, commands, and runtime composition are absent.

- [x] **Step 3: Implement translation-only commands and session composition**

Each command should follow this shape:

```rust
#[tauri::command]
pub async fn review_add_feedback(
    state: tauri::State<'_, DesktopRuntime>,
    request: ReviewAddFeedbackRequest,
) -> Result<ReviewSessionSnapshotDto, CommandError> {
    let (service, command) = state.review_mutation(request.try_into()?).await?;
    service.add_feedback(command).await.map(Into::into).map_err(Into::into)
}
```

No command reads the filesystem or Domain collections directly.

- [x] **Step 4: Run desktop, security-boundary, and architecture-contract tests**

Run:

```bash
cargo test --locked -p viewer-desktop --test review_commands --test security_boundaries --test video_security_boundaries
pnpm architecture:contracts
```

Expected: all pass; project browse remains available for every review-only failure.

- [x] **Step 5: Commit**

```bash
git add src-tauri/src/dto/review.rs src-tauri/src/dto/mod.rs src-tauri/src/commands/review.rs src-tauri/src/commands/mod.rs src-tauri/src/state/review.rs src-tauri/src/state/mod.rs src-tauri/src/state/session.rs src-tauri/src/error.rs src-tauri/src/lib.rs src-tauri/tests/review_commands.rs
git commit -m "feat: expose desktop review sessions"
```

### Task 8: Add the typed React bridge and isolated review coordinator

**Files:**

- Modify: `ui/src/api/types.ts`
- Modify: `ui/src/api/viewer.ts`
- Modify: `ui/src/api/viewer.test.ts`
- Modify: `ui/src/app/workspace/ports.ts`
- Modify: `ui/src/app/workspace/contracts.test.ts`
- Create: `ui/src/app/review/reviewModel.ts`
- Create: `ui/src/app/review/reviewModel.test.ts`
- Create: `ui/src/app/review/useReviewSessionCoordinator.ts`
- Create: `ui/src/app/review/useReviewSessionCoordinator.test.tsx`

**Interfaces:**

```ts
export type ReviewPort = Pick<
  ViewerBridge,
  | 'reviewStatus'
  | 'reviewPreviewStart'
  | 'reviewStart'
  | 'reviewResume'
  | 'reviewAddFeedback'
  | 'reviewUpdateFeedback'
  | 'reviewDeleteFeedback'
  | 'reviewCompletionSummary'
  | 'reviewComplete'
  | 'reviewAbandon'
  | 'reviewCancelTask'
  | 'listenReviewProgress'
>

export interface ReviewSessionCoordinator {
  snapshot: ReviewSessionSnapshot
  proposal: ReviewScopeProposal | null
  completion: ReviewCompletionProposal | null
  editor: ReviewEditorState
  progress: ReviewProgress | null
  previewStart(scope: ReviewScopeRequest): Promise<void>
  confirmStart(): Promise<void>
  resume(): Promise<void>
  submitFeedback(): Promise<void>
  beginEdit(feedbackId: string): void
  saveEdit(): Promise<void>
  deleteFeedback(feedbackId: string): Promise<void>
  prepareCompletion(): Promise<void>
  confirmCompletion(): Promise<void>
  abandon(): Promise<void>
  cancelTask(): Promise<void>
}
```

- Only `ui/src/api/viewer.ts` imports Tauri APIs. Add exact invoke names from Task 7 and listen only to `viewer://review-progress`.
- The coordinator owns review snapshots, dialogs, editor text, save state, and progress. It resets on `sessionId/generation` change and calls `reviewStatus` for the new project.
- It gets current selection/folder/aggregate/search context as inputs but does not modify central `ViewerState`. Selected search results may form `Selection`; no-selection search mode disables implicit scope start.
- Serialize mutations in one promise queue or busy gate. Always use the latest saved snapshot guard. On stale revision, reload status but preserve editor text and selected targets.
- Nonempty unsaved text requires confirmation before inspector/dialog close, project close, or review context replacement.
- Rename the existing workspace `FeedbackCoordinator` only if necessary for clarity; prefer referring to the new feature as `ReviewSessionCoordinator` and the old one as global notice coordination to avoid a broad rename.

- [x] **Step 1: Write bridge and coordinator behavior tests**

Cover exact command names/payloads, progress unsubscribe, session reset, selection vs folder scope, search no-selection exclusion, same-revision serialization, save failure, stale reload, editor preservation, frozen targets, cancel, and unsaved-text guard.

- [x] **Step 2: Run focused UI tests and observe missing review surface**

Run:

```bash
pnpm --dir ui exec vitest run src/api/viewer.test.ts src/app/review/reviewModel.test.ts src/app/review/useReviewSessionCoordinator.test.tsx
```

Expected: FAIL before review types, bridge methods, and coordinator exist.

- [x] **Step 3: Implement the bridge first, then pure model, then hook**

Keep all scope derivation in pure functions with exhaustive tagged-union switches. Never infer `pass` or `unreviewable` in TypeScript; render backend counts only.

- [x] **Step 4: Verify boundary and TypeScript checks**

Run:

```bash
pnpm --dir ui exec vitest run src/api/viewer.test.ts src/app/workspace/contracts.test.ts src/app/review
pnpm --dir ui check
```

Expected: all pass; contract test proves review components/coordinator do not import Tauri or `useViewerController`.

- [x] **Step 5: Commit**

```bash
git add ui/src/api/types.ts ui/src/api/viewer.ts ui/src/api/viewer.test.ts ui/src/app/workspace/ports.ts ui/src/app/workspace/contracts.test.ts ui/src/app/review/reviewModel.ts ui/src/app/review/reviewModel.test.ts ui/src/app/review/useReviewSessionCoordinator.ts ui/src/app/review/useReviewSessionCoordinator.test.tsx
git commit -m "feat: coordinate review sessions in ui"
```

### Task 9: Layer the review workflow over the existing workspace

**Files:**

- Create: `ui/src/components/review/ReviewWorkspaceLayer.tsx`
- Create: `ui/src/components/review/ReviewStartDialog.tsx`
- Create: `ui/src/components/review/ReviewContextBar.tsx`
- Create: `ui/src/components/review/ReviewInspector.tsx`
- Create: `ui/src/components/review/ReviewCompletionDialog.tsx`
- Create: `ui/src/components/review/ReviewRecoveryNotice.tsx`
- Create: `ui/src/components/review/ReviewConflictList.tsx`
- Create: `ui/src/components/review/reviewComponents.test.tsx`
- Modify: `ui/src/app/workspace/WorkspaceProjectView.tsx`
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/App.test.tsx`
- Create: `ui/src/styles/review.css`
- Modify: `ui/src/styles/app.css`

**Composition contract:**

```tsx
<WorkspaceProjectView
  {...existingWorkspaceProps}
  reviewLayer={
    <ReviewWorkspaceLayer
      review={review}
      selectedEntityIds={organization.selection}
      onReturnToMembers={viewing.showEntities}
    />
  }
/>
```

- Add a visible toolbar “开始评审” entry. Selection present -> selected images/videos only. Selection absent -> current Folder Projection and aggregate flag. Search with no selection -> disabled with an explanation; selected search results remain valid.
- The start dialog displays backend image/video/excluded counts and confirms the proposal ID. It does not enumerate or reconstruct evidence.
- Active review shows a top context bar with fixed total, Feedback count, unreviewable count, “返回本轮素材”, Inspector toggle, completion, and abandon. It never displays viewed/progress counts.
- Inspector targets only selected fixed members, shows nonmember selection as ineligible, supports one/many targets, `⌘ Enter`, edit, delete, saved/saving/error text, and Feedback counts with text/icon/accessible name.
- Completion dialog renders total/revise/unreviewable/default-pass, every natural-language Feedback with target count, conflicts, and pending items. Confirm is disabled unless backend `canComplete` is true.
- Resume is always explicit after open. Busy/read-only/unsupported/invalid/recovery states are review-local notices and leave browser/preview controls usable.
- Completed state is read-only. Starting a later round requires no Active Draft and uses the same manual Stream through the backend.
- Reuse `ModalSheet` focus trapping and Escape semantics. Add an explicit discard confirmation for nonempty unsaved editor text; do not silently close it.
- Keep `WorkspaceProjectView.tsx` changes to props/slots and shallow placement. All conditional review rendering belongs in focused components.

- [x] **Step 1: Write component and App regression tests**

Cover toolbar visibility/disabled states, counts, no viewed text, return-to-members, member target rules, `metaKey + Enter`, edit/delete, discard guard, save error, resume prompt, Busy/read-only/recovery, conflict-disabled completion, Completed read-only, focus return, ARIA live state, Escape, and unchanged existing browsing/preview/marker actions.

- [x] **Step 2: Run focused tests and capture failing component imports**

Run:

```bash
pnpm --dir ui exec vitest run src/components/review/reviewComponents.test.tsx src/App.test.tsx
```

Expected: FAIL before review components and App composition are added.

- [x] **Step 3: Implement semantic structure before styling**

Use buttons, labels, lists, and `aria-live`/`aria-describedby` relationships first. Add CSS only after behavior tests pass. At 200% zoom, Inspector must remain usable without covering completion controls or forcing horizontal page scrolling.

- [x] **Step 4: Verify review UI and all current UI behavior**

Run:

```bash
pnpm --dir ui exec vitest run src/components/review src/app/review src/App.test.tsx
pnpm --dir ui test
pnpm --dir ui check
pnpm --dir ui build
```

Expected: all tests/check/build pass; no existing App/component test is weakened or deleted.

- [x] **Step 5: Commit**

```bash
git add ui/src/components/review ui/src/app/workspace/WorkspaceProjectView.tsx ui/src/App.tsx ui/src/App.test.tsx ui/src/styles/review.css ui/src/styles/app.css
git commit -m "feat: add exception-driven review workspace"
```

### Task 10: Add deterministic acceptance scenes and visual/accessibility checks

**Files:**

- Modify: `ui/src/acceptance/acceptanceBridge.ts`
- Modify: `ui/src/acceptance/acceptanceFixtures.ts`
- Create: `ui/src/acceptance/scenes/reviewScenes.tsx`
- Create: `ui/src/acceptance/scenes/reviewScenes.test.tsx`
- Modify: `ui/src/acceptance/scenes/index.ts`
- Modify: `ui/src/acceptance/acceptanceStateCatalog.json`
- Modify: `ui/src/acceptance/acceptanceStateCatalog.test.ts`

**Acceptance states:**

```text
RVW-01 review-start-selection
RVW-02 review-start-folder-aggregate
RVW-03 review-active-grid
RVW-04 review-active-preview
RVW-05 review-inspector-multi-target
RVW-06 review-save-error
RVW-07 review-resume
RVW-08 review-writer-busy
RVW-09 review-read-only
RVW-10 review-completion-summary
RVW-11 review-version-conflict
RVW-12 review-recovery-required
RVW-13 review-completed-read-only
RVW-14 review-keyboard-focus
RVW-15 review-zoom-200
```

- Every scene uses the real production component tree and deterministic bridge fixtures.
- Selection/grid/preview scenes prove that entering full preview is optional and does not change completion eligibility.
- Completion scenes must contain no viewed-count or “review progress” language.
- Conflict paths are project-relative; no fixture or rendered error includes an absolute path.

- [x] **Step 1: Add catalog entries and failing scene-contract tests**

Assert every review state has one registered scene, every scene uses the shared harness, and no scene bypasses the coordinator by supplying protocol JSON directly to a component.

- [x] **Step 2: Run acceptance unit tests and observe missing scenes**

Run: `pnpm --dir ui exec vitest run src/acceptance/scenes/reviewScenes.test.tsx src/acceptance/acceptanceStateCatalog.test.ts`

Expected: FAIL until all catalog IDs resolve.

- [x] **Step 3: Implement fixtures/scenes and inspect rendered output**

Run the visual acceptance server and capture all review states with the existing acceptance script. Inspect normal, dark/light if supported by the harness, keyboard focus, and 200% zoom. Fix component CSS, not scene-only CSS.

- [x] **Step 4: Verify acceptance gates**

Run:

```bash
pnpm test:visual-acceptance
pnpm build:visual-acceptance
pnpm accept:visual -- --id RVW-01 --id RVW-02 --id RVW-03 --id RVW-04 --id RVW-05 --id RVW-06 --id RVW-07 --id RVW-08 --id RVW-09 --id RVW-10 --id RVW-11 --id RVW-12 --id RVW-13 --id RVW-14 --id RVW-15
```

Expected: deterministic tests/build pass and the review captures contain no clipping, hidden primary action, color-only status, or focus loss. If the last command requires the local display/runtime, record that environment precondition explicitly; do not count an unrun capture as passed.

Capture record (2026-08-25): the deterministic tests and acceptance build passed. All 15 states reached the ready and geometry gates and produced 30 `product.png` captures at 1024×720 and 1440×900 under `target/viewer-visual-acceptance/review-loop-r6/`; both contact sheets were inspected. `RVW-15` used the verified 2× browser environment. The harness does not expose a light/dark color-scheme switch, so no unsupported dark-theme result is claimed. Finalization returned `ENOENT` only after capture because these new review states have no legacy Atlas `reference.png`; therefore no reference comparison or passed visual verdict is claimed.

- [x] **Step 5: Commit**

```bash
git add ui/src/acceptance/acceptanceBridge.ts ui/src/acceptance/acceptanceFixtures.ts ui/src/acceptance/scenes/reviewScenes.tsx ui/src/acceptance/scenes/reviewScenes.test.tsx ui/src/acceptance/scenes/index.ts ui/src/acceptance/acceptanceStateCatalog.json ui/src/acceptance/acceptanceStateCatalog.test.ts
git commit -m "test: add review acceptance states"
```

### Task 11: Prove the completed manual Stream is Agent-readable and regression-safe

**Files:**

- Create: `src-tauri/examples/review_loop_harness.rs`
- Create: `scripts/review-protocol/manual-round-e2e.mjs`
- Create: `scripts/review-protocol/manual-round-e2e.test.mjs`
- Modify: `package.json`
- Modify: `scripts/repository-policy.test.mjs`

**Gate contract:**

```json
{
  "scripts": {
    "test:review-loop": "node --test scripts/review-protocol/manual-round-e2e.test.mjs",
    "quality": "pnpm test:policy && pnpm test:review-protocol && pnpm test:review-loop && pnpm architecture:boundaries && pnpm test:video:packaging && node --test scripts/verify-clean.test.mjs && pnpm --dir ui check && pnpm --dir ui test && pnpm --dir ui build && cargo fmt --check && cargo clippy --locked --workspace --all-targets -- -D warnings && cargo test --locked --workspace"
  }
}
```

- The deterministic e2e seam creates a real temporary Project, builds an indexed selection, starts a manual Draft, writes one single-target and one multi-target Feedback, tears down/reopens the session, resumes, completes, and invokes `scripts/review-protocol/read-latest.mjs` with the exact manual Stream ID.
- `src-tauri/examples/review_loop_harness.rs` is the fixed seam. Node invokes `cargo run --quiet --locked -p viewer-desktop --example review_loop_harness -- --project <temporary-project> --scenario <name>`. The harness composes the production Application and Infrastructure review adapters, emits one bounded JSON result containing `streamId`, `roundId`, and safe relative assertions, and never becomes a shipping application binary.
- Assert Draft is ignored before completion; Completed exact head contains revise/unreviewable/pass covering exactly the frozen set; Feedback text is byte-exact; no marker/favorite state influenced Outcomes.
- Add separate cases for read-only, Busy, corrupted image, stable video failure, replacement, move, delete, publish fault recovery, and multiple Draft fail-closed.
- Do not install or invoke Codex, Claude, OpenCode, network services, signing, notarization, installers, or release systems.

- [x] **Step 1: Write the e2e tests against a missing driver**

Run: `pnpm test:review-loop`

Expected: FAIL because the driver/test seam is absent.

- [x] **Step 2: Implement the fixed Rust harness and Node driver**

Implement these exact harness scenarios: `standard`, `read_only`, `writer_busy`, `corrupt_image`, `stable_video_failure`, `replaced`, `moved`, `deleted`, `publish_recovery`, and `multiple_drafts`. The Node test must call the existing Agent-independent reference reader as a separate process for `standard` and `publish_recovery`, then assert its JSON output rather than importing reader internals.

- [x] **Step 3: Freeze architecture policy**

Add repository-policy assertions that:

- only `ui/src/api/viewer.ts` imports `@tauri-apps/api`;
- review components do not import protocol DTOs or Tauri;
- Tauri review commands do not contain `serde_json`, filesystem writes, Outcome construction, or BLAKE3;
- `viewer-application` has no dependency on `viewer-infrastructure` or Tauri;
- the review-loop test remains in `quality`.

- [x] **Step 4: Run protocol, e2e, policy, and architecture gates**

Run:

```bash
pnpm test:review-protocol
pnpm test:review-loop
pnpm test:policy
pnpm architecture:boundaries
```

Expected: all pass and the exact manual Stream head is readable by the phase-1 reader.

- [x] **Step 5: Commit**

```bash
git add src-tauri/examples/review_loop_harness.rs scripts/review-protocol/manual-round-e2e.mjs scripts/review-protocol/manual-round-e2e.test.mjs package.json scripts/repository-policy.test.mjs
git commit -m "test: gate the manual review loop"
```

### Task 12: Update product truth and run the complete verification matrix

**Files:**

- Modify: `docs/PRODUCT_SPEC.md`
- Modify: `docs/protocol/README.md`
- Modify: `docs/README.md`
- Modify: `docs/superpowers/specs/2026-08-25-viewer-exception-driven-review-loop-design.md`
- Modify: `docs/superpowers/plans/2026-08-25-viewer-exception-driven-review-loop.md`

**Documentation contract:**

- Mark phase 2 “current” only after its gates pass.
- Describe current manual review scope, exception-driven default pass, Draft resume, stable technical failure, conflict blocking, manual Stream, and external Completed reader.
- Keep Production Manifest execution, revision relationships, comparative re-review, region/time anchors, CLI/MCP/API, and concrete Agent integration explicitly future.
- Preserve the development-stage statement: signing, Apple notarization, formal installer, public release, sale, and sales tasks are not ordinary completion items and must not be proposed without explicit authorization.
- Do not rewrite or discard unrelated documentation edits already present in the working tree. Reconcile line-by-line and stage only phase-2 hunks.

- [ ] **Step 1: Run focused verification from a clean task worktree**

```bash
cargo fmt --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
pnpm --dir ui check
pnpm --dir ui test
pnpm --dir ui build
pnpm test:review-protocol
pnpm test:review-loop
pnpm architecture:boundaries
pnpm test:policy
```

Expected: every command exits 0. Record exact output for any environment-conditioned native check; a skip is a skip, not a pass.

- [ ] **Step 2: Run the repository's complete verification and cleanliness checks**

Run:

```bash
pnpm verify
pnpm verify:clean
```

Expected: both commands exit 0; `verify` runs quality and security gates, and `verify:clean` proves the worktree contains no untracked/generated artifacts outside explicit ignored acceptance/brainstorm paths.

- [ ] **Step 3: Exercise the real development application manually**

Run: `pnpm start:viewer`

Use a disposable project and verify this exact flow:

```text
open project
  -> browse image/video grid normally
  -> create review from selection
  -> add single-target and multi-target natural-language Feedback
  -> open image/video preview and return without changing review semantics
  -> close/reopen project and explicitly resume Draft
  -> inspect completion summary with no viewed count
  -> trigger one replacement conflict and verify completion is blocked
  -> restore original file evidence
  -> complete Round
  -> reopen project and view Completed read-only
  -> run the Node reader against the exact manual Stream
  -> verify search/compare/markers/favorite/file operations still work
```

Do not use the user's production materials. Stop the dev process cleanly after verification.

- [ ] **Step 4: Update documentation status only from verified facts**

Change the design status to `Implemented` and this plan status to `Complete` only after Steps 1–3 pass. If any required check is blocked, document the exact blocker and keep status `In progress`.

- [ ] **Step 5: Review the final diff for scope and architecture**

Run:

```bash
git diff --check
git diff --stat
git status --short
rg -n "TODO|FIXME|placeholder|coming soon|not implemented" crates/viewer-* src-tauri/src ui/src scripts/review-protocol docs
rg -n "sign|notari|installer|release|发售|公证|签名|正式安装包" docs/PRODUCT_SPEC.md docs/protocol/README.md docs/superpowers/specs/2026-08-25-viewer-exception-driven-review-loop-design.md
```

Expected: no accidental placeholders; release-related matches appear only in explicit out-of-scope/development-stage language; unrelated user modifications remain unstaged and unchanged.

- [ ] **Step 6: Commit the verified product truth**

```bash
git add -p docs/PRODUCT_SPEC.md
git add docs/protocol/README.md docs/README.md docs/superpowers/specs/2026-08-25-viewer-exception-driven-review-loop-design.md docs/superpowers/plans/2026-08-25-viewer-exception-driven-review-loop.md
git diff --cached --check
git commit -m "docs: record the manual review loop"
```

Do not stage unrelated pre-existing documentation hunks. After commit, re-run `git status --short` and compare the preserved user-file diff hash captured before execution.

---

## Reviewer Checkpoints

- After Task 3: confirm source identity, no-follow evidence, bounded hashing, failure mapping, and watcher change semantics before Application work continues.
- After Task 6: confirm the state machine, manual Stream selection, clone-save-swap, exact-head recovery, and lease release before exposing commands.
- After Task 7: confirm Tauri is translation-only and review failure cannot block ordinary project opening/browsing.
- After Task 9: confirm the UI reuses the existing grid/preview/compare systems and contains no hidden “viewed means reviewed” logic.
- After Task 11: confirm the phase-1 Agent-independent reader consumes the exact Completed manual Stream without any Viewer-specific shortcut.
- Before Task 12 documentation status changes: require fresh verification evidence, not earlier task output.

## Definition of Done

- One Project cannot create two Active Drafts through the use case, even across concurrent Viewer instances.
- A user can freeze an explicit selection or current Folder Projection, excluding non-media with backend counts.
- Feedback is natural language, durable before UI success, independently editable/deletable, and supports one/many fixed targets.
- Grid review and full preview have equal status; no per-asset opening or viewed count is required.
- Completion explicitly summarizes revise/unreviewable/default-pass and is blocked by conflict or unresolved technical state.
- Replacement, move, deletion, or content change never silently becomes pass.
- Draft resumes after restart, remains invisible to the external reader, and can be explicitly abandoned without history.
- Completed is immutable/read-only and the existing Node reader reads the exact manual Stream head.
- Review failures do not break browsing, preview, search, compare, marker, favorite, or file-operation workflows.
- Full deterministic verification passes, environment-conditioned checks are truthfully labeled, and product documentation distinguishes current behavior from future phases.
- No signing, notarization, installer, release, sale, or sales task is added to completion scope.
