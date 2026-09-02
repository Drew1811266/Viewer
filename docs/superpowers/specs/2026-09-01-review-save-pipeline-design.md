# Viewer 快速评审保存与后台发布设计

> Status: Approved direction, pending written-spec review
>
> Date: 2026-09-01
>
> Scope: Continuous-review authoring, evidence materialization, publication, native reader gating,
> and save-state presentation. This design does not add product capabilities.

## 1. Context

Viewer's current continuous-review save path treats one user confirmation as one synchronous public
publication. The UI waits for command preparation and application. Application then prepares recovery,
transitions the complete state, prepares evidence for the complete state, checks sources, installs and
verifies evidence, commits immutable records and the mutable index, reloads the full workspace view, and
checks current sources again before replying.

This is correct but places unrelated costs in the foreground interaction. On the real development corpus,
an asset-only save takes about 1.64 seconds and a region save that rebuilds image evidence takes about
2.62 seconds. The current evidence directory is about 37 MiB. The latency comes from the transaction
boundary, not from the amount of feedback text.

Professional editing systems separate durable edit decisions from source media, generated previews,
caches, and backups. Viewer should adopt the same separation without weakening the existing rule that an
Agent may only receive complete, verified, source-bound review data.

## 2. Goals

1. A normal feedback save durably records the user's logical intent without waiting for PNG rendering,
   evidence installation, full repository verification, or a complete workspace reload.
2. The user can continue reviewing immediately after the logical commit.
3. The external Agent reader never returns a partial publication or labels an older publication as the
   latest review state.
4. Existing v3 immutable state, archive, usage, evidence, index, and reader result formats remain the
   externally published contract for ready data.
5. Existing command idempotency, exact feedback/target revision identity, archive semantics, recovery,
   source binding, immutable evidence, and atomic index publication remain intact.
6. Existing v3 projects activate without regenerating evidence or rewriting history.
7. The implementation adds no network service and no new third-party runtime dependency.

## 3. Non-goals

- No automatic Agent execution or notification.
- No new review tools, feedback types, approval states, or workflow actions.
- No change to the meaning of archive, restore, withdraw, usage declaration, or source confirmation.
- No video time-range annotation implementation.
- No cloud synchronization or multi-host writer support.
- No signing, notarization, formal installer, store listing, public release, or sales work.
- No guarantee that evidence materialization itself finishes within the foreground save budget.

## 4. Required invariants

### 4.1 Authoring durability

- A successful save acknowledgement means the command, generated identities, logical state, transition,
  and materialization outbox entry are durable in one local transaction.
- A failed acknowledgement must not silently publish input. The editor input remains recoverable.
- Duplicate `command_id` plus the same payload digest returns the original authoring receipt. The same
  command ID plus a different digest remains a conflict.

### 4.2 Agent publication safety

- `authoring_head` identifies the newest durable logical state.
- `published_head` identifies the newest fully materialized and verified public v3 state.
- A current Agent read is permitted only when a read transaction observes
  `authoring_head == published_head`.
- When the heads differ, current read returns the typed error `publication_pending`; it must not return
  `published_head` as if it were current.
- Exact already-published historical selectors remain readable while a newer current publication is
  pending.

### 4.3 Atomic publication ordering

For a target authoring revision, publication order is fixed:

1. Write and sync new immutable evidence objects.
2. Write and sync the immutable v3 state and any archive/usage records.
3. Atomically publish and sync the v3 index.
4. In a short database transaction, advance `published_head` to the target authoring revision.

The last step is the visibility point for the native current reader. A crash before it produces a pending
state, never a partial successful read. A crash after immutable files or the v3 index were written can
leave reusable objects, but cannot expose incomplete bytes.

### 4.4 Source and evidence identity

- Logical save uses the exact `AssetVersionId` and authorized preview basis already held by the review
  session. It never silently captures a replacement source.
- A clean base evidence object is immutable for an `AssetVersionId`.
- Agent actionable projection keeps its full source-content verification at the reader boundary.
- Existing immutable objects may be reused by digest. Missing, malformed, substituted, or digest-mismatched
  cache entries are cache misses or integrity errors; they are never trusted by path alone.

## 5. Architecture

```text
Image review UI
    |
    | save(command request)
    v
ContinuousReviewAuthoringService
    |
    | one short SQLite transaction
    v
AuthoringStore ----------------------------+
  command log                              |
  logical snapshots                        |
  authoring/published heads                |
  materialization outbox                   |
    |                                      |
    | SaveAck + ReviewPatch                | queued target revision
    v                                      v
UI continues                         ReviewMaterializer
                                           |
                                           | exact state + reusable evidence
                                           v
                                      v3 Repository
                                           |
                                           | atomic index, then published head
                                           v
                                     Native Agent Reader
```

The new AuthoringStore is the control plane. Existing `.viewer/reviews` v3 records and evidence are the
published data plane. Rendering and public v3 encoding do not occur inside the foreground authoring
transaction.

## 6. Internal persistence model

### 6.1 Location and SQLite policy

Authoring data is stored in dedicated review tables in the existing `.viewer/metadata.sqlite`. Reusing
the existing project database avoids another dependency and another WAL/checkpoint lifecycle. Review
tables use the same project-root ownership and file-kind validation already required by metadata access.

Writable connections configure:

- `PRAGMA journal_mode=WAL`
- `PRAGMA synchronous=FULL`
- `PRAGMA foreign_keys=ON`
- a bounded busy timeout

No rendering, hashing, image decoding, source traversal, or JSON file publication may occur while a
SQLite write transaction is open. If WAL cannot be enabled on the current filesystem, the store uses
SQLite's atomic rollback journal with `synchronous=FULL`; correctness is preserved and the application
reports the degraded persistence mode through diagnostics.

### 6.2 Tables

`review_authoring_streams`

- `stream_id` primary key
- `project_id`
- `production_scope`
- `authoring_seq`
- `authoring_snapshot_id`
- `published_seq`
- `published_snapshot_id`
- `updated_at_ms`

`review_authoring_snapshots`

- `(stream_id, seq)` primary key
- unique `(stream_id, snapshot_id)`
- unique `command_id`
- `parent_seq`
- `payload_digest`
- bounded canonical logical-state bytes
- bounded canonical transition bytes
- generated identities needed to retry or reconcile the command
- `barrier_kind`: `none`, `archive`, `restore`, or `migration`
- `created_at_ms`

`review_materialization_jobs`

- `(stream_id, target_seq)` primary key
- `status`: `queued`, `running`, `retryable`, or `blocked`
- `attempt_count`
- `lease_epoch`
- `next_attempt_at_ms`
- stable bounded error code, never an arbitrary OS path or diagnostic body

`review_evidence_action_cache`

- `action_key` primary key
- verified base evidence reference
- optional verified annotated evidence reference
- renderer version and output-policy version

The action cache is derived and disposable. Authoring snapshots and heads are authoritative and are not
deleted because a cache entry is missing.

### 6.3 Authoring transaction

A normal save uses `BEGIN IMMEDIATE` and performs only bounded metadata work:

1. Look up `command_id` for idempotency.
2. Compare the expected authoring snapshot ID.
3. Run the pure Domain transition against the current logical state.
4. Insert the new logical snapshot and transition.
5. Advance `authoring_head`.
6. Insert the materialization outbox row in the same transaction.
7. Commit and return `ReviewAuthoringReceipt` plus a minimal `ReviewPatch`.

The persisted logical state excludes rendered evidence bytes. It contains the complete current Domain
state and the information required to deterministically build the public v3 snapshot.

## 7. Application interfaces

### 7.1 Authoring ports

```rust
pub trait ContinuousReviewAuthoringStorePort: Send + Sync {
    fn load_heads(&self, stream: ReviewStreamId) -> Result<ReviewHeads, ReviewCommitError>;
    fn load_current(&self, stream: ReviewStreamId) -> Result<Option<StoredAuthoringState>, ReviewCommitError>;
    fn find_command(&self, stream: ReviewStreamId, command: ReviewCommandId) -> Result<AuthoringCommandLookup, ReviewCommitError>;
    fn commit(&self, request: ReviewAuthoringCommitRequest) -> Result<ReviewAuthoringReceipt, ReviewCommitError>;
}

pub struct ReviewHeads {
    pub authoring: Option<AuthoringHead>,
    pub published: Option<AuthoringHead>,
}

pub struct ReviewAuthoringApplyResult {
    pub receipt: ReviewAuthoringReceipt,
    pub patch: ReviewWorkspacePatch,
    pub publication: ReviewPublicationStatus,
}

pub enum ReviewPublicationStatus {
    Ready,
    Pending { pending_revisions: u32 },
    Blocked { code: ReviewMaterializationFailure },
}
```

The foreground command no longer returns a freshly rebuilt `ReviewWorkspaceView`. Full `view()` remains
the resynchronization API for activation, explicit refresh, stale-state recovery, and uncertain outcomes.

### 7.2 Materialization ports

```rust
pub trait ReviewMaterializationQueuePort: Send + Sync {
    fn next(&self) -> Result<Option<ClaimedReviewMaterialization>, ReviewCommitError>;
    fn retry(&self, claim: &ClaimedReviewMaterialization, code: ReviewMaterializationFailure) -> Result<(), ReviewCommitError>;
    fn block(&self, claim: &ClaimedReviewMaterialization, code: ReviewMaterializationFailure) -> Result<(), ReviewCommitError>;
}

pub trait ReviewPublicationPort: Send + Sync {
    fn materialize(&self, target: StoredAuthoringState, cancellation: ReviewTaskCancellation) -> Result<PreparedReviewPublication, ReviewWorkspaceError>;
    fn publish(&self, prepared: PreparedReviewPublication) -> Result<ReviewPublicationReceipt, ReviewWorkspaceError>;
}
```

Materialization claims are restartable leases, not ownership of user data. A process restart requeues
expired `running` jobs.

## 8. Evidence materialization

### 8.1 Prewarming

When an image preview is authorized, Viewer schedules clean base-evidence capture for its exact
`AssetVersionId`. This work is cancellable and bounded, and it never changes review state. The expected
result is that the user examines and annotates the image while base capture completes in the background.

If a save occurs before base capture completes, logical save still succeeds. Publication remains pending.
If the source changes before an exact base can be captured, materialization blocks with `source_changed`;
the saved text and normalized geometry remain available for explicit rebinding and are not discarded.

### 8.2 Action key

An evidence render is addressed by:

```text
BLAKE3(
  "viewer.review.evidence-action/1" ||
  source_content_digest ||
  canonical_annotation_scene_digest ||
  renderer_version ||
  output_policy_version ||
  upright_width || upright_height
)
```

The canonical scene contains ordered target identities, revisions, normalized geometry, marker ordinals,
and drawing style policy. Feedback text is excluded because it does not alter image pixels. A text-only
edit therefore reuses the same image evidence.

### 8.3 Dirty-set processing

The materializer compares the target logical state with the last published logical state and processes
only assets whose evidence action key changed. It reuses unchanged evidence bindings without reading or
hashing their PNG files during save-time materialization.

New PNG bytes are encoded once into a private temporary file while BLAKE3 and byte count are computed in
the same streaming pass. The object is then synced and installed by atomic create-once. The existing
native reader still performs independent verification at the external trust boundary.

### 8.4 Scheduling and compaction

- Rendering concurrency is bounded to one job per project and at most two image encodes process-wide so
  background work cannot starve preview interaction.
- Normal feedback-edit jobs may be compacted to the newest pending authoring revision for a stream.
- The command log and logical snapshots are not compacted by the render scheduler; every acknowledged
  command remains recoverable and idempotent.
- Archive, restore, and migration revisions are barriers. The scheduler may not compact across them.
- When adjacent unpublished normal edits are compacted, public v3 `changes` are deterministically folded
  from the last published state to the chosen target state. Add-then-edit becomes one addition of the
  final version; edit-then-edit becomes one edit from the published basis; withdrawal and availability
  changes keep their typed cause. Archive and restore changes are never folded away.

## 9. Public v3 publication

The publisher consumes one immutable authoring target and produces the same complete v3 objects expected
by today's repository and native reader:

- complete current state
- exact command identity and digest
- typed net changes from the previous public state
- complete evidence bindings
- archive and usage records when present
- atomically replaced index

Existing repository verification remains authoritative for newly produced objects. The performance change
is that this work runs outside the user's foreground save and verifies only new or changed evidence during
materialization. It does not weaken external reads.

The publisher may publish an intermediate target even if a newer authoring revision now exists. The native
current reader still returns `publication_pending` until the heads become equal, so an intermediate public
index is never mislabeled as the latest review.

## 10. Native Agent reader gate

The native reader opens a read transaction on `.viewer/metadata.sqlite` before selecting current review:

1. Read and pin the stream's authoring and published heads.
2. If no authoring control row exists, use the existing v3/legacy behavior for backward compatibility.
3. If current heads differ, return `publication_pending` with no current feedback payload.
4. If heads match, pin and verify the v3 index and immutable objects as today.
5. Return the complete current result.

The database read transaction supplies snapshot isolation. A save committed before the read transaction
is visible and causes `publication_pending`; a save committed after the reader began does not invalidate
the consistent state observed at invocation start.

Exact history requests do not require current heads to match, but their selected v3 objects must still pass
all existing identity, reachability, evidence, and source rules.

## 11. Archive, restore, and migration barriers

Manual archive is infrequent and needs an exact public basis. Before committing an archive checkpoint,
the application flushes materialization through the selected authoring head. If publication is blocked,
archive reports the existing source/evidence error and makes no checkpoint. Once the basis is public, the
archive command is committed to the authoring store and its result is materialized normally.

Restore and migration use the same barrier rule because they consume verified historical bytes. They do
not infer a basis from an unpublished logical state.

This keeps ordinary feedback saves fast while preserving the stronger semantics at the few operations
that actually join current and historical truth.

## 12. UI state and interaction

The UI keeps the newly drawn marker and text in its existing in-memory workbench state. On save:

- `saving_authoring`: short durable logical commit; save/cancel controls are guarded against duplicate
  submission.
- `saved_pending_publication`: editor closes, marker remains visible, review continues, and a nonblocking
  status says `已保存，Agent 数据生成中`.
- `ready`: status says `已保存，可供外部读取`.
- `publication_blocked`: status says `评审已保存，Agent 数据生成失败` and offers the existing retry or
  source-confirmation path appropriate to the stable error code.

Materialization failure never removes a saved marker or reopens the editor. Authoring failure retains the
editor input and draft exactly as today.

The UI applies `ReviewWorkspacePatch` to the current coordinator snapshot. A full-screen loading transition
is forbidden after a normal save. Full refresh is reserved for activation, explicit refresh, stale-snapshot
rebase, uncertain outcome reconciliation, and migration.

## 13. Recovery and failure matrix

| Failure point | Required outcome |
| --- | --- |
| Before authoring commit | No acknowledgement; editor input remains recoverable |
| After WAL commit before reply | Retry by command ID returns the original authoring receipt |
| During base capture or PNG encode | Authoring state remains saved; job retries or blocks with a stable code |
| After immutable object sync before v3 index | Orphan/reusable object; current reader remains pending |
| After v3 index publication before published-head update | Recovery verifies the complete target and advances the head; reader remains pending meanwhile |
| After published-head update before UI notification | Refresh observes heads equal and reports ready |
| Source replaced before first exact base capture | Publication blocks; Agent receives no current payload; user can explicitly rebind |
| Cache record references a missing/corrupt object | Cache miss or integrity failure; never a successful publication |

Startup reconciliation performs no speculative feedback changes. It resets expired job leases, compares
the heads, verifies any already-published target that can close the gap, and otherwise resumes or blocks
materialization deterministically.

## 14. Compatibility and activation

For an existing verified v3 stream with no authoring row:

1. Read and verify the current v3 state through the existing repository.
2. Insert one bootstrap authoring snapshot representing that exact public state.
3. Set authoring and published heads to the same snapshot in one transaction.
4. Do not regenerate evidence, rewrite the v3 index, or alter archive/history bytes.

The bootstrap is idempotent and occurs under the existing review writer gate. Legacy v1/v2 projects keep
their explicit migration path; the authoring store does not reinterpret a legacy Completed round as v3
current state.

During rollout, an internal feature gate may keep the old synchronous command path available for rollback.
The gate is not a user preference and cannot create two simultaneous writers for the same stream.

## 15. Performance requirements

Performance budgets apply to a normal authoring save containing at most 64 KiB of new logical command data
on the supported local macOS filesystem:

- p50 acknowledgement at or below 50 ms.
- p95 acknowledgement at or below 150 ms.
- p99 acknowledgement at or below 300 ms.
- No image decode, PNG encode, evidence-file scan, whole-project source hash, or full workspace view reload
  on the measured foreground span.
- Saving one asset must not scale with the number or total bytes of unchanged evidence objects.
- UI retains the image, transform, selection, and review overlays without a loading-page transition.

Materialization has separate diagnostics and service-level observations. Its duration does not fail the
foreground save budget, but the queue must be bounded, restartable, observable, and prevented from starving
interactive image work.

## 16. Verification strategy

### 16.1 Unit and contract tests

- Authoring commit CAS and command deduplication.
- Same-transaction outbox insertion.
- Head comparison and `publication_pending` reader result.
- `ReviewWorkspacePatch` equivalence to a full view after each command type.
- Evidence action-key stability and invalidation by geometry, source, renderer, and output policy.
- Text-only edits reuse evidence.
- Dirty-set selection excludes unchanged assets.
- Barrier-aware compaction and net typed changes.
- Existing v3 fixtures decode identically after bootstrap.

### 16.2 Fault injection

Inject process interruption or IO failure at every row in the failure matrix. Each test restarts from a
real temporary project and asserts both UI-authoring recovery and native-reader behavior. A current reader
must produce exactly one of complete verified current, `publication_pending`, or a stable integrity/source
error; partial success is forbidden.

### 16.3 Performance tests

Add a release-mode benchmark/harness using copies of the real 30-image development corpus. Measure:

- asset-only feedback
- first region feedback on an uncached base
- additional region feedback on a cached base
- text-only edit
- 30 sequential saves while materialization is intentionally slow
- save time with 1, 10, 100, and 1,000 unchanged evidence objects

The authoring latency assertion excludes first-time compilation and fixture setup but includes transaction
sync and real IPC serialization. Materialization timings are reported separately.

### 16.4 Regression gates

- Complete Rust workspace tests, formatting, Clippy, UI tests, protocol tests, architecture boundaries,
  dependency policy, and clean-worktree verification.
- Native cold restart with pending work.
- Source replacement before and after publication.
- External current and history reads while a new publication is pending.
- Existing manual archive, history, restore, retry, redraw, edit, and delete flows.

## 17. Implementation sequence

1. Add timing spans and freeze the current latency baseline.
2. Add AuthoringStore schema, domain-independent persistence types, and bootstrap without activating writes.
3. Move normal logical command commit to AuthoringStore and return patches while keeping synchronous
   materialization temporarily behind the new receipt.
4. Add durable outbox, worker lifecycle, restart reconciliation, and dual-head status.
5. Move evidence preparation/public v3 commit fully behind the worker.
6. Gate the native reader on equal heads.
7. Add evidence prewarming, action-key cache, dirty-set materialization, and bounded scheduling.
8. Add barrier-aware job compaction after non-compacted dual-head publication is proven correct.
9. Activate UI status states and remove the normal-save full refresh.
10. Run fault, performance, real-corpus, and full regression gates before removing the old internal fallback.

Each step must be independently testable and preserve a readable v3 public state. No step may temporarily
allow the Agent reader to consume an unpublished authoring state.

## 18. Rejected alternatives

### Optimize only the synchronous path

Dirty-only rendering and fewer hashes would improve average latency but leave PNG encoding, filesystem
sync, public verification, and full view refresh on the interaction path. It cannot make the p95 independent
of source/evidence size and does not solve the transaction-boundary problem.

### Return success before any durable logical commit

Pure optimistic UI would feel fast but could lose acknowledged review intent on crash. Viewer must not call
an edit saved until the authoring transaction is durable.

### Let the Agent read the last published state while a newer state is pending

This would be technically consistent but product-dangerous: the Agent could interpret stale feedback as
the user's latest instructions. The reader therefore fails closed with `publication_pending`.

### Replace v3 raster evidence with vector-only public evidence immediately

Vector annotations are the long-term canonical model, but removing raster evidence now would expand the
scope to an external protocol migration and Agent compatibility change. This design keeps v3 complete
evidence and uses normalized geometry internally to make future evolution possible.
