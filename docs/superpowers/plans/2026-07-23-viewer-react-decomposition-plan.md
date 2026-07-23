# Viewer React Decomposition Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split the oversized React reducer, controller, App, and ContentBrowser into focused modules while preserving the existing public hooks, props, state transitions, and UI behavior.

**Architecture:** Keep `viewerReducer`, `useViewerController`, `App`, and `ContentBrowser` as compatibility facades. Extract pure reducer domains first, then hook controllers, then App and browser presentation coordinators; run existing characterization tests after every extraction.

**Tech Stack:** React 19, TypeScript 6 with `strict` and `noUncheckedIndexedAccess`, Vitest 4, Testing Library, Biome.

## Global Constraints

- Plan 2 must be complete and `pnpm verify:clean` must pass before this plan starts.
- Do not change `ViewerBridge`, IPC DTOs, visible labels, keyboard shortcuts, focus behavior, event names, or reducer action strings.
- Preserve the exact object keys returned by `useViewerController`.
- Preserve stale request rejection based on session, generation, revision, request ID, and batch ID.
- Preserve current App props: `App({ bridge = tauriViewerBridge }: AppProps)`.
- Preserve all current component props and accessibility roles during extraction.
- Every task runs focused tests and `pnpm --dir ui check`.

---

### Task 1: Freeze React Facade Contracts

**Files:**
- Create: `ui/src/state/viewerControllerContract.test.tsx`
- Modify: `ui/src/state/useViewerController.ts`

**Interfaces:**
- Produces: exported `ViewerController` return type.
- Preserves: all 30 current controller commands plus `state`.

- [ ] **Step 1: Export the inferred facade type**

At the bottom of `useViewerController.ts`, add:

```ts
export type ViewerController = ReturnType<typeof useViewerController>
```

- [ ] **Step 2: Write a compile-time facade contract test**

Create `ui/src/state/viewerControllerContract.test.tsx`:

```tsx
import { describe, expectTypeOf, it } from 'vitest'
import type { ViewerController } from './useViewerController'

type ExpectedCommands = Pick<
  ViewerController,
  | 'openProject'
  | 'closeProject'
  | 'reselectProject'
  | 'selectFolder'
  | 'showAllDescendants'
  | 'cancelTask'
  | 'setSearchText'
  | 'setSearchScope'
  | 'setSearchFilters'
  | 'setSearchSort'
  | 'setSearchLayout'
  | 'removeSearchFilter'
  | 'clearSearchFilters'
  | 'setVisibleSearchHits'
  | 'setSearchPage'
  | 'returnToFolderContext'
  | 'setSelectedEntityIds'
  | 'setReviewState'
  | 'toggleFavorite'
  | 'previewRename'
  | 'preflightFileCommand'
  | 'executeFileCommand'
  | 'cancelOperation'
  | 'loadOperationResults'
  | 'undoLastOperation'
  | 'setPreviewEntityId'
  | 'setCompareEntityIds'
  | 'consumeContextRepair'
  | 'clearCloseBlocked'
  | 'openPermissionSettings'
>

describe('ViewerController facade', () => {
  it('retains the state and command contract used by App', () => {
    expectTypeOf<ViewerController>().toHaveProperty('state')
    expectTypeOf<ViewerController>().toMatchTypeOf<
      ExpectedCommands & { state: ViewerController['state'] }
    >()
  })
})
```

- [ ] **Step 3: Run the contract and existing controller tests**

```bash
pnpm --dir ui test -- viewerControllerContract.test.tsx useViewerController.test.tsx
pnpm --dir ui check
```

Expected: PASS.

- [ ] **Step 4: Commit the facade contract**

```bash
git add ui/src/state/useViewerController.ts ui/src/state/viewerControllerContract.test.tsx
git commit -m "test(ui): freeze viewer controller facade"
```

### Task 2: Extract Viewer State Types and Initializers

**Files:**
- Create: `ui/src/state/viewerState.ts`
- Modify: `ui/src/state/viewerReducer.ts`
- Test: `ui/src/state/viewerReducer.test.ts`

**Interfaces:**
- Produces: `ViewerState`, `ViewerStatus`, `ViewerAction`, `SearchState`, `OperationState`, `ContextRepair`, `ScanState`, `SearchFilterChip`.
- Produces: `initialViewerState`, `initialSearchState()`, `initialOperationState()`, `freshViewerState(status)`.
- `viewerReducer.ts` re-exports all existing public names.

- [ ] **Step 1: Add a state reset characterization**

Append to `viewerReducer.test.ts`:

```ts
it('creates independent nested search and operation state after every project reset', () => {
  const first = viewerReducer(initialViewerState, { type: 'project_open_requested' })
  const second = viewerReducer(first, { type: 'project_closed' })
  expect(second.search).not.toBe(first.search)
  expect(second.operation).not.toBe(first.operation)
  expect(second.selectedEntityIds).toEqual([])
  expect(second.compareEntityIds).toEqual([])
})
```

- [ ] **Step 2: Run the test**

```bash
pnpm --dir ui test -- viewerReducer.test.ts
```

Expected: PASS against the current implementation; this is a characterization test.

- [ ] **Step 3: Move types and initializers**

Create `viewerState.ts` and move the existing definitions without changing field names. Export these initializers:

```ts
export function initialSearchState(): SearchState {
  return {
    focusRequest: 0,
    showResults: false,
    query: initialSearchQuery,
    queryVersion: 0,
    schedule: 'immediate',
    revision: 0,
    status: 'idle',
    page: null,
    snippets: {},
    visibleEntityIds: [],
    offset: 0,
  }
}

export function initialOperationState(): OperationState {
  return { kind: null, active: null, results: null, finishing: false, pending: false }
}

export function freshViewerState(status: ViewerStatus): ViewerState {
  return {
    ...initialViewerState,
    status,
    search: initialSearchState(),
    operation: initialOperationState(),
    selectedEntityIds: [],
    compareEntityIds: [],
  }
}
```

In `viewerReducer.ts`, re-export:

```ts
export {
  emptySearchFilters,
  initialSearchQuery,
  initialViewerState,
  type SearchFilterChip,
  type ViewerAction,
  type ViewerState,
  type ViewerStatus,
} from './viewerState'
```

- [ ] **Step 4: Run reducer tests and typecheck**

```bash
pnpm --dir ui test -- viewerReducer.test.ts
pnpm --dir ui check
```

Expected: PASS.

- [ ] **Step 5: Commit state extraction**

```bash
git add ui/src/state/viewerState.ts ui/src/state/viewerReducer.ts ui/src/state/viewerReducer.test.ts
git commit -m "refactor(ui): extract viewer state contracts"
```

### Task 3: Split Reducer Domains Behind the Existing Facade

**Files:**
- Create: `ui/src/state/reducers/projectReducer.ts`
- Create: `ui/src/state/reducers/searchReducer.ts`
- Create: `ui/src/state/reducers/workspaceReducer.ts`
- Create: `ui/src/state/reducers/operationReducer.ts`
- Create: `ui/src/state/reducers/shared.ts`
- Modify: `ui/src/state/viewerReducer.ts`
- Test: `ui/src/state/viewerReducer.test.ts`

**Interfaces:**
- Each reducer exports `reduce*Action(state: ViewerState, action: ViewerAction): ViewerState | undefined`.
- `undefined` means “action not owned”; returning the original state means “owned but intentionally ignored”.
- `viewerReducer` calls reducers in fixed order: project, workspace, search, operation.

- [ ] **Step 1: Add ownership ordering tests**

Append:

```ts
it('keeps project-change refresh and search state updates atomic', () => {
  let active = viewerReducer(initialViewerState, { type: 'project_opened', project })
  active = viewerReducer(active, { type: 'search_text_changed', text: 'query' })
  const changed = viewerReducer(active, {
    type: 'project_changed_received',
    change: projectChange(project.sessionId, project.generation),
  })
  expect(changed.pendingProjectChange).not.toBeNull()
  expect(changed.search.queryVersion).toBe(active.search.queryVersion + 1)
})
```

- [ ] **Step 2: Run the characterization**

```bash
pnpm --dir ui test -- viewerReducer.test.ts
```

Expected: PASS.

- [ ] **Step 3: Extract shared pure helpers**

Move unchanged implementations of `unique`, `sameStrings`, `isCurrentEvent`, and `isCurrentProjection` into `reducers/shared.ts`:

```ts
export function unique(values: string[]): string[] {
  return [...new Set(values)]
}

export function sameStrings(left: string[], right: string[]): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index])
}
```

Keep their current typed session/generation guards unchanged.

- [ ] **Step 4: Extract reducer cases**

Move switch cases as follows:

- `projectReducer`: open/reconcile/open-failed/close/stayed/close-failed/closed/input-rejected/close-blocked.
- `workspaceReducer`: scan/index/projection/selection/context-repair/preview/compare/project-changed/selection-info/marker changes.
- `searchReducer`: every `search_*` and `visible_search_hits_changed`.
- `operationReducer`: every `operation_*`.

Each module exports `reduce*Action(state: ViewerState, action: ViewerAction): ViewerState | undefined`, preserves the exact bodies of its assigned action cases, and returns `undefined` from the switch default.

Replace `viewerReducer` with:

```ts
export function viewerReducer(state: ViewerState, action: ViewerAction): ViewerState {
  return (
    reduceProjectAction(state, action) ??
    reduceWorkspaceAction(state, action) ??
    reduceSearchAction(state, action) ??
    reduceOperationAction(state, action) ??
    state
  )
}
```

Move helper functions with the domain that owns them:

- search: `queryChanged`, `removeFilterChip`;
- workspace: `repairContextAfterProjection`, `orderedWorkspaceEntityIds`, `applyMarkerChanges`, `updateWorkspaceMarkers`, `reduceScan`;
- project: `freshViewerState`;
- operation: no cross-domain helper.

- [ ] **Step 5: Run all reducer tests**

```bash
pnpm --dir ui test -- viewerReducer.test.ts
pnpm --dir ui check
```

Expected: PASS with no action-string or state-shape change.

- [ ] **Step 6: Commit reducer domains**

```bash
git add ui/src/state/reducers ui/src/state/viewerReducer.ts ui/src/state/viewerReducer.test.ts
git commit -m "refactor(ui): split viewer reducer domains"
```

### Task 4: Add Shared Controller Core and Project Session Controller

**Files:**
- Create: `ui/src/state/controllers/types.ts`
- Create: `ui/src/state/controllers/useProjectSessionController.ts`
- Modify: `ui/src/state/useViewerController.ts`
- Test: `ui/src/state/useViewerController.test.tsx`

**Interfaces:**
- Produces: `ControllerCore`.
- Produces: `RefreshProjection`.
- Produces: `useProjectSessionController(core): ProjectSessionController`.
- Produces: monotonically increasing `sessionEpoch`.

- [ ] **Step 1: Run the existing close/reopen characterizations**

```bash
pnpm --dir ui test -- useViewerController.test.tsx -t "drops a terminal refresh that settles after closing and reopening the same backend identity"
pnpm --dir ui test -- useViewerController.test.tsx -t "retains the complete session when close stays and clears it only after a chosen close"
```

Expected: both tests pass before extraction.

- [ ] **Step 2: Define the shared core**

Create `controllers/types.ts`:

```ts
import type { Dispatch, MutableRefObject } from 'react'
import type { ViewerBridge } from '../../api/viewer'
import type { ProjectSnapshot } from '../../api/types'
import type { ViewerAction, ViewerState } from '../viewerState'

export interface ControllerCore {
  bridge: ViewerBridge
  state: ViewerState
  stateRef: MutableRefObject<ViewerState>
  sessionEpochRef: MutableRefObject<number>
  dispatch: Dispatch<ViewerAction>
}

export type RefreshProjection = (
  project: ProjectSnapshot,
  selectedFolderId: string | null,
  selectedFolderPath: string,
  showingAggregate: boolean,
  repairMissingFolder?: boolean,
) => Promise<void>
```

- [ ] **Step 3: Extract project session behavior**

Move `refreshProjection`, `openProject`, `requestProjectClose`, `closeProject`, `reselectProject`, `selectFolder`, `showAllDescendants`, `cancelTask`, project drop subscription, scan/index subscriptions, and project-close lifecycle into `useProjectSessionController`.

Return this interface:

```ts
export interface ProjectSessionController {
  sessionEpoch: number
  refreshProjection: RefreshProjection
  openProject(path: string): Promise<void>
  closeProject(choice?: CloseChoice, target?: CloseTarget): Promise<CloseRequestOutcome | undefined>
  reselectProject(): Promise<CloseRequestOutcome | undefined>
  selectFolder(entityId: string | null): Promise<void>
  showAllDescendants(): Promise<void>
  cancelTask(taskId: string): Promise<void>
}
```

Increment both `sessionEpochRef.current` and the rendered `sessionEpoch` value before an open request and before terminal close cleanup commits the session closed. Also increment them in the backend `project_closed` listener. Keep projection request IDs and `desiredProjectionRef` in this hook so projection invalidation is synchronous.

- [ ] **Step 4: Compose it from the facade**

In `useViewerController`, create `const sessionEpochRef = useRef(0)`, include it in `ControllerCore`, call `useProjectSessionController(core)`, and spread only its public commands into the existing return object. Do not expose `refreshProjection` or `sessionEpoch` through the public facade.

- [ ] **Step 5: Run focused tests**

```bash
pnpm --dir ui test -- useViewerController.test.tsx App.test.tsx
pnpm --dir ui check
```

Expected: PASS.

- [ ] **Step 6: Commit project controller**

```bash
git add ui/src/state/controllers/types.ts ui/src/state/controllers/useProjectSessionController.ts ui/src/state/useViewerController.ts ui/src/state/useViewerController.test.tsx
git commit -m "refactor(ui): extract project session controller"
```

### Task 5: Extract Search and Selection/Marker Controllers

**Files:**
- Create: `ui/src/state/controllers/useSearchController.ts`
- Create: `ui/src/state/controllers/useSelectionMarkerController.ts`
- Modify: `ui/src/state/useViewerController.ts`
- Modify: `ui/src/state/viewerReducer.test.ts`
- Test: `ui/src/state/useViewerController.test.tsx`

**Interfaces:**
- `useSearchController(core, sessionEpoch)` owns search revision/snippet refs and search effects.
- `useSelectionMarkerController(core, sessionEpoch)` owns selection info and marker requests.

- [ ] **Step 1: Add a stale-generation marker characterization**

Append to `viewerReducer.test.ts`:

```ts
it('ignores marker changes from a stale generation', () => {
  const active = viewerReducer(initialViewerState, { type: 'project_opened', project })
  const changed = viewerReducer(active, {
    type: 'marker_changes_applied',
    sessionId: project.sessionId,
    generation: project.generation - 1,
    changes: [
      {
        entityId: 'folder-1',
        relativePath: 'id-1',
        kind: 'directory',
        marker: { reviewState: 'keep', favorite: true },
      },
    ],
  })
  expect(changed).toEqual(active)
})
```

Keep the existing controller tests `debounces text by 120 ms and ignores a late older response` and `preserves selection after marker responses and suppresses writes for read-only projects` unchanged.

- [ ] **Step 2: Run the tests before extraction**

```bash
pnpm --dir ui test -- viewerReducer.test.ts
pnpm --dir ui test -- useViewerController.test.tsx -t "debounces text by 120 ms and ignores a late older response"
pnpm --dir ui test -- useViewerController.test.tsx -t "preserves selection after marker responses and suppresses writes for read-only projects"
```

Expected: PASS.

- [ ] **Step 3: Extract search behavior**

Move search request/debounce/snippet effects and setters. Return:

```ts
export interface SearchController {
  setSearchText(text: string): void
  setSearchScope(folderId: string | null): void
  setSearchFilters(filters: SearchFilters): void
  setSearchSort(sort: SearchSort): void
  setSearchLayout(layout: SearchLayout): void
  removeSearchFilter(chip: SearchFilterChip): void
  clearSearchFilters(): void
  setVisibleSearchHits(entityIds: string[]): void
  setSearchPage(offset: number): void
  returnToFolderContext(): void
}
```

When `sessionEpoch` changes, increment `searchRevisionRef` and clear `requestedSnippetsRef`. Search and snippet requests capture `core.sessionEpochRef.current` and compare it before dispatching or caching a response.

- [ ] **Step 4: Extract selection and markers**

Move `refreshSelectionInfo`, `setSelectedEntityIds`, `setReviewState`, `toggleFavorite`, `setPreviewEntityId`, `setCompareEntityIds`, and `consumeContextRepair`. Return those exact names and current signatures. Clear the selection request counter when `sessionEpoch` changes, and compare captured async work with `core.sessionEpochRef.current` before dispatch.

- [ ] **Step 5: Compose and test**

```bash
pnpm --dir ui test -- useViewerController.test.tsx viewerReducer.test.ts
pnpm --dir ui check
```

Expected: PASS.

- [ ] **Step 6: Commit the controllers**

```bash
git add ui/src/state/controllers/useSearchController.ts ui/src/state/controllers/useSelectionMarkerController.ts ui/src/state/useViewerController.ts ui/src/state/viewerReducer.test.ts
git commit -m "refactor(ui): extract search and marker controllers"
```

### Task 6: Extract Operation and Lifecycle Controllers

**Files:**
- Create: `ui/src/state/controllers/useOperationController.ts`
- Create: `ui/src/state/controllers/useLifecycleSubscriptions.ts`
- Modify: `ui/src/state/useViewerController.ts`
- Test: `ui/src/state/useViewerController.test.tsx`

**Interfaces:**
- `useOperationController(core, sessionEpoch, refreshProjection)` owns operation refs and commands.
- `useLifecycleSubscriptions(core, handlers)` owns operation-progress, project-change, and close-blocked subscriptions.

- [ ] **Step 1: Run existing terminal-operation characterizations**

```bash
pnpm --dir ui test -- useViewerController.test.tsx -t "keeps the operation in finishing state until results and projection cleanup settle"
pnpm --dir ui test -- useViewerController.test.tsx -t "drops a terminal refresh that settles after closing and reopening the same backend identity"
pnpm --dir ui test -- useViewerController.test.tsx -t "releases a terminal batch after the same project advances generation"
```

Expected: all three tests pass before extraction.

- [ ] **Step 2: Extract operation behavior**

Move `previewRename`, `preflightFileCommand`, `finishOperation`, `receiveOperationProgress`, `executeFileCommand`, `cancelOperation`, `loadOperationResults`, `undoLastOperation`, and operation request refs.

Return:

```ts
export interface OperationController {
  previewRename: ViewerController['previewRename']
  preflightFileCommand: ViewerController['preflightFileCommand']
  executeFileCommand: ViewerController['executeFileCommand']
  cancelOperation: ViewerController['cancelOperation']
  loadOperationResults: ViewerController['loadOperationResults']
  undoLastOperation: ViewerController['undoLastOperation']
  receiveOperationProgress(progress: OperationProgressEvent): void
}
```

Clear operation request/result/completed-batch refs when `sessionEpoch` changes. Every async operation captures `core.sessionEpochRef.current` and rejects settlement when the ref no longer matches, preserving the current synchronous stale-session invalidation.

- [ ] **Step 3: Extract lifecycle subscriptions**

Move `listenOperationProgress`, `listenProjectChanged`, and `listenCloseBlocked` effects into:

```ts
export function useLifecycleSubscriptions(
  core: ControllerCore,
  handlers: {
    receiveOperationProgress(progress: OperationProgressEvent): void
    refreshProjection: RefreshProjection
  },
): void
```

Preserve the disposed/unlisten pattern exactly.

- [ ] **Step 4: Run controller and App tests**

```bash
pnpm --dir ui test -- useViewerController.test.tsx App.test.tsx
pnpm --dir ui check
```

Expected: PASS; `useViewerController` still satisfies the contract test.

- [ ] **Step 5: Commit operation/lifecycle extraction**

```bash
git add ui/src/state/controllers/useOperationController.ts ui/src/state/controllers/useLifecycleSubscriptions.ts ui/src/state/useViewerController.ts ui/src/state/useViewerController.test.tsx
git commit -m "refactor(ui): extract operation lifecycle controllers"
```

### Task 7: Extract App Coordinators

**Files:**
- Create: `ui/src/app/useAppShellState.ts`
- Create: `ui/src/app/usePreviewSession.ts`
- Create: `ui/src/app/useOperationDialogs.ts`
- Create: `ui/src/app/useRadialMenuSession.ts`
- Modify: `ui/src/App.tsx`
- Test: `ui/src/App.test.tsx`

**Interfaces:**
- Hooks own local UI session state only; they do not invoke Tauri directly.
- `App` remains the only default export and keeps the same `AppProps`.
- `useAppShellState(projectSessionId)` and `usePreviewSession(projectSessionId)` reset only when the backend session changes.
- `useOperationDialogs` also receives project status, access, and active-operation lifecycle so a close request or busy operation invalidates an open mutation dialog immediately.
- `useRadialMenuSession` receives `sessionId:generation`, project status, and App's organization/overlay context key so generation and visible-context changes invalidate a frozen radial snapshot.

- [ ] **Step 1: Run existing App session-reset characterizations**

```bash
pnpm --dir ui test -- App.test.tsx -t "closes the radial menu when external projection repair removes its file"
pnpm --dir ui test -- App.test.tsx -t "invalidates an open operation dialog when project closing begins"
pnpm --dir ui test -- App.test.tsx -t "invalidates a radial snapshot when the same session advances generation"
pnpm --dir ui test -- App.test.tsx -t "keeps grid selection and scroll mounted across image preview"
```

Expected: all four tests pass before extraction.

- [ ] **Step 2: Extract local state hooks**

Use these public shapes:

```ts
import type { PointerEvent as ReactPointerEvent } from 'react'

export interface AppShellState {
  sidebarCollapsed: boolean
  sidebarWidth: number
  projectMenuOpen: boolean
  setProjectMenuOpen(open: boolean): void
  toggleSidebar(): void
  startSidebarResize(event: ReactPointerEvent<HTMLButtonElement>): void
}

export interface PreviewSessionState {
  activePreview: PreviewSession | null
  dimensions: Record<string, { width: number; height: number } | undefined>
  openPreview(session: PreviewSession): void
  closePreview(): void
  recordDimensions(entityId: string, width: number, height: number): void
}
```

Move `OperationDialog`, `RadialMenuSession`, and `PreviewSession` types into the hook modules that own them and export only types used by App.

Preserve these reset keys:

- App shell/preview/dialog session cleanup: `state.project?.sessionId ?? 'no-session'`;
- radial identity: `${state.project.sessionId}:${state.project.generation}` or `no-project`;
- radial context: the existing `organizationWorkspaceIdentity` plus preview/compare/info/dialog/results visibility;
- dialog validity: `state.status === 'active'`, read-write access, and no non-completed active operation.

- [ ] **Step 3: Compose App from the hooks**

Keep cross-domain decisions in App:

- controller commands;
- current project identity;
- workspace routing;
- operation/preview/compare transitions.

Move local state mechanics and event-listener cleanup into the hooks.

- [ ] **Step 4: Run App and dialog tests**

```bash
pnpm --dir ui test -- App.test.tsx RadialFileMenu.test.tsx CloseOperationDialog.test.tsx DestinationDialog.test.tsx
pnpm --dir ui check
```

Expected: PASS.

- [ ] **Step 5: Commit App coordinators**

```bash
git add ui/src/app ui/src/App.tsx ui/src/App.test.tsx
git commit -m "refactor(ui): extract app session coordinators"
```

### Task 8: Extract ContentBrowser Presentation Units

**Files:**
- Create: `ui/src/components/contentBrowser/ImageCell.tsx`
- Create: `ui/src/components/contentBrowser/OrganizationDragHandle.tsx`
- Create: `ui/src/components/contentBrowser/contentSelection.ts`
- Create: `ui/src/components/contentBrowser/contentSelection.test.ts`
- Modify: `ui/src/components/ContentBrowser.tsx`
- Modify: `ui/src/components/ContentBrowser.test.tsx`

**Interfaces:**
- Preserve `ContentBrowserProps`.
- `ImageCell` receives only one file, selection/focus state, image request callbacks, marker label and pointer callbacks.
- `contentSelection.ts` contains pure ordered-selection helpers.

- [ ] **Step 1: Add pure selection tests**

Create `contentSelection.test.ts`:

```ts
import { describe, expect, it } from 'vitest'
import { rangeSelection, toggleSelection } from './contentSelection'

describe('content selection', () => {
  const order = ['a', 'b', 'c', 'd']

  it('selects a forward range', () => {
    expect(rangeSelection(order, 'b', 'd')).toEqual(['b', 'c', 'd'])
  })

  it('normalizes a reverse range to display order', () => {
    expect(rangeSelection(order, 'd', 'b')).toEqual(['b', 'c', 'd'])
  })

  it('falls back to the target when the anchor is absent', () => {
    expect(rangeSelection(order, 'missing', 'c')).toEqual(['c'])
  })

  it('adds an unselected target', () => {
    expect(toggleSelection(['a'], 'c')).toEqual(['a', 'c'])
  })

  it('removes an already selected target', () => {
    expect(toggleSelection(['a', 'c'], 'c')).toEqual(['a'])
  })
})
```

- [ ] **Step 2: Run and verify the new test fails before implementation**

```bash
pnpm --dir ui test -- contentSelection.test.ts
```

Expected: FAIL because the module does not exist.

- [ ] **Step 3: Implement pure selection helpers**

```ts
export function rangeSelection(
  orderedEntityIds: readonly string[],
  anchorId: string,
  targetId: string,
): string[] {
  const anchor = orderedEntityIds.indexOf(anchorId)
  const target = orderedEntityIds.indexOf(targetId)
  if (target < 0) return []
  if (anchor < 0) return [targetId]
  const start = Math.min(anchor, target)
  const end = Math.max(anchor, target)
  return orderedEntityIds.slice(start, end + 1)
}

export function toggleSelection(
  selectedEntityIds: readonly string[],
  targetId: string,
): string[] {
  return selectedEntityIds.includes(targetId)
    ? selectedEntityIds.filter((entityId) => entityId !== targetId)
    : [...selectedEntityIds, targetId]
}
```

For Shift-click and Shift-arrow integration, preserve additive selection semantics:

```ts
const range = rangeSelection(
  allFiles.map((candidate) => candidate.entityId),
  anchorId.current,
  file.entityId,
)
commitSelection(new Set([...selected, ...range]))
```

Use `toggleSelection([...selected], file.entityId)` only for Command-click. Keep ordinary click, marquee, active item, and anchor updates unchanged.

- [ ] **Step 4: Move leaf components unchanged**

Move the existing `ImageCell` and `OrganizationDragHandle` implementations, preserving props, roles, labels, pointer capture and focus behavior. Export named components and import them from `ContentBrowser.tsx`.

- [ ] **Step 5: Run browser interaction tests**

```bash
pnpm --dir ui test -- ContentBrowser.test.tsx contentSelection.test.ts
pnpm --dir ui check
```

Expected: PASS.

- [ ] **Step 6: Commit browser extraction**

```bash
git add ui/src/components/contentBrowser ui/src/components/ContentBrowser.tsx ui/src/components/ContentBrowser.test.tsx
git commit -m "refactor(ui): split content browser presentation"
```

### Task 9: Run the React Exit Gate

**Files:**
- Verify only.

- [ ] **Step 1: Run all UI tests**

```bash
pnpm --dir ui test
```

Expected: every UI test file passes.

- [ ] **Step 2: Run strict check and production build**

```bash
pnpm --dir ui check
pnpm --dir ui build
```

Expected: PASS.

- [ ] **Step 3: Run full clean verification**

```bash
pnpm verify:clean
```

Expected: PASS with no new worktree entries.

- [ ] **Step 4: Inspect facade and module boundaries**

```bash
git diff --check
rg -n '^export function viewerReducer|^export function useViewerController|^export default function App|^export default function ContentBrowser' ui/src
```

Expected: exactly one compatibility facade for each existing public entry point.
