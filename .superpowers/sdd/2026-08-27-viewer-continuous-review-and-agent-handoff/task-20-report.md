# Task 20 report — history restore and source confirmation

## Result

Added an internal-only history viewer, exact restore flow, and explicit source/anchor confirmation.
The production entry remains inactive because the workspace layer only exposes the history action when
its internal `historySelector` is supplied; Task 21 remains responsible for activation. No Rust,
protocol, or `useImageReviewWorkbench` changes were made.

## RED → GREEN

- RED: the new history decision-model suite initially failed because `reviewHistoryModel` did not
  exist. It now proves archive selection filtering, empty snapshot display, duplicate restore
  suppression, preserve-current no-write behavior, and exact continuation references.
- RED: a deferred `prepareAssets` response was able to reopen source confirmation after the history
  selector changed. The lifecycle now invalidates source requests on selector and panel close; the
  regression test passes.
- GREEN: focused Task 20 and coordinator coverage verifies partial restore, zero-restored preview
  suppression, explicit multi-target continuation, candidate/position confirmation reset, same-path
  ambiguity, independent no-candidate behavior, evidence role selection, session staleness, and
  recovery non-replay.

## Files

- Added `ReviewHistoryPanel`, `ReviewSourceConfirmation`, `useContinuousHistoryReview`, and the
  pure `reviewHistoryModel`, each with focused tests.
- Updated `ReviewWorkspaceLayer` and `review.css` for the internal-only UI composition.

## Architecture decisions

- `reviewHistoryModel` owns display scoping and restore-decision calculation. `preserve_current`
  is removed from the submitted decision set, so an all-preserve selection never starts a restore
  write.
- The history hook owns session/selector/entity tokens and discards stale asynchronous history,
  evidence, restore, and source-candidate results. The coordinator remains the authority that
  prepares new identities when historical text is continued.
- Archive entries render only `entry.selected`; an empty selection renders a full snapshot only for
  a snapshot selector. Historical preview URLs are used only after `getEvidence`; annotated media is
  requested only when it exists, otherwise base media is requested. Legacy absence remains a
  capability limitation while reported evidence corruption remains an integrity error.

## Verification

- `pnpm --dir ui test` — 145 files passed, 1271 tests passed, 1 skipped.
- `pnpm --dir ui check` — passed (existing Biome deprecation notice only).
- `pnpm --dir ui build` — passed (existing chunk-size advisory only).
- `pnpm architecture:boundaries` — passed, including contracts and Rust security-boundary tests.
- `pnpm architecture:trends` — 47 existing warnings. The clean Task 19 commit `20bff71` was
  independently measured at the same 47 warnings, including its pre-existing ratio warning
  (`0.4678307422648986`). Task 20 improves that ratio to `0.4709299041665882` and introduces no
  new Task 20 trend warning; the baseline was not changed.
- `git diff --check` — passed.

## Commit

`e567900e242a2d1ada9fb0c434cfc57e0797b9b5` — `feat(review): restore history with explicit source confirmation`

## Fix round 2

修复提交：`19e8b98fb4e358796735a0e4756197d572beafdd`

- Busy state is now two explicit models: presentation reads (`history`, `evidence`, source
  preparation, restore preview) and submitted coordinator transactions (restore commit and
  historical continuation). Lifecycle changes can discard presentation state only; a transaction
  guard remains until the coordinator promise settles, prevents duplicates/close/reopen, and never
  claims a write was cancelled.
- Entity changes invalidate only source preparation. Selector/entity changes retain the final
  same-session transaction notice/error, while a true session replacement filters old results from
  the new session.
- The continuous context bar prioritizes a history integrity error over a pre-existing dirty archive
  notice. Source confirmation derives candidate and target validity synchronously and verifies again
  on click, so removed candidates or changed targets cannot be confirmed during an effect turn.

### Fix round 2 verification

- RED was observed for missing transaction state and for dirty archive notice masking an initial
  history integrity failure; the focused suite is GREEN at 5 files / 55 tests.
- `pnpm --dir ui test`: 145 files / 1289 passed, 1 skipped.
- `pnpm --dir ui check`, `pnpm --dir ui build`, `pnpm architecture:boundaries` (including
  contracts), `pnpm architecture:trends`, and `git diff --check` passed.
- Trends reports 47 non-blocking existing warnings and no new Task 20 warning. Production remains
  inactive; no Rust, protocol, `useImageReviewWorkbench`, or original-media write changes.
