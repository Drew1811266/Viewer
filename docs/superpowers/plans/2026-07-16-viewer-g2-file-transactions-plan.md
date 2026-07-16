# Viewer G2 File Transactions Prototype Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prove that Viewer file operations remain safe and recoverable across conflicts, partial batch failure and forced termination at every persisted state.

**Architecture:** Domain defines operation plans and legal transitions. Application serializes file mutation tasks. Infrastructure persists a rollback-journal SQLite operation log and executes verified filesystem steps; macOS Adapter supplies Trash and volume/file identity operations.

**Tech Stack:** Rust, rusqlite, BLAKE3, tempfile-based integration tests, trash-rs, macOS filesystem identity APIs.

## Global Constraints

- Sources and destinations must resolve inside the current project root; `.viewer` is never a user-operation target.
- Same-volume rename/move uses atomic rename when supported.
- Copy writes a hidden temporary sibling, verifies it, then atomically renames it to the final name.
- Replace moves the existing destination to macOS Trash before placing the new target; it never silently overwrites.
- Batch success is per item; successful items are not rolled back because another item failed.
- Copy and Trash are not undoable by Viewer.
- Undo supports review state, favorite, rename and in-project move during the current session only.
- Recovery never guesses that an unknown user file is safe to delete.

---

## Target File Map

```text
crates/viewer-domain/src/operation.rs
crates/viewer-application/src/operation.rs
crates/viewer-application/src/ports.rs
crates/viewer-infrastructure/migrations/portable/0001_initial.sql
crates/viewer-infrastructure/src/operation/{mod.rs,journal.rs,executor.rs,copy.rs,rename.rs,recovery.rs}
crates/viewer-platform-macos/src/files/{mod.rs,identity.rs,trash.rs}
crates/viewer-test-support/src/{faults.rs,project_fixture.rs}
tests/file_transactions.rs
scripts/run-g2-file-transaction-gate.sh
docs/adr/0002-file-transaction-protocol.md
```

### Task 1: Define operation states, plans and legal transitions

**Files:**
- Create: `crates/viewer-domain/src/operation.rs`
- Modify: `crates/viewer-domain/src/lib.rs`

**Interfaces:**
- Produces: `OperationKind`, `OperationState`, `ConflictPolicy`, `OperationPlan`, `OperationItemPlan`, `transition_to`.

- [ ] **Step 1: Write failing transition tests**

```rust
#[test]
fn operation_accepts_only_forward_protocol_transitions() {
    let mut state = OperationState::Prepared;
    state.transition_to(OperationState::Staged).unwrap();
    state.transition_to(OperationState::FsApplied).unwrap();
    state.transition_to(OperationState::Verified).unwrap();
    state.transition_to(OperationState::MetaCommitted).unwrap();
    state.transition_to(OperationState::IndexSynced).unwrap();
    state.transition_to(OperationState::Completed).unwrap();
    assert_eq!(state, OperationState::Completed);
}

#[test]
fn operation_rejects_skipping_verification() {
    let mut state = OperationState::FsApplied;
    assert_eq!(
        state.transition_to(OperationState::MetaCommitted),
        Err(OperationTransitionError::Invalid {
            from: OperationState::FsApplied,
            to: OperationState::MetaCommitted,
        })
    );
}
```

Run `cargo test -p viewer-domain operation`. Expected: FAIL.

- [ ] **Step 2: Implement the state model**

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum OperationState {
    Prepared,
    Staged,
    FsApplied,
    Verified,
    MetaCommitted,
    IndexSynced,
    Completed,
    Failed,
}

impl OperationState {
    pub fn transition_to(&mut self, next: Self) -> Result<(), OperationTransitionError> {
        let allowed = matches!(
            (*self, next),
            (Self::Prepared, Self::Staged)
                | (Self::Staged, Self::FsApplied)
                | (Self::FsApplied, Self::Verified)
                | (Self::Verified, Self::MetaCommitted)
                | (Self::MetaCommitted, Self::IndexSynced)
                | (Self::IndexSynced, Self::Completed)
        ) || (next == Self::Failed && *self != Self::Completed);
        if allowed {
            *self = next;
            Ok(())
        } else {
            Err(OperationTransitionError::Invalid { from: *self, to: next })
        }
    }
}
```

Define operation kinds `Rename`, `Copy`, `Move`, `Trash`, `SetReviewState`, `SetFavorite`; conflict policies `Skip`, `KeepBoth`, `Replace`; and plans containing batch ID, operation ID, entity ID, source relative path, optional destination relative path and selected conflict policy.

- [ ] **Step 3: Verify and commit**

Run `cargo test -p viewer-domain operation`. Expected: all transition tests PASS.

```bash
git add crates/viewer-domain Cargo.lock
git commit -m "feat: define recoverable operation state machine"
```

### Task 2: Add the portable metadata schema and operation journal

**Files:**
- Create: `crates/viewer-infrastructure/migrations/portable/0001_initial.sql`
- Create: `crates/viewer-infrastructure/src/operation/journal.rs`
- Create: `crates/viewer-infrastructure/src/operation/mod.rs`
- Modify: `crates/viewer-infrastructure/src/lib.rs`
- Modify: `crates/viewer-infrastructure/Cargo.toml`

**Interfaces:**
- Produces: `OperationJournal::{create_batch,record_item,advance,fail,incomplete_items}`.

- [ ] **Step 1: Write a failing persistence/reopen test**

Create a temporary `metadata.sqlite`, record an item at `FsApplied`, drop the repository, reopen it, and assert `incomplete_items()` returns exactly that operation and state.

Run `cargo test -p viewer-infrastructure operation_journal`. Expected: FAIL.

- [ ] **Step 2: Add the exact migration**

```sql
PRAGMA foreign_keys = ON;

CREATE TABLE schema_migrations (
  version INTEGER PRIMARY KEY,
  applied_at_ms INTEGER NOT NULL
);

CREATE TABLE operation_batches (
  batch_id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  created_at_ms INTEGER NOT NULL,
  completed_at_ms INTEGER
);

CREATE TABLE operation_items (
  operation_id TEXT PRIMARY KEY,
  batch_id TEXT NOT NULL REFERENCES operation_batches(batch_id),
  entity_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  state TEXT NOT NULL,
  source_path TEXT NOT NULL,
  destination_path TEXT,
  temporary_path TEXT,
  expected_size INTEGER,
  expected_hash BLOB,
  conflict_policy TEXT NOT NULL,
  error_code TEXT,
  updated_at_ms INTEGER NOT NULL
);

CREATE INDEX operation_items_incomplete
ON operation_items(state)
WHERE state NOT IN ('completed', 'failed');

INSERT INTO schema_migrations(version, applied_at_ms) VALUES (1, 0);
```

Open portable metadata with `journal_mode=DELETE`, `synchronous=FULL`, foreign keys enabled and a 5-second busy timeout. Use one writer connection guarded by the repository; do not enable WAL for this database.

- [ ] **Step 3: Implement state-checked updates**

`advance(operation_id, expected_current, next)` must execute:

```sql
UPDATE operation_items
SET state = ?3, updated_at_ms = ?4
WHERE operation_id = ?1 AND state = ?2;
```

Require `changed_rows == 1`; otherwise return `JournalError::ConcurrentStateChange`. This prevents two recovery/execution paths from advancing the same item.

- [ ] **Step 4: Verify and commit**

Run `cargo test -p viewer-infrastructure operation_journal`. Expected: persistence, reopen and illegal advance tests PASS.

```bash
git add crates/viewer-infrastructure Cargo.lock
git commit -m "feat: persist file operation journal"
```

### Task 3: Implement verified sibling-temp copy

**Files:**
- Create: `crates/viewer-application/src/operation.rs`
- Modify: `crates/viewer-application/src/ports.rs`
- Create: `crates/viewer-infrastructure/src/operation/copy.rs`
- Create: `crates/viewer-infrastructure/src/operation/executor.rs`
- Create: `crates/viewer-test-support/src/project_fixture.rs`
- Test: `tests/file_transactions.rs`

**Interfaces:**
- Produces: `FileMutationPort`, `CopyExecutor::execute(OperationItemPlan)`.

- [ ] **Step 1: Define the mutation port**

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileSnapshot {
    pub len: u64,
    pub volume_id: u64,
    pub file_id: Option<u128>,
}

#[async_trait]
pub trait FileMutationPort: Send + Sync {
    async fn snapshot(&self, path: &Path) -> Result<FileSnapshot, FileOperationError>;
    async fn copy_and_hash(&self, source: &Path, temporary: &Path) -> Result<(u64, [u8; 32]), FileOperationError>;
    async fn rename(&self, source: &Path, destination: &Path) -> Result<(), FileOperationError>;
    async fn remove_registered_temporary(&self, path: &Path) -> Result<(), FileOperationError>;
}
```

- [ ] **Step 2: Write failing copy integration tests**

Tests use a temporary project root and assert:

- destination bytes equal source bytes;
- destination hash equals the streaming BLAKE3 hash;
- final destination appears only after verification;
- failure before final rename leaves source intact and only a journal-registered hidden temporary;
- retry after reopen completes or safely cleans the registered temporary.

Run `cargo test --test file_transactions verified_copy`. Expected: FAIL.

- [ ] **Step 3: Implement the copy protocol**

Use a destination sibling named `.viewer-copy-<operation-uuid>.part`. Reject any temporary path not derived from the current operation ID. Execute and persist in this exact order:

```text
Prepared → create sibling temp → Staged
stream copy + BLAKE3 + sync_all → FsApplied
re-open temp, verify size and BLAKE3 → Verified
atomic rename temp to final → MetaCommitted
update disposable session index → IndexSynced
publish completion → Completed
```

The source remains unchanged. On `Cancelled`, close and remove only the registered temporary, then mark the item failed with code `cancelled`.

- [ ] **Step 4: Verify and commit**

Run:

```bash
cargo test --test file_transactions verified_copy -- --nocapture
```

Expected: all copy cases PASS.

```bash
git add crates/viewer-application crates/viewer-infrastructure crates/viewer-test-support tests/file_transactions.rs Cargo.lock
git commit -m "feat: add verified file copy protocol"
```

### Task 4: Implement atomic moves and cycle-safe batch rename

**Files:**
- Create: `crates/viewer-infrastructure/src/operation/rename.rs`
- Modify: `crates/viewer-infrastructure/src/operation/executor.rs`
- Test: `tests/file_transactions.rs`

**Interfaces:**
- Produces: `RenamePlanner::plan`, `RenameExecutor::execute`.

- [ ] **Step 1: Write failing planner tests**

Use these exact cases:

```text
A.jpg → B.jpg, B.jpg → A.jpg        => two temporary stages, then two final stages
A.jpg → C.jpg, B.jpg → C.jpg        => DuplicateDestination error
A.jpg → a.jpg on insensitive volume => case-only temporary stage then final stage
../A.jpg → B.jpg                    => OutsideProject error
```

Run `cargo test -p viewer-infrastructure rename_planner`. Expected: FAIL.

- [ ] **Step 2: Implement deterministic rename stages**

`RenamePlanner` accepts project root, actual volume case-sensitivity and `(EntityId, source, destination)` mappings. It validates every source/destination before returning:

```rust
pub enum RenameStage {
    ToTemporary { entity_id: EntityId, source: PathBuf, temporary: PathBuf },
    ToFinal { entity_id: EntityId, source: PathBuf, destination: PathBuf },
}

pub struct RenamePlan { pub stages: Vec<RenameStage> }
```

Every member of a cycle and every case-only rename on a case-insensitive volume receives a unique `.viewer-rename-<operation-id>.part` sibling. Non-cyclic, non-conflicting renames go directly to final.

- [ ] **Step 3: Execute and verify each stage**

Persist the item state before and after each stage. After rename, compare file ID where supported; otherwise compare size and stored quick fingerprint. If a later item fails, retain completed items and return a per-item batch summary.

- [ ] **Step 4: Verify and commit**

Run `cargo test --test file_transactions rename -- --nocapture`. Expected: cycles, case-only rename and partial failure PASS.

```bash
git add crates/viewer-infrastructure tests/file_transactions.rs
git commit -m "feat: add cycle-safe batch rename"
```

### Task 5: Add conflict policies, Trash and session undo

**Files:**
- Create: `crates/viewer-platform-macos/src/files/mod.rs`
- Create: `crates/viewer-platform-macos/src/files/trash.rs`
- Create: `crates/viewer-platform-macos/src/files/identity.rs`
- Modify: `crates/viewer-platform-macos/src/lib.rs`
- Create: `crates/viewer-application/src/undo.rs`
- Test: `tests/file_transactions.rs`

**Interfaces:**
- Produces: `TrashPort::trash`, `VolumePort::{volume_id,is_case_sensitive}`, `UndoStack`.

- [ ] **Step 1: Write failing conflict and undo tests**

Verify:

- `Skip` performs no mutation;
- `KeepBoth` selects `name copy.jpg`, then `name copy 2.jpg` deterministically;
- `Replace` calls Trash on the existing destination before the source is placed;
- copy and Trash never enter `UndoStack`;
- rename and in-project move enter one batch-level undo item and revalidate identity before execution.

- [ ] **Step 2: Implement macOS ports**

Wrap `trash-rs` behind:

```rust
#[async_trait]
pub trait TrashPort: Send + Sync {
    async fn trash(&self, path: &Path) -> Result<(), FileOperationError>;
}
```

Do not expose trash-rs types outside `viewer-platform-macos`. Use platform metadata to supply volume and stable file IDs when available.

- [ ] **Step 3: Implement conflict and undo behavior**

`Replace` transitions only after the existing destination successfully reaches Trash. If placement then fails, recovery reports that the previous destination is in system Trash and leaves the source intact or at its registered staged path.

`UndoStack` stores reverse plans in memory by session. It clears on session close and refuses an undo whose current source/destination identity differs from the recorded expected snapshots.

- [ ] **Step 4: Verify and commit**

Run `cargo test --test file_transactions conflict undo -- --nocapture`. Expected: all cases PASS; tests use a fake Trash port and never modify the developer's real Trash.

```bash
git add crates/viewer-platform-macos crates/viewer-application tests/file_transactions.rs Cargo.lock
git commit -m "feat: add file conflict and undo policies"
```

### Task 6: Add deterministic fault injection and recovery

**Files:**
- Create: `crates/viewer-test-support/src/faults.rs`
- Create: `crates/viewer-infrastructure/src/operation/recovery.rs`
- Modify: `crates/viewer-infrastructure/src/operation/mod.rs`
- Test: `tests/file_transactions.rs`

**Interfaces:**
- Produces: `FaultInjector`, `NoFaults`, `FailAfterState`, `RecoveryService::recover_project`.

- [ ] **Step 1: Define the fault port**

```rust
pub trait FaultInjector: Send + Sync {
    fn after_persist(&self, operation_id: OperationId, state: OperationState) -> Result<(), InjectedCrash>;
}

pub struct NoFaults;
pub struct FailAfterState(pub OperationState);
```

Call it immediately after every journal state commit in test builds. Production composition uses `NoFaults`.

- [ ] **Step 2: Write a matrix test for every persisted state**

For copy, rename, move and replace, execute once with `FailAfterState` at each applicable state, drop all services without cleanup, reopen the project, run recovery twice, and assert idempotence:

```rust
let first = recovery.recover_project(&root).await.unwrap();
let second = recovery.recover_project(&root).await.unwrap();
assert!(second.actions.is_empty());
assert_project_has_no_unknown_data_loss(&root, &original_manifest);
```

Run `cargo test --test file_transactions recovery_matrix`. Expected: FAIL before recovery exists.

- [ ] **Step 3: Implement evidence-based recovery**

Recovery may automatically:

- remove a registered temp whose source still exists and whose hash is incomplete;
- finish final rename when the verified temp matches stored size/hash and final is absent;
- commit metadata when final target exists with expected identity;
- rescan indexes after filesystem truth is established.

Recovery must stop and return `NeedsUserReview` if both source and final exist with conflicting contents, an unknown file occupies the final path, or stored evidence matches multiple candidates.

- [ ] **Step 4: Verify and commit**

Run `cargo test --test file_transactions recovery_matrix -- --nocapture`. Expected: complete state matrix PASS and second recovery is a no-op.

```bash
git add crates/viewer-test-support crates/viewer-infrastructure tests/file_transactions.rs
git commit -m "feat: recover interrupted file operations"
```

### Task 7: Record G2 gate evidence

**Files:**
- Create: `scripts/run-g2-file-transaction-gate.sh`
- Create: `docs/adr/0002-file-transaction-protocol.md`
- Modify: `docs/TECHNICAL_FOUNDATIONS.md`

**Interfaces:**
- Produces: reproducible file-operation safety report.

- [ ] **Step 1: Create the gate script**

The script creates a disposable project on the local SSD, runs unit tests, real-filesystem integration tests, the recovery matrix and a manual real-Trash smoke test that requires explicit `VIEWER_ALLOW_REAL_TRASH_TEST=1`.

Without that variable, the script skips only the real-Trash smoke test and still runs the fake-port contract test. CI must never set the variable.

- [ ] **Step 2: Run the gate**

Run:

```bash
./scripts/run-g2-file-transaction-gate.sh
```

Expected: no unexplained file loss, no unregistered temporary files, all recovery reruns idempotent, and every partial batch produces a correct summary.

- [ ] **Step 3: Write ADR and commit**

ADR 0002 records state transitions, filesystem atomicity limits, replace-to-Trash behavior, recovery decision table and test evidence.

```bash
git add scripts/run-g2-file-transaction-gate.sh docs/adr/0002-file-transaction-protocol.md docs/TECHNICAL_FOUNDATIONS.md
git commit -m "docs: record G2 file transaction protocol"
```

## G2 Completion Check

Run:

```bash
./scripts/run-g2-file-transaction-gate.sh
git status --short
```

Expected: gate exits 0, ADR includes the complete recovery decision table, and the working tree is clean.

