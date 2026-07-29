# Viewer Adaptive Text Panel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Viewer’s unconditional, fixed-height text-file area with a project-session-aware smart bottom shelf that disappears in image-only folders, expands adaptively in mixed folders, fills text-only folders, and offers explicit file-type choices for select all.

**Architecture:** Keep selection and file interaction ownership in `ContentBrowser`, derive display and select-all decisions through a pure model, store only the user’s mixed-folder expansion preference at the active-project session boundary, and split presentation into focused measured-layout, text-shelf, and select-all-choice components. A frame-batched height observer feeds the existing `AspectVirtualGrid` without remounting it, while a new isolated stylesheet establishes the bounded flex layout and strict 20% mixed-shelf cap.

**Tech Stack:** React 19, TypeScript 6, CSS, Vitest 4, Testing Library, existing `AspectVirtualGrid` and Viewer file-operation callbacks; no new runtime dependency.

## Global Constraints

- The approved source of truth is `docs/superpowers/specs/2026-07-28-viewer-adaptive-text-panel-design.md`.
- Work from `/Users/abc/Project/Viewer` on the latest local `main`, including the user’s uncommitted changes.
- Preserve and never stage the current user-owned changes in:
  - `ui/src/components/FolderTree.test.tsx`
  - `ui/src/components/FolderTree.tsx`
  - `ui/src/components/VirtualList.tsx`
  - `ui/src/styles/app.css`
  - `ui/src/styles/app.test.ts`
  - `tests/fixtures/images/.viewer/`
- Do not edit `ui/src/styles/app.css` or `ui/src/styles/app.test.ts`; place this feature’s layout rules and style contracts in new focused files.
- Image-only folders mount no text heading, disclosure, list, separator, or placeholder.
- Mixed folders start collapsed in each new project session; a manual preference survives folder navigation, image-only folders, and text-only folders in that same session.
- A mixed expanded shelf participates in layout and is strictly capped at 20% of the available content-body height, including its disclosure heading.
- A text-only folder forces the shelf open, uses the full content body, and never overwrites the mixed-folder preference.
- Expanding or collapsing the shelf must preserve the `AspectVirtualGrid` DOM node, image scroll position, recovered dimensions, cached thumbnails, and pending thumbnail requests.
- A collapsed shelf may hide selected text rows, but must show `已选 N` and must not expose an `aria-activedescendant` that points at an unmounted row.
- Mixed select-all offers `全选图片`, `全选文本文件`, and `全部选择`; single-type folders select directly without opening a choice panel.
- Command-A and the visible `全选当前文件夹` action use the same file-type-aware policy.
- Selection is replaced by the chosen select-all scope; no option silently adds another file type.
- The choice panel is anchored, compact, non-modal, keyboard operable, and closes without mutating selection on cancellation or stale workspace changes.
- No backend, scan, index, IPC, Markdown preview, TXT preview, radial-menu, Finder-drag, organization-drag, marker, or file-operation contract changes are permitted.
- Add no third-party popover, layout, or virtualization dependency.
- Use the repository’s canonical `pnpm start:viewer` command for final live verification; never launch a packaged `Viewer.app` or another worktree executable.

---

## File Map

### New files

- `ui/src/components/contentBrowser/adaptiveTextPanelModel.ts` — pure content-mode and select-all-scope policy.
- `ui/src/components/contentBrowser/adaptiveTextPanelModel.test.ts` — complete mode and scope matrix.
- `ui/src/app/useTextPanelPreference.ts` — active-project-session mixed-folder expansion preference.
- `ui/src/components/contentBrowser/useMeasuredElementHeight.ts` — valid-height observation with animation-frame coalescing and fallback.
- `ui/src/components/contentBrowser/useMeasuredElementHeight.test.tsx` — observer, fallback, deduplication, and cleanup tests.
- `ui/src/components/contentBrowser/TextFilePanel.tsx` — collapsed/expanded/text-only shelf and existing text-row interaction rendering.
- `ui/src/components/contentBrowser/TextFilePanel.test.tsx` — disclosure, cap contract hooks, focus, accessibility, and row forwarding.
- `ui/src/components/contentBrowser/SelectAllChoicePanel.tsx` — anchored three-command choice panel.
- `ui/src/components/contentBrowser/SelectAllChoicePanel.test.tsx` — focus, keyboard, pointer, cancellation, and outside-click tests.
- `ui/src/styles/adaptiveTextPanel.css` — bounded content shell, adaptive shelf, choice-panel, and responsive rules.
- `ui/src/styles/adaptiveTextPanel.test.ts` — focused static style contracts without touching the user-dirty baseline style test.

### Modified files

- `ui/src/app/appSessionCoordinators.test.tsx` — session reset and stale-callback tests for the new preference hook.
- `ui/src/App.tsx` — own the session preference, pass it to `ContentBrowser`, and mark a bounded content workspace.
- `ui/src/App.test.tsx` — project-session and folder-navigation integration.
- `ui/src/components/ContentBrowser.tsx` — compose the adaptive layout, preserve selection behavior, and integrate scoped select all.
- `ui/src/components/ContentBrowser.test.tsx` — mode, measurement, state-transition, preservation, select-all, and regression coverage.
- `ui/src/main.tsx` — load the new focused stylesheet after `app.css`.
- `docs/PRODUCT_SPEC.md` — replace the unconditional text-region requirement with the approved adaptive behavior.
- `docs/README.md` — move this plan from Active to Historical after implementation is complete.

---

## Task 1: Define Pure Content-Mode and Select-All Policy

**Files:**

- Create: `ui/src/components/contentBrowser/adaptiveTextPanelModel.ts`
- Create: `ui/src/components/contentBrowser/adaptiveTextPanelModel.test.ts`

**Interfaces:**

- Produces:
  - `AdaptiveContentMode`
  - `SelectAllScope`
  - `SelectAllRequest`
  - `SelectableContent`
  - `resolveAdaptiveContentMode(imageCount, textCount, preferredExpanded)`
  - `resolveSelectAllRequest(imageCount, textCount)`
  - `filesForSelectAllScope(content, scope)`
- Consumes: `BrowserFile`.

- [ ] **Step 1: Write the failing mode and select-all matrix tests**

Create `ui/src/components/contentBrowser/adaptiveTextPanelModel.test.ts`:

```ts
import { describe, expect, it } from 'vitest'
import type { BrowserFile } from '../../api/types'
import {
  filesForSelectAllScope,
  resolveAdaptiveContentMode,
  resolveSelectAllRequest,
} from './adaptiveTextPanelModel'

const file = (entityId: string, kind: 'jpeg' | 'text'): BrowserFile => ({
  entityId,
  relativePath: entityId,
  name: entityId,
  kind,
  size: 1,
  modifiedNs: '1',
  marker: { reviewState: null, favorite: false },
  imageMetadata: null,
  imageUrl: null,
})

describe('adaptiveTextPanelModel', () => {
  it.each([
    [2, 0, false, 'image_only'],
    [2, 3, false, 'mixed_collapsed'],
    [2, 3, true, 'mixed_expanded'],
    [0, 3, false, 'text_only'],
    [0, 3, true, 'text_only'],
    [0, 0, false, 'empty'],
  ] as const)(
    'resolves %i images, %i texts and preferred=%s as %s',
    (imageCount, textCount, preferredExpanded, expected) => {
      expect(
        resolveAdaptiveContentMode(imageCount, textCount, preferredExpanded),
      ).toBe(expected)
    },
  )

  it('resolves direct, choice, and empty select-all behavior', () => {
    expect(resolveSelectAllRequest(3, 0)).toEqual({
      kind: 'direct',
      scope: 'images',
    })
    expect(resolveSelectAllRequest(0, 3)).toEqual({
      kind: 'direct',
      scope: 'text',
    })
    expect(resolveSelectAllRequest(3, 2)).toEqual({ kind: 'choice' })
    expect(resolveSelectAllRequest(0, 0)).toEqual({ kind: 'none' })
  })

  it('returns exactly the requested files in stable display order', () => {
    const content = {
      images: [file('image-1', 'jpeg'), file('image-2', 'jpeg')],
      textFiles: [file('text-1', 'text'), file('text-2', 'text')],
    }
    expect(filesForSelectAllScope(content, 'images').map(({ entityId }) => entityId)).toEqual([
      'image-1',
      'image-2',
    ])
    expect(filesForSelectAllScope(content, 'text').map(({ entityId }) => entityId)).toEqual([
      'text-1',
      'text-2',
    ])
    expect(filesForSelectAllScope(content, 'all').map(({ entityId }) => entityId)).toEqual([
      'image-1',
      'image-2',
      'text-1',
      'text-2',
    ])
  })
})
```

- [ ] **Step 2: Run the test and verify the missing-module failure**

```bash
pnpm --dir ui test -- src/components/contentBrowser/adaptiveTextPanelModel.test.ts
```

Expected: FAIL because `adaptiveTextPanelModel.ts` does not exist.

- [ ] **Step 3: Implement the pure model**

Create `ui/src/components/contentBrowser/adaptiveTextPanelModel.ts`:

```ts
import type { BrowserFile } from '../../api/types'

export type AdaptiveContentMode =
  | 'empty'
  | 'image_only'
  | 'mixed_collapsed'
  | 'mixed_expanded'
  | 'text_only'

export type SelectAllScope = 'images' | 'text' | 'all'

export type SelectAllRequest =
  | { kind: 'none' }
  | { kind: 'direct'; scope: Exclude<SelectAllScope, 'all'> }
  | { kind: 'choice' }

export interface SelectableContent {
  images: readonly BrowserFile[]
  textFiles: readonly BrowserFile[]
}

export function resolveAdaptiveContentMode(
  imageCount: number,
  textCount: number,
  preferredExpanded: boolean,
): AdaptiveContentMode {
  if (imageCount <= 0 && textCount <= 0) return 'empty'
  if (imageCount <= 0) return 'text_only'
  if (textCount <= 0) return 'image_only'
  return preferredExpanded ? 'mixed_expanded' : 'mixed_collapsed'
}

export function resolveSelectAllRequest(
  imageCount: number,
  textCount: number,
): SelectAllRequest {
  if (imageCount <= 0 && textCount <= 0) return { kind: 'none' }
  if (textCount <= 0) return { kind: 'direct', scope: 'images' }
  if (imageCount <= 0) return { kind: 'direct', scope: 'text' }
  return { kind: 'choice' }
}

export function filesForSelectAllScope(
  content: SelectableContent,
  scope: SelectAllScope,
): readonly BrowserFile[] {
  if (scope === 'images') return content.images
  if (scope === 'text') return content.textFiles
  return [...content.images, ...content.textFiles]
}
```

- [ ] **Step 4: Run the focused test and type-check**

```bash
pnpm --dir ui test -- src/components/contentBrowser/adaptiveTextPanelModel.test.ts
pnpm --dir ui typecheck
```

Expected: PASS.

- [ ] **Step 5: Commit the pure policy**

```bash
git add \
  ui/src/components/contentBrowser/adaptiveTextPanelModel.ts \
  ui/src/components/contentBrowser/adaptiveTextPanelModel.test.ts
git commit -m "feat: define adaptive text panel policy"
```

---

## Task 2: Own the Mixed-Folder Preference at Project-Session Scope

**Files:**

- Create: `ui/src/app/useTextPanelPreference.ts`
- Modify: `ui/src/app/appSessionCoordinators.test.tsx`
- Modify: `ui/src/App.test.tsx`

**Interfaces:**

- Produces:
  - `TextPanelPreferenceState { expanded: boolean; setExpanded(expanded: boolean): void }`
  - `useTextPanelPreference(projectSessionId: string): TextPanelPreferenceState`
- Task 4 will wire the hook to the controlled `ContentBrowser` props after the
  shelf exists.

- [ ] **Step 1: Write failing session-preference tests**

In `ui/src/app/appSessionCoordinators.test.tsx`, import
`useTextPanelPreference` and append:

```tsx
describe('Text panel project-session preference', () => {
  it('starts collapsed, persists within one session, and resets synchronously for another', () => {
    const hook = renderHook(
      ({ sessionId }) => useTextPanelPreference(sessionId),
      {
        initialProps: { sessionId: 'session-1' },
        wrapper: strictWrapper,
      },
    )

    expect(hook.result.current.expanded).toBe(false)
    act(() => hook.result.current.setExpanded(true))
    expect(hook.result.current.expanded).toBe(true)

    hook.rerender({ sessionId: 'session-2' })
    expect(hook.result.current.expanded).toBe(false)
  })

  it('ignores a stale setter captured by an earlier project session', () => {
    const hook = renderHook(
      ({ sessionId }) => useTextPanelPreference(sessionId),
      {
        initialProps: { sessionId: 'session-1' },
        wrapper: strictWrapper,
      },
    )
    const staleSetExpanded = hook.result.current.setExpanded

    hook.rerender({ sessionId: 'session-2' })
    act(() => staleSetExpanded(true))

    expect(hook.result.current.expanded).toBe(false)
  })
})
```

Update the type-contract test in `ui/src/App.test.tsx` to assert the hook takes
a `string` session ID and returns `TextPanelPreferenceState`.

- [ ] **Step 2: Run the coordinator tests and verify red**

```bash
pnpm --dir ui test -- src/app/appSessionCoordinators.test.tsx src/App.test.tsx
```

Expected: FAIL because `useTextPanelPreference` and its type do not exist.

- [ ] **Step 3: Implement the session-safe preference hook**

Create `ui/src/app/useTextPanelPreference.ts`:

```ts
import { useCallback, useRef, useState } from 'react'

export interface TextPanelPreferenceState {
  expanded: boolean
  setExpanded(expanded: boolean): void
}

interface StoredPreference {
  sessionId: string
  expanded: boolean
}

export function useTextPanelPreference(projectSessionId: string): TextPanelPreferenceState {
  const latestSessionId = useRef(projectSessionId)
  latestSessionId.current = projectSessionId
  const [stored, setStored] = useState<StoredPreference>(() => ({
    sessionId: projectSessionId,
    expanded: false,
  }))
  const expanded = stored.sessionId === projectSessionId ? stored.expanded : false

  const setExpanded = useCallback(
    (next: boolean) => {
      if (latestSessionId.current !== projectSessionId) return
      setStored((current) =>
        current.sessionId === projectSessionId && current.expanded === next
          ? current
          : { sessionId: projectSessionId, expanded: next },
      )
    },
    [projectSessionId],
  )

  return { expanded, setExpanded }
}
```

This representation makes a new session read as collapsed on its first render
instead of briefly exposing the previous session’s preference.

- [ ] **Step 4: Run the focused tests**

```bash
pnpm --dir ui test -- src/app/appSessionCoordinators.test.tsx src/App.test.tsx
pnpm --dir ui typecheck
```

Expected: PASS.

- [ ] **Step 5: Commit the completed session owner**

Commit the hook and its coordinator/type-contract coverage:

```bash
git add \
  ui/src/app/useTextPanelPreference.ts \
  ui/src/app/appSessionCoordinators.test.tsx \
  ui/src/App.test.tsx
git commit -m "feat: scope text panel preference to project sessions"
```

---

## Task 3: Measure the Real Image Slot and Establish a Bounded Layout

**Files:**

- Create: `ui/src/components/contentBrowser/useMeasuredElementHeight.ts`
- Create: `ui/src/components/contentBrowser/useMeasuredElementHeight.test.tsx`
- Create: `ui/src/styles/adaptiveTextPanel.css`
- Create: `ui/src/styles/adaptiveTextPanel.test.ts`
- Modify: `ui/src/main.tsx`
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/components/ContentBrowser.tsx`
- Modify: `ui/src/components/ContentBrowser.test.tsx`

**Interfaces:**

- Produces:
  - `UseMeasuredElementHeightResult { ref: RefObject<HTMLDivElement | null>; height: number }`
  - `useMeasuredElementHeight(fallbackHeight?: number)`
- Consumes: a bounded image-slot element and passes the last valid integer
  height to `AspectVirtualGrid`.

- [ ] **Step 1: Write failing measurement-hook tests**

Create `ui/src/components/contentBrowser/useMeasuredElementHeight.test.tsx`
with a harness that renders the returned height and attaches `ref` to a `div`.
Cover these exact cases:

```tsx
it('publishes only the latest valid height once per animation frame', () => {
  const observer = installResizeObserver()
  const frames = installAnimationFrameQueue()
  render(<HeightHarness fallbackHeight={520} />)
  const slot = screen.getByTestId('measured-slot')

  observer.trigger(slot, 900, 480)
  observer.trigger(slot, 900, 460)
  observer.trigger(slot, 900, Number.NaN)
  expect(frames.pending()).toBe(1)
  expect(screen.getByRole('status')).toHaveTextContent('520')

  act(() => frames.flush())
  expect(screen.getByRole('status')).toHaveTextContent('460')

  observer.trigger(slot, 900, 460)
  act(() => frames.flush())
  expect(screen.getByRole('status')).toHaveTextContent('460')

  observer.trigger(slot, 900, 420)
  observer.trigger(slot, 900, 460)
  expect(frames.pending()).toBe(1)
  act(() => frames.flush())
  expect(screen.getByRole('status')).toHaveTextContent('460')
})

it('uses the bounding rectangle without ResizeObserver and keeps the last valid height', () => {
  vi.stubGlobal('ResizeObserver', undefined)
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({
    width: 800,
    height: 410,
    top: 0,
    right: 800,
    bottom: 410,
    left: 0,
    x: 0,
    y: 0,
    toJSON: () => undefined,
  })
  render(<HeightHarness fallbackHeight={520} />)
  expect(screen.getByRole('status')).toHaveTextContent('410')
})
```

Also assert observer disconnection and queued-frame cancellation on unmount.
Reuse the map-based observer and frame helpers already established in
`useCompareLayout.test.tsx`; do not use the old single-callback
`ContentBrowser.test.tsx` stub.

- [ ] **Step 2: Run the hook test and verify red**

```bash
pnpm --dir ui test -- src/components/contentBrowser/useMeasuredElementHeight.test.tsx
```

Expected: FAIL because the hook does not exist.

- [ ] **Step 3: Implement frame-batched valid-height measurement**

Create `ui/src/components/contentBrowser/useMeasuredElementHeight.ts` with:

```ts
import { type RefObject, useCallback, useEffect, useRef, useState } from 'react'

export interface UseMeasuredElementHeightResult {
  ref: RefObject<HTMLDivElement | null>
  height: number
}

const validHeight = (height: number): number | null =>
  Number.isFinite(height) && height > 0 ? Math.max(1, Math.round(height)) : null

export function useMeasuredElementHeight(
  fallbackHeight = 520,
): UseMeasuredElementHeightResult {
  const ref = useRef<HTMLDivElement | null>(null)
  const latest = useRef(validHeight(fallbackHeight) ?? 520)
  const queued = useRef<number | null>(null)
  const pending = useRef<number | null>(null)
  const [height, setHeight] = useState(latest.current)

  const publish = useCallback((candidate: number) => {
    const next = validHeight(candidate)
    if (next === null) return
    if (queued.current === null && next === latest.current) return
    pending.current = next
    if (queued.current !== null) return
    const flush = () => {
      queued.current = null
      const value = pending.current
      pending.current = null
      if (value === null || value === latest.current) return
      latest.current = value
      setHeight(value)
    }
    if (typeof requestAnimationFrame === 'undefined') {
      flush()
    } else {
      queued.current = requestAnimationFrame(flush)
    }
  }, [])

  useEffect(() => {
    const node = ref.current
    if (node === null) return
    if (typeof ResizeObserver === 'undefined') {
      const bounds = node.getBoundingClientRect()
      const next = validHeight(bounds.height)
      if (next !== null && next !== latest.current) {
        latest.current = next
        setHeight(next)
      }
      return
    }
    const observer = new ResizeObserver((entries) => {
      const bounds = entries[0]?.contentRect
      if (bounds !== undefined) publish(bounds.height)
    })
    observer.observe(node)
    return () => observer.disconnect()
  }, [publish])

  useEffect(
    () => () => {
      if (queued.current !== null && typeof cancelAnimationFrame !== 'undefined') {
        cancelAnimationFrame(queued.current)
      }
    },
    [],
  )

  return { ref, height }
}
```

If implementation review finds the synchronous no-observer branch causes a
test-only effect timing warning, use `useLayoutEffect` for measurement rather
than weakening the assertion or publishing zero.

- [ ] **Step 4: Write failing image-only and measured-slot integration tests**

Refactor the `ContentBrowser.test.tsx` ResizeObserver stub into a map keyed by
the observed element so the image-slot height observer and virtual-grid width
observer can coexist. Add helpers:

```ts
function triggerResize(node: Element, width: number, height: number) {
  resizeCallbacks.get(node)?.(
    [{ target: node, contentRect: { width, height } } as ResizeObserverEntry],
    {} as ResizeObserver,
  )
}
```

Add:

```tsx
it('removes the entire text surface and measures all available image height in image-only mode', () => {
  render(<ContentBrowser workspace={ratioWorkspace([{ width: 1, height: 1 }])} />)
  expect(screen.queryByText('文本文件')).not.toBeInTheDocument()
  expect(screen.queryByRole('listbox', { name: '文本文件' })).not.toBeInTheDocument()

  const slot = screen.getByTestId('content-image-slot')
  triggerResize(slot, 900, 688)
  flushAnimationFrames()
  expect(screen.getByRole('listbox', { name: '图片文件' })).toHaveStyle({
    height: '688px',
  })
})

it('keeps the same image grid node and scroll offset when its measured height changes', () => {
  render(<ContentBrowser workspace={ratioWorkspace(Array(40).fill({ width: 1, height: 1 }))} />)
  const slot = screen.getByTestId('content-image-slot')
  const grid = screen.getByRole('listbox', { name: '图片文件' })
  grid.scrollTop = 180
  fireEvent.scroll(grid)

  triggerResize(slot, 900, 420)
  flushAnimationFrames()
  triggerResize(slot, 900, 650)
  flushAnimationFrames()

  expect(screen.getByRole('listbox', { name: '图片文件' })).toBe(grid)
  expect(grid.scrollTop).toBe(180)
})
```

- [ ] **Step 5: Reshape `ContentBrowser` without adding the text shelf yet**

In `ContentBrowser.tsx`:

- derive `mode` through `resolveAdaptiveContentMode`;
- call `useMeasuredElementHeight(viewportHeight)`;
- render the toolbar and a new body wrapper;
- render the image slot only when `workspace.images.length > 0`;
- keep the `AspectVirtualGrid` instance directly inside that stable slot;
- pass the measured height instead of the fixed default;
- make the existing `<h2>` and `.text-file-list` conditional on
  `workspace.textFiles.length > 0` so image-only mode is correct while all
  existing text-row behavior stays green until Task 4 replaces this temporary
  block with `TextFilePanel`.

The structural target is:

```tsx
<section className="content-browser" data-content-mode={mode} aria-label="文件内容">
  <div className="content-toolbar">
    <div>
      <strong>{currentPath ?? '当前文件夹'}</strong>
      <span>· {workspace.images.length} 张图片</span>
      {workspace.textFiles.length > 0 && <span>· {workspace.textFiles.length} 个文本文件</span>}
    </div>
    <details className="content-view-menu">
      <summary>视图</summary>
      <div>
        <button type="button" onClick={selectAllFiles} disabled={allFiles.length === 0}>
          全选当前文件夹
        </button>
      </div>
    </details>
  </div>
  <div className="content-browser-body">
    {workspace.images.length > 0 && (
      <div
        ref={imageSlot.ref}
        className="content-browser-image-slot"
        data-testid="content-image-slot"
      >
        <AspectVirtualGrid
          items={workspace.images}
          imageHeight={THUMBNAIL_HEIGHT[density]}
          viewportHeight={imageSlot.height}
          getKey={imageEntityId}
          getDimensions={dimensionsForImage}
          ariaLabel="图片文件"
          activeKey={activeId ?? undefined}
          activeDescendant={activeId ? `file-${activeId}` : undefined}
          onNavigate={navigateToIndex}
          onKeyDown={handleKeyboard}
          ariaMultiselectable
          onMarqueeSelectionChange={updateMarqueeSelection}
          renderItem={(file, _index, rect: AspectRect) => (
            <ImageCell
              file={file}
              rect={rect}
              dimensionsKnown={validDimensions(dimensionsForImage(file))}
              selected={selected.has(file.entityId)}
              active={activeId === file.entityId}
              loadThumbnail={loadThumbnail}
              onNaturalDimensions={rememberNaturalDimensions}
              markerLabel={markerLabel(file.marker)}
              onClick={selectFile}
              onPreview={(selectedFile) => onPreview?.(selectedFile)}
              onRadialMenuPointerDown={openRadialMenuFromPointer}
              onRadialMenuContextMenu={openRadialMenuFromContext}
              organizationDragDisabled={organizationDragDisabled}
              onFinderDragStart={startFinderDrag}
              onPointerDown={startPointerOrganization}
              onPointerMove={movePointerOrganization}
              onPointerUp={endPointerOrganization}
              onPointerCancel={cancelPointerOrganization}
            />
          )}
        />
      </div>
    )}
    {workspace.textFiles.length > 0 && (
      <>
        <h2>文本文件</h2>
        <div
          className="text-file-list"
          role="listbox"
          aria-label="文本文件"
          tabIndex={0}
          onKeyDown={handleTextListKeyboard}
        >
          {workspace.textFiles.map((file) => (
            <div
              role="option"
              id={`file-${file.entityId}`}
              aria-label={file.name}
              aria-selected={selected.has(file.entityId)}
              tabIndex={-1}
              key={file.entityId}
              className="text-file-row"
              onPointerDown={(event) => openRadialMenuFromPointer(file, event)}
              onContextMenu={(event) => openRadialMenuFromContext(file, event)}
              onClick={(event) => selectFile(file, event)}
              onDoubleClick={() => onPreview?.(file)}
            >
              <div
                className="file-export-surface"
                draggable
                title="拖到 Finder"
                onDragStart={(event) => startFinderDrag(file, event)}
              >
                <span className="text-file-name">{file.name}</span>
                <span className="text-file-path">{file.relativePath}</span>
                {markerLabel(file.marker) && (
                  <span className="file-marker">{markerLabel(file.marker)}</span>
                )}
              </div>
              <OrganizationDragHandle
                file={file}
                disabled={organizationDragDisabled}
                onPointerDown={startPointerOrganization}
                onPointerMove={movePointerOrganization}
                onPointerUp={endPointerOrganization}
                onPointerCancel={cancelPointerOrganization}
              />
            </div>
          ))}
        </div>
      </>
    )}
  </div>
</section>
```

Do not key the image slot, grid, or content browser by `mode`.

- [ ] **Step 6: Add the isolated bounded-layout stylesheet and contract test**

Create `ui/src/styles/adaptiveTextPanel.css`:

```css
.workspace.workspace--content {
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
}

.workspace--content > .content-workspace-surface,
.workspace--content > .compare-workspace {
  flex: 1 1 auto;
  min-height: 0;
}

.workspace--content > .compare-workspace {
  height: auto;
}

.content-workspace-surface {
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.content-workspace-surface[hidden] {
  display: none;
}

.content-browser {
  display: flex;
  flex: 1 1 auto;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

.content-browser > .content-toolbar {
  flex: 0 0 auto;
}

.content-browser-body {
  display: flex;
  flex: 1 1 auto;
  flex-direction: column;
  min-height: 0;
}

.content-browser-image-slot {
  flex: 1 1 auto;
  min-height: 0;
  overflow: hidden;
}
```

Create `ui/src/styles/adaptiveTextPanel.test.ts` and read only
`adaptiveTextPanel.css`. Assert the exact `height`, `min-height`, `overflow`,
`flex`, and `[hidden]` declarations above. Reuse a local declaration parser;
do not import or modify `app.test.ts`.

Import the new stylesheet after `app.css` in `ui/src/main.tsx`:

```ts
import './styles/app.css'
import './styles/adaptiveTextPanel.css'
```

In `App.tsx`, derive:

```ts
const contentWorkspaceActive =
  !state.search.showResults && state.workspace?.workspace === 'content'
```

and apply:

```tsx
<section
  className={contentWorkspaceActive ? 'workspace workspace--content' : 'workspace'}
  aria-label="项目内容"
>
```

- [ ] **Step 7: Run the focused layout suite**

```bash
pnpm --dir ui test -- \
  src/components/contentBrowser/useMeasuredElementHeight.test.tsx \
  src/components/ContentBrowser.test.tsx \
  src/styles/adaptiveTextPanel.test.ts
pnpm --dir ui check
```

Expected: hook, image-only, measurement, existing image-grid, and style tests
PASS, including the unchanged text-row regression tests.

- [ ] **Step 8: Commit the measured bounded shell**

```bash
git add \
  ui/src/components/contentBrowser/useMeasuredElementHeight.ts \
  ui/src/components/contentBrowser/useMeasuredElementHeight.test.tsx \
  ui/src/components/ContentBrowser.tsx \
  ui/src/components/ContentBrowser.test.tsx \
  ui/src/styles/adaptiveTextPanel.css \
  ui/src/styles/adaptiveTextPanel.test.ts \
  ui/src/main.tsx \
  ui/src/App.tsx
git commit -m "feat: measure the adaptive content viewport"
```

Before committing, use `git diff --cached --name-only` and confirm neither
`app.css` nor `app.test.ts` is staged.

---

## Task 4: Build and Integrate the Smart Text Shelf

**Files:**

- Create: `ui/src/components/contentBrowser/TextFilePanel.tsx`
- Create: `ui/src/components/contentBrowser/TextFilePanel.test.tsx`
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/components/ContentBrowser.tsx`
- Modify: `ui/src/components/ContentBrowser.test.tsx`
- Modify: `ui/src/styles/adaptiveTextPanel.css`
- Modify: `ui/src/styles/adaptiveTextPanel.test.ts`
- Modify: `ui/src/App.test.tsx`

**Interfaces:**

- `TextFilePanelProps`:
  - `mode: 'mixed_collapsed' | 'mixed_expanded' | 'text_only'`
  - `files: readonly BrowserFile[]`
  - `selectedIds: ReadonlySet<string>`
  - `activeId: string | null`
  - `organizationDragDisabled: boolean`
  - `onExpandedChange(expanded: boolean): void`
  - `onListKeyDown(event): void`
  - the existing select, preview, radial, Finder-drag, and organization-pointer callbacks.
- Produces no independent selection state.

- [ ] **Step 1: Write failing shelf component tests**

Create `ui/src/components/contentBrowser/TextFilePanel.test.tsx`. Use two text
fixtures and no image fixtures. Cover:

```tsx
it('renders one complete collapsed disclosure with selected hidden count', () => {
  renderPanel({
    mode: 'mixed_collapsed',
    selectedIds: new Set(['text-2']),
  })
  const disclosure = screen.getByRole('button', {
    name: '文本文件 · 2 · 已选 1',
  })
  expect(disclosure).toHaveAttribute('aria-expanded', 'false')
  expect(disclosure).toHaveAttribute('aria-controls', 'content-text-file-list')
  expect(screen.queryByRole('listbox', { name: '文本文件' })).not.toBeInTheDocument()
})

it('uses a native disclosure button and reports the controlled next state', () => {
  const onExpandedChange = vi.fn()
  renderPanel({ mode: 'mixed_collapsed', onExpandedChange })
  const disclosure = screen.getByRole('button', { name: '文本文件 · 2' })
  expect(disclosure.tagName).toBe('BUTTON')
  fireEvent.click(disclosure)
  expect(onExpandedChange).toHaveBeenCalledOnce()
  expect(onExpandedChange).toHaveBeenCalledWith(true)
})

it('collapses an expanded mixed shelf on Escape and restores disclosure focus', () => {
  const onExpandedChange = vi.fn()
  const rendered = renderPanel({ mode: 'mixed_expanded', onExpandedChange })
  const list = screen.getByRole('listbox', { name: '文本文件' })
  list.focus()
  fireEvent.keyDown(list, { key: 'Escape' })
  expect(onExpandedChange).toHaveBeenCalledWith(false)
  rendered.rerender(renderPanelElement({ mode: 'mixed_collapsed', onExpandedChange }))
  expect(screen.getByRole('button', { name: '文本文件 · 2' })).toHaveFocus()
})

it('renders text-only content as a non-collapsible labelled primary list', () => {
  renderPanel({ mode: 'text_only' })
  expect(screen.getByRole('heading', { name: '文本文件 · 2' })).toBeVisible()
  expect(screen.queryByRole('button', { name: /文本文件/ })).not.toBeInTheDocument()
  expect(screen.getByRole('listbox', { name: '文本文件' })).toBeVisible()
})
```

Also assert one representative row forwards click, double-click,
right-pointer/context-menu, Finder drag, and organization-handle pointer
events to the supplied callbacks without changing their file argument.

- [ ] **Step 2: Run the shelf test and verify red**

```bash
pnpm --dir ui test -- src/components/contentBrowser/TextFilePanel.test.tsx
```

Expected: FAIL because `TextFilePanel.tsx` does not exist.

- [ ] **Step 3: Implement the controlled shelf and existing row rendering**

Create `TextFilePanel.tsx` by moving the existing `.text-file-row` markup out
of `ContentBrowser`. Keep `OrganizationDragHandle` unchanged. Use one stable
list ID:

```ts
const TEXT_LIST_ID = 'content-text-file-list'
```

The mixed disclosure text is:

```ts
const selectedCount = files.filter((file) => selectedIds.has(file.entityId)).length
const disclosureLabel =
  selectedCount > 0
    ? `文本文件 · ${files.length} · 已选 ${selectedCount}`
    : `文本文件 · ${files.length}`
```

The component skeleton is:

```tsx
<section className={`text-file-panel text-file-panel--${mode}`}>
  {mode === 'text_only' ? (
    <h2 id="content-text-file-heading">文本文件 · {files.length}</h2>
  ) : (
    <button
      ref={disclosureRef}
      type="button"
      className="text-file-disclosure"
      aria-expanded={mode === 'mixed_expanded'}
      aria-controls={TEXT_LIST_ID}
      onClick={() => onExpandedChange(mode !== 'mixed_expanded')}
    >
      {/* disclosure indicator plus exact label */}
    </button>
  )}
  {mode !== 'mixed_collapsed' && (
    <div
      id={TEXT_LIST_ID}
      role="listbox"
      aria-label={mode === 'text_only' ? undefined : '文本文件'}
      aria-labelledby={mode === 'text_only' ? 'content-text-file-heading' : undefined}
      aria-activedescendant={
        activeId !== null && files.some(({ entityId }) => entityId === activeId)
          ? `file-${activeId}`
          : undefined
      }
      tabIndex={0}
      className="text-file-list"
      onKeyDown={handleListKeyDown}
    >
      {files.map((file) => (
        <div
          role="option"
          id={`file-${file.entityId}`}
          aria-label={file.name}
          aria-selected={selectedIds.has(file.entityId)}
          tabIndex={-1}
          key={file.entityId}
          className="text-file-row"
          onPointerDown={(event) => onRadialMenuPointerDown(file, event)}
          onContextMenu={(event) => onRadialMenuContextMenu(file, event)}
          onClick={(event) => onSelect(file, event)}
          onDoubleClick={() => onPreview(file)}
        >
          <div
            className="file-export-surface"
            draggable
            title="拖到 Finder"
            onDragStart={(event) => onFinderDragStart(file, event)}
          >
            <span className="text-file-name">{file.name}</span>
            <span className="text-file-path">{file.relativePath}</span>
            {markerLabel(file.marker) && (
              <span className="file-marker">{markerLabel(file.marker)}</span>
            )}
          </div>
          <OrganizationDragHandle
            file={file}
            disabled={organizationDragDisabled}
            onPointerDown={onOrganizationPointerDown}
            onPointerMove={onOrganizationPointerMove}
            onPointerUp={onOrganizationPointerUp}
            onPointerCancel={onOrganizationPointerCancel}
          />
        </div>
      ))}
    </div>
  )}
</section>
```

For Escape focus restoration, remember the pending focus in a ref when Escape
requests collapse, then focus the still-mounted disclosure in a layout effect
after `mode` becomes `mixed_collapsed`. Do not use an arbitrary timeout.

Do not add a custom Enter/Space key handler to the disclosure: native button
activation already supplies both without double dispatch. Use a text
disclosure character already rendered by the platform (`›`/`⌄`) as textual UI
chrome, not a fabricated image asset. Mark it `aria-hidden`.

- [ ] **Step 4: Integrate every content mode in `ContentBrowser`**

In `ContentBrowser.tsx`:

- make `textPanelExpanded` and `onTextPanelExpandedChange` required props;
- derive the mode with the pure model;
- render no shelf in `image_only`;
- render `TextFilePanel` for `mixed_collapsed`, `mixed_expanded`, and
  `text_only`;
- render no image slot in `text_only`;
- keep `empty` defensive and render neither list;
- pass the unchanged row callbacks into `TextFilePanel`;
- wrap preview and radial callbacks so later Task 5 can close stale menus in
  one place.

In `App.tsx`, import `useTextPanelPreference`, call it beside the other
project-session coordinators, and pass the controlled values:

```tsx
const textPanelPreference = useTextPanelPreference(projectSessionId)

textPanelExpanded={textPanelPreference.expanded}
onTextPanelExpandedChange={textPanelPreference.setExpanded}
```

Add the last two JSX attributes to the existing `ContentBrowser` invocation;
the snippet intentionally shows only the new lines so no current callback is
removed or reordered.

Do not place this preference in folder workspace state and do not reset it on
`state.workspace`, `state.selectedFolderId`, image-only mode, or text-only
mode.

Correct active-descendant ownership:

```ts
const activeImageId = workspace.images.some(({ entityId }) => entityId === activeId)
  ? activeId
  : null
const activeTextId = workspace.textFiles.some(({ entityId }) => entityId === activeId)
  ? activeId
  : null
```

Pass only `activeImageId` to `AspectVirtualGrid` and only `activeTextId` to the
mounted text list. Selection itself remains unchanged when the shelf
collapses.

Update the `ContentBrowser.test.tsx` wrapper to provide controlled defaults:

```tsx
function ContentBrowser({
  density = 'standard',
  textPanelExpanded = false,
  onTextPanelExpandedChange = () => undefined,
  ...props
}: ContentBrowserTestProps) {
  return (
    <ContentBrowserComponent
      {...props}
      density={density}
      textPanelExpanded={textPanelExpanded}
      onTextPanelExpandedChange={onTextPanelExpandedChange}
    />
  )
}
```

Existing tests that directly operate on text rows must pass
`textPanelExpanded` explicitly. Tests that merely use the mixed `workspace()`
fixture must not assume text rows are mounted.

- [ ] **Step 5: Add transition and preservation integration tests**

Add a controlled harness that owns `expanded` and rerenders the same
`ContentBrowser`. Cover:

- default mixed shelf is collapsed;
- expanding shows the list;
- collapsing leaves a selected text file selected and reports `已选 1`;
- image-only to mixed restores the controlled preference;
- text-only is visibly expanded but does not call
  `onTextPanelExpandedChange`;
- mixed expanded to text-only and back leaves the mixed preference unchanged;
- removing the last text file unmounts the shelf;
- removing the last image switches to full text-only mode;
- collapsing while a text row is active removes every hidden
  `aria-activedescendant`;
- the image grid node and `scrollTop` are identical before and after toggle;
- cached/pending thumbnail request keys are not requested a second time after
  a shelf toggle.

Add a mixed-workspace helper to `ui/src/App.test.tsx`, then add:

```tsx
it('keeps the mixed text shelf preference while navigating inside one project session', async () => {
  const viewer = bridge()
  vi.mocked(viewer.folderTree).mockResolvedValue([
    {
      entityId: 'folder-b',
      parentEntityId: null,
      relativePath: 'folder-b',
      name: 'folder-b',
      marker: { reviewState: null, favorite: false },
    },
  ])
  vi.mocked(viewer.queryFolder).mockResolvedValue(mixedContentWorkspace())
  render(<App bridge={viewer} />)

  fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
  const disclosure = await screen.findByRole('button', { name: /文本文件 · 1/ })
  expect(disclosure).toHaveAttribute('aria-expanded', 'false')
  fireEvent.click(disclosure)
  expect(disclosure).toHaveAttribute('aria-expanded', 'true')

  fireEvent.click(await screen.findByRole('treeitem', { name: 'folder-b' }))
  await waitFor(() =>
    expect(screen.getByRole('button', { name: /文本文件 · 1/ })).toHaveAttribute(
      'aria-expanded',
      'true',
    ),
  )
})
```

`mixedContentWorkspace()` must clone `contentWorkspace()` and add one valid
`text` file named `notes.txt`.

- [ ] **Step 6: Add strict-cap and text-only CSS**

Append to `adaptiveTextPanel.css`:

```css
.text-file-panel {
  display: flex;
  flex: 0 0 auto;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
}

.text-file-panel--mixed-expanded {
  flex: 0 1 auto;
  max-height: 20%;
}

.text-file-panel--text_only {
  flex: 1 1 auto;
  max-height: none;
}

.text-file-disclosure {
  align-items: center;
  background: #fff;
  border: 1px solid #c8ced6;
  border-radius: 6px;
  color: #1f2328;
  display: flex;
  flex: 0 0 auto;
  gap: 6px;
  min-height: 34px;
  padding: 6px 10px;
  text-align: left;
  width: 100%;
}

.text-file-panel > h2 {
  border: 0;
  flex: 0 0 auto;
  margin: 0;
  padding: 8px 0;
}

.text-file-panel > .text-file-list {
  flex: 1 1 auto;
  min-height: 0;
  overflow: auto;
  overscroll-behavior: contain;
}
```

Add focus-visible, hover, expanded-state, separator, and spacing rules using
the existing light Viewer colors. Do not add dark-theme overrides or re-open
the previously resolved preview-theme work.

Extend `adaptiveTextPanel.test.ts` to assert:

- mixed expanded `max-height` is exactly `20%`;
- text-only `max-height` is `none` and `flex` is `1 1 auto`;
- the list owns `overflow: auto` and `min-height: 0`;
- the disclosure is visually button-shaped and has a focus-visible outline.

- [ ] **Step 7: Run the complete shelf regression set**

```bash
pnpm --dir ui test -- \
  src/components/contentBrowser/TextFilePanel.test.tsx \
  src/components/ContentBrowser.test.tsx \
  src/app/appSessionCoordinators.test.tsx \
  src/App.test.tsx \
  src/styles/adaptiveTextPanel.test.ts
pnpm --dir ui check
```

Expected: all shelf, session, App integration, image-grid, text-preview,
marker, radial, Finder-drag, organization-drag, and style tests PASS.

- [ ] **Step 8: Commit the smart shelf**

```bash
git add \
  ui/src/components/contentBrowser/TextFilePanel.tsx \
  ui/src/components/contentBrowser/TextFilePanel.test.tsx \
  ui/src/App.tsx \
  ui/src/components/ContentBrowser.tsx \
  ui/src/components/ContentBrowser.test.tsx \
  ui/src/styles/adaptiveTextPanel.css \
  ui/src/styles/adaptiveTextPanel.test.ts \
  ui/src/App.test.tsx
git commit -m "feat: add the adaptive text file shelf"
```

---

## Task 5: Add File-Type-Aware Select All

**Files:**

- Create: `ui/src/components/contentBrowser/SelectAllChoicePanel.tsx`
- Create: `ui/src/components/contentBrowser/SelectAllChoicePanel.test.tsx`
- Modify: `ui/src/components/ContentBrowser.tsx`
- Modify: `ui/src/components/ContentBrowser.test.tsx`
- Modify: `ui/src/styles/adaptiveTextPanel.css`
- Modify: `ui/src/styles/adaptiveTextPanel.test.ts`

**Interfaces:**

- `SelectAllChoicePanelProps`:
  - `open: boolean`
  - `anchorRef: RefObject<HTMLButtonElement | null>`
  - `onChoose(scope: SelectAllScope): void`
  - `onCancel(): void`
- The panel has exactly three `menuitem` commands in source order:
  `images`, `text`, `all`.

- [ ] **Step 1: Write failing choice-panel tests**

Create `SelectAllChoicePanel.test.tsx` with a harness containing the source
button and panel. Cover:

```tsx
it('focuses the first command and chooses one exact scope', () => {
  const choose = vi.fn()
  render(<ChoiceHarness open onChoose={choose} />)
  expect(screen.getByRole('menuitem', { name: '全选图片' })).toHaveFocus()
  fireEvent.click(screen.getByRole('menuitem', { name: '全选文本文件' }))
  expect(choose).toHaveBeenCalledWith('text')
})

it('cycles menu focus with arrow keys and activates with Enter', () => {
  const choose = vi.fn()
  render(<ChoiceHarness open onChoose={choose} />)
  const menu = screen.getByRole('menu', { name: '选择全选范围' })
  fireEvent.keyDown(menu, { key: 'ArrowDown' })
  expect(screen.getByRole('menuitem', { name: '全选文本文件' })).toHaveFocus()
  fireEvent.keyDown(menu, { key: 'ArrowUp' })
  expect(screen.getByRole('menuitem', { name: '全选图片' })).toHaveFocus()
  fireEvent.keyDown(menu, { key: 'Enter' })
  expect(choose).toHaveBeenCalledWith('images')
})

it('keeps every command in the natural Tab order without trapping Tab', () => {
  render(<ChoiceHarness open />)
  const items = screen.getAllByRole('menuitem')
  expect(items).toHaveLength(3)
  expect(items.every((item) => item.getAttribute('tabindex') !== '-1')).toBe(true)
  const tab = createEvent.keyDown(items[0], { key: 'Tab' })
  fireEvent(items[0], tab)
  expect(tab.defaultPrevented).toBe(false)
})

it('cancels with Escape, preserves selection ownership, and restores source focus', () => {
  const choose = vi.fn()
  const cancel = vi.fn()
  render(<ChoiceHarness open onChoose={choose} onCancel={cancel} />)
  fireEvent.keyDown(screen.getByRole('menu', { name: '选择全选范围' }), {
    key: 'Escape',
  })
  expect(cancel).toHaveBeenCalledOnce()
  expect(choose).not.toHaveBeenCalled()
  expect(screen.getByRole('button', { name: '全选当前文件夹' })).toHaveFocus()
})

it('cancels when the source is reactivated and restores source focus', () => {
  const choose = vi.fn()
  const cancel = vi.fn()
  render(<ChoiceHarness open onChoose={choose} onCancel={cancel} />)
  fireEvent.click(screen.getByRole('button', { name: '全选当前文件夹' }))
  expect(cancel).toHaveBeenCalledOnce()
  expect(choose).not.toHaveBeenCalled()
  expect(screen.getByRole('button', { name: '全选当前文件夹' })).toHaveFocus()
})

it('cancels on an outside pointer and restores source focus', () => {
  const choose = vi.fn()
  const cancel = vi.fn()
  render(<ChoiceHarness open onChoose={choose} onCancel={cancel} />)
  fireEvent.pointerDown(screen.getByRole('button', { name: '外部目标' }))
  expect(cancel).toHaveBeenCalledOnce()
  expect(choose).not.toHaveBeenCalled()
  expect(screen.getByRole('button', { name: '全选当前文件夹' })).toHaveFocus()
})
```

`ChoiceHarness` must route source reactivation to the same `onCancel` path as
the production wrapper and include one neutral `外部目标` button. Stale
workspace-driven closure is separate and must not attempt to focus an
unmounted source. Import `createEvent`, `fireEvent`, `render`, and `screen`
from Testing Library for these exact interactions.

- [ ] **Step 2: Run the choice-panel test and verify red**

```bash
pnpm --dir ui test -- src/components/contentBrowser/SelectAllChoicePanel.test.tsx
```

Expected: FAIL because the component does not exist.

- [ ] **Step 3: Implement the anchored, non-modal menu**

Create `SelectAllChoicePanel.tsx`:

- return `null` while closed;
- render `role="menu"` and three native buttons with `role="menuitem"`;
- focus the first item in a layout effect;
- keep item refs in stable source order;
- ArrowDown/ArrowUp wrap, Home/End move to the endpoints, Enter/Space invoke
  the focused command, and Escape calls `onCancel`;
- attach a capture-phase `pointerdown` listener while open and cancel only
  when the target is outside both the panel and `anchorRef.current`;
- after user-triggered cancellation, focus `anchorRef.current` if it remains
  connected;
- disconnect the listener on cleanup;
- do not store file objects or selection snapshots inside the component.

The render contract is:

```tsx
<div
  ref={panelRef}
  className="select-all-choice-panel"
  role="menu"
  aria-label="选择全选范围"
  onKeyDown={handleKeyDown}
>
  <button type="button" role="menuitem" onClick={() => onChoose('images')}>
    全选图片
  </button>
  <button type="button" role="menuitem" onClick={() => onChoose('text')}>
    全选文本文件
  </button>
  <button type="button" role="menuitem" onClick={() => onChoose('all')}>
    全部选择
  </button>
</div>
```

- [ ] **Step 4: Replace unconditional select all in `ContentBrowser`**

Add refs/state:

```ts
const viewMenuRef = useRef<HTMLDetailsElement>(null)
const selectAllButtonRef = useRef<HTMLButtonElement>(null)
const [selectAllChoiceOpen, setSelectAllChoiceOpen] = useState(false)
const selectAllRequest = resolveSelectAllRequest(
  workspace.images.length,
  workspace.textFiles.length,
)
```

Replace `selectAllFiles()` with:

```ts
function commitSelectAll(scope: SelectAllScope) {
  const files = filesForSelectAllScope(workspace, scope)
  const first = files[0] ?? null
  setActiveId(first?.entityId ?? null)
  anchorId.current = first?.entityId ?? null
  commitSelection(new Set(files.map(({ entityId }) => entityId)))
  setSelectAllChoiceOpen(false)
}

function requestSelectAll() {
  const request = resolveSelectAllRequest(
    workspace.images.length,
    workspace.textFiles.length,
  )
  if (request.kind === 'none') return
  if (request.kind === 'direct') {
    commitSelectAll(request.scope)
    return
  }
  if (viewMenuRef.current !== null) viewMenuRef.current.open = true
  setSelectAllChoiceOpen(true)
}
```

Use `requestSelectAll` from both the visible action and Command-A.

Wrap the source and panel:

```tsx
<details
  ref={viewMenuRef}
  className="content-view-menu"
  onToggle={(event) => {
    if (!event.currentTarget.open) setSelectAllChoiceOpen(false)
  }}
>
  <summary>视图</summary>
  <div>
    <div className="select-all-control">
      <button
        ref={selectAllButtonRef}
        type="button"
        aria-haspopup={selectAllRequest.kind === 'choice' ? 'menu' : undefined}
        aria-expanded={selectAllChoiceOpen || undefined}
        onClick={() => {
          if (selectAllChoiceOpen) {
            setSelectAllChoiceOpen(false)
            selectAllButtonRef.current?.focus()
          } else {
            requestSelectAll()
          }
        }}
      >
        全选当前文件夹
      </button>
      <SelectAllChoicePanel
        open={selectAllChoiceOpen}
        anchorRef={selectAllButtonRef}
        onChoose={commitSelectAll}
        onCancel={() => setSelectAllChoiceOpen(false)}
      />
    </div>
  </div>
</details>
```

Compute `resolveSelectAllRequest` once per render rather than calling it
inside JSX. Keep the exact action button enabled whenever at least one file
exists.

- [ ] **Step 5: Close stale choices at every approved boundary**

Add an effect keyed by the current collection identities and path:

```ts
const contentIdentity = useMemo(
  () =>
    JSON.stringify(
      [...workspace.images, ...workspace.textFiles].map(({ entityId, modifiedNs }) => [
        entityId,
        modifiedNs,
      ]),
    ),
  [workspace.images, workspace.textFiles],
)

useEffect(() => {
  setSelectAllChoiceOpen(false)
}, [contentIdentity, currentPath])

useEffect(() => {
  if (organizationDragDisabled) setSelectAllChoiceOpen(false)
}, [organizationDragDisabled])
```

Also close before:

- invoking `onPreview`;
- invoking `onRadialMenuRequest` (comparison entry is reached through this
  owner);
- a choice commit.

Do not clear the current file selection in any close path.

- [ ] **Step 6: Replace the old select-all tests with the approved matrix**

In `ContentBrowser.test.tsx`, remove the two tests that expect mixed
Command-A/button activation to select all immediately. Add:

- mixed button opens exactly three choices and changes no selection;
- mixed Command-A opens the same panel and moves focus to `全选图片`;
- each command selects exactly its scope and closes;
- selecting hidden text keeps the shelf collapsed and shows `已选 N`;
- image-only button and Command-A select all images directly with no menu;
- text-only button and Command-A select all texts directly with no menu;
- Escape, source reactivation, and outside click preserve the previous
  selection;
- content identity change, folder path change, preview, and radial request
  close the panel before a stale choice can run;
- rerendering with `organizationDragDisabled` after comparison entry closes
  the panel even though the hidden `ContentBrowser` stays mounted;
- choice order is images, text, all;
- opening the choice panel does not force the shelf open.

Use `selectedLabels()` only on mounted options. For collapsed hidden texts,
assert the `onSelectionChange` payload and disclosure `已选 N` copy instead of
expecting hidden rows in the DOM.

- [ ] **Step 7: Style and pin the anchored panel**

Append:

```css
.select-all-control {
  position: relative;
}

.select-all-choice-panel {
  background: #fff;
  border: 1px solid #c8ced6;
  border-radius: 8px;
  box-shadow: 0 10px 28px rgb(34 42 53 / 18%);
  display: grid;
  gap: 4px;
  min-width: 148px;
  padding: 6px;
  position: absolute;
  right: calc(100% + 8px);
  top: 0;
  z-index: 24;
}

.select-all-choice-panel > button {
  text-align: left;
  white-space: nowrap;
}
```

Add focus-visible and hover rules consistent with the existing toolbar
buttons. Extend the focused style test for relative anchoring, absolute
placement, compact minimum width, surface, border, radius, shadow, and
`z-index`.

- [ ] **Step 8: Run select-all and complete ContentBrowser tests**

```bash
pnpm --dir ui test -- \
  src/components/contentBrowser/adaptiveTextPanelModel.test.ts \
  src/components/contentBrowser/SelectAllChoicePanel.test.tsx \
  src/components/contentBrowser/TextFilePanel.test.tsx \
  src/components/ContentBrowser.test.tsx \
  src/App.test.tsx \
  src/styles/adaptiveTextPanel.test.ts
pnpm --dir ui check
```

Expected: PASS.

- [ ] **Step 9: Commit file-type-aware select all**

```bash
git add \
  ui/src/components/contentBrowser/SelectAllChoicePanel.tsx \
  ui/src/components/contentBrowser/SelectAllChoicePanel.test.tsx \
  ui/src/components/ContentBrowser.tsx \
  ui/src/components/ContentBrowser.test.tsx \
  ui/src/styles/adaptiveTextPanel.css \
  ui/src/styles/adaptiveTextPanel.test.ts
git commit -m "feat: add scoped select all choices"
```

---

## Task 6: Document, Verify, and Inspect the Latest Development Build

**Files:**

- Modify: `docs/PRODUCT_SPEC.md`
- Modify: `docs/README.md`
- Test: all new and affected UI suites
- Test: repository policy and full quality gates

- [ ] **Step 1: Update the product requirement**

Replace the unconditional browsing sentence in `docs/PRODUCT_SPEC.md` with:

```md
内容文件夹中的图片默认以缩略图网格展示。图片文件夹不显示文本区域；
图文混合文件夹默认显示可展开的紧凑“文本文件”底部栏，展开高度按内容自适应且
最多占内容区可用高度的 20%；纯文本文件夹自动展开文本列表并使用完整内容区。
文本文件仍作为独立列表参与预览、选择、标记和整理。
```

Add the file-type-aware select-all rule immediately after it:

```md
“全选当前文件夹”在单一文件类型中直接全选；图文混合时由用户明确选择图片、
文本文件或全部文件。
```

- [ ] **Step 2: Run the complete UI validation**

```bash
pnpm --dir ui test
pnpm --dir ui check
pnpm --dir ui build
```

Expected: all PASS with no React act warnings, unhandled promise rejections,
duplicate thumbnail calls, accessibility reference warnings, or TypeScript
errors.

- [ ] **Step 3: Run repository-wide gates**

```bash
pnpm test:policy
pnpm quality
```

Expected: all policy, architecture, UI, Rust, formatting, lint, type-check,
build, and test gates PASS.

- [ ] **Step 4: Prove unrelated dirty work remains untouched**

```bash
git status --short
git diff -- ui/src/styles/app.css ui/src/styles/app.test.ts | git patch-id --stable
git diff --cached --name-only
```

Expected:

- the pre-existing `app.css` plus `app.test.ts` patch ID remains
  `18e7f09004c2e06622fd15e0b20c6c02dd62e6a1`;
- the user-owned FolderTree, VirtualList, style, and fixture changes remain
  unstaged;
- only this feature’s documented files are staged for its commits.

- [ ] **Step 5: Launch exactly the latest local development tree**

First record source state:

```bash
git branch --show-current
git rev-parse --short HEAD
git status --short
```

Then run only:

```bash
pnpm start:viewer
```

Verify exactly one process uses:

```text
/Users/abc/Project/Viewer/target/debug/viewer-desktop
```

Do not open `/Applications/Viewer.app`, a packaged app, or any removed
worktree. The launcher must report the current dirty tree and preserve it.

- [ ] **Step 6: Perform live acceptance in the native app**

Using the project fixture or the user’s current project, inspect these states
at the same window size:

1. image-only: no text surface and image grid reaches the content bottom;
2. mixed collapsed: compact button-shaped bottom bar;
3. mixed expanded with few texts: only the intrinsic height is used;
4. mixed expanded with many texts: shelf remains within 20% and list scrolls;
5. text-only: list fills the body without an empty image slot;
6. selected hidden text: collapsed bar shows `已选 N`;
7. mixed select all: anchored three-choice panel and correct keyboard focus;
8. toggle the shelf after scrolling/loading thumbnails: no scroll jump,
   flashing reload, duplicate Viewer process, or stale black/packaged UI.

Resize the window shorter and taller and confirm the 20% cap, disclosure, and
grid remain usable. A screenshot is evidence only; also interact with the
disclosure, list scroll, select-all choices, preview, radial menu, Finder drag,
and organization handle.

- [ ] **Step 7: Mark the plan historical and run policy again**

Move the plan’s `docs/README.md` row from “Active sources of truth” to
“Historical implementation plans” with status `Historical`. Keep the approved
design spec Active.

```bash
pnpm test:policy
```

Expected: PASS.

- [ ] **Step 8: Commit documentation and final verification evidence**

```bash
git add \
  docs/PRODUCT_SPEC.md \
  docs/README.md
git commit -m "docs: record adaptive text panel behavior"
```

Do not stage the user-owned dirty files.

- [ ] **Step 9: Final self-review before claiming completion**

Review:

```bash
git log --oneline --decorate -8
git show --stat --oneline HEAD
git status --short --branch
```

Then compare the implementation line by line with every item in the design
spec’s Test And Acceptance Matrix. Do not claim completion if any focused
test, full gate, native interaction, strict-cap case, session reset, stale
choice close, or dirty-worktree preservation check is missing.
