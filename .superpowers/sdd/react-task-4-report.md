# React Decomposition Task 4 Report

## Result

Status: DONE

Base: `4a6556cfc6dfb72e607802d1f9ebd65eb8140098`

Implemented the shared `ControllerCore`/`RefreshProjection` types and extracted the
project/session responsibility into `useProjectSessionController`, while retaining the exact
ViewerController facade inventory.

## Baseline

Before production edits, both required characterization commands passed:

- terminal refresh after close/reopen of the same backend identity: 36 files, 278 tests passed;
- stayed close followed by chosen terminal close: 36 files, 278 tests passed.

The worktree was clean and HEAD was the requested base.

## TDD RED

Added a compile/type contract to `ui/src/state/useViewerController.test.tsx` importing:

- `ControllerCore`;
- `RefreshProjection`;
- `ProjectSessionController`.

Accepted RED: `pnpm --dir ui check` exited 2 with only TS2307 missing-module errors for
`./controllers/types` and `./controllers/useProjectSessionController`.

Setup attempts not counted as the true RED:

1. Biome stopped on test import order/formatting.
2. TypeScript also reported a missing `expectTypeOf` import.
3. The unresolved type caused an additional `keyof` assertion diagnostic; the contract was
   simplified to useful property/interface coverage.

The next run produced only the two intended missing-module errors before production modules were
added.

## GREEN and verification

- Contract test in isolation: 1 passed, 21 skipped.
- Focused controller + App + facade contract: 3 files, 69 tests passed.
- Required repository command `pnpm --dir ui test -- useViewerController.test.tsx App.test.tsx`:
  36 files, 279 tests passed (the package wrapper passes `--` through and Vitest runs all files).
- Post-extraction terminal close/reopen characterization: 36 files, 279 tests passed.
- Post-extraction stayed/chosen-close characterization: 36 files, 279 tests passed.
- `pnpm --dir ui check`: passed; Biome checked 87 files and TypeScript completed successfully.

No async test flaked and no isolated rerun was required because of a failure.

## Public return-key inventory

A BASE-versus-current static inventory comparison reported 31/31 keys, same order, exact match:

1. `state`
2. `openProject`
3. `closeProject`
4. `reselectProject`
5. `selectFolder`
6. `showAllDescendants`
7. `cancelTask`
8. `setSearchText`
9. `setSearchScope`
10. `setSearchFilters`
11. `setSearchSort`
12. `setSearchLayout`
13. `removeSearchFilter`
14. `clearSearchFilters`
15. `setVisibleSearchHits`
16. `setSearchPage`
17. `returnToFolderContext`
18. `setSelectedEntityIds`
19. `setReviewState`
20. `toggleFavorite`
21. `previewRename`
22. `preflightFileCommand`
23. `executeFileCommand`
24. `cancelOperation`
25. `loadOperationResults`
26. `undoLastOperation`
27. `setPreviewEntityId`
28. `setCompareEntityIds`
29. `consumeContextRepair`
30. `clearCloseBlocked`
31. `openPermissionSettings`

This is exactly `state` plus 30 commands. `refreshProjection` and `sessionEpoch` are not exposed by
the ViewerController facade.

## Epoch-path audit

`advanceSessionEpoch` computes one next value, assigns it synchronously to
`sessionEpochRef.current`, and enqueues that same value for rendered `sessionEpoch`.

Audited increment paths:

1. Open: called after validating the path/status and before `project_open_requested` and
   `bridge.openProject`; both resolve and reject settlement compare their captured epoch to
   `sessionEpochRef.current`.
2. Terminal explicit close: both the normal closed outcome and terminal cache-cleanup-failure path
   call `resetProjectSessionRequests` before dispatching `project_closed`.
3. Backend `project_closed`: the listener calls `resetProjectSessionRequests` before clearing the
   reconcile marker and dispatching `project_closed`.

Stayed closes and non-terminal close failures do not advance the epoch. Later preview, preflight,
and operation settlement guards use the shared `sessionEpochRef`, preserving same-backend-identity
stale invalidation.

## Projection settlement audit

- Every projection request synchronously updates the desired projection and increments its request
  ID.
- The success path checks the request ID after both folder/workspace work settles and before
  updating desired projection or dispatching `projection_loaded`.
- The failure path checks the same request ID before dispatching `projection_failed`.
- Every terminal session reset increments the projection request ID and resets desired projection
  synchronously before committing the closed state.
- Later marker/operation/project-change responsibilities remain in the facade and call an internal
  desired-projection adapter backed by the session hook's ref. This preserves an in-flight desired
  folder/aggregate target rather than substituting the last rendered state.

## Moved responsibility

- project open and stale open settlement;
- close request, stayed/failed/terminal close handling, and reselect;
- projection loading, repair, request IDs, and desired projection;
- folder selection and aggregate view;
- scan-task cancellation;
- scan reconciliation and scan subscription;
- index progress subscription;
- backend project-closed lifecycle;
- project-drop subscription;
- close-blocked event subscription.

Search, marker, file-operation, preview/compare, permission, and project-change responsibilities
remain in the facade. The project-close dispatch adapter synchronously resets their existing
request refs before the reducer receives `project_closed`, matching the original cleanup ordering.

## Mechanical comparison and self-review

- Moved command/effect bodies were compared against BASE. ViewerBridge methods, DTO arguments,
  action payloads, localized messages, default close target, terminal cleanup classification, and
  promise error ordering are unchanged.
- Each moved async listener retains the original `disposed` flag, late-cleanup call, unlisten
  closure, and swallowed subscription error behavior.
- The only intentional body changes are shared-core destructuring/dependencies, epoch ref plus
  rendered state advancement, split ownership of close-time request reset, and the internal
  desired-projection adapter needed by responsibilities that remain in the facade.
- `git diff --check` passed.
- Pre-report code/test status contained only the four task files: two modified files and the new
  `ui/src/state/controllers/` directory.
- Pre-report code/test diff: 523 insertions, 411 deletions. The facade decreased from 1,120 to 770
  lines; the new session controller is 421 lines and shared types are 20 lines.

No unrelated cleanup was included.

## Warning

`pnpm --dir ui check` emits the repository's existing Biome configuration deprecation notice for
the `linter.recommended` field. It is informational and the command exits 0.
