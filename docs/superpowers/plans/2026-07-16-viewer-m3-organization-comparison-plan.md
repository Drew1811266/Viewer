# Viewer M3 Organization and Comparison Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver Viewer M3: safe single/batch file organization, session undo, two-to-four-image comparison, native/internal drag, external-change reconciliation, and complete read-only/lifecycle behavior.

**Architecture:** M3 composes the accepted G2 transaction and G3 Watcher primitives behind one serial Rust write lane. Portable metadata is committed before the disposable session projection, Watcher events only request targeted disk reconciliation, and React sends opaque entity-ID intents while owning dialogs and compare transforms. The existing restricted image protocol supplies compare proxies; no source path or broad capability enters the frontend.

**Tech Stack:** Rust stable, Tauri 2, React, TypeScript, SQLite/rusqlite, Tokio, BLAKE3, notify/FSEvents, Darwin `renamex_np`, AppKit drag/pasteboard, macOS Trash, Vitest.

## Global Constraints

- Build only `aarch64-apple-darwin`; minimum macOS is 13.0.
- Supported project content remains JPG/JPEG, PNG, Markdown and TXT at arbitrary folder depth.
- One process/window owns at most one active project and one serial file-operation batch.
- React receives entity IDs, safe relative paths and opaque image URLs only; no absolute project/cache path or original bytes cross IPC.
- The frontend receives no filesystem, shell, database, network or updater capability.
- `.viewer` is reserved and may contain only versioned portable identity, metadata/journal SQLite and approved migration backup/SQLite sidecar files.
- Rename/move retain markers; copies start unmarked; Trash is macOS Trash and is never Viewer-undoable.
- No operation silently overwrites. Conflicts are `skip`, `keep_both` or `replace`, and replace moves the old destination to Trash first.
- Watcher events are hints; filesystem re-read is truth and stale session/generation work never publishes.
- Read-only projects allow browse/search/preview/info/compare and reject every marker or filesystem mutation in both UI and Rust.
- Compare admits exactly 2–4 images and uses viewport proxies by default; four original decodes are never unconditional.
- No new direct dependency is accepted without an Apache-2.0-compatible license review, repository policy update and notice update.
- Every product change follows red-green-refactor, a focused verification command and a small commit.

---

## Requirement ownership

| Requirement | M3 task ownership |
| --- | --- |
| `REQ-FLOW-ORGANIZE` | Tasks 4–6, 9–11 |
| `REQ-FLOW-BATCH-RENAME` | Tasks 3, 6, 11 |
| `REQ-FLOW-DRAG-DROP` | Task 12 |
| `REQ-FLOW-UNDO` | Task 7 |
| `REQ-FLOW-SHORTCUTS` (M3) | Tasks 11, 14, 15 |
| `REQ-FLOW-COMPARE` | Tasks 13–14 |
| `REQ-FLOW-EXTERNAL-CHANGES` | Tasks 8–10, 15 |
| `REQ-FLOW-READONLY-ERRORS` | Tasks 9, 15 |
| `REQ-FLOW-LIFECYCLE` (file tasks/Watcher) | Tasks 9, 15 |
| `REQ-IA-SURFACES`, `REQ-IA-IMAGE-PREVIEW`, `REQ-IA-TASK-BAR` (M3) | Tasks 10–15 |
| `REQ-TECH-FILE-CONSISTENCY` | Tasks 2, 4–9 |
| `REQ-TECH-WATCHER` | Task 8 |
| `REQ-TECH-PATH-SECURITY` (M3) | Tasks 3–9, 12, 16 |
| `REQ-TECH-MEMORY`, `REQ-RELEASE-ACCEPTANCE` (M3) | Tasks 13–18 |

### Task 1: Freeze the M3 execution baseline

**Files:**
- Create: `docs/superpowers/plans/2026-07-16-viewer-m3-organization-comparison-plan.md`
- Modify: `docs/superpowers/plans/2026-07-16-viewer-0.1-roadmap.md`
- Modify: `docs/superpowers/plans/2026-07-16-viewer-m2-review-efficiency-plan.md`

**Interfaces:**
- Consumes: accepted M3 design, M2 gate and merged main commit `7b12cf2`.
- Produces: executable M3 task order and requirement mapping.

- [x] **Step 1: Verify the isolated branch starts at merged M2**

Run: `git merge-base --is-ancestor main HEAD && pnpm install --frozen-lockfile && pnpm gate:m2`

Expected: exact M2 gate passes with 46 UI tests, all locked Rust tests/security/license checks and the 20-run G3 benchmark.

- [x] **Step 2: Record frozen decisions and task ownership**

Require one Rust write lane, truthful journal barriers, project-internal destinations, copy-only Finder export, session-only undo, 2–4 proxy-first compare, targeted disk reconciliation and dual-layer read-only enforcement. Map every table row above to at least one focused test task and one packaged-app acceptance step.

- [x] **Step 3: Commit the M3 plan**

```bash
git add docs/superpowers/plans/2026-07-16-viewer-m3-organization-comparison-plan.md docs/superpowers/plans/2026-07-16-viewer-0.1-roadmap.md docs/superpowers/plans/2026-07-16-viewer-m2-review-efficiency-plan.md
git commit -m "docs: plan M3 organization and comparison"
```

### Task 2: Upgrade portable operations to schema v3 and truthful barriers

**Files:**
- Create: `crates/viewer-infrastructure/migrations/portable/0003_operation_results.sql`
- Create: `crates/viewer-application/src/operation_commit.rs`
- Modify: `crates/viewer-application/src/{lib,operation,ports}.rs`
- Modify: `crates/viewer-infrastructure/src/portable/schema.rs`
- Modify: `crates/viewer-infrastructure/src/operation/{journal,executor,rename,conflict,recovery}.rs`
- Test: `tests/m3_operation_journal.rs`
- Test: `tests/file_transactions.rs`

**Interfaces:**
- Produces: `OperationCommit { operation_id, entity_id, kind, source, destination }`, `OperationCommitPort::{commit_metadata,sync_index}`, batch lifecycle/count/result-code journal APIs, schema v3 and `metadata.sqlite.v2.bak`.

- [x] **Step 1: Write failing migration and barrier tests**

Cover v2→v3 one-time backup/migration, exact schema rejection, batch requested/completed/failed/skipped counts, stable result codes, and fault injection immediately before/after real metadata and index callbacks. Assert `MetaCommitted` is impossible before `commit_metadata` succeeds and `IndexSynced` is impossible before `sync_index` succeeds.

Run: `cargo test --test m3_operation_journal -- --nocapture`

Expected: RED because schema v3 and commit callbacks do not exist and G2 executors currently advance logical barriers internally.

- [x] **Step 2: Add schema v3 and strict journal APIs**

Add batch `state`, counts and timestamps plus item `result_code` with database CHECK constraints. Keep DELETE/FULL/foreign keys, exclusive synced backup creation and exact version sequence `[1,2,3]`. Expose `begin_batch`, `complete_item`, `fail_item`, `skip_item`, `finish_batch` and paged result reads; reject invalid counters/transitions.

- [x] **Step 3: Make commit barriers truthful**

Refactor filesystem executors so verified placement calls `OperationCommitPort::commit_metadata`, advances `Verified → MetaCommitted`, calls `sync_index`, advances `MetaCommitted → IndexSynced`, then completes. Recovery uses the same port. A failed callback leaves the last truthful durable state for replay and never reports completion.

- [x] **Step 4: Verify G2 recovery remains exact**

Run: `cargo test --test m3_operation_journal && cargo test --test file_transactions && ./scripts/run-g2-file-transaction-gate.sh`

Expected: all seven-state/four-operation crash cases and second-run idempotence pass with real barrier fakes.

- [x] **Step 5: Commit**

```bash
git add crates/viewer-application crates/viewer-infrastructure tests/m3_operation_journal.rs tests/file_transactions.rs
git commit -m "feat: make operation barriers truthful"
```

### Task 3: Implement exact rename rules and preflight

**Files:**
- Create: `crates/viewer-application/src/rename.rs`
- Modify: `crates/viewer-application/src/lib.rs`
- Modify: `crates/viewer-domain/src/operation.rs`
- Modify: `crates/viewer-infrastructure/src/operation/rename.rs`
- Test: `tests/m3_rename_preflight.rs`

**Interfaces:**
- Produces: `RenameRuleSet { find, replacement, prefix, suffix, sequence }`, `SequenceRule { start, digits }`, `RenamePreviewRow`, `RenamePreflight`, `preview_rename`, and platform `name_max` validation through `VolumePort`.

- [x] **Step 1: Write failing rule/validation matrix**

Cover literal replace, prefix, suffix-before-extension, fixed-order composition, visible-order numbering, start 0/maximum, 1–6 digits, dotfiles/multiple extensions, Unicode, no-op, empty/`.`/`..`, slash/NUL, reserved/temp names, volume name limit, same-batch duplicate, case-fold collision, occupied target, cycles and case-only rename.

Run: `cargo test --test m3_rename_preflight -- --nocapture`

Expected: RED because no product rename preflight exists.

- [x] **Step 2: Implement pure deterministic preview**

Take ordered current nodes plus rules and return every old/new relative path without filesystem mutation. Each row has stable error codes; any error makes `executable=false`. Preserve extensions according to the design and never infer order from a hash map or directory enumeration.

- [x] **Step 3: Connect volume/collision preflight**

Revalidate canonical parents, writable destination, `_PC_NAME_MAX`, case sensitivity, existing occupants, source uniqueness and `.viewer`/hidden-temporary boundaries. Pass valid mappings to the accepted cycle/case staging planner.

- [x] **Step 4: Verify GREEN**

Run: `cargo test --test m3_rename_preflight && cargo test --test file_transactions rename && cargo clippy --locked --workspace --all-targets -- -D warnings`

- [x] **Step 5: Commit**

```bash
git add crates/viewer-domain crates/viewer-application crates/viewer-infrastructure tests/m3_rename_preflight.rs
git commit -m "feat: preflight single and batch rename"
```

### Task 4: Add portable marker-path and session-index mutation projections

**Files:**
- Modify: `crates/viewer-application/src/metadata.rs`
- Modify: `crates/viewer-infrastructure/src/portable/markers.rs`
- Modify: `crates/viewer-infrastructure/src/search/index.rs`
- Test: `tests/m3_operation_projections.rs`

**Interfaces:**
- Produces: `PortableMetadataPort::move_paths`, `OperationProjectionPort::{apply_copy,apply_move,apply_trash}`, and atomic `SessionIndex` operation projections.

- [x] **Step 1: Write failing metadata/index projection tests**

Prove rename/move retain marker UUID/state/favorite and update paths atomically, copy adds a new unmarked node, Trash removes session/FTS rows but leaves a dormant portable marker at its old path, directory/subtree moves rewrite descendant marker paths, duplicate destinations roll back and absolute/reserved paths are rejected.

Run: `cargo test --test m3_operation_projections -- --nocapture`

- [x] **Step 2: Implement portable path transactions**

Use one IMMEDIATE transaction to validate every old/new relative path, detect case/duplicate collisions and update all matching marker rows. Never create a marker for an unmarked copy and never write a source file.

- [x] **Step 3: Implement disposable projection transactions**

Move/rename preserves entity ID and derived values when evidence still matches. Copy inserts the verified destination as a fresh node and schedules derivation. Trash removes the node/subtree and FTS rows. Every projection can be rebuilt from disk when commit returns stale.

- [x] **Step 4: Verify GREEN and M2 hydration**

Run: `cargo test --test m3_operation_projections && cargo test --test m2_portable_metadata && cargo test --test m2_session_projection`

- [x] **Step 5: Commit**

```bash
git add crates/viewer-application crates/viewer-infrastructure tests/m3_operation_projections.rs
git commit -m "feat: project file operation projections"
```

### Task 5: Build the serial batch command service

**Files:**
- Create: `crates/viewer-application/src/file_commands.rs`
- Modify: `crates/viewer-application/src/{lib,operation,ports}.rs`
- Modify: `crates/viewer-domain/src/operation.rs`
- Test: `tests/m3_command_service.rs`

**Interfaces:**
- Produces: `FileCommand`, `FileCommandKind`, `BatchId`, `BatchProgress`, `BatchSummary`, `BatchItemResult`, `FileCommandService::{preflight,execute,cancel_pending}`, and one-active-batch admission.

- [x] **Step 1: Write failing service-state tests**

Cover read-only rejection, stale session/generation, empty/duplicate/over-10,000 targets, one active batch, ordered preflight, per-item success/skip/failure, apply-to-remaining conflict choice, cancellation before start, active-item completion, coalesced progress and batch final counts.

Run: `cargo test --test m3_command_service -- --nocapture`

- [x] **Step 2: Implement command/result domain contracts**

Model queued/running/cancelling/completed lifecycle and `completed + failed + skipped + cancelled == requested`. Item results contain entity ID, safe relative path, status and stable code only. Page detail at 200 rows.

- [x] **Step 3: Implement single-writer admission and cancellation**

Serialize batches with a Tokio mutex owned by the active project session. Revalidate session/access before preflight and again before each item. Cancellation marks queued items cancelled and signals stream-copy cancellation; no completed item is reversed.

- [x] **Step 4: Verify GREEN**

Run: `cargo test --test m3_command_service && cargo test -p viewer-application undo && cargo fmt --check`

- [x] **Step 5: Commit**

```bash
git add crates/viewer-domain crates/viewer-application tests/m3_command_service.rs
git commit -m "feat: coordinate serial file command batches"
```

### Task 6: Compose rename, copy, move, conflict and Trash execution

**Files:**
- Create: `crates/viewer-infrastructure/src/operation/service.rs`
- Modify: `crates/viewer-infrastructure/src/operation/{mod,copy,executor,rename,conflict,recovery}.rs`
- Modify: `crates/viewer-platform-macos/src/files/{mod,identity,trash}.rs`
- Test: `tests/m3_file_commands.rs`

**Interfaces:**
- Produces: `LocalFileCommandPort`, same-volume atomic move, cross-volume verified-copy-then-Trash, copy conflict execution, safe Trash batch and real operation progress callbacks.

- [x] **Step 1: Write failing end-to-end operation matrix**

Use disposable projects to cover single/batch rename, cycle/case rename, copy/move to deep folders, same/cross-volume routing fakes, corrupt-but-regular source operations, Skip/KeepBoth/Replace, destination race, permission loss, disk/write/copy failure, cancellation, partial success and Trash adapter fakes.

Run: `cargo test --test m3_file_commands -- --nocapture`

- [x] **Step 2: Implement rename/copy/move routing**

Resolve entity IDs immediately before work. Use `MacVolumePort` to choose atomic rename or verified copy plus source Trash. Extend replace to copy without overwriting. Register deterministic temporaries and expected changes before mutation.

- [x] **Step 3: Implement Trash and safe results**

Trash only validated supported regular files under the root. Record intent before calling `MacTrashPort`, verify source disappearance, commit portable/session projections and return `moved_to_trash`. Never expose the resulting Trash location or offer permanent delete.

- [x] **Step 4: Verify GREEN and the fault matrix**

Run: `cargo test --test m3_file_commands && cargo test --test file_transactions && ./scripts/run-g2-file-transaction-gate.sh`

- [x] **Step 5: Commit**

```bash
git add crates/viewer-infrastructure crates/viewer-platform-macos tests/m3_file_commands.rs
git commit -m "feat: execute recoverable file command batches"
```

### Task 7: Integrate session undo for markers and file moves

**Files:**
- Modify: `crates/viewer-application/src/{metadata,undo,file_commands}.rs`
- Modify: `src-tauri/src/state.rs`
- Test: `tests/m3_undo.rs`
- Test: `tests/m2_desktop_runtime.rs`

**Interfaces:**
- Produces: `UndoStack` recording committed marker/favorite/rename/move batches, `UndoService::undo_last`, batch prevalidation and close reset.

- [x] **Step 1: Write failing undo matrix**

Cover single/batch review, favorite, rename and move; LIFO order; mixed partial file results; occupied restore; identity/path drift; symlink/alias/root escape; copy/Trash exclusion; failed reverse action; close/reopen reset and read-only rejection.

Run: `cargo test --test m3_undo -- --nocapture`

- [x] **Step 2: Record prior marker values after durable commit**

M2 marker commands capture every previous marker, write portable truth, sync the index, then append one undo batch. Undo uses one inverse marker batch and does not recursively create another undo entry.

- [x] **Step 3: Execute file undo through the same coordinator**

Prevalidate the complete top batch before mutation. Reverse only completed rename/move items with matching snapshots; retain the stack entry on validation failure. Successful inverse moves use journal/expected-change/projection paths and consume the entry.

- [x] **Step 4: Verify GREEN**

Run: `cargo test --test m3_undo && cargo test -p viewer-desktop --test m2_desktop_runtime marker && cargo clippy --locked --workspace --all-targets -- -D warnings`

- [x] **Step 5: Commit**

```bash
git add crates/viewer-application src-tauri/src/state.rs tests/m3_undo.rs tests/m2_desktop_runtime.rs
git commit -m "feat: undo session review and file moves"
```

### Task 8: Implement targeted Watcher truth reconciliation

**Files:**
- Create: `crates/viewer-infrastructure/src/scan/reconcile_service.rs`
- Modify: `crates/viewer-application/src/watcher.rs`
- Modify: `crates/viewer-infrastructure/src/scan/{mod,reconcile,walker}.rs`
- Modify: `crates/viewer-infrastructure/src/search/index.rs`
- Modify: `crates/viewer-infrastructure/src/portable/markers.rs`
- Modify: `crates/viewer-platform-macos/src/watcher.rs`
- Test: `tests/m3_watcher_runtime.rs`
- Test: `tests/watcher_reconcile.rs`

**Interfaces:**
- Produces: `ProjectReconciler::reconcile(ReconcileRequest) -> ReconcileSummary`, project-relative subtree walking, atomic index delta, same-session identity marker relocation and derived invalidation.

- [x] **Step 1: Write failing real-filesystem Watcher scenarios**

Cover create, partial-write then finalize, image/text modify, rename, move, delete, directory subtree move, burst coalescing, Viewer expected changes, wrong identity, hidden/reserved/symlink/alias exclusion, overflow root minimization, stale generation, active close and marker relocation by device/inode.

Run: `cargo test --test m3_watcher_runtime -- --nocapture`

- [x] **Step 2: Implement safe project-relative subtree snapshots**

Walk only requested roots while deriving paths against the canonical project root. Return folders before files, supported regular items only and isolated per-item failures. Never follow a link/alias or enumerate `.viewer`.

- [x] **Step 3: Compute and commit one identity-aware delta**

Compare indexed/current nodes by entity ID first and exact path second. In one index transaction, update renamed paths, remove vanished node/FTS rows, add nodes and reset derived statuses for changed sources. Move portable markers only for an unambiguous same-session identity; M4 retains cross-reopen fingerprint recovery.

- [x] **Step 4: Verify GREEN and G3 regression**

Run: `cargo test --test m3_watcher_runtime && cargo test --test watcher_reconcile && ./scripts/run-g3-scan-search-gate.sh`

- [x] **Step 5: Commit**

```bash
git add crates/viewer-application crates/viewer-infrastructure crates/viewer-platform-macos tests/m3_watcher_runtime.rs tests/watcher_reconcile.rs
git commit -m "feat: reconcile external project changes"
```

### Task 9: Expose narrow M3 desktop runtime commands

**Files:**
- Create: `src-tauri/src/commands/operations.rs`
- Create: `src-tauri/src/operation_runtime.rs`
- Create: `src-tauri/src/watcher_runtime.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/{lib,state,dto,error}.rs`
- Test: `tests/m3_desktop_runtime.rs`
- Test: `tests/security_boundaries.rs`

**Interfaces:**
- Produces: `preview_rename`, `execute_file_command`, `operation_status`, `operation_results`, `cancel_operation`, `undo_last_operation`, `open_permission_settings`, `viewer://operation-progress`, `viewer://project-changed` and `viewer://close-blocked`.

- [x] **Step 1: Write failing DTO/runtime/security tests**

Cover exact camelCase/deny-unknown DTOs, session/generation/access validation, target/order bounds, destination folder validation, conflict enums, one active batch, progress/result pagination, cancellation, undo, Watcher start/drop, close wait/cancel choices, recovery reports and safe error redaction.

Run: `cargo test -p viewer-desktop --test m3_desktop_runtime -- --nocapture`

- [x] **Step 2: Split operation and Watcher runtime ownership**

Keep `DesktopRuntime` as session owner but delegate write-lane and Watcher loops to focused modules. `DesktopSession` owns journal/store/index, undo, cancellation, operation status, expected ledger and Watcher subscription. Open performs safe recovery before active publication; close stops new commands before dropping Watcher/resources.

- [x] **Step 3: Register only Viewer-owned commands/events**

No raw path command is accepted. Destination folders are current-session entity IDs; rename preview names are validated in Rust. Errors contain stable code/category/message/retry/task/item and no absolute path, SQLite text, hash or temporary name.

- [x] **Step 4: Verify GREEN and security boundaries**

Run: `cargo test -p viewer-desktop --test m3_desktop_runtime && cargo test -p viewer-desktop --test security_boundaries && ./scripts/check-tauri-security.sh`

- [x] **Step 5: Commit**

```bash
git add src-tauri tests/m3_desktop_runtime.rs tests/security_boundaries.rs
git commit -m "feat: expose safe file operation commands"
```

### Task 10: Add frontend operation and reconciliation state

**Files:**
- Modify: `ui/src/api/{types,viewer}.ts`
- Modify: `ui/src/state/{viewerReducer,useViewerController}.ts`
- Test: `ui/src/state/{viewerReducer,useViewerController}.test.tsx`

**Interfaces:**
- Produces: exact M3 bridge, operation/dialog/recovery state, current task/results, external-change context repair and close coordination intents.

- [x] **Step 1: Write failing reducer/controller tests**

Cover command preflight/execute, stale operation/project events, task progress/results, cancellation, projection/search refresh after mutation, selected entity preservation by ID, deleted selection/preview repair intents, compare removal intents, error summaries, read-only suppression and project-close reset.

Run: `pnpm --dir ui test -- src/state/viewerReducer.test.ts src/state/useViewerController.test.tsx`

- [x] **Step 2: Implement exact bridge and session-only state**

Add no path-bearing API. Operation events apply only to the current session/generation/task. Successful results trigger one projection/search refresh; old responses cannot reopen dialogs or alter a new session. Project close clears tasks, conflicts, recovery and operation selections.

- [x] **Step 3: Implement active-context repair**

For `project-changed`, keep live entity IDs, remove confirmed missing IDs and choose the nearest surviving file using the prior ordered workspace. Emit one concise message only when active selection/preview/compare changed.

- [x] **Step 4: Verify GREEN**

Run: `pnpm --dir ui test && pnpm --dir ui build`

- [x] **Step 5: Commit**

```bash
git add ui/src/api ui/src/state
git commit -m "feat: coordinate organization workspace state"
```

### Task 11: Build operation actions, dialogs and task results

**Files:**
- Create: `ui/src/components/FileActionToolbar.tsx`
- Create: `ui/src/components/RenameDialog.tsx`
- Create: `ui/src/components/BatchRenameDialog.tsx`
- Create: `ui/src/components/DestinationDialog.tsx`
- Create: `ui/src/components/TrashConfirmation.tsx`
- Create: `ui/src/components/OperationResults.tsx`
- Create matching `*.test.tsx` files
- Modify: `ui/src/{App.tsx,styles/app.css}`
- Modify: `ui/src/components/{ContentBrowser,TaskBar}.tsx`

**Interfaces:**
- Produces: selection-aware actions, virtualized full rename preview, destination/conflict choices, safe Trash confirmation, paged results and Enter/Delete/Command-Z behavior.

- [x] **Step 1: Write failing interaction/accessibility tests**

Cover button capability matrix, single vs batch rename, full preview/error focus, destination tree, per-item/apply-rest conflict policy, cancel, partial result summary, corrupt-file actions, task bar progress, Enter/Delete/Command-Z, focus trap/restore and text-input shortcut suppression.

Run: `pnpm --dir ui test -- src/components/FileActionToolbar.test.tsx src/components/RenameDialog.test.tsx src/components/BatchRenameDialog.test.tsx src/components/DestinationDialog.test.tsx src/components/TrashConfirmation.test.tsx src/components/OperationResults.test.tsx`

- [x] **Step 2: Implement transient operation surfaces**

Keep dialogs inside the right workspace/overlay layer and the two-column shell intact. Batch preview renders every mapping through a virtual list. Validation blocks Execute and identifies exact rows. Trash copy says restoration uses macOS Trash; no permanent-delete control exists.

- [x] **Step 3: Wire commands and safe keyboard behavior**

Actions use current selected entity IDs in visible order. Disable while read-only/closing/another batch is active. Command-Z calls undo only outside editable/content-selection/modal controls; Delete always opens confirmation and never executes immediately.

- [x] **Step 4: Verify GREEN**

Run: `pnpm --dir ui test && pnpm --dir ui build`

- [x] **Step 5: Commit**

```bash
git add ui/src
git commit -m "feat: add safe organization dialogs"
```

### Task 12: Implement internal drag and native Finder copy export

**Files:**
- Create: `crates/viewer-application/src/finder_drag.rs`
- Create: `crates/viewer-platform-macos/src/files/drag.rs`
- Create: `src-tauri/src/commands/finder_drag.rs`
- Modify: `crates/viewer-application/src/{lib,ports}.rs`
- Modify: `crates/viewer-platform-macos/src/files/mod.rs`
- Modify: `src-tauri/src/{commands/mod,lib,state}.rs`
- Modify: `ui/src/components/{ContentBrowser,FolderTree}.tsx`
- Test: `tests/m3_drag_export.rs`
- Test: `ui/src/components/ContentBrowser.test.tsx`
- Test: `ui/src/components/FolderTree.test.tsx`

**Interfaces:**
- Produces: `FinderDragPort`, `begin_finder_drag(entity_ids)`, entity-only HTML drag payload, move/Option-copy target state and drop-to-command equivalence.

- [x] **Step 1: Write failing internal-drag tests**

Cover multi-selection payload, default move, Option-copy at drop, valid target highlight, self/descendant/read-only/reserved/outside rejection, drop command equivalence and stale selection suppression.

- [x] **Step 2: Implement entity-only internal drag**

The frontend transfer holds a Viewer MIME marker and in-memory entity IDs only, never paths. Folder targets calculate visual validity but Rust revalidates every drop. Dropping calls the same destination preflight/execute controller used by dialogs.

- [x] **Step 3: Implement AppKit Finder export**

Resolve and canonicalize current-session files in Rust, obtain the WKWebView native view on the main thread, and start an AppKit dragging session with file URLs and copy operation mask. Use `NSApplication.currentEvent`; reject absent mouse-drag context, directories, stale IDs, symlink/alias and outside-root paths. Add no filesystem capability and no user-supplied path command.

- [x] **Step 4: Verify automated and opt-in real drag evidence**

Run: `cargo test --test m3_drag_export && pnpm --dir ui test -- src/components/ContentBrowser.test.tsx src/components/FolderTree.test.tsx && ./scripts/check-tauri-security.sh`

Expected: adapter/payload tests pass. A physical packaged-app drag to Finder is mandatory in Task 17.

- [x] **Step 5: Commit**

```bash
git add crates/viewer-application crates/viewer-platform-macos src-tauri ui/src/components tests/m3_drag_export.rs
git commit -m "feat: drag files within Viewer and to Finder"
```

### Task 13: Define reusable compare transform and memory contracts

**Files:**
- Create: `ui/src/state/compareModel.ts`
- Create: `ui/src/state/compareModel.test.ts`
- Modify: `crates/viewer-domain/src/image.rs`
- Test: `tests/m3_compare_budget.rs`

**Interfaces:**
- Produces: `CompareState`, `PaneTransform { scale, centerX, centerY, rotation }`, synchronized/independent reducers, 2/3/4 layouts and proxy/original admission evidence.

- [x] **Step 1: Write failing compare model tests**

Cover exactly 2–4 unique JPG/PNG IDs, layout selection, fit/100%/zoom clamp, normalized pan, synchronized propagation with per-pane clamp, independent retention, rotation, removal/reflow, one-pane single-preview fallback and zero-pane grid fallback.

Run: `pnpm --dir ui test -- src/state/compareModel.test.ts && cargo test --test m3_compare_budget -- --nocapture`

- [x] **Step 2: Implement pure compare state transitions**

Keep transforms keyed by entity ID and a separate shared normalized transform. Mode switching seeds independent panes from the shared transform or derives shared state from the active pane. Removal never resets surviving transforms.

- [x] **Step 3: Lock proxy-first memory admission**

Extend tests around existing `DecodeBudget` for four viewport proxies, denial/degradation of unsafe simultaneous originals and cancellation of removed panes. Do not add a second image cache or return source paths.

- [x] **Step 4: Verify GREEN**

Run: `pnpm --dir ui test -- src/state/compareModel.test.ts && cargo test --test m3_compare_budget && ./scripts/run-g1-image-gate.sh`

- [x] **Step 5: Commit**

```bash
git add ui/src/state/compareModel.ts ui/src/state/compareModel.test.ts crates/viewer-domain/src/image.rs tests/m3_compare_budget.rs
git commit -m "feat: define bounded comparison model"
```

### Task 14: Build the two-to-four-image compare workspace

**Files:**
- Create: `ui/src/components/CompareWorkspace.tsx`
- Create: `ui/src/components/ComparePane.tsx`
- Create: `ui/src/components/CompareWorkspace.test.tsx`
- Create: `ui/src/components/ComparePane.test.tsx`
- Modify: `ui/src/{App,styles/app}.css`
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/components/{ContentBrowser,MarkerControls}.tsx`

**Interfaces:**
- Produces: `C`/toolbar compare entry, 2-column/3-asymmetric/2×2 layouts, synchronized/independent pan/zoom, inline marker/favorite and graceful pane removal.

- [x] **Step 1: Write failing workspace interaction tests**

Cover invalid cardinality/type feedback, all layouts, actual viewport proxy requests, fit/100%/zoom/pan/rotation, sync toggle, keyboard focus, inline single-pane marker updates without selection loss, removal/reflow, one-pane preview transition, external deletion and read-only marker disable.

Run: `pnpm --dir ui test -- src/components/CompareWorkspace.test.tsx src/components/ComparePane.test.tsx src/App.test.tsx`

- [x] **Step 2: Implement reusable pane rendering**

Use ResizeObserver viewport dimensions and existing `requestImage` opaque URLs. Cancel/ignore stale pane requests by entity/request revision. Show a bounded-memory downgrade message when 100% cannot be supplied.

- [x] **Step 3: Integrate workspace and inline review**

Open only from 2–4 selected images. Preserve folder/grid selection underneath. Pane marker actions target that pane's entity through the existing marker command and refresh all projections. `C` is suppressed in text/editable/modal contexts.

- [x] **Step 4: Verify GREEN and UI build**

Run: `pnpm --dir ui test && pnpm --dir ui build && cargo test --test m3_compare_budget`

- [x] **Step 5: Commit**

```bash
git add ui/src tests/m3_compare_budget.rs
git commit -m "feat: compare two to four images"
```

### Task 15: Complete read-only, permission and close lifecycle UX

**Files:**
- Create: `ui/src/components/ReadOnlyBanner.tsx`
- Create: `ui/src/components/CloseOperationDialog.tsx`
- Create matching `*.test.tsx` files
- Create: `crates/viewer-platform-macos/src/settings.rs`
- Modify: `crates/viewer-platform-macos/src/lib.rs`
- Modify: `src-tauri/src/{lib,state}.rs`
- Modify: `ui/src/{App.tsx,styles/app.css}`
- Modify: `ui/src/state/{viewerReducer,useViewerController}.ts`
- Test: `tests/m3_readonly_lifecycle.rs`

**Interfaces:**
- Produces: fixed Privacy & Security opener, reselect-directory flow, full read-only capability matrix and wait/cancel-pending/stay close coordination.

- [x] **Step 1: Write failing dual-layer capability tests**

For read-only with/without `.viewer`, cover browse/search/filter/sort/image/text/info/compare allowed; marker/rename/copy/move/Trash/undo/internal drop rejected; no metadata created/changed; banner persistent; settings/reselect direct-click actions; unreadable root safe failure.

- [x] **Step 2: Implement fixed permission/reselect actions**

The settings command opens only the compile-time macOS Privacy & Security destination and accepts no URL/path. Reselect ends the session through normal cleanup and shows the empty picker surface; it does not remember the old path.

- [x] **Step 3: Coordinate close with the write lane**

When an operation is active, native/app close emits wait/cancel-pending/stay. Wait closes after idle. Cancel stops queued/cancellable copy work then closes after the active atomic step. Stay leaves session unchanged. Stop Watcher before index/cache teardown; clear undo and all frontend state.

- [x] **Step 4: Verify GREEN**

Run: `cargo test --test m3_readonly_lifecycle && pnpm --dir ui test && pnpm --dir ui build && ./scripts/check-tauri-security.sh`

- [x] **Step 5: Commit**

```bash
git add crates/viewer-platform-macos src-tauri ui/src tests/m3_readonly_lifecycle.rs
git commit -m "feat: complete read-only and close lifecycle"
```

### Task 16: Add the exact M3 gate and schema-v3 validator

**Files:**
- Create: `scripts/run-m3-organization-gate.sh`
- Create: `scripts/m3-portable-metadata.test.mjs`
- Modify: `scripts/validate-m2-portable-metadata.mjs`
- Modify: `scripts/repository-policy.test.mjs`
- Modify: `scripts/check-scope-coverage.mjs`
- Modify: `package.json`
- Test: all M3 suites

**Interfaces:**
- Produces: `pnpm gate:m3`, exact schema-v3 portable allow-list and frozen M3 dependency/capability/scope policy.

- [x] **Step 1: Write failing gate/validator policy tests**

Generate valid/invalid v3 projects. Accept only current manifest/database, exact SQLite sidecars and prior-version backup. Validate exact tables/columns/enums/transitions/counters/relative paths/result codes. Reject originals, text bodies, thumbnails/proxies, absolute/cache paths, unknown tables/files/symlinks and unreviewed licenses.

Run: `node --test scripts/m3-portable-metadata.test.mjs`

Expected: RED because validator expects v2 and no M3 gate exists.

- [x] **Step 2: Implement the exact aggregate gate**

`pnpm gate:m3` runs inherited M2, UI tests/build, fmt, strict Clippy, all locked workspace tests, every M3 suite, G2 fault matrix, G3 Watcher/search benchmark, repository/dependency/license/audit/security/scope checks and portable schema-v3 live validation.

- [x] **Step 3: Verify two consecutive clean passes**

Run: `pnpm gate:m3 && pnpm gate:m3`

Expected: both runs exit 0 with no stale artifact substitution.

- [x] **Step 4: Commit**

```bash
git add scripts package.json
git commit -m "test: gate M3 organization and comparison"
```

### Task 17: Perform packaged-app M3 acceptance

**Files:**
- Create: `docs/reviews/2026-07-16-m3-organization-comparison-review.md`
- Modify: `docs/superpowers/plans/2026-07-16-viewer-m3-organization-comparison-plan.md`

- [x] **Step 1: Build and inspect the release artifacts**

Run: `pnpm build:macos`

Require arm64 only, macOS 13.0 minimum, valid `.app`/DMG, ad-hoc signing explicitly recorded, and no updater/telemetry/network capability.

- [x] **Step 2: Exercise real file operations on a disposable project**

Using the packaged app, cover single/batch rename rules including cycle/case, copy/move to deep folders, all conflict policies, partial permission failure, active copy cancellation, corrupt-file operations, real Trash and system restoration. Hash source/destination evidence and verify no silent overwrite/data loss.

- [x] **Step 3: Exercise undo, drag and comparison**

Cover marker/favorite/rename/move Command-Z and unsafe-undo refusal; internal move and Option-copy target feedback; physical copy-only drag into Finder; 2/3/4 compare layouts, sync/independent pan/zoom, inline markers, removal and one-pane fallback.

Packaged undo and comparison behavior passed. Drag behavior passed the complete UI/Rust/AppKit contract suite, but Computer Use could not emit the WebView HTML5 `dragstart` event; the review records the physical Finder gesture as an explicit M4 human release check rather than claiming GUI evidence.

- [x] **Step 4: Exercise external changes, read-only and lifecycle**

From Finder, create/modify/rename/move/delete while Viewer is open and verify tree/grid/search/preview/compare repair. Open read-only projects with/without metadata and verify the complete capability matrix. Close during an active operation through wait/cancel/stay paths and confirm Watcher/cache/undo teardown.

- [x] **Step 5: Validate portable/recovery/privacy boundaries**

Validate copied project identity/markers, schema-v3 journal/results, a fault-injected recoverable batch, cache deletion, `.viewer` content allow-list, no absolute/text/original/proxy content and no release-process TCP/UDP socket.

- [x] **Step 6: Record evidence and defects**

Map every M3 requirement to automated and GUI evidence. Critical/Important defects block approval, return to the relevant TDD task and rerun the exact M3 gate.

### Task 18: Final M3 review, approval and local merge

**Files:**
- Modify: `docs/reviews/2026-07-16-m3-organization-comparison-review.md`
- Modify: `docs/superpowers/plans/2026-07-16-viewer-0.1-roadmap.md`
- Modify: `docs/superpowers/plans/2026-07-16-viewer-m3-organization-comparison-plan.md`

- [ ] **Step 1: Run fresh verification-before-completion evidence**

Run: `git status --short && pnpm gate:m3 && pnpm build:macos`

Expected: clean start, exact gate pass and fresh valid arm64/macOS 13 artifacts.

- [ ] **Step 2: Review the complete branch diff**

Run: `git diff --check main...HEAD && git diff --stat main...HEAD && git log --oneline main..HEAD`

Review architecture direction, data-loss safety, durable state truth, read-only enforcement, recovery, cancellation, Watcher generations, image budget, native unsafe code, path/security/privacy boundaries, accessibility baseline, test quality and scope exclusions. No unresolved Critical/Important finding may remain.

- [ ] **Step 3: Approve and commit the stage review**

Mark M3 complete in the roadmap only after approval.

```bash
git add docs/reviews/2026-07-16-m3-organization-comparison-review.md docs/superpowers/plans/2026-07-16-viewer-0.1-roadmap.md docs/superpowers/plans/2026-07-16-viewer-m3-organization-comparison-plan.md
git commit -m "docs: approve M3 organization and comparison"
```

- [ ] **Step 4: Fast-forward merge and verify on main**

From `/Users/abc/Project/Viewer`:

```bash
git checkout main
git merge --ff-only codex/m3-organization-comparison
pnpm install --frozen-lockfile
pnpm gate:m3
```

- [ ] **Step 5: Begin M4 only after merged-main verification**

Create the M4 Internal Release design/plan/worktree from verified `main`; do not implement M4 before its plan is committed.

## M3 exit criteria

- All required organization operations are journaled, recoverable, partial-result aware and never silently overwrite/delete.
- Rename/move preserve markers and all successful filesystem changes synchronize portable/session/search/UI state.
- Marker/favorite/rename/move undo is session-only and refuses unsafe reversal; copy/Trash are excluded.
- Internal drag, Option-copy and physical Finder copy export work without frontend paths or broad capability.
- Compare handles exactly 2–4 images, inline review and removal with bounded proxy-first memory behavior.
- External changes reconcile from disk truth and repair active context without stale generation publication.
- Read-only and close lifecycle matrices pass in Rust, UI and packaged-app acceptance.
- Exact M3 gate passes twice on the branch and once after fast-forward merge to `main`.
- No Critical/Important review finding remains open and M3 does not pull M4 release-only scope forward.
