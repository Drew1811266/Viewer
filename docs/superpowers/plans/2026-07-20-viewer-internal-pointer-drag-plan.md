# Viewer Internal Pointer Drag Implementation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task by task. Use `superpowers:test-driven-development` for every behavior change, `superpowers:requesting-code-review` at every task checkpoint, and `superpowers:verification-before-completion` before claiming the stage is complete.

**Goal:** Replace Viewer’s nonfunctional HTML5/DataTransfer internal organization drag with a tested Pointer Events session while preserving Finder export, marquee selection, native Finder-folder import, and the existing Rust move/copy safety pipeline.

**Architecture:** `ContentBrowser` owns only pointer normalization and selection freezing at the `⋮⋮` handle. A new `useOrganizationPointerDrag` coordinator owns threshold activation, folder hit testing, validation, visual state, edge autoscroll, cancellation, and one-shot submission. `FolderTree` is a passive, controlled target surface identified only by opaque entity IDs. File mutation continues through the existing `dropFiles` callback and Rust command journal.

**Tech Stack:** React 19, TypeScript, Pointer Events, Vitest, Testing Library, Tauri 2, Rust, AppKit Finder export, pnpm.

---

## Non-negotiable constraints

- Keep Tauri’s native incoming file-drop handler enabled; never set `dragDropEnabled: false`.
- Do not use `DataTransfer`, a custom MIME type, HTML `dragover`, or HTML `drop` for Viewer-internal organization.
- The outer `role="option"` file item must not be draggable. A dedicated sibling body surface owns Finder export; the `⋮⋮` handle owns Pointer Events organization.
- Freeze ordered entity IDs and `move`/`copy` mode at PointerDown. Option means copy only when PointerDown occurs.
- Activate after 4 CSS px. Folder-tree edge autoscroll begins within 32 px and is capped at 18 px per animation frame.
- DOM state contains opaque entity IDs only, never absolute paths.
- Invalid, absent, cancelled, blurred, replaced, read-only, busy, compare-open, and unmounted sessions perform no file operation.
- Do not add runtime dependencies or Tauri capabilities.
- Commit and independently review each implementation task. Do not merge the M3 branch until automated and physical acceptance both pass.

## Task 1: Extract shared pointer geometry without changing marquee behavior

**Files:**

- Create: `ui/src/components/pointerGeometry.ts`
- Create: `ui/src/components/pointerGeometry.test.ts`
- Modify: `ui/src/components/marqueeSelection.ts`
- Test: `ui/src/components/marqueeSelection.test.ts`

### Step 1: Write failing generic geometry tests

Add tests that import APIs that do not yet exist:

```ts
import { describe, expect, it } from 'vitest'
import { pointDistance, verticalEdgeScrollDelta } from './pointerGeometry'

describe('pointerGeometry', () => {
  it('measures pointer distance in CSS pixels', () => {
    expect(pointDistance({ x: 10, y: 20 }, { x: 13, y: 24 })).toBe(5)
  })

  it('returns capped edge-scroll deltas and zero away from edges', () => {
    expect(verticalEdgeScrollDelta(100, 100, 500)).toBe(-18)
    expect(verticalEdgeScrollDelta(116, 100, 500)).toBe(-9)
    expect(verticalEdgeScrollDelta(300, 100, 500)).toBe(0)
    expect(verticalEdgeScrollDelta(484, 100, 500)).toBe(9)
    expect(verticalEdgeScrollDelta(500, 100, 500)).toBe(18)
  })
})
```

Run:

```bash
pnpm --dir ui vitest run src/components/pointerGeometry.test.ts
```

Expected: FAIL because `pointerGeometry.ts` does not exist.

### Step 2: Implement the generic helpers

Create:

```ts
export interface PointerPoint {
  x: number
  y: number
}

export function pointDistance(start: PointerPoint, current: PointerPoint): number {
  return Math.hypot(current.x - start.x, current.y - start.y)
}

export function verticalEdgeScrollDelta(
  pointerY: number,
  viewportTop: number,
  viewportBottom: number,
): number {
  const edge = 32
  const maximum = 18

  if (pointerY < viewportTop + edge) {
    return -Math.min(
      maximum,
      Math.max(0, ((viewportTop + edge - pointerY) / edge) * maximum),
    )
  }
  if (pointerY > viewportBottom - edge) {
    return Math.min(
      maximum,
      Math.max(0, ((pointerY - (viewportBottom - edge)) / edge) * maximum),
    )
  }
  return 0
}
```

Change `marqueeSelection.ts` to import the generic APIs while keeping its public API stable:

```ts
import {
  pointDistance,
  verticalEdgeScrollDelta,
  type PointerPoint,
} from './pointerGeometry'

export type MarqueePoint = PointerPoint
export const marqueeDistance = pointDistance
export const verticalAutoScrollDelta = verticalEdgeScrollDelta
```

Remove only the now-duplicated local interface and helper implementations. Leave marquee rectangle and grid intersection logic unchanged.

### Step 3: Verify focused and existing marquee tests

Run:

```bash
pnpm --dir ui vitest run src/components/pointerGeometry.test.ts src/components/marqueeSelection.test.ts src/components/VirtualGrid.test.tsx
pnpm --dir ui typecheck
```

Expected: PASS.

### Step 4: Review and commit

Review for exact compatibility with the existing marquee exports and no UI change. Then:

```bash
git add ui/src/components/pointerGeometry.ts ui/src/components/pointerGeometry.test.ts ui/src/components/marqueeSelection.ts
git commit -m "refactor: share pointer drag geometry"
```

## Task 2: Build the organization pointer coordinator as a tested state machine

**Files:**

- Create: `ui/src/state/useOrganizationPointerDrag.ts`
- Create: `ui/src/state/useOrganizationPointerDrag.test.tsx`

### Step 1: Define the public contract in failing tests

The hook must expose these path-free types:

```ts
export type OrganizationDragMode = 'move' | 'copy'

export type OrganizationPointerInput =
  | {
      type: 'start'
      pointerId: number
      entityIds: string[]
      mode: OrganizationDragMode
      clientX: number
      clientY: number
      captureNode: HTMLElement
    }
  | { type: 'move' | 'end'; pointerId: number; clientX: number; clientY: number }
  | { type: 'cancel'; pointerId: number }

export interface OrganizationDropTarget {
  entityId: string
  mode: OrganizationDragMode
  valid: boolean
}

export interface OrganizationDragView {
  clientX: number
  clientY: number
  itemCount: number
  mode: OrganizationDragMode
}
```

The options and result are:

```ts
interface UseOrganizationPointerDragOptions {
  disabled: boolean
  resetKey: string
  isDropTargetValid: (
    entityIds: readonly string[],
    destinationId: string,
    mode: OrganizationDragMode,
  ) => boolean
  onDrop: (
    entityIds: string[],
    destinationId: string,
    mode: OrganizationDragMode,
  ) => void
}

interface OrganizationPointerDragResult {
  dragView: OrganizationDragView | null
  dropTarget: OrganizationDropTarget | null
  handlePointerInput: (input: OrganizationPointerInput) => void
  cancel: () => void
}
```

Use `renderHook` with a real DOM surface/row fixture:

```ts
const surface = document.createElement('div')
surface.dataset.organizationDropSurface = ''
const row = document.createElement('div')
row.dataset.organizationFolderId = 'folder-target'
surface.append(row)
document.body.append(surface)
```

Stub `document.elementFromPoint`, `getBoundingClientRect`, `scrollBy`, pointer capture, `requestAnimationFrame`, and `cancelAnimationFrame` at their narrow boundaries.

Write RED tests proving:

1. A move shorter than 4 px stays armed, renders nothing, and PointerUp does not submit.
2. A move of exactly 4 px activates, resolves the closest folder row, and exposes a valid controlled target.
3. The hook calls `isDropTargetValid` with frozen ordered IDs and frozen mode.
4. Valid PointerUp clears state before calling `onDrop`, submits once, and releases pointer capture once.
5. Invalid and absent targets never call `onDrop`.
6. Escape, matching `pointercancel`, `window.blur`, `disabled` becoming true, `resetKey` changing, and unmount all cancel without submission.
7. An event from another pointer ID is ignored.
8. A pointer within either 32 px edge scrolls no faster than 18 px/frame, re-hit-tests after each frame, and stops RAF on cancel or center movement.
9. Rerendered validation and drop callbacks are current, not stale closures.

Run:

```bash
pnpm --dir ui vitest run src/state/useOrganizationPointerDrag.test.tsx
```

Expected: FAIL because the hook does not exist.

### Step 2: Implement session state and lifecycle cleanup

Use a ref for the authoritative session so move/up/RAF callbacks cannot observe stale React state. Keep React state only for rendering `dragView` and `dropTarget`.

Required constants and selectors:

```ts
export const ORGANIZATION_FOLDER_ATTRIBUTE = 'data-organization-folder-id'
export const ORGANIZATION_DROP_SURFACE_ATTRIBUTE = 'data-organization-drop-surface'
const ACTIVATION_DISTANCE = 4
```

Implement idempotent cleanup in this order:

1. cancel the outstanding RAF;
2. clear the session ref;
3. clear both React visual states;
4. release capture on the saved node only when it still owns the pointer;
5. never invoke `onDrop` from cleanup.

At start, reject disabled state, nonunique/empty IDs, or a second session. The component filters the mouse button before emitting `start`. Copy the ID array before storing it and call `setPointerCapture(pointerId)` inside a guarded block.

At activation and every dragging move:

```ts
const hit = document.elementFromPoint(clientX, clientY)
const row = hit?.closest<HTMLElement>('[data-organization-folder-id]') ?? null
const surface = row?.closest<HTMLElement>('[data-organization-drop-surface]') ?? null
const destinationId = surface ? row?.dataset.organizationFolderId ?? null : null
```

Compute validity only through the latest callback ref. PointerUp copies the valid session values, cleans up, then calls the latest `onDrop` exactly once.

Install and remove `keydown` and `blur` listeners in effects. Cancel whenever `disabled` or `resetKey` changes, including when a project/workspace generation is replaced.

For edge scrolling, locate the single marked surface, read its bounds, apply `verticalEdgeScrollDelta`, call `scrollBy({ top: delta })`, and schedule another frame only while the latest session remains dragging near an edge. Re-run hit testing after scrolling so virtualized folder rows can become targets.

### Step 3: Make every state-machine test green

Run:

```bash
pnpm --dir ui vitest run src/state/useOrganizationPointerDrag.test.tsx
pnpm --dir ui typecheck
```

Expected: PASS with no act warnings and no leaked RAF/listeners.

### Step 4: Review and commit

Review specifically for stale closure risks, duplicate submission, pointer-capture leaks, and DOM path leakage. Then:

```bash
git add ui/src/state/useOrganizationPointerDrag.ts ui/src/state/useOrganizationPointerDrag.test.tsx
git commit -m "feat: add organization pointer drag coordinator"
```

## Task 3: Integrate mutually exclusive gesture surfaces into the browser and tree

**Files:**

- Modify: `ui/src/components/VirtualList.tsx`
- Modify: `ui/src/components/FolderTree.tsx`
- Modify: `ui/src/components/FolderTree.test.tsx`
- Modify: `ui/src/components/ContentBrowser.tsx`
- Modify: `ui/src/components/ContentBrowser.test.tsx`
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/App.test.tsx`
- Modify: `ui/src/styles/app.css`
- Modify: `scripts/check-tauri-security.sh`

### Step 1: Replace the old component contracts in RED tests

Add an intentionally narrow VirtualList prop so FolderTree can mark its true scrolling viewport:

```ts
interface VirtualListProps<T> {
  // existing props remain unchanged
  viewportProps?: {
    'data-organization-drop-surface'?: string
  }
}
```

Spread `viewportProps` on the viewport `<div>` before Viewer-owned `ref`, `className`, `style`, and `onScroll` so callers cannot override behavior.

Replace FolderTree’s HTML drag props with one controlled prop:

```ts
interface FolderTreeProps {
  // existing non-drag props remain unchanged
  organizationDropTarget?: OrganizationDropTarget | null
}
```

RED FolderTree tests must prove:

- the VirtualList viewport has `data-organization-drop-surface=""`;
- every visible row has `data-organization-folder-id` equal to the opaque folder entity ID;
- only the controlled matching row receives `data-drop-mode` or `data-drop-invalid`;
- no `dragover`, `drop`, or `DataTransfer` callback is required.

Replace ContentBrowser’s old callbacks with:

```ts
interface ContentBrowserProps {
  // existing props remain unchanged
  onOrganizationPointerInput?: (input: OrganizationPointerInput) => void
}
```

RED ContentBrowser tests must prove for both image cells and text rows:

- the outer `role="option"` has no `draggable="true"`;
- `.file-export-surface` has `draggable="true"` and body DragStart invokes only Finder export;
- `.organization-drag-handle` has `draggable="false"` and never invokes Finder export;
- primary PointerDown on the handle freezes ordered selection and emits `start` with `move`;
- Option at PointerDown emits `copy`, and later modifier changes cannot alter it;
- PointerMove/Up/Cancel emit normalized input for the captured pointer;
- a nonprimary button and disabled state do not start;
- pointer handlers prevent selection click/double-click bubbling from the handle.

Update the App integration test to use Pointer Events rather than `fireEvent.dragStart/dragOver/drop`. Stub `document.elementFromPoint` to return the real rendered folder row, cross the threshold, release, resolve conflict if needed, and assert the existing move/copy bridge receives the ordered IDs, target ID, and frozen mode.

Run:

```bash
pnpm --dir ui vitest run src/components/FolderTree.test.tsx src/components/ContentBrowser.test.tsx src/App.test.tsx
```

Expected: FAIL against the existing HTML5 organization implementation.

### Step 2: Implement the ContentBrowser surface split

Delete `VIEWER_SELECTION_MIME`, `startOrganizationDrag`, `onOrganizationDragStart`, and `onOrganizationDragEnd`.

For image and text items, make the structure semantically equivalent to:

```tsx
<article role="option" aria-selected={selected} className="image-cell">
  <div
    className="file-export-surface"
    draggable
    title="拖到 Finder"
    onDragStart={(event) => startFinderExport(file, event)}
  >
    {filePreviewAndMetadata}
  </div>
  <button
    type="button"
    className="organization-drag-handle"
    aria-label={`整理 ${file.name}`}
    draggable={false}
    onDragStart={(event) => {
      event.preventDefault()
      event.stopPropagation()
    }}
    onPointerDown={startPointerOrganization}
    onPointerMove={movePointerOrganization}
    onPointerUp={endPointerOrganization}
    onPointerCancel={cancelPointerOrganization}
  >
    ⋮⋮
  </button>
</article>
```

Keep the existing outer click, Command/Shift selection, double-click preview, visible metadata, and accessibility semantics. The handlers must pass only entity IDs and pointer coordinates. PointerDown filters `event.button === 0`, calls `freezeDragSelection`, freezes `event.altKey`, passes `event.currentTarget` as `captureNode`, prevents default, and stops propagation. The coordinator is the sole owner of `setPointerCapture` and `releasePointerCapture`.

### Step 3: Make FolderTree passive and App the coordinator owner

Remove FolderTree’s `DragEvent` import, local `dropTarget` state, DataTransfer-type inspection, `onDragOver`, `onDragLeave`, and `onDrop`.

Each row renders:

```tsx
data-organization-folder-id={folder.entityId}
data-drop-mode={
  organizationDropTarget?.entityId === folder.entityId && organizationDropTarget.valid
    ? organizationDropTarget.mode
    : undefined
}
data-drop-invalid={
  organizationDropTarget?.entityId === folder.entityId && !organizationDropTarget.valid
    ? true
    : undefined
}
```

Mark the virtual-list viewport with `viewportProps={{ 'data-organization-drop-surface': '' }}`.

In `App.tsx`:

1. Delete `InternalDragState`, `internalDrag`, `setInternalDrag`, and derived `draggedEntityIds`.
2. Refactor target validation to accept explicit frozen IDs:

```ts
const isOrganizationDropTargetValid = useCallback(
  (entityIds: readonly string[], folderId: string, mode: OrganizationDragMode) => {
    return existingValidationUsing(entityIds, folderId, mode)
  },
  [existingValidationDependencies],
)
```

3. Create the hook after existing callbacks are available. Disable it when the project is absent/read-only, an operation is busy, or comparison is open. Build `resetKey` from project session/generation and workspace identity so rescans and replacements cancel stale sessions.
4. Route its valid `onDrop` directly to the existing `dropFiles` callback.
5. Pass `dropTarget` to FolderTree and `handlePointerInput` to ContentBrowser.
6. Render a fixed, `pointer-events: none` `.organization-drag-preview` with `移动 N 项` or `复制 N 项`, translated from the current pointer and clamped by CSS to the viewport.

Do not change `dropFiles` conflict handling, Rust commands, operation journal, watcher reconciliation, or keyboard Move/Copy dialogs.

### Step 4: Add styles and a repository policy regression guard

Style the dedicated export surface without changing card geometry. Give the handle a grabbing cursor only while active. Keep the existing blue move, green copy, and red invalid folder target styles. The floating preview must be above the workspace, readable in light/dark appearance, compact, and unable to receive pointer events.

Extend `scripts/check-tauri-security.sh` to fail if the Viewer source reintroduces any of:

```text
application/x-viewer-selection
dataTransfer.setData
onDragOver in FolderTree.tsx
onDrop in FolderTree.tsx
```

Retain the existing assertion that rejects `mainWindow.dragDropEnabled === false`.

### Step 5: Run focused and full UI verification

Run:

```bash
pnpm --dir ui vitest run src/components/FolderTree.test.tsx src/components/ContentBrowser.test.tsx src/state/useOrganizationPointerDrag.test.tsx src/App.test.tsx
pnpm --dir ui test
pnpm --dir ui typecheck
pnpm lint
./scripts/check-tauri-security.sh
```

Expected: PASS, with no React warnings, no open handles, and no HTML5 internal organization remnants.

### Step 6: Review and commit

Perform a requirements review first and a code-quality review second. Resolve every Critical/Important finding and rerun the affected tests. Then:

```bash
git add ui/src/components/VirtualList.tsx ui/src/components/FolderTree.tsx ui/src/components/FolderTree.test.tsx ui/src/components/ContentBrowser.tsx ui/src/components/ContentBrowser.test.tsx ui/src/App.tsx ui/src/App.test.tsx ui/src/styles/app.css scripts/check-tauri-security.sh
git commit -m "fix: replace internal html drag with pointer events"
```

## Task 4: Complete automated gate, package, and final physical acceptance

**Files:**

- Modify after evidence exists: `.superpowers/sdd/progress.md`
- Modify after evidence exists: `docs/acceptance/m3-organization-comparison-acceptance.md` if this branch already uses that record; otherwise create it at that path.

### Step 1: Run the exact M3 gate from a clean tracked tree

Run:

```bash
git status --short
pnpm gate:m3
```

Expected: tracked tree clean before the command and final output `M3 organization and comparison gate passed`.

If the gate fails, stop packaging, diagnose through `superpowers:systematic-debugging`, add a failing regression test, fix, review, commit, and restart this task from Step 1.

### Step 2: Build and fingerprint the Apple Silicon package

Run:

```bash
pnpm build:macos
shasum -a 256 target/aarch64-apple-darwin/release/bundle/macos/Viewer.app/Contents/MacOS/viewer-desktop
shasum -a 256 target/aarch64-apple-darwin/release/bundle/dmg/*.dmg
```

Expected: signed/notarized distribution is not required for internal 0.1, but the Apple Silicon macOS 13-compatible `.app` and `.dmg` must be produced and hashes recorded.

### Step 3: Prepare a fresh immutable physical fixture

Create a new timestamped fixture root outside the repository. Copy the accepted JPG, PNG, Markdown, and TXT sources into its project using ordinary `cp`; compute SHA-256 for every source before launch. Create empty internal `organization-move` and `organization-copy` folders and distinct external destinations for image multi-export, text multi-export, cancelled drag, folder import, and post-relaunch export.

Do not reuse a destination containing earlier evidence. Do not delete earlier evidence.

### Step 4: Perform final physical acceptance on the just-built app

Use the exact packaged executable, not a development server. Complete and record all of the following:

1. Draw a visible marquee over JPG and PNG, drag one selected body export surface to Finder, and verify both destination hashes.
2. Command-select Markdown and TXT, drag one selected body export surface to Finder, and verify both destination hashes.
3. Start a Finder export and cancel outside a destination; verify source/destination listings and hashes are unchanged.
4. Drag the JPG `⋮⋮` handle to `organization-move` without Option; verify source absent, target present, and hash unchanged.
5. PointerDown the PNG `⋮⋮` handle while Option is down, release Option before PointerUp over `organization-copy`; verify source and target both present with equal hashes.
6. Close the project, then drag the fixture project folder from Finder into empty Viewer; verify the project opens and its nested files are indexed. This proves native incoming drop remains enabled.
7. Quit Viewer completely, relaunch the same packaged executable, import the fixture, and perform one fresh body export to a new destination; verify the hash.

For internal move/copy, also query the portable SQLite operation journal and confirm completed operation batches/items exist with the expected operation type. Do not inspect or record absolute user paths in the acceptance document; record fixture-relative paths and hashes only.

Any failed physical item blocks M3 completion and merging.

### Step 5: Record evidence only after all checks pass

Update the SDD ledger and acceptance record with:

- exact commit SHA;
- gate command and result;
- app/DMG hashes;
- each physical acceptance result and file hash;
- reviewer outcomes;
- explicit statement that native Finder-folder import passed and internal organization used no HTML5 DataTransfer.

Run a documentation consistency check:

```bash
rg -n "Pointer Events|DataTransfer|Finder|移动|复制|SHA-256|gate:m3" .superpowers/sdd/progress.md docs/acceptance/m3-organization-comparison-acceptance.md
git diff --check
```

Then commit:

```bash
git add .superpowers/sdd/progress.md docs/acceptance/m3-organization-comparison-acceptance.md
git commit -m "docs: record pointer drag acceptance evidence"
```

## Task 5: Final branch review and local merge to main

**Files:** Review the complete branch diff from merge base `7b12cf23e70df7dad0c975552c7317d1e5cbcc76` through final HEAD.

### Step 1: Verify the branch as a whole

Run:

```bash
git status --short --branch
git diff --check 7b12cf23e70df7dad0c975552c7317d1e5cbcc76..HEAD
git log --oneline --decorate 7b12cf23e70df7dad0c975552c7317d1e5cbcc76..HEAD
pnpm gate:m3
```

Expected: clean branch, no whitespace errors, intelligible task commits, exact M3 gate pass.

### Step 2: Request final independent review

Review the complete M3 diff against the approved product/design documents. The reviewer must check at least:

- correct behavior and regression risk;
- path/privacy and filesystem safety boundaries;
- stale session/generation handling;
- input ownership among Finder export, marquee, and organization handle;
- operation journal and conflict-policy reuse;
- automated/physical evidence completeness.

Resolve all Critical/Important findings through new tested commits and restart Task 5 Step 1. Do not merge on a conditional approval.

### Step 3: Merge locally only after unconditional approval

Use `superpowers:finishing-a-development-branch`. Confirm main has no unrelated uncommitted changes, then from `/Users/abc/Project/Viewer`:

```bash
git switch main
git merge --no-ff codex/m3-organization-comparison
pnpm gate:m3
git status --short --branch
```

Expected: local merge succeeds, post-merge gate passes, and `main` is clean. Do not push or create a pull request unless the user separately requests it.
