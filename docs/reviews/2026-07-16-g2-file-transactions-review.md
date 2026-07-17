# G2 file transaction stage review

- Status: Passed
- Date: 2026-07-16
- Base: `5aee3abdfc28d6f262b7c0445a2f6f62ecbfa14f`
- Reviewed head: `2bdd630b0c00cfc1b60747f76e375c6797404159`
- Review method: roadmap/spec traceability, full-diff inspection, deterministic crash matrix, real-filesystem safety gate, repository verification and dependency policy audit

## Exit-criteria traceability

| G2 exit criterion | Evidence | Result |
| --- | --- | --- |
| Durable operation journal | Portable SQLite schema, forward-only state validation, conditional state updates, `DELETE` rollback journal, `synchronous=FULL`, foreign keys and reopen tests | Pass |
| Verified copy | Unique `create_new` sibling, 1 MiB streaming copy, BLAKE3 size/hash evidence, file and parent synchronization, no-replace final rename, cancellation cleanup and resume tests | Pass |
| Atomic rename/move and cycles | Darwin `renamex_np(RENAME_EXCL)`, dependency-aware direct ordering, all-member cycle staging, case-only staging, stable identity/hash verification and per-item partial summaries | Pass |
| Conflict and Trash behavior | Skip has no mutation; KeepBoth naming is deterministic; Replace calls Trash before placement and reports the post-Trash failure state; macOS adapter is isolated behind `TrashPort` | Pass |
| Session undo | Only review/favorite/rename/in-project move enter the stack; copy/Trash are refused; current identity, restore-parent containment and destination occupancy are revalidated; session close clears history | Pass |
| Recovery after every persisted state | Copy, rename, move and replace each inject termination after all 7 states (28 state points), reopen SQLite, recover twice and preserve explainable user data | Pass |
| Unknown data is never guessed safe to delete | Unknown final occupants, multiple hash matches, tampered temp registrations, deterministic-name collisions, symlink escapes and project-outside conflict paths stop without destructive mutation | Pass |
| Reproducible gate and decision | `run-g2-file-transaction-gate.sh` uses disposable local fixtures; ADR 0002 contains the state protocol, atomicity limits, Trash/undo policy and complete recovery table | Pass |

## Review findings

No critical or important findings remain open.

The first full-diff review found two blocking boundary gaps: the standalone conflict executor relied on its caller to enforce the project root, and undo validation did not inspect the restore parent/destination. Commit `2bdd630` fixed both, added canonical project containment before any mutation, synchronized conflict placements, rejected outside source/destination paths, rejected escaped restore parents and refused independently occupied undo targets. The focused regressions and the complete G2 gate pass after the fixes.

The review also checked journal transition races, deterministic temporary ownership, pre-existing temp collisions, destination no-overwrite behavior, source identity drift, copy cancellation, rename partial failure, filesystem-ahead-of-journal windows, post-Trash placement failure, repeated recovery, `.viewer` reservation and canonical `/var` versus `/private/var` path aliases.

## Accepted limitations

- The real macOS Trash smoke test is intentionally not part of unattended verification. Its exact opt-in test target is valid, but this accepted run used fake-port contract coverage and did not set `VIEWER_ALLOW_REAL_TRASH_TEST=1`.
- G2 accepts atomic rename/move only on one volume. A later command service must detect cross-volume movement and compose verified copy with an explicit source-to-Trash step; ADR 0002 does not claim cross-volume rename atomicity.
- G2 proves file-operation primitives and recovery, not user-facing Tauri commands, progress UI, conflict dialogs or drag-and-drop. Those are M3 responsibilities.
- `MetaCommitted` and `IndexSynced` are durable barriers in this prototype. G3/M2 connect them to concrete session indexes and portable business metadata.

These limits are explicit in ADR 0002 and mapped to later roadmap work; none invalidates the G2 prototype exit criteria.

## Verification

The accepted stage evidence includes:

```text
./scripts/run-g2-file-transaction-gate.sh   PASS
  file transaction integration tests       19/19 PASS
  recovery safety scenarios                 5/5 PASS
  injected persisted-state points          28/28 PASS
pnpm verify                                 PASS
cargo clippy --workspace --all-targets      PASS with -D warnings
cargo deny --offline --locked check         PASS
git diff --check                            PASS
```

The dependency audit reports only duplicate-version and unused-license-allowance warnings already permitted by repository policy; advisories, bans, licenses and sources pass. The opt-in real-Trash test is skipped by default as designed.

## Decision

G2 meets its roadmap exit criteria and is approved for a fast-forward merge into `main`. G3 may begin only after the merge is verified from the main worktree.
