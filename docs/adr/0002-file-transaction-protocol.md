# ADR 0002: recoverable local file transaction protocol

> Status: Active

- Status: Accepted
- Date: 2026-07-16
- Gate: G2 file transaction prototype
- Reproduce: `./scripts/run-g2-file-transaction-gate.sh`

## Context

Viewer must rename, copy, move and delete local project files without silently overwriting user data. A project may contain deeply nested ID folders and hundreds of large files, while outside applications can change the same paths at any time. File operations therefore need durable intent, deterministic conflict behavior, per-item batch results and recovery after termination at every persisted state.

The portable journal lives in `.viewer/metadata.sqlite`. It uses SQLite rollback journaling with `synchronous=FULL`; each item records its operation/batch/entity IDs, relative source and destination, conflict policy, state, deterministic temporary path and available BLAKE3 size/hash evidence. Absolute paths and application cache paths are never persisted as operation targets.

## Decision

### State protocol

Every filesystem item follows this forward-only state machine:

```text
Prepared → Staged → FsApplied → Verified → MetaCommitted → IndexSynced → Completed
    └─────────────── any non-terminal state may transition to Failed ───────────┘
```

- `Prepared`: durable intent and any deterministic temporary/evidence registration exist.
- `Staged`: the executor may begin or has completed its reversible staging step.
- `FsApplied`: the intended final filesystem bytes/path exist and size/hash evidence is durable.
- `Verified`: filesystem truth has been re-read and matches the evidence.
- `MetaCommitted`: portable metadata is consistent with filesystem truth.
- `IndexSynced`: rebuildable session indexes have been synchronized or scheduled.
- `Completed` and `Failed`: terminal outcomes excluded from automatic forward replay; a terminal item that retains a registered temporary is still inspected only to resolve a proven-absent cleanup obligation or request user review.

The G2 prototype advances metadata/index states as explicit journal barriers; G3 and M2 connect those barriers to the concrete metadata and session-index stores.

### Filesystem protocols and atomicity

- Copy first journals its deterministic hidden sibling path, then one combined filesystem primitive binds the source and parent identities, creates the leaf with `create_new`, immediately binds the created leaf identity, streams bytes through a 1 MiB buffer while computing BLAKE3, calls `sync_all` and returns content evidence plus the created leaf snapshot. The executor carries that snapshot into a later identity-bound no-replace placement. A pre-existing collision is never deleted. Every failure after creation either removes only the bound created identity and synchronizes its bound parent, or retains the registered path as an explicit recovery obligation; no pathname cleanup API is exposed.
- Same-volume rename and in-project move use Darwin `renamex_np(..., RENAME_EXCL)` so an existing destination is refused atomically. Rename cycles and case-only renames first move every affected source to operation-ID-derived temporary siblings.
- A cross-volume rename is not treated as atomic by this protocol. The later command service must route it through verified copy plus an explicit source-to-Trash step; G2 does not claim cross-volume atomic rename support.
- Parent directories are synchronized after temporary creation and final placement. No filesystem can make the whole multi-item batch atomic, so each item has an independent result and completed items are not rolled back when another item fails.
- Paths must resolve under the canonical project root. Symlink escapes and the reserved `.viewer` directory are rejected.

### Conflict and Trash policy

- `Skip` performs no mutation.
- `KeepBoth` deterministically tries `name copy.ext`, `name copy 2.ext`, and so on; final placement still uses no-replace rename to close the race window.
- `Replace` moves the existing destination to macOS Trash before placing the replacement. It never uses an overwriting rename. If placement fails after Trash succeeds, the error explicitly says the previous destination is in system Trash and leaves the replacement at its source or registered staged path.
- User deletion and replacement Trash actions are not placed in Viewer undo history. Restoration remains a system Trash operation.
- `TrashPort` contains the third-party adapter boundary, so `trash-rs` types do not enter application/domain layers. The real-Trash gate requires explicit local opt-in and CI must not set it.

### Undo policy

`UndoStack` is memory-only and bound to one project session. Rename, in-project move, review state and favorite changes may add one batch-level reverse plan. Copy and Trash never do. Before returning a reverse plan, Viewer canonicalizes the current path, rejects symlinked/escaped restore parents, refuses an independently occupied restore destination and revalidates the recorded file snapshot; identity drift refuses the undo. Closing the owning session clears the stack.

## Recovery decision table

Recovery opens the durable journal, considers non-terminal items plus terminal items that retain a registered cleanup obligation, canonicalizes project-relative paths, validates every registered temporary against its operation-ID-derived name, and hashes regular-file candidates. “Match” means both stored size and BLAKE3 match.

| Persisted state / observed filesystem truth | Automatic action | Stop with `NeedsUserReview` |
| --- | --- | --- |
| Copy `Prepared`/`Staged`; source is a regular file; exact registered temp is absent | Mark item `Failed`; there is nothing to remove | Any occupant at the registered path because no persisted leaf identity proves ownership; source missing/symlinked; or registered temp is not the deterministic operation path |
| Copy `FsApplied`; temp is the only match; final absent | Verify temp, advance to `Verified`, place final, synchronize, complete | Unknown content at temp/final, neither candidate matches, or both candidates match |
| Copy `Verified`; exactly one of temp/final matches | If temp matches, place it; if final matches, treat filesystem as one step ahead; complete | Unknown occupant or evidence matches both paths |
| Copy `MetaCommitted`/`IndexSynced`; only final matches | Finish remaining metadata/index barriers | Final missing/mismatched or an additional matching temp exists |
| Rename/move `Prepared`; original source is the only match | Leave source untouched; mark `Failed` | Evidence absent, unknown candidate, or multiple candidates match |
| Direct rename/move `Staged`; source matches and final absent | Leave source untouched; mark `Failed` | Final has unknown content or more than one path matches |
| Staged cycle/case temporary is the only match; final absent | Place verified temp at final and complete | Final is occupied by unknown content; if safe placement cannot be proven, restore the source or stop |
| Rename/move `Staged`; source/temp absent and final matches | Record `FsApplied`, verify and complete | Final mismatches, or source/temp also match |
| Replace `Prepared`/`Staged`; replacement temp matches and original source is absent | Restore replacement source and mark `Failed`; if final is absent, report that the previous destination is already in Trash | Source restoration target is occupied, evidence is missing, or several candidates match |
| Rename/move/replace `FsApplied` through `IndexSynced`; only final matches | Complete remaining forward barriers | Final unknown/missing, source/temp also match, or evidence is ambiguous |

Recovery never deletes an arbitrary journal path, never overwrites an occupied final path, and never selects between multiple matching candidates. A second recovery run after a completed automatic action is a no-op; unresolved terminal cleanup obligations remain visible until absence is proven or the user resolves them.

## Evidence

The accepted gate runs on the standard Apple Silicon development device and creates all fixtures under a disposable local temporary directory.

| Acceptance | Automated evidence |
| --- | --- |
| Copy integrity | Source and destination bytes match; size/BLAKE3 evidence is durable; cancellation removes and durably records only the bound created identity, or retains an explicit cleanup/recovery obligation |
| Atomic/no-overwrite placement | Real filesystem tests cover destination refusal, symlink escapes and deterministic collision handling |
| Rename/move | Cycle swap, case-only rename, stable file identity and per-item partial failure summary pass |
| Conflict policies | Skip, KeepBoth and Trash-before-Replace order pass with fake ports |
| Crash recovery | Copy, rename, move and replace each inject termination after all 7 persisted states: 28 state points total |
| Idempotence | Every matrix case reopens the SQLite journal and runs recovery twice; the second pass has no actions |
| Unsafe evidence | Unknown target occupant, duplicate matches, tampered temp registration and pre-existing deterministic temp all stop without deleting user files |
| Trash edge | Placement failure after Trash restores the replacement source and preserves the previous bytes in fake Trash |
| Real macOS Trash | Adapter smoke test is skipped by default and runs only with `VIEWER_ALLOW_REAL_TRASH_TEST=1` |
| Dependency policy | Locked advisories, licenses, bans and sources pass under `cargo deny`; existing approved duplicate-version warnings remain informational |

## Consequences

- The protocol favors explainability and data preservation over automatically finishing an ambiguous operation.
- BLAKE3 verification adds sequential I/O to copy and rename preparation, but avoids loading whole large files into memory and supplies portable recovery evidence when stable file IDs are unavailable.
- Operation logs remain portable with the project; session undo intentionally does not.
- Batch UI can report completed and failed items independently without pretending the whole batch is one filesystem transaction.
- G3 may rebuild indexes from established filesystem truth, and M3 can compose these primitives into user-facing commands, progress, conflicts and undo.
