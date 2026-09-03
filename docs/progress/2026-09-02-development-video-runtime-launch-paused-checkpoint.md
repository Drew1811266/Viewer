# Development Video Runtime Launch — Paused Checkpoint

**Date:** 2026-09-02
**Status:** Completed after resume
**Branch:** `codex/review-save-pipeline`
**Worktree:** `/Users/abc/Project/Viewer/.worktrees/review-save-pipeline`
**Starting HEAD:** `e2b1dd89310c943e856cfbf467fa9f0952b2c5c1`

## Objective and boundary

Make the canonical development command `pnpm start:viewer` reliably launch a
fresh worktree when the reviewed video runtime exists at the repository staging
location but has not been manually copied into `target/debug`.

This is a development-launch defect fix. It does not add product features,
change user-visible product behavior, relax the production runtime trust
boundary, or alter release packaging. Signing, notarization, formal installers,
store submission, and release work remain out of scope.

## Confirmed root cause

The Tauri development resource copy places the configured runtime below an
`_up_/target/...` path, while the strict application loader resolves
`ViewerVideoRuntime` directly below the development executable's resource
directory: `target/debug/ViewerVideoRuntime`.

The launcher previously started `pnpm tauri dev` without preparing that direct
resource path. Existing worktrees could appear healthy only when a prior manual
copy happened to remain under ignored `target/` output.

## Implemented, uncommitted changes

- Added `scripts/development-video-runtime.mjs`:
  - verifies the reviewed staged runtime before any copy;
  - reuses a verified destination only when both inventory and lock identity
    match the source;
  - copies a missing, incomplete, or stale runtime into a same-filesystem
    temporary directory;
  - verifies the candidate before activation;
  - replaces the old generated runtime through a recoverable backup/rename
    sequence;
  - never uses a symbolic-link shortcut and never signs, downloads, or builds a
    runtime.
- Updated `scripts/viewer-dev-launcher.mjs`:
  - derives the staged source, direct development destination, and existing
    verifier paths from the repository root;
  - prepares the development runtime immediately before spawning Tauri;
  - leaves production runtime lookup and `src-tauri/tauri.conf.json` unchanged.
- Added focused tests in `scripts/development-video-runtime.test.mjs`.
- Extended `scripts/viewer-dev-launcher.test.mjs` with a clean-target launch
  regression and complete runtime prerequisites for existing spawn tests.

## TDD evidence recorded before pause

Each behavior was observed failing for its intended reason before the matching
implementation was added:

1. Clean target: failed with `ENOENT` for
   `target/debug/ViewerVideoRuntime/bin/ffmpeg`.
2. Unchanged target reuse: failed with `ENOTEMPTY` because the launcher tried to
   replace an already valid directory.
3. Incomplete target repair: failed with `ENOTEMPTY` because no replacement
   transaction existed.
4. Stale but structurally valid identity: returned `reused` instead of the
   required `installed` result.

The final focused run passed:

```text
node --test scripts/development-video-runtime.test.mjs scripts/viewer-dev-launcher.test.mjs
24 tests, 24 passed, 0 failed
```

Additional completed checks:

```text
pnpm test:video:packaging
30 tests, 30 passed, 0 failed

git diff --check
node --check scripts/development-video-runtime.mjs
node --check scripts/viewer-dev-launcher.mjs
all exited 0
```

## Verification state at pause

`pnpm verify:clean` was started and then deliberately interrupted when the user
requested a pause. It did not report a failure before interruption. The
following stages completed successfully during that partial run:

- repository policy and scope coverage;
- review protocol tests;
- continuous review end-to-end tests;
- architecture boundaries and architecture contracts;
- desktop security and video security boundary tests;
- video packaging tests;
- clean-verification parser tests;
- UI check and TypeScript build;
- UI tests: 151 files passed, 1381 tests passed, 1 skipped;
- UI production build;
- Rust formatting and clippy reached successful completion;
- a substantial portion of the Rust workspace test suite.

The command was interrupted while the remaining Rust workspace tests were still
running. Therefore `pnpm verify:clean`, the remaining Rust tests, and the final
security chain must be rerun from the beginning and must not be reported as
complete yet.

## Exact resume steps

1. Run the focused tests again:

   ```bash
   node --test scripts/development-video-runtime.test.mjs scripts/viewer-dev-launcher.test.mjs
   ```

2. Run a fresh complete repository verification:

   ```bash
   pnpm verify:clean
   ```

3. Perform the real cold-start regression:
   - stop the current worktree Viewer process;
   - move the generated `target/debug/ViewerVideoRuntime` aside without touching
     the reviewed staged source;
   - run `pnpm start:viewer` from this worktree;
   - confirm the launcher recreates and verifies the direct runtime, starts only
     this worktree's `target/debug/viewer-desktop`, and records no unsafe-runtime
     error in `target/dev-launcher/tauri-dev.log`;
   - remove only the disposable generated backup after success.

4. Inspect `git diff --check`, the complete diff, and `git status --short`.

## State at pause

- Viewer process still running from this worktree at pause: PID `60922`.
- No `pnpm verify:clean`, `verify-clean.mjs`, or workspace Cargo test process was
  left running after the pause.
- Source changes are uncommitted.
- Tracked/untracked source files at pause:
  - `scripts/development-video-runtime.mjs` (new)
  - `scripts/development-video-runtime.test.mjs` (new)
  - `scripts/viewer-dev-launcher.mjs` (modified)
  - `scripts/viewer-dev-launcher.test.mjs` (modified)
  - this checkpoint document (new)
- No merge, commit, push, package, signing, notarization, installer, store, or
  release action has been performed.

## Resume completion

The paused work was resumed on 2026-09-02 and completed without changing the
production runtime boundary or user-visible product behavior.

- Focused development launcher and runtime tests passed: 24/24.
- A real cold start with no `target/debug/ViewerVideoRuntime` succeeded. The
  launcher materialized a normal directory from the verified staged source,
  preserved inventory and lock identity, and started exactly one Viewer from
  this worktree without runtime errors in the fresh launcher log.
- `test:dev-launcher` was added to the canonical `quality` chain and frozen by
  the repository policy test so these regressions cannot silently fall out of
  routine verification.
- The final `pnpm verify:clean` completed with exit code 0, including policy,
  architecture, UI, Rust, security, and video runtime checks.
- The final `git diff --check` completed with exit code 0.
