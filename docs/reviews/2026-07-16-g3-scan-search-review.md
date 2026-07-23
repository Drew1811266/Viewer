# G3 Scan and Search Stage Review

> **Historical governance note (2026-07-23):** References to M4 or Viewer 0.1 release acceptance below describe the plan at the time this evidence was recorded. ADR 0005 cancelled M4 and replaced release acceptance with continuous development governance. The measured evidence in this document is unchanged.

- Review date: 2026-07-16
- Reviewed range: `main..feature/g3-scan-search`
- Decision: **Approved for local merge to `main`**

## Outcome

G3 meets its prototype gate. Viewer can publish folders before files, build and discard a session index, extract bounded UTF-8 text, search Unicode filenames/paths/body with filters, reject stale generations, and convert macOS filesystem hints into scoped reconciliation requests without directly mutating index truth.

## Acceptance review

| Area | Evidence | Result |
| --- | --- | --- |
| Contracts | Typed file/search/generation models; application ports remain platform-independent | Pass |
| Safe progressive scan | Folder-first batches; hidden/`.viewer`/symlink/alias/outside-root exclusion; root and per-item error behavior | Pass |
| Session index | WAL/NORMAL disposable database; atomic batch rollback/reopen; hierarchy and subtree deletion | Pass |
| Text/search | 10 MiB boundary, BOM/newline normalization, invalid UTF-8 isolation, FTS5 trigram, bounded short queries, nucleo rank bands, subtree/type/review/favorite filters | Pass |
| Stale publication | Entry and publication-boundary `(SessionId, Generation)` checks; atomic reserved scan send; cancellation invalidates queued work | Pass |
| Watcher/reconcile | 250 ms coalescing, expected-change identity/expiry, overflow ancestor, hidden/outside filtering, real macOS watch lifecycle | Pass |
| Performance/rebuild | 20 fresh session indexes, deterministic 1,100-file corpus, portable metadata hash unchanged | Pass |
| Repository regression | UI test/build and all workspace Rust tests through `pnpm verify` | Pass |
| Dependency policy | Locked graph accepted by `cargo deny`; direct dependencies and license families documented | Pass |

## Findings

One blocking boundary issue was found during stage review: a symlink used as the project root was followed by the initial metadata check even though child symlinks were excluded. A failing regression test reproduced the behavior; commit `a2bb1eb` changed root validation to `symlink_metadata`, explicitly rejects symlinked roots, and also rejects a macOS alias root. The complete G3 gate and repository verification passed after the fix.

No unresolved correctness, data-safety, stale-publication, or root-containment findings remain for the G3 scope.

## Final verification

`./scripts/run-g3-scan-search-gate.sh` passed after the review fix:

- scan integration tests: 7 passed;
- search integration tests: 9 passed;
- watcher/reconciliation tests: 6 passed;
- first usable folder event p95: 20.08 ms (budget 1,500 ms);
- base scan p95: 39.17 ms (budget 3,000 ms);
- indexed search p95: 0.86 ms across 80 samples (budget 100 ms);
- peak RSS: 6.67 MB (budget 700 MB);
- stale generation publication: zero in controlled races;
- portable metadata sentinel: unchanged after 20 session-index rebuilds;
- formatting, strict workspace Clippy, advisories, bans, licenses, and sources: passed.

`pnpm verify` also passed after installing the lockfile-exact dependencies in the isolated worktree. It covered the repository policy test, UI Vitest/build, all crate unit tests, G1 image behavior, all 19 G2 file-transaction tests, and all G3 integration tests.

## Accepted limitations

- Generated images are small metadata placeholders. They validate scan/index architecture, not combined decoding of the user's eventual approximately 10 MB images; M4 release validation must use the supplied real project folder.
- One/two-character body search intentionally examines at most 2,000 already-filtered text entities.
- This stage proves core ports/adapters and evidence. Tauri command wiring and production UI flows remain milestone work rather than G3 gate requirements.
