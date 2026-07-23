# Viewer Infrastructure Decomposition Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split the largest file-operation and search-index implementations into focused modules while preserving the Application-facing adapters and all filesystem safety guarantees.

**Architecture:** Keep `LocalFileMutation`, `LocalFileCommandAdapter`, and `SessionIndex` as stable facades. Extract bound filesystem primitives, staged-copy lifecycle, evidence, preflight, execution, result settlement, schema, writes, queries, and projections behind those facades.

**Tech Stack:** Rust 1.97.0 edition 2024, Tokio, rusqlite/SQLite, BLAKE3, libc/macOS filesystem primitives, viewer-domain/application.

## Global Constraints

- Plans 1 and 2 must be complete before this plan starts.
- Preserve every Application port signature.
- Preserve no-follow path resolution, bound parent/leaf identity, no-replace placement, cleanup ownership, journal barriers, recovery obligations, cancellation, and source/destination evidence checks.
- Never replace an identity-bound filesystem primitive with pathname-only cleanup or deletion.
- Preserve operation result codes, journal states, migration SQL, FTS query semantics, marker projections, and current transaction boundaries.
- Platform-specific `#[cfg(target_os = "macos")]` and fallback implementations stay paired in the same responsibility module.
- Existing fault-injection and race tests are mandatory characterization tests.

---

### Task 1: Freeze Infrastructure Facades and Target Layout

**Files:**
- Modify: `tests/file_transactions.rs`
- Modify: `tests/search.rs`
- Modify: `scripts/repository-policy.test.mjs`

**Interfaces:**
- Produces: compile-time facade checks.
- Produces: source policy for focused Infrastructure module files.

- [ ] **Step 1: Add facade compile tests**

Add to `tests/file_transactions.rs`:

```rust
#[test]
fn file_operation_facades_keep_their_public_paths() {
    fn mutation(_: Option<viewer_infrastructure::operation::copy::LocalFileMutation>) {}
    fn adapter(
        _: Option<viewer_infrastructure::operation::service::LocalFileCommandAdapter>,
    ) {
    }
    mutation(None);
    adapter(None);
}
```

Add to `tests/search.rs`:

```rust
#[test]
fn session_index_keeps_its_public_path() {
    fn index(_: Option<viewer_infrastructure::search::index::SessionIndex>) {}
    index(None);
}
```

- [ ] **Step 2: Add a failing focused-layout policy**

Append to `scripts/repository-policy.test.mjs`:

```js
test('infrastructure adapters are backed by focused modules', async () => {
  const required = [
    'crates/viewer-infrastructure/src/operation/copy/mod.rs',
    'crates/viewer-infrastructure/src/operation/copy/file_reference.rs',
    'crates/viewer-infrastructure/src/operation/copy/staged.rs',
    'crates/viewer-infrastructure/src/operation/copy/evidence.rs',
    'crates/viewer-infrastructure/src/operation/service/mod.rs',
    'crates/viewer-infrastructure/src/operation/service/preflight.rs',
    'crates/viewer-infrastructure/src/operation/service/execution.rs',
    'crates/viewer-infrastructure/src/search/index/mod.rs',
    'crates/viewer-infrastructure/src/search/index/schema.rs',
    'crates/viewer-infrastructure/src/search/index/writer.rs',
    'crates/viewer-infrastructure/src/search/index/projection.rs',
  ]
  await Promise.all(required.map((path) => stat(new URL(`../${path}`, import.meta.url))))
})
```

- [ ] **Step 3: Run pre-change characterization**

```bash
cargo test --locked -p viewer-infrastructure --test file_transactions file_operation_facades_keep_their_public_paths
cargo test --locked -p viewer-infrastructure --test search session_index_keeps_its_public_path
node --test scripts/repository-policy.test.mjs
```

Expected: Rust tests pass; repository policy fails on missing module paths.

- [ ] **Step 4: Commit facade tests**

```bash
git add tests/file_transactions.rs tests/search.rs scripts/repository-policy.test.mjs
git commit -m "test: freeze infrastructure facade paths"
```

### Task 2: Extract Bound File Reference Primitives

**Files:**
- Create: `crates/viewer-infrastructure/src/operation/copy/mod.rs`
- Create: `crates/viewer-infrastructure/src/operation/copy/evidence.rs`
- Create: `crates/viewer-infrastructure/src/operation/copy/file_reference.rs`
- Create: `crates/viewer-infrastructure/src/operation/copy/local.rs`
- Create: `crates/viewer-infrastructure/src/operation/copy/placement.rs`
- Create: `crates/viewer-infrastructure/src/operation/copy/staged.rs`
- Delete: `crates/viewer-infrastructure/src/operation/copy.rs`

**Interfaces:**
- `operation::copy::LocalFileMutation` keeps its public path.
- `create_temporary_sync`, `hash_file_sync`, and `sync_parent` remain `pub(crate)` re-exports.
- File-reference internals remain private to `operation::copy`.

- [ ] **Step 1: Run bound-reference race tests**

```bash
cargo test --locked -p viewer-infrastructure operation::copy::tests::identity_bound_cleanup_unlinks_the_created_leaf_after_its_name_is_swapped
cargo test --locked -p viewer-infrastructure operation::copy::tests::verified_rename_rejects_a_parent_fd_reparented_outside_after_leaf_validation
cargo test --locked -p viewer-infrastructure operation::copy::tests::post_rename_leaf_escape_cleans_the_bound_identity_and_preserves_its_replacement
```

Expected: PASS.

- [ ] **Step 2: Create the copy facade**

Create `operation/copy/mod.rs`:

```rust
mod evidence;
mod file_reference;
mod local;
mod placement;
mod staged;

pub use local::LocalFileMutation;
pub(crate) use evidence::hash_file_sync;
pub(crate) use local::create_temporary_sync;
pub(crate) use placement::sync_parent;
```

Move `hash_file_sync` to `evidence.rs` and `sync_parent` to `placement.rs`, retaining their exact signatures and bodies. Create `staged.rs` without public items; Task 3 will populate it. Keep every other copy/evidence/placement implementation in `local.rs` for this commit.

- [ ] **Step 3: Move file-reference definitions and primitives**

Move unchanged to `file_reference.rs`:

- `BoundFileReference`;
- `bind_file_reference`;
- `bind_open_file_reference`;
- `bind_open_file_reference_with_hook`;
- `resolve_file_reference`;
- `resolve_open_file_path`;
- `open_path_no_follow`;
- `unlink_file_reference`;
- `file_reference_error`;
- `open_bound_parent`;
- `encoded_file_name`;
- `openat_file`;
- `file_snapshot`;
- `validate_bound_parent_location`;
- platform `renameat_no_replace` primitives used directly by bound placement.

Expose only sibling-required items as `pub(super)`.

- [ ] **Step 4: Move the Application port facade**

Move `LocalFileMutation` and `impl FileMutationPort for LocalFileMutation` to `local.rs`. Keep method bodies unchanged and import sibling helpers through `super::`.

Leave all staged-copy helpers and every evidence/placement helper other than `hash_file_sync` and `sync_parent` in `local.rs` through this commit; do not duplicate them.

- [ ] **Step 5: Run compile and race tests**

```bash
cargo fmt
cargo check --locked -p viewer-infrastructure --all-targets
cargo test --locked -p viewer-infrastructure operation::copy::tests
```

Expected: all existing copy unit tests pass.

- [ ] **Step 6: Commit bound-reference extraction**

```bash
git add crates/viewer-infrastructure/src/operation/copy crates/viewer-infrastructure/src/operation/copy.rs
git commit -m "refactor(infra): extract bound file references"
```

### Task 3: Extract Staged Copy, Placement and Evidence

**Files:**
- Modify: `crates/viewer-infrastructure/src/operation/copy/staged.rs`
- Modify: `crates/viewer-infrastructure/src/operation/copy/placement.rs`
- Modify: `crates/viewer-infrastructure/src/operation/copy/evidence.rs`
- Modify: `crates/viewer-infrastructure/src/operation/copy/local.rs`
- Modify: `crates/viewer-infrastructure/src/operation/copy/mod.rs`

**Interfaces:**
- `MacStagedCopyLease` owns cleanup through `Drop`.
- `placement` owns verified no-replace finalization and parent sync.
- `evidence` owns copy/hash/snapshot calculations.

- [ ] **Step 1: Run staged-copy lifecycle characterization**

```bash
cargo test --locked -p viewer-infrastructure operation::copy::tests::dropping_an_unplaced_staged_copy_unlinks_its_identity_after_parent_reparent
cargo test --locked -p viewer-infrastructure operation::copy::tests::successful_staged_placement_disarms_cleanup_and_keeps_destination
cargo test --locked -p viewer-infrastructure operation::copy::tests::registered_copy_cleanup_failure_preserves_primary_and_owned_temporary
```

Expected: PASS.

- [ ] **Step 2: Move lease ownership**

Move to `staged.rs`:

- `MacStagedCopyLease`;
- `MacStagedCopyBuild`;
- `Drop for MacStagedCopyLease`;
- `StagedCopyLeasePort` implementation;
- registered temporary creation/binding;
- staged copy construction and cleanup-obligation helpers.

Keep the invariant explicit:

```rust
impl Drop for MacStagedCopyLease {
    fn drop(&mut self) {
        if self.armed {
            let _ = unlink_file_reference(&self.temporary_reference, &self.temporary_path);
            let _ = self.temporary_parent.sync_all();
            self.armed = false;
        }
    }
}
```

Do not replace identity-bound cleanup with path-based `std::fs::remove_file`.

- [ ] **Step 3: Move verified placement**

Move to `placement.rs`:

- `place_bound_staged_copy_sync`;
- hook variants;
- recoverable destination validation;
- verified rename and rollback;
- parent identity verification;
- `rename_no_replace_sync`;
- `sync_parent`.

Keep all hook variants in this module because race tests depend on their exact barrier positions.

- [ ] **Step 4: Move content evidence**

Move to `evidence.rs`:

- `hash_file_sync`;
- file snapshot and timestamp helpers not owned by bound reference;
- copy-and-hash sync/async worker bodies;
- cancellable copy loops;
- open-file evidence validation;
- worker error conversion used only by copy evidence.

Use this sibling interface:

```rust
#[cfg(not(target_os = "macos"))]
pub(super) fn copy_open_files_and_evidence(
    source_file: File,
    temporary_file: File,
    source: &Path,
    temporary: &Path,
    cancellation: &FileCommandCancellation,
    expected_source: &FileSnapshot,
) -> Result<FileContentEvidence, FileOperationError>
```

Do not introduce a new evidence DTO.

- [ ] **Step 5: Run copy and transaction suites**

```bash
cargo fmt
cargo test --locked -p viewer-infrastructure operation::copy::tests
cargo test --locked -p viewer-infrastructure --test file_transactions
```

Expected: all copy race, cancellation, recovery and fault-matrix tests pass.

- [ ] **Step 6: Commit staged/evidence extraction**

```bash
git add crates/viewer-infrastructure/src/operation/copy
git commit -m "refactor(infra): split staged copy and evidence"
```

### Task 4: Split File Command Service Preflight and Batch State

**Files:**
- Create: `crates/viewer-infrastructure/src/operation/service/mod.rs`
- Create: `crates/viewer-infrastructure/src/operation/service/types.rs`
- Create: `crates/viewer-infrastructure/src/operation/service/preflight.rs`
- Create: `crates/viewer-infrastructure/src/operation/service/execution.rs`
- Create: `crates/viewer-infrastructure/src/operation/service/results.rs`
- Create: `crates/viewer-infrastructure/src/operation/service/undo.rs`
- Delete: `crates/viewer-infrastructure/src/operation/service.rs`

**Interfaces:**
- `LocalFileCommandAdapter` keeps its public path.
- `trash_temporary_for` remains `pub(crate)`.
- Private prepared batch/item types live in `types.rs`.
- The one `LocalFileCommandPort` trait implementation remains in `service/mod.rs` and delegates to inherent methods owned by sibling modules.

- [ ] **Step 1: Run command preflight characterization**

```bash
cargo test --locked -p viewer-infrastructure --test m3_file_commands repeated_display_preflights_leave_no_prepared_batch_registered
cargo test --locked -p viewer-infrastructure --test m3_file_commands duplicate_batch_destinations_are_resolved_in_visible_order
cargo test --locked -p viewer-infrastructure --test m3_rename_preflight
```

Expected: PASS.

- [ ] **Step 2: Create service facade and move shared types**

Create `service/mod.rs`:

```rust
mod execution;
mod preflight;
mod results;
mod types;
mod undo;

pub use types::LocalFileCommandAdapter;
pub(crate) use execution::trash_temporary_for;
```

Move `PreparedRoute`, `PreparedItem`, `PreparedBatch`, and `LocalFileCommandAdapter` fields/constructor to `types.rs`. Mark the three prepared types and every adapter/prepared field used by sibling modules `pub(super)`; keep the adapter type and its constructor public.

Keep the single trait implementation in `service/mod.rs`:

```rust
#[async_trait]
impl LocalFileCommandPort for LocalFileCommandAdapter {
    async fn preflight(
        &self,
        batch_id: BatchId,
        command: &FileCommand,
    ) -> Result<Vec<LocalFileCommandPreflightItem>, LocalFileCommandError> {
        self.preflight_command(batch_id, command).await
    }

    async fn discard_preflight(
        &self,
        batch_id: BatchId,
    ) -> Result<(), LocalFileCommandError> {
        self.discard_preflight_command(batch_id).await
    }

    async fn execute_item(
        &self,
        request: FileCommandItemExecution,
        cancellation: FileCommandCancellation,
    ) -> Result<LocalFileCommandOutcome, LocalFileCommandError> {
        self.execute_prepared_item(request, cancellation).await
    }

    async fn settle_unstarted(
        &self,
        request: FileCommandItemExecution,
        outcome: LocalFileCommandOutcome,
    ) -> Result<LocalFileCommandOutcome, LocalFileCommandError> {
        self.settle_unstarted_item(request, outcome).await
    }

    async fn take_undo_actions(
        &self,
        batch_id: BatchId,
    ) -> Result<Vec<UndoAction>, LocalFileCommandError> {
        self.take_prepared_undo_actions(batch_id).await
    }
}
```

- [ ] **Step 3: Move preflight**

Move to `preflight.rs`:

- `prepare`;
- `prepare_simple`;
- `prepare_rename`;
- `destination_for`;
- source/destination resolution and revalidation;
- case-sensitivity check;
- duplicate destination marking;
- rename error classification used only by preflight;
- `preflight_command` and `discard_preflight_command` delegates.

Implement these as `pub(super)` inherent methods in sibling `impl LocalFileCommandAdapter` blocks; do not add another trait implementation or a second adapter type.

- [ ] **Step 4: Run preflight tests**

```bash
cargo fmt
cargo test --locked -p viewer-infrastructure --test m3_rename_preflight
cargo test --locked -p viewer-infrastructure --test m3_file_commands repeated_display_preflights_leave_no_prepared_batch_registered
```

Expected: PASS.

- [ ] **Step 5: Commit preflight split**

```bash
git add crates/viewer-infrastructure/src/operation/service crates/viewer-infrastructure/src/operation/service.rs
git commit -m "refactor(infra): extract file command preflight"
```

### Task 5: Split File Command Execution, Results and Undo

**Files:**
- Modify: `crates/viewer-infrastructure/src/operation/service/execution.rs`
- Modify: `crates/viewer-infrastructure/src/operation/service/results.rs`
- Modify: `crates/viewer-infrastructure/src/operation/service/undo.rs`
- Modify: `crates/viewer-infrastructure/src/operation/service/types.rs`

**Interfaces:**
- Execution owns mutation routes and commit barriers.
- Results owns terminal settlement and delivery.
- Undo owns only `UndoFilePort`.

- [ ] **Step 1: Run execution/recovery characterization**

```bash
cargo test --locked -p viewer-infrastructure --test m3_file_commands
cargo test --locked -p viewer-infrastructure --test m3_undo
```

Expected: PASS.

- [ ] **Step 2: Move execution routes**

Move to `execution.rs`:

- journal start;
- prepared-item lookup;
- rename group/single rename;
- copy/move route;
- trash route;
- finish commit;
- expected-change registration;
- regular source/path validation;
- `execute_prepared_item`;
- `settle_unstarted_item`;
- temporary path helpers.

Keep every journal transition adjacent to the filesystem barrier it records.

- [ ] **Step 3: Move terminal result settlement**

Move to `results.rs`:

- `mark_failed`;
- `mark_cancelled`;
- `settle_recovery_required`;
- `fail_outcome_for_entity`;
- `maybe_finish_batch`;
- `mark_delivered`;
- result/error code classifiers;
- batch lock and time helpers used by settlement.

Keep terminal accounting idempotent.

- [ ] **Step 4: Move undo**

Move `take_prepared_undo_actions`, `UndoFilePort::reverse_batch`, and undo-only error mapping to `undo.rs`.

- [ ] **Step 5: Run file command and recovery suites**

```bash
cargo fmt
cargo test --locked -p viewer-infrastructure --test m3_file_commands
cargo test --locked -p viewer-infrastructure --test m3_undo
cargo test --locked -p viewer-infrastructure --test file_transactions
```

Expected: PASS.

- [ ] **Step 6: Commit execution split**

```bash
git add crates/viewer-infrastructure/src/operation/service
git commit -m "refactor(infra): split command execution and settlement"
```

### Task 6: Split SessionIndex Storage and Projections

**Files:**
- Create: `crates/viewer-infrastructure/src/search/index/mod.rs`
- Create: `crates/viewer-infrastructure/src/search/index/schema.rs`
- Create: `crates/viewer-infrastructure/src/search/index/writer.rs`
- Create: `crates/viewer-infrastructure/src/search/index/query.rs`
- Create: `crates/viewer-infrastructure/src/search/index/projection.rs`
- Delete: `crates/viewer-infrastructure/src/search/index.rs`

**Interfaces:**
- `SessionIndex` keeps its public path and public methods.
- One `Mutex<Connection>` remains the shared connection owner.
- Existing transaction boundaries remain inside one locked connection.

- [ ] **Step 1: Run index and projection characterization**

```bash
cargo test --locked -p viewer-infrastructure --test search
cargo test --locked -p viewer-infrastructure --test m2_session_projection
cargo test --locked -p viewer-infrastructure --test m3_operation_projections
```

Expected: PASS.

- [ ] **Step 2: Create the index facade**

Create `search/index/mod.rs`:

```rust
mod projection;
mod query;
mod schema;
mod writer;

use std::sync::Mutex;
use rusqlite::Connection;

pub struct SessionIndex {
    connection: Mutex<Connection>,
}
```

Re-export no internal SQL helpers.

- [ ] **Step 3: Move schema/open/close**

Move to `schema.rs`:

- database open;
- PRAGMA configuration;
- schema creation/migration;
- FTS availability/setup;
- close.

Expose:

```rust
pub(super) fn open_connection(path: &Path) -> Result<Connection, SessionIndexError>
```

Keep existing SQL text and migration order unchanged.

- [ ] **Step 4: Move ordinary writes and queries**

Move to `writer.rs`:

- `upsert_batch`;
- `remove_subtree`;
- `replace_text`;
- `mark_text_failed`;
- `replace_image_metadata`;
- `hydrate_markers`;
- `set_review_metadata`;
- identity/subtree reconciliation.

Move to `query.rs`:

- `indexed_node`;
- `index_progress`;
- `directory_children`;
- `BrowseIndexPort` read methods.

- [ ] **Step 5: Move projections**

Move to `projection.rs`:

- `MarkerProjectionPort::sync_markers`;
- `OperationCommitPort` projection methods;
- copy/move/trash projection helpers;
- path staging used to avoid unique constraint collisions.

- [ ] **Step 6: Run all index tests**

```bash
cargo fmt
cargo test --locked -p viewer-infrastructure --test search
cargo test --locked -p viewer-infrastructure --test m2_session_projection
cargo test --locked -p viewer-infrastructure --test m2_browse_projections
cargo test --locked -p viewer-infrastructure --test m3_operation_projections
```

Expected: PASS.

- [ ] **Step 7: Commit index split**

```bash
git add crates/viewer-infrastructure/src/search/index crates/viewer-infrastructure/src/search/index.rs
git commit -m "refactor(infra): split session index responsibilities"
```

### Task 7: Run Infrastructure Exit Gate

**Files:**
- Verify only.

- [ ] **Step 1: Run module policy**

```bash
node --test scripts/repository-policy.test.mjs
```

Expected: PASS.

- [ ] **Step 2: Run focused safety suites**

```bash
cargo test --locked -p viewer-infrastructure
cargo test --locked -p viewer-infrastructure --test file_transactions
cargo test --locked -p viewer-infrastructure --test m3_file_commands
cargo test --locked -p viewer-infrastructure --test search
```

Expected: PASS with no ignored data-safety failure.

- [ ] **Step 3: Run full clean verification**

```bash
pnpm verify:clean
```

Expected: PASS with no new worktree entries.

- [ ] **Step 4: Inspect stable facades**

```bash
git diff --check
rg -n 'pub struct LocalFileMutation|pub struct LocalFileCommandAdapter|pub struct SessionIndex' crates/viewer-infrastructure/src
```

Expected: one facade definition for each stable type.
