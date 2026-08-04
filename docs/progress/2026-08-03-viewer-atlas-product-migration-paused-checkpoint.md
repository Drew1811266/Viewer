# Viewer Atlas-to-Product visual acceptance paused checkpoint

Recorded: 2026-08-03 (America/Los_Angeles)

## Repository position

- Main repository: `/Users/abc/Project/Viewer`
- Active worktree: `/Users/abc/Project/Viewer/.worktrees/viewer-atlas-product-migration`
- Active branch: `codex/viewer-atlas-product-migration`
- Verified product implementation baseline: `c8e020e7761cf8a852278fa5a85a31adffa124b4`
  (`fix: align mixed-content shelf geometry`)
- The worktree was clean immediately before this checkpoint document was added.
- Do not merge or describe the visual migration as complete. Product migration Tasks 1–14 are
  implemented, but Task 15's 89-state, two-viewport native visual gate is still open.

## Development process at pause

- Canonical launcher: `VIEWER_TAURI_CONFIG=target/atlas-product-migration-1440.json pnpm start:viewer`
- One exact Viewer process was running at pause: PID `99798`.
- Executable:
  `/Users/abc/Project/Viewer/.worktrees/viewer-atlas-product-migration/target/debug/viewer-desktop`
- Launcher reported source `codex/viewer-atlas-product-migration @ c8e020e`.
- Acceptance viewport: `1440 × 900` from
  `target/atlas-product-migration-1440.json`.
- The process may naturally be gone when work resumes. Recheck it instead of assuming the PID is
  still valid, then use the canonical launcher if a restart is needed.

## Completed immediately before pause

The latest native comparison exposed two real defects in mixed image/other-file content:

1. The other-file shelf was being pushed to the bottom of the viewport instead of following the
   compact image rows.
2. The persistent selection summary could cover the last expanded other-file row.

Commit `c8e020e` fixes both without adding parent layout state:

- `AspectVirtualGrid` can fit its viewport to the actual laid-out row height.
- `ContentBrowser` enables that mode only for mixed content.
- Selected content reserves 58 px for the persistent selection summary.
- The mixed image slot is allowed to shrink to its actual row geometry.
- A focused regression test locks the 144 px compact three-image geometry and selection-summary
  reservation.

The abandoned parent callback/state implementation must not be restored. It caused deterministic
regressions in aggregate-folder clicking and same-folder drag-target rejection. The committed
self-contained grid solution leaves those behaviors green.

## Last verified automated evidence

All commands below ran against the product baseline `c8e020e` and exited `0`:

- Focused mixed layout: 1 passed, 73 skipped.
- Previously regressed `App` behaviors: 2 passed, 77 skipped.
- `ContentBrowser` plus `AspectVirtualGrid`: 87 passed.
- Full UI suite: 67 test files, 668 passed, 1 skipped.
- `pnpm --dir ui check`: passed; only the existing Biome `recommended` deprecation information was
  reported.
- `pnpm --dir ui build`: passed.
- `cargo build --locked -p viewer-desktop`: passed and produced the process listed above.
- The native acceptance controller suite last passed 87 tests after `1865f0c`. Controller sources
  were unchanged by `c8e020e`, but a fresh run is still required before new capture.
- `git diff --check`: passed before the product commit.

## Visual evidence status

- Authoritative atlas hash:
  `94a51900e1f65e16a8eec5b75895543ad475a88737b3df5013f25c6219aaebe4`.
- Reference root:
  `target/atlas-product-migration-reference/<atlas-hash>/<viewport>/<ID>/reference.png`.
- Acceptance root:
  `target/atlas-product-migration-acceptance/<commit>/<viewport>/<ID>/combined.png`.
- Existing `1440 × 900` captures for `THU-06`, `OTH-01`, `OTH-02`, `SEA-04`, `SEA-05` and
  `MEN-02` were made at `1865f0c`. They are diagnostic evidence only after `c8e020e`.
- No native comparison has been captured after the `c8e020e` mixed-content fix. Therefore its
  visual result is not yet accepted.
- `OTH-01`, `OTH-02` and `OTH-03` must be recaptured first because the changed geometry directly
  affects them.
- `THU-05`, `THU-06` and `THU-07` must also be recaptured because the selection-summary space is
  now reserved for every selected content view.
- The final gate still requires current-commit combined evidence at both `1024 × 720` and
  `1440 × 900` for all 89 ledger rows. Historical or earlier-commit images cannot close a row.

## Known visual-authority decisions to preserve

- Combined evidence is always atlas/reference on the left and native product on the right.
- macOS title-bar chrome is an allowed platform exception; Viewer-owned surfaces are not.
- Fixture names, counts, images and real progress values may differ when structure and visual
  hierarchy remain faithful. Do not fake atlas data.
- `MEN-01` is a documented atlas/spec conflict: the atlas image shows generic thumbnail view
  commands, while the approved interaction specification requires grouped/flat commands in search
  context. Preserve the written search behavior and record the exception or correct the atlas;
  do not regress the product to the conflicting exemplar.
- Filter conditions retain the real complete product capability even when atlas exemplar text is
  shorter. Judge the panel structure and component language, not fabricated filter semantics.
- Automated accessibility contracts are evidence, but do not support a claim of full WCAG
  conformance. Native keyboard, focus restoration, reduced motion, high contrast and zoom checks
  remain required.

## Exact resume sequence

1. Read this checkpoint, then run `git status --short`, `git rev-parse HEAD` and inspect the exact
   Viewer process. Preserve any user changes.
2. If the 1440 process is absent or does not point to this worktree, rebuild with
   `cargo build --locked -p viewer-desktop` and restart only through:
   `VIEWER_TAURI_CONFIG=target/atlas-product-migration-1440.json pnpm start:viewer`.
3. Run `pnpm test:native-acceptance` and
   `pnpm accept:native -- --preflight --viewport 1440x900`.
4. Capture and jointly inspect `OTH-01`, `OTH-02`, `OTH-03`, `THU-05`, `THU-06` and `THU-07` at
   `1440 × 900`. Fix and recapture any P0/P1/P2 mismatch before broad capture.
5. Once the local geometry is accepted, capture all 39 Wave 1 rows at the current final product
   commit at both required viewports and update the ledger only from current combined evidence.
6. Implement real native entry plans for Wave 2–4 before attempting their captures. The controller
   lists all 89 recipes, but `buildStateEntryPlan` currently has real executable plans only for the
   39 Wave 1 states and intentionally rejects the remaining IDs.
7. Complete Wave 2–4 visual comparison, manual accessibility/platform checks and the full final
   command gate. Only then update every ledger row and the verification report, and only then claim
   visual completion.

## Safety note

Do not stage generated `target/atlas-product-migration-acceptance/` screenshots. Keep exact local
paths and conclusions in the ledger. Do not reuse earlier-commit captures as completion evidence,
do not bypass the PID/executable/viewport preflight, and do not mark a state `pass` before its
reference and native screenshots have been judged together.
