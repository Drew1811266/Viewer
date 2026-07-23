# Viewer Radial Context-Menu Fallback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the file radial menu open reliably for every macOS secondary-click path, including Control-click and the organization handle, without duplicating standard right-click requests or breaking held radial gestures.

**Architecture:** `ContentBrowser` will keep the existing secondary-button pointer path for held gestures and add a shared `contextmenu` fallback for click mode. A one-second, entity-scoped record will deduplicate the normal pointer/context event pair, while the organization handle will allow secondary and Control-click input to bubble to the file item.

**Tech Stack:** React 19, TypeScript 6, Testing Library, Vitest 4

## Global Constraints

- Preserve secondary-button `pointerdown` with its pointer id for press-drag-release marking.
- Use `contextmenu` only as a click-mode fallback with a null pointer id.
- A paired `pointerdown` and `contextmenu` sequence must create exactly one request.
- The entire image or text item, including the organization handle, must accept secondary input.
- Do not change selection, preview, focus restoration, keyboard behavior, or primary-button organization dragging.
- Add no dependencies.

---

### Task 1: Add the macOS context-menu fallback

**Files:**
- Modify: `ui/src/components/ContentBrowser.tsx:1-615`
- Test: `ui/src/components/ContentBrowser.test.tsx:540-620`

**Interfaces:**
- Consumes: `RadialMenuRequest` with `pointerId: number | null`
- Produces: `openRadialMenuFromPointer(file, event)` for held gestures and `openRadialMenuFromContext(file, event)` for click-mode fallback

- [ ] **Step 1: Write failing component tests**

Replace the native-menu-only assertion with tests that exercise the real macOS event paths:

```tsx
it('falls back to click-mode radial requests for image and text context-menu events', () => {
  const request = vi.fn()
  render(<ContentBrowser workspace={workspace(1)} onRadialMenuRequest={request} />)

  for (const [name, entityId, listName] of [
    ['1.jpg', 'image-1', '图片文件'],
    ['prompt.md', 'text-1', '文本文件'],
  ] as const) {
    const option = screen.getByRole('option', { name })
    const event = createEvent.contextMenu(option, {
      button: 0,
      ctrlKey: true,
      clientX: 240,
      clientY: 180,
    })
    fireEvent(option, event)

    expect(event.defaultPrevented).toBe(true)
    expect(request).toHaveBeenLastCalledWith({
      files: [expect.objectContaining({ entityId })],
      origin: { x: 240, y: 180 },
      pointerId: null,
      returnFocusTarget: screen.getByRole('listbox', { name: listName }),
    })
  }
  expect(request).toHaveBeenCalledTimes(2)
})

it('deduplicates a secondary pointerdown followed by contextmenu', () => {
  const request = vi.fn()
  render(<ContentBrowser workspace={workspace(1)} onRadialMenuRequest={request} />)
  const option = screen.getByRole('option', { name: '1.jpg' })

  fireEvent.pointerDown(option, {
    pointerId: 73,
    button: 2,
    clientX: 210,
    clientY: 160,
  })
  fireEvent.contextMenu(option, {
    button: 2,
    clientX: 210,
    clientY: 160,
  })

  expect(request).toHaveBeenCalledTimes(1)
  expect(request.mock.calls[0]?.[0]).toEqual(
    expect.objectContaining({ pointerId: 73 }),
  )
})

it('routes secondary and Control-click input on the organization handle to the radial menu', () => {
  const request = vi.fn()
  const organize = vi.fn()
  render(
    <ContentBrowser
      workspace={workspace(1)}
      onRadialMenuRequest={request}
      onOrganizationPointerInput={organize}
    />,
  )
  const handle = screen.getByRole('button', { name: '整理 1.jpg' })

  fireEvent.pointerDown(handle, {
    pointerId: 74,
    button: 2,
    clientX: 220,
    clientY: 170,
  })
  fireEvent.contextMenu(handle, {
    button: 2,
    clientX: 220,
    clientY: 170,
  })
  fireEvent.pointerDown(handle, {
    pointerId: 75,
    button: 0,
    ctrlKey: true,
    clientX: 225,
    clientY: 175,
  })
  fireEvent.contextMenu(handle, {
    button: 0,
    ctrlKey: true,
    clientX: 225,
    clientY: 175,
  })

  expect(request).toHaveBeenCalledTimes(2)
  expect(request.mock.calls.map(([item]) => item.pointerId)).toEqual([74, null])
  expect(organize).not.toHaveBeenCalled()
})
```

- [ ] **Step 2: Run the focused tests and verify RED**

Run:

```bash
pnpm --dir ui exec vitest run src/components/ContentBrowser.test.tsx -t "falls back to click-mode|deduplicates a secondary|routes secondary and Control-click"
```

Expected: the fallback test reports no radial request and the handle test reports
that secondary input was intercepted. The paired-event test already passes
because `contextmenu` currently never requests a menu; it becomes the regression
lock that prevents the fallback implementation from creating duplicates.

- [ ] **Step 3: Implement the minimal dual-event router**

In `ContentBrowser`, add an entity-scoped deduplication record and clear its timer on unmount:

```tsx
const radialContextDeduplication = useRef<{
  entityId: string
  timeoutId: number
} | null>(null)

useEffect(
  () => () => {
    if (radialContextDeduplication.current !== null) {
      window.clearTimeout(radialContextDeduplication.current.timeoutId)
    }
  },
  [],
)
```

Replace the current pointer-only helper with shared request preparation plus two event adapters:

```tsx
function requestRadialMenu(
  file: BrowserFile,
  eventTarget: HTMLElement,
  origin: { x: number; y: number },
  pointerId: number | null,
) {
  if (onRadialMenuRequest === undefined) return
  const returnFocusTarget =
    eventTarget.closest<HTMLElement>('[role="listbox"]') ?? eventTarget
  const contextSelection = selected.has(file.entityId) ? selected : new Set([file.entityId])
  if (!selected.has(file.entityId)) {
    anchorId.current = file.entityId
    setActiveId(file.entityId)
    commitSelection(contextSelection)
  }
  onRadialMenuRequest({
    files: allFiles.filter((candidate) => contextSelection.has(candidate.entityId)),
    origin,
    pointerId,
    returnFocusTarget,
  })
}

function clearRadialContextDeduplication() {
  if (radialContextDeduplication.current === null) return
  window.clearTimeout(radialContextDeduplication.current.timeoutId)
  radialContextDeduplication.current = null
}

function openRadialMenuFromPointer(file: BrowserFile, event: PointerEvent<HTMLElement>) {
  if (event.button !== 2 || onRadialMenuRequest === undefined) return
  event.preventDefault()
  event.stopPropagation()
  clearRadialContextDeduplication()
  const timeoutId = window.setTimeout(() => {
    if (radialContextDeduplication.current?.timeoutId === timeoutId) {
      radialContextDeduplication.current = null
    }
  }, 1_000)
  radialContextDeduplication.current = { entityId: file.entityId, timeoutId }
  requestRadialMenu(
    file,
    event.currentTarget,
    { x: event.clientX, y: event.clientY },
    event.pointerId,
  )
}

function openRadialMenuFromContext(file: BrowserFile, event: MouseEvent<HTMLElement>) {
  event.preventDefault()
  event.stopPropagation()
  if (onRadialMenuRequest === undefined) return
  const alreadyHandled =
    radialContextDeduplication.current?.entityId === file.entityId
  clearRadialContextDeduplication()
  if (alreadyHandled) return
  requestRadialMenu(
    file,
    event.currentTarget,
    { x: event.clientX, y: event.clientY },
    null,
  )
}
```

Wire both image and text items to the adapters. Pass both handlers into
`ImageCell`:

```tsx
<ImageCell
  file={file}
  selected={selected.has(file.entityId)}
  active={activeId === file.entityId}
  maxPixels={cellPixels}
  scaleMilli={scaleMilli}
  loadThumbnail={loadThumbnail}
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
```

Add the context handler to the `ImageCell` parameters and type:

```tsx
function ImageCell({
  file,
  selected,
  active,
  maxPixels,
  scaleMilli,
  loadThumbnail,
  onClick,
  onPreview,
  onRadialMenuPointerDown,
  onRadialMenuContextMenu,
  organizationDragDisabled,
  onFinderDragStart,
  onPointerDown,
  onPointerMove,
  onPointerUp,
  onPointerCancel,
}: {
  file: BrowserFile
  selected: boolean
  active: boolean
  maxPixels: number
  scaleMilli: number
  loadThumbnail: (file: BrowserFile, maxPixels: number, scaleMilli: number) => Promise<string>
  onClick: (file: BrowserFile, event: MouseEvent) => void
  onPreview: (file: BrowserFile) => void
  onRadialMenuPointerDown: (file: BrowserFile, event: PointerEvent<HTMLElement>) => void
  onRadialMenuContextMenu: (file: BrowserFile, event: MouseEvent<HTMLElement>) => void
  organizationDragDisabled: boolean
  onFinderDragStart: (file: BrowserFile, event: DragEvent<HTMLElement>) => void
  onPointerDown: (file: BrowserFile, event: PointerEvent<HTMLElement>) => void
  onPointerMove: (event: PointerEvent<HTMLElement>) => void
  onPointerUp: (event: PointerEvent<HTMLElement>) => void
  onPointerCancel: (event: PointerEvent<HTMLElement>) => void
}) {
```

Replace the image cell's inline native-menu suppression:

```tsx
onPointerDown={(event) => onRadialMenuPointerDown(file, event)}
onContextMenu={(event) => onRadialMenuContextMenu(file, event)}
```

For text rows:

```tsx
onPointerDown={(event) => openRadialMenuFromPointer(file, event)}
onContextMenu={(event) => openRadialMenuFromContext(file, event)}
```

Allow secondary and Control-click input to bubble out of the organization handle:

```tsx
function startPointerOrganization(file: BrowserFile, event: PointerEvent<HTMLElement>) {
  if (event.button !== 0 || event.ctrlKey) return
  event.preventDefault()
  event.stopPropagation()
  if (organizationDragDisabled) return
  const entityIds = freezeDragSelection(file)
  if (entityIds.length === 0) return
  onOrganizationPointerInput?.({
    type: 'start',
    pointerId: event.pointerId,
    entityIds,
    mode: event.altKey ? 'copy' : 'move',
    clientX: event.clientX,
    clientY: event.clientY,
    captureNode: event.currentTarget,
  })
}
```

- [ ] **Step 4: Run focused tests and verify GREEN**

Run:

```bash
pnpm --dir ui exec vitest run src/components/ContentBrowser.test.tsx
```

Expected: `ContentBrowser.test.tsx` passes with no warnings.

- [ ] **Step 5: Run application-level radial tests**

Run:

```bash
pnpm --dir ui exec vitest run src/App.test.tsx src/components/RadialFileMenu.test.tsx
```

Expected: both test files pass, including held gesture, click fallback, focus restoration, frozen selection, and close-session coverage.

- [ ] **Step 6: Run full repository verification**

Run:

```bash
pnpm verify
```

Expected: repository policy, all UI tests, UI production build, Rust formatting, Clippy with warnings denied, and all Rust tests pass.

- [ ] **Step 7: Commit the implementation**

```bash
git add ui/src/components/ContentBrowser.tsx ui/src/components/ContentBrowser.test.tsx
git commit -m "fix(ui): support macOS radial context menus"
```
