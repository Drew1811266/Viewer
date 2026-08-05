# Viewer Progressive Folder Loading Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the artificial project-loading delay and full-workspace thumbnail flash, then add bounded project-session thumbnail reuse so folder changes feel immediate and images appear progressively in stable slots.

**Architecture:** Keep folder projection metadata and image representation loading as separate state layers. The Viewer reducer owns an optimistic `projectionTransition`, the project-session controller keeps request ordering authoritative, `ViewerWorkspace` owns one pure project-thumbnail cache, and `ContentBrowser`/`FolderOverview` share its stable request function while retaining item-local thumbnail states and existing virtualization.

**Tech Stack:** React 19, TypeScript 6, Vitest 4, Testing Library, Tauri `ViewerBridge`, Biome, existing Viewer visual-acceptance and native development launch tooling.

## Global Constraints

- The approved behavior is design option 2 in `docs/superpowers/specs/2026-08-05-viewer-progressive-folder-loading-design.md`.
- Remove the fixed `600ms` native initial-projection hold; do not replace it with another artificial delay.
- Project initial loading may cover the workspace only while no usable `workspace` exists.
- Folder projection progress appears only after `120ms`, never changes layout, never blocks input, and is cleared on success, failure, replacement, close, and unmount.
- Thumbnail work must never hide the folder tree, project-root row, or content browser.
- Thumbnail cache keys are `projectSessionId + entityId + modifiedNs + maxPixels + scaleMilli`.
- Keep a separate in-flight Promise registry and a `512`-entry resolved LRU; failed results are not cached.
- Cache clearing increments a generation so old in-flight requests cannot repopulate the cleared session.
- Preserve existing visible-row virtualization and item-local placeholder/error behavior.
- Do not add adjacent-folder prefetching, new dependencies, Rust decoder work, Windows-native work, large fades, or full-surface crossfades.
- Do not modify macOS display resolution, display scaling, or Dock settings during validation.
- Use deterministic deferred Promises and fake timers for repeated performance/race checks; run only one bounded native observation after automated checks pass.

---

## File Structure

### New files

- `ui/src/app/projectThumbnailCache.ts` — pure cache keying, in-flight request coalescing, resolved LRU, failure retry, and generation-safe clearing.
- `ui/src/app/projectThumbnailCache.test.ts` — deterministic cache contract tests.
- `ui/src/app/useDelayedProjectionProgress.ts` — the single `120ms` delayed-visibility hook for folder projection progress.
- `ui/src/app/useDelayedProjectionProgress.test.tsx` — fake-timer cleanup and replacement tests.
- `docs/reviews/2026-08-05-viewer-progressive-loading-acceptance.md` — final automated and bounded native evidence, using only measured values.

### Modified files

- `ui/src/state/viewerState.ts` — formal `ProjectionTransition` state and `projection_requested` action.
- `ui/src/state/reducers/workspaceReducer.ts` — begin/settle transition semantics without clearing the last good workspace on failure.
- `ui/src/state/viewerReducer.test.ts` — reducer transition and failure-preservation contracts.
- `ui/src/state/controllers/useProjectSessionController.ts` — remove the fixed hold, dispatch optimistic transitions, and keep newest-request-wins behavior.
- `ui/src/state/useViewerController.test.tsx` — immediate startup, optimistic folder selection, stale response, and stale error tests.
- `ui/src/App.tsx` — derive displayed folder selection, show delayed projection progress, remove global thumbnail blocking, and own the shared cache.
- `ui/src/App.test.tsx` — replace the obsolete thumbnail skeleton assertion and add deterministic folder-switch/cache integration coverage.
- `ui/src/components/ContentBrowser.tsx` — remove its private URL/Promise maps while retaining per-visible-item work feedback.
- `ui/src/components/ContentBrowser.test.tsx` — preserve progressive item behavior without assuming component-local representation caching.
- `ui/src/styles/app.css` — delete global thumbnail concealment and add the non-blocking progress line.
- `ui/src/styles/app.test.ts` — lock the absence of global concealment and the geometry/reduced-motion contract of progress.

---

### Task 1: Immediate, Ordered Folder Projection State

**Files:**
- Modify: `ui/src/state/viewerState.ts`
- Modify: `ui/src/state/reducers/workspaceReducer.ts`
- Modify: `ui/src/state/viewerReducer.test.ts`
- Modify: `ui/src/state/controllers/useProjectSessionController.ts`
- Modify: `ui/src/state/useViewerController.test.tsx`

**Interfaces:**
- Produces: `ProjectionTransition = { selectedFolderId: string | null; selectedFolderPath: string; showingAggregate: boolean }`.
- Produces: `ViewerState.projectionTransition: ProjectionTransition | null`.
- Produces: `ViewerAction` member `projection_requested` carrying session/generation plus all three transition fields.
- Preserves: `ProjectSessionController` public method signatures and the `RefreshProjection` signature.
- Consumed by Task 2: `state.projectionTransition` is the optimistic sidebar target and progress trigger.

- [ ] **Step 1: Add failing reducer tests for transition begin, success, failure, and close**

Add a focused test in `ui/src/state/viewerReducer.test.ts` that starts with a loaded content workspace, dispatches the following action, and asserts the old workspace remains while the target is recorded:

```ts
state = viewerReducer(state, {
  type: 'projection_requested',
  sessionId: project.sessionId,
  generation: project.generation,
  selectedFolderId: 'folder-b',
  selectedFolderPath: 'folder-b',
  showingAggregate: false,
})
expect(state.projectionTransition).toEqual({
  selectedFolderId: 'folder-b',
  selectedFolderPath: 'folder-b',
  showingAggregate: false,
})
expect(state.workspace).toEqual(previousWorkspace)
```

In the same test, assert `projection_loaded` clears `projectionTransition`; then begin another transition, dispatch `projection_failed`, and assert it clears the transition, retains `previousWorkspace`, and sets `errorMessage`. Assert `project_closed` returns `projectionTransition: null` through `initialViewerState`.

- [ ] **Step 2: Add failing controller tests for zero hold and newest-request-wins**

Replace `keeps the native launch loading surface visible before publishing the first projection` in `ui/src/state/useViewerController.test.tsx` with an immediate-start contract:

```ts
it('requests the first projection immediately after the project opens', async () => {
  const viewer = bridge()
  const workspace = deferred<Awaited<ReturnType<ViewerBridge['queryFolder']>>>()
  vi.mocked(viewer.queryFolder).mockImplementation(() => workspace.promise)
  const { result } = renderHook(() => useViewerController(viewer))

  let opening!: ReturnType<typeof result.current.openProject>
  act(() => {
    opening = result.current.openProject('/fixture/project')
  })
  await act(async () => {
    await Promise.resolve()
    await Promise.resolve()
  })

  expect(viewer.queryFolder).toHaveBeenCalledOnce()
  expect(result.current.state.projectionTransition).toEqual({
    selectedFolderId: null,
    selectedFolderPath: '',
    showingAggregate: false,
  })

  await act(async () => {
    workspace.resolve({ workspace: 'empty' })
    await opening
  })
  expect(result.current.state.projectionTransition).toBeNull()
})
```

Extend the local `deferred<T>()` helper with `reject`, then add a second test with folder tree entries `folder-a`, `folder-b`, and `folder-c` plus three deferred `queryFolder` results. Call `selectFolder('folder-a')`, `selectFolder('folder-b')`, and `selectFolder('folder-c')` without awaiting the earlier calls. Assert the transition immediately targets C; resolve C first, resolve A, and reject B; assert the final workspace and `selectedFolderId` remain C and the stale failure does not change `errorMessage`.

- [ ] **Step 3: Run the focused tests and verify RED**

Run:

```bash
pnpm --dir ui test src/state/viewerReducer.test.ts src/state/useViewerController.test.tsx
```

Expected: FAIL because `projection_requested`/`projectionTransition` do not exist and the first projection still waits for the `600ms` timer.

- [ ] **Step 4: Implement the transition state and reducer actions**

In `ui/src/state/viewerState.ts`, add:

```ts
export interface ProjectionTransition {
  selectedFolderId: string | null
  selectedFolderPath: string
  showingAggregate: boolean
}
```

Add `projectionTransition: ProjectionTransition | null` immediately after `workspace` in `ViewerState`, initialize it to `null` immediately after `workspace` in `initialViewerState`, and add this action to `ViewerAction`:

```ts
| {
    type: 'projection_requested'
    sessionId: string
    generation: number
    selectedFolderId: string | null
    selectedFolderPath: string
    showingAggregate: boolean
  }
```

In `reduceWorkspaceAction`, add the current-projection guard and transitions:

```ts
case 'projection_requested':
  if (!isCurrentProjection(state, action.sessionId, action.generation)) return state
  return {
    ...state,
    projectionTransition: {
      selectedFolderId: action.selectedFolderId,
      selectedFolderPath: action.selectedFolderPath,
      showingAggregate: action.showingAggregate,
    },
    errorMessage: null,
  }
case 'projection_loaded':
  if (!isCurrentProjection(state, action.sessionId, action.generation)) return state
  return repairContextAfterProjection(state, {
    ...state,
    folders: action.folders,
    workspace: action.workspace,
    projectionTransition: null,
    selectedFolderId: action.selectedFolderId,
    selectedFolderPath: action.selectedFolderPath,
    showingAggregate: action.showingAggregate,
    errorMessage: null,
  })
case 'projection_failed':
  if (!isCurrentProjection(state, action.sessionId, action.generation)) return state
  return { ...state, projectionTransition: null, errorMessage: action.message }
```

- [ ] **Step 5: Remove the hold and dispatch every current projection request immediately**

Delete `NATIVE_INITIAL_PROJECTION_HOLD_MS`, `holdNativeInitialProjectionForVisualStability`, `initialProjectionHoldRef`, every write to that ref, and the scan-listener early return that depends on it.

At the start of `refreshProjection`, after updating `desiredProjectionRef` and assigning `requestId`, dispatch:

```ts
dispatch({
  type: 'projection_requested',
  sessionId: project.sessionId,
  generation: project.generation,
  selectedFolderId,
  selectedFolderPath,
  showingAggregate,
})
```

Keep the existing `if (requestId !== projectionRequestRef.current) return` guards before both success and failure dispatches. In `openProject`, dispatch `project_opened` and immediately `await refreshProjection(project, null, '', false)` with no timer between them.

- [ ] **Step 6: Run focused state/controller tests and verify GREEN**

Run:

```bash
pnpm --dir ui test src/state/viewerReducer.test.ts src/state/useViewerController.test.tsx
```

Expected: PASS; fake timers no longer need a native-loading global, current failure retains the last workspace, and stale success/failure cannot replace the newest folder.

- [ ] **Step 7: Commit the projection state change**

```bash
git add ui/src/state/viewerState.ts ui/src/state/reducers/workspaceReducer.ts ui/src/state/viewerReducer.test.ts ui/src/state/controllers/useProjectSessionController.ts ui/src/state/useViewerController.test.tsx
git commit -m "fix: make folder projection transitions immediate"
```

---

### Task 2: Non-Blocking Projection and Thumbnail Feedback

**Files:**
- Create: `ui/src/app/useDelayedProjectionProgress.ts`
- Create: `ui/src/app/useDelayedProjectionProgress.test.tsx`
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/App.test.tsx`
- Modify: `ui/src/styles/app.css`
- Modify: `ui/src/styles/app.test.ts`

**Interfaces:**
- Consumes: `ProjectionTransition | null` from Task 1.
- Produces: `useDelayedProjectionProgress(transition, workspaceAvailable, delayMs?): boolean`.
- Produces: `.projection-progress[role="progressbar"][aria-label="正在切换文件夹"]` after `120ms` only.
- Preserves: `thumbnailTask` as TaskBar feedback; it no longer controls workspace visibility.

- [ ] **Step 1: Write failing fake-timer tests for delayed progress visibility**

Create `ui/src/app/useDelayedProjectionProgress.test.tsx` with a harness that renders the hook result as `data-visible`. Cover all four cases:

```tsx
function Harness({ transition, workspaceAvailable }: Props) {
  const visible = useDelayedProjectionProgress(transition, workspaceAvailable)
  return <output data-testid="progress-state" data-visible={visible} />
}
```

Assertions:

1. pending for `119ms` remains false;
2. at `120ms` becomes true;
3. success (`transition=null`) hides immediately;
4. replacing transition A with a new object B restarts the full delay, and unmount clears the timer without a React state-update warning.

- [ ] **Step 2: Rewrite the obsolete App skeleton test and add optimistic switching coverage**

Replace `keeps the approved project skeleton visible while initial thumbnails are pending` in `ui/src/App.test.tsx` with `keeps the project structure and item placeholders visible while thumbnails are pending`. With `requestImage` deferred, assert:

```ts
expect(screen.queryByRole('status', { name: '项目内容加载中' })).not.toBeInTheDocument()
expect(within(sidebar).getByRole('button', { name: 'Catalog' })).toBeVisible()
expect(sidebar.querySelectorAll('.folder-tree-skeleton-row')).toHaveLength(0)
expect(screen.getByRole('option', { name: 'front.jpg' })).toBeVisible()
expect(screen.getAllByLabelText('缩略图加载中').length).toBeGreaterThan(0)
expect(screen.getByText('正在生成缩略图')).toBeVisible()
```

Add a second App test with a loaded A workspace and deferred B query. Click B and assert B becomes the tree's selected item while A content remains. Advance `119ms` and assert no progressbar; advance one more millisecond and assert `正在切换文件夹`; resolve B and assert B content replaces A and the progressbar disappears.

- [ ] **Step 3: Add failing CSS contract assertions**

In `ui/src/styles/app.test.ts`, assert no parsed rule has either legacy selector:

```ts
expect(
  rules.find(
    ({ selector }) =>
      selector === ".content-workspace-surface[data-thumbnail-loading='true'] > .content-browser" ||
      selector === ".content-workspace-surface[data-thumbnail-loading='true'] > .workspace-loading",
  ),
).toBeUndefined()
```

Assert `.projection-progress` uses `position: absolute`, `height: 2px`, `inset-inline: 0`, `top: 0`, a Viewer accent token, and `pointer-events: none`. Assert its reduced-motion rule does not use a continuous animation.

- [ ] **Step 4: Run the hook, App, and style tests and verify RED**

Run:

```bash
pnpm --dir ui test src/app/useDelayedProjectionProgress.test.tsx src/App.test.tsx src/styles/app.test.ts
```

Expected: FAIL because the hook/progressbar do not exist and thumbnail tasks still hide the content and sidebar.

- [ ] **Step 5: Implement the delayed progress hook**

Create `ui/src/app/useDelayedProjectionProgress.ts`:

```ts
import { useEffect, useState } from 'react'
import type { ProjectionTransition } from '../state/viewerState'

export const PROJECTION_PROGRESS_DELAY_MS = 120

export function useDelayedProjectionProgress(
  transition: ProjectionTransition | null,
  workspaceAvailable: boolean,
  delayMs = PROJECTION_PROGRESS_DELAY_MS,
) {
  const [visible, setVisible] = useState(false)
  useEffect(() => {
    setVisible(false)
    if (transition === null || !workspaceAvailable) return
    const timer = window.setTimeout(() => setVisible(true), delayMs)
    return () => window.clearTimeout(timer)
  }, [delayMs, transition, workspaceAvailable])
  return visible
}
```

Using the transition object as a dependency intentionally restarts the timer for a replacement request, even if both requests target the same folder identity.

- [ ] **Step 6: Separate App's initial, projection, and thumbnail states**

In `ViewerWorkspace`, remove `thumbnailLoading`. Add:

```ts
const displayedFolderId =
  state.projectionTransition?.selectedFolderId ?? state.selectedFolderId
const projectionProgressVisible = useDelayedProjectionProgress(
  state.projectionTransition,
  state.workspace !== null,
)
```

Use `displayedFolderId` for project-root `aria-pressed` and `FolderTree selectedId`. Render the project root whenever `state.workspace !== null`. Set `FolderTree loading={state.workspace === null}`.

Inside the workspace section, render:

```tsx
{projectionProgressVisible && (
  <div
    className="projection-progress"
    role="progressbar"
    aria-label="正在切换文件夹"
  />
)}
```

Remove `data-thumbnail-loading`, remove the thumbnail-driven `WorkspaceLoadingState`, and leave the existing `state.workspace === null` loading state unchanged. Keep `onThumbnailTaskChange={setThumbnailTask}` so TaskBar remains informative.

- [ ] **Step 7: Replace the concealment CSS with the progress line**

Delete all three `[data-thumbnail-loading='true']` rules. Ensure `.workspace` is a positioning context and add:

```css
.workspace {
  position: relative;
}

.projection-progress {
  background: var(--viewer-accent);
  block-size: 2px;
  height: 2px;
  inset-inline: 0;
  pointer-events: none;
  position: absolute;
  top: 0;
  z-index: 2;
}

@media (prefers-reduced-motion: reduce) {
  .projection-progress {
    animation: none;
  }
}
```

Do not add opacity transitions to the workspace, grid, sidebar, or image cards.

- [ ] **Step 8: Run focused UI tests and verify GREEN**

Run:

```bash
pnpm --dir ui test src/app/useDelayedProjectionProgress.test.tsx src/App.test.tsx src/styles/app.test.ts
```

Expected: PASS; unresolved thumbnails leave the grid and sidebar visible, and only a projection request lasting at least `120ms` shows the non-blocking line.

- [ ] **Step 9: Commit the non-blocking feedback change**

```bash
git add ui/src/app/useDelayedProjectionProgress.ts ui/src/app/useDelayedProjectionProgress.test.tsx ui/src/App.tsx ui/src/App.test.tsx ui/src/styles/app.css ui/src/styles/app.test.ts
git commit -m "fix: keep Viewer interactive during image loading"
```

---

### Task 3: Generation-Safe Project Thumbnail Cache

**Files:**
- Create: `ui/src/app/projectThumbnailCache.ts`
- Create: `ui/src/app/projectThumbnailCache.test.ts`

**Interfaces:**
- Produces: `ThumbnailLoader = (file: BrowserFile, maxPixels: number, scaleMilli: number) => Promise<string>`.
- Produces: `ProjectThumbnailCache = { request: ThumbnailLoader; clear(): void }`.
- Produces: `createProjectThumbnailCache(projectSessionId, loader, resolvedCapacity?): ProjectThumbnailCache`.
- Produces: `PROJECT_THUMBNAIL_CACHE_CAPACITY = 512`.
- Consumed by Task 4: one cache instance per `projectSessionId` in `ViewerWorkspace`.

- [ ] **Step 1: Write failing cache key and request-coalescing tests**

Create `ui/src/app/projectThumbnailCache.test.ts`. Use a local `BrowserFile` fixture and deferred loader. Assert:

```ts
const first = cache.request(file, 198, 1_000)
const duplicate = cache.request({ ...file }, 198, 1_000)
expect(loader).toHaveBeenCalledOnce()
expect(duplicate).toBe(first)
```

Resolve the request, call again, and assert no new loader call. Then vary `modifiedNs`, `maxPixels`, and `scaleMilli` one at a time and assert each variant causes a distinct loader call.

- [ ] **Step 2: Write failing failure, LRU, and clear-generation tests**

Cover these exact behaviors:

- a rejected Promise is removed so the next same-key call invokes the loader again;
- `PROJECT_THUMBNAIL_CACHE_CAPACITY` equals `512`;
- a cache created with test capacity `2` touches key A, inserts C, and evicts B, not A;
- after `clear()`, the same key starts a new loader request;
- resolving the old pre-clear Promise after the new request does not overwrite or delete the new in-flight request;
- a later same-key read returns the new URL and does not return the old URL.

- [ ] **Step 3: Run the cache test and verify RED**

Run:

```bash
pnpm --dir ui test src/app/projectThumbnailCache.test.ts
```

Expected: FAIL because the cache module does not exist.

- [ ] **Step 4: Implement the cache with separate in-flight and resolved maps**

Create the production module with these declarations:

```ts
export const PROJECT_THUMBNAIL_CACHE_CAPACITY = 512

export type ThumbnailLoader = (
  file: BrowserFile,
  maxPixels: number,
  scaleMilli: number,
) => Promise<string>

export interface ProjectThumbnailCache {
  request: ThumbnailLoader
  clear(): void
}
```

Build the key exactly as:

```ts
const key = [projectSessionId, file.entityId, file.modifiedNs, maxPixels, scaleMilli].join(':')
```

Implement `request` in this order:

1. return and touch a resolved URL;
2. return the exact existing in-flight Promise;
3. capture the current cache generation and call `loader` once;
4. on success, remove only itself from `inFlight`, and write/touch/prune `resolved` only when its generation is still current;
5. on failure, remove only itself and rethrow;
6. put the Promise into `inFlight` before returning it.

The self-removal guard must compare identity so an old request cannot delete a new request for the same key:

```ts
if (inFlight.get(key) === request) inFlight.delete(key)
```

Implement `clear()` as:

```ts
generation += 1
inFlight.clear()
resolved.clear()
```

Prune only `resolved`; never evict `inFlight`.

- [ ] **Step 5: Run cache tests and verify GREEN**

Run:

```bash
pnpm --dir ui test src/app/projectThumbnailCache.test.ts
```

Expected: PASS for coalescing, failure retry, key invalidation, LRU order, and clear-generation isolation.

- [ ] **Step 6: Commit the cache core**

```bash
git add ui/src/app/projectThumbnailCache.ts ui/src/app/projectThumbnailCache.test.ts
git commit -m "feat: add project thumbnail cache"
```

---

### Task 4: Share the Cache Across Content and Folder Overview

**Files:**
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/App.test.tsx`
- Modify: `ui/src/components/ContentBrowser.tsx`
- Modify: `ui/src/components/ContentBrowser.test.tsx`

**Interfaces:**
- Consumes: `createProjectThumbnailCache` and `ThumbnailLoader` from Task 3.
- Produces: one stable `requestThumbnail: ThumbnailLoader` shared by `ContentBrowser`, `FolderOverview`, and `FolderFilmstripRow` for the active project session.
- Preserves: `ContentBrowserProps.requestThumbnail` and `FolderOverviewProps.requestThumbnail` signatures.
- Preserves: existing `AspectThumbnail` local loading/ready/failed state and visible-range mounting.

- [ ] **Step 1: Add a failing App test for cross-surface request reuse**

In `ui/src/App.test.tsx`, configure the first projection as `categoryWorkspace()` and the selected folder projection as content containing the same `image-1`, `modifiedNs`, ratio, and `198/1000` representation. Wait for the filmstrip thumbnail, click `打开 B01`, wait for the content option, then assert:

```ts
const matchingCalls = vi.mocked(viewer.requestImage).mock.calls.filter(
  ([request]) =>
    request.entityId === 'image-1' &&
    request.representation.kind === 'thumbnail' &&
    request.representation.maxPixels === 198 &&
    request.representation.scaleMilli === 1_000,
)
expect(matchingCalls).toHaveLength(1)
```

Add a companion session-reset assertion: close the project, reopen it, render the same image/representation, and expect the matching call count to increase to `2`, proving cache isolation instead of global reuse.

- [ ] **Step 2: Adjust ContentBrowser tests to make the cache boundary explicit**

Rename the shelf-toggle test to `keeps mounted thumbnail requests stable across shelf toggles` and retain its resolved/pending progressive assertions. Remove assertions that describe `ContentBrowser` as the owner of a URL cache. Add a test that rerenders with one thumbnail resolved and one pending, then asserts the resolved image is visible while the pending cell still has `缩略图加载中`; this locks progressive item behavior independently of cache ownership.

Keep `FolderOverview` and `FolderFilmstripRow` free of representation caches; their existing regression tests continue to prove that the injected request function is forwarded to mounted item-local thumbnails. Duplicate suppression belongs only to the project cache.

- [ ] **Step 3: Run integration/component tests and verify RED**

Run:

```bash
pnpm --dir ui test src/App.test.tsx src/components/ContentBrowser.test.tsx
```

Expected: FAIL because category and content surfaces still call `bridge.requestImage` independently and `ContentBrowser` still owns private representation maps.

- [ ] **Step 4: Own one cache in ViewerWorkspace**

Replace the duplicate direct request functions with one raw loader and one cache:

```ts
const loadThumbnail = useCallback<ThumbnailLoader>(
  (file, maxPixels, scaleMilli) =>
    bridge
      .requestImage({
        entityId: file.entityId,
        representation: { kind: 'thumbnail', maxPixels, scaleMilli },
      })
      .then((image) => image.url),
  [bridge],
)

const thumbnailCache = useMemo(
  () => createProjectThumbnailCache(projectSessionId, loadThumbnail),
  [loadThumbnail, projectSessionId],
)

useEffect(() => () => thumbnailCache.clear(), [thumbnailCache])
const requestThumbnail = thumbnailCache.request
```

Pass this exact `requestThumbnail` to both `FolderOverview` and `ContentBrowser`. Delete `requestContentThumbnail`.

- [ ] **Step 5: Remove ContentBrowser's private representation cache**

Delete:

```ts
const cache = useRef(new Map<string, string>())
const pending = useRef(new Map<string, Promise<string>>())
```

Keep the existing work counters, but make `loadThumbnail` call the injected shared loader directly:

```ts
const loadThumbnail = useCallback(
  (file: BrowserFile, maxPixels: number, scaleMilli: number) => {
    if (requestThumbnail === undefined) {
      return Promise.reject(new Error('thumbnail unavailable'))
    }
    setWork((current) => ({ ...current, requested: current.requested + 1 }))
    return requestThumbnail(file, maxPixels, scaleMilli).then(
      (url) => {
        setWork((current) => ({ ...current, completed: current.completed + 1 }))
        return url
      },
      (error: unknown) => {
        setWork((current) => ({ ...current, failed: current.failed + 1 }))
        throw error
      },
    )
  },
  [requestThumbnail],
)
```

Add a component-lifetime ref so completion of a shared request after `ContentBrowser` unmounts still populates the project cache but does not update the unmounted browser's task counter:

```ts
const mounted = useRef(true)
useEffect(
  () => () => {
    mounted.current = false
  },
  [],
)
```

Guard only the two `setWork` completion calls with `if (mounted.current)`. The cache itself must settle regardless of the consumer's lifetime.

Do not change `AspectVirtualGrid`, `AspectThumbnail`, or filmstrip visible-window logic.

- [ ] **Step 6: Run cache and surface integration tests and verify GREEN**

Run:

```bash
pnpm --dir ui test src/app/projectThumbnailCache.test.ts src/App.test.tsx src/components/ContentBrowser.test.tsx src/components/FolderOverview.test.tsx src/components/FolderFilmstripRow.test.tsx src/components/AspectThumbnail.test.tsx src/components/AspectVirtualGrid.test.tsx
```

Expected: PASS; same-session same-key category/content calls total `1`, reopening the project totals `2`, per-item progressive states remain local, and visible-window request counts stay bounded.

- [ ] **Step 7: Commit the shared-cache integration**

```bash
git add ui/src/App.tsx ui/src/App.test.tsx ui/src/components/ContentBrowser.tsx ui/src/components/ContentBrowser.test.tsx
git commit -m "perf: reuse thumbnails across Viewer surfaces"
```

---

### Task 5: Tiered Verification, Visual Acceptance, and Bounded Native Observation

**Files:**
- Create: `docs/reviews/2026-08-05-viewer-progressive-loading-acceptance.md`

**Interfaces:**
- Consumes: completed Tasks 1–4.
- Produces: committed evidence that separates deterministic contracts, affected visual states, and one native observation.
- Does not produce: another native 89-state loop, display-setting changes, Dock changes, adjacent-folder prefetch, or backend decoder changes.

- [ ] **Step 1: Run all affected deterministic tests as one bounded batch**

Run:

```bash
pnpm --dir ui test \
  src/state/viewerReducer.test.ts \
  src/state/useViewerController.test.tsx \
  src/app/useDelayedProjectionProgress.test.tsx \
  src/app/projectThumbnailCache.test.ts \
  src/App.test.tsx \
  src/components/ContentBrowser.test.tsx \
  src/components/FolderOverview.test.tsx \
  src/components/FolderFilmstripRow.test.tsx \
  src/components/AspectThumbnail.test.tsx \
  src/components/AspectVirtualGrid.test.tsx \
  src/styles/app.test.ts
```

Expected: PASS in one Vitest process with no real-time `600ms` sleep and no repeated native file opening.

- [ ] **Step 2: Run static checks and the complete repository gate**

Run:

```bash
pnpm --dir ui check
pnpm verify
```

Expected: both commands exit `0`; TypeScript, Biome, UI tests/build, Rust checks/tests, policy, security, dependency, and license gates pass.

- [ ] **Step 3: Confirm checks left no uncommitted formatting changes**

Run:

```bash
git diff --check
git status --short
```

Expected: no whitespace errors and no uncommitted product/test files. A check failure returns to the task that owns the listed file; do not stage unrelated worktree changes.

- [ ] **Step 4: Capture only the two affected loading visual states**

Run the deterministic browser acceptance at the primary compact viewport:

```bash
pnpm accept:visual -- --id LAU-05 --id LAU-06 --viewport 1024x720
```

Expected: `2/2` states pass, with `LAU-05` showing the true project/workspace loading state and `LAU-06` showing the real content grid with item-local thumbnail placeholders and TaskBar feedback, not a full-grid concealment layer.

Inspect the two product/combined PNGs once. Do not loop screenshots after a pass.

- [ ] **Step 5: Run one native development observation without changing system settings**

Before launch, record and later re-read these system values:

```bash
defaults read com.apple.dock autohide
system_profiler SPDisplaysDataType
```

Start the canonical single-instance development build:

```bash
pnpm start:viewer
```

In the existing Viewer acceptance fixture, perform exactly one sequence: open project, select two previously unvisited folders, return to the first, quickly select three folder rows, then move between project overview and a content folder. Observe that selection responds immediately, old content remains only until metadata returns, the new placeholder grid appears without a white flash, and the return visit reuses thumbnails. Close only the test Viewer instance.

Re-read the two system values and assert they match the pre-launch values. Do not open System Settings and do not change resolution, scale, or Dock autohide.

- [ ] **Step 6: Write the acceptance record using actual outputs only**

Create `docs/reviews/2026-08-05-viewer-progressive-loading-acceptance.md` with these sections and every cell populated from the current run:

```markdown
# Viewer Progressive Loading Acceptance

## Build identity
- Branch and commit
- Exact Viewer executable path

## Deterministic contracts
| Contract | Command | Observed result |
| --- | --- | --- |
| Artificial initial delay | focused controller test | 0ms fixed hold; pass/fail count |
| Non-blocking thumbnail work | focused App test | sidebar/grid/root visibility result |
| Progressive items | ContentBrowser test | resolved/pending item result |
| Request coalescing | cache test | same-key bridge call count |
| LRU/session isolation | cache test | eviction and reopen call counts |
| Newest folder wins | controller test | final selected workspace |

## Visual evidence
- LAU-05 output directory and verdict
- LAU-06 output directory and verdict

## Bounded native observation
- Steps performed once
- First-visit behavior
- Return-visit behavior
- Rapid-switch final folder
- Console/process errors

## System restoration check
- Display state before and after
- Dock autohide before and after
```

Copy the actual test counts, commit, executable path, evidence directories, native observations, and before/after system values from this run. If a check fails, record the failure and return to the responsible task; never write a pass without evidence.

- [ ] **Step 7: Commit the acceptance evidence**

```bash
git add docs/reviews/2026-08-05-viewer-progressive-loading-acceptance.md
git commit -m "docs: record progressive loading acceptance"
```

- [ ] **Step 8: Verify the final branch is clean and identify the delivered commits**

Run:

```bash
git status --short
git log --oneline -6
```

Expected: no uncommitted files from this work and a readable sequence containing the projection, non-blocking feedback, cache core, cache integration, and acceptance commits.

---

## Spec Coverage Map

| Approved requirement | Implemented by |
| --- | --- |
| Remove fixed `600ms` hold | Task 1 |
| Immediate optimistic folder selection | Tasks 1–2 |
| Keep last content during metadata query | Task 1 |
| Ignore stale success and stale failure | Task 1 |
| Show non-blocking progress only after `120ms` | Task 2 |
| Never hide sidebar/root/grid for thumbnails | Task 2 |
| Stable per-item placeholders and progressive image reveal | Tasks 2 and 4 |
| Project-session shared cache | Tasks 3–4 |
| Key includes session/file/version/size/scale | Task 3 |
| Same-key Promise coalescing and failure retry | Task 3 |
| `512` resolved-entry LRU | Task 3 |
| Generation-safe clear and session isolation | Tasks 3–4 |
| Preserve visible-range virtualization | Task 4 |
| No adjacent-folder prefetch or backend rewrite | Global constraints and Tasks 3–4 |
| Deterministic tests instead of repeated native loops | Tasks 1–5 |
| Affected visual acceptance only | Task 5 |
| One bounded native observation | Task 5 |
| Preserve/verify display and Dock settings | Task 5 |
