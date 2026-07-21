# Viewer Image Marquee Selection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add visible, Finder-style marquee selection to the virtualized image grid without changing card-body Finder export or organization-handle drag ownership.

**Architecture:** `marqueeSelection.ts` owns pure geometry and edge-scroll calculations. `VirtualGrid` owns the low-level pointer-capture session, canonical virtual-layout hit testing, overlay, and auto-scroll; it emits ordered keys and lifecycle phases. `ContentBrowser` alone owns Viewer selection semantics and folds those low-level events into the existing `commitSelection` path.

**Tech Stack:** React 19, TypeScript 6, Pointer Events, `requestAnimationFrame`, Vitest 4, Testing Library, CSS.

## Global Constraints

- macOS target remains Apple Silicon on macOS 13 or newer; Windows is not part of Viewer 0.1.
- Add no runtime or development dependency.
- Only a primary-button gesture that begins on the image-grid background may own a marquee session.
- Image-card body drag remains immediate, copy-only native Finder export; the `⋮⋮` handle remains the only internal organization drag initiator.
- The frontend continues to use opaque entity IDs and must not receive or publish filesystem paths.
- Marquee hit testing must use `VirtualGrid`'s canonical width/column/cell/gap layout and must cover virtualized items that are not mounted.
- Ordinary marquee replaces selection; Command state is frozen at pointer-down and toggles hits against the start snapshot; Shift retains its existing contiguous-range meaning only.
- The marquee appears after 4 CSS px, uses a visible blue overlay and live item highlight, and auto-scrolls within 32 CSS px of the top/bottom edge at no more than 18 CSS px per animation frame.
- Pointer cancel or Escape restores the start snapshot. Workspace replacement and unmount stop pointer capture/animation without publishing stale keys.
- Text-file rows do not gain marquee selection in this change.
- Every production behavior is preceded by a failing test and every task ends with a focused commit.

---

## File map

- Create `ui/src/components/marqueeSelection.ts`: pure point/rectangle normalization, virtual-grid hit testing, activation-distance and vertical edge-scroll calculations.
- Create `ui/src/components/marqueeSelection.test.ts`: exhaustive pure-function boundary tests, including unmounted virtual items.
- Create `ui/src/components/VirtualGrid.test.tsx`: pointer ownership, overlay, callback lifecycle, cancellation and auto-scroll component tests.
- Modify `ui/src/components/VirtualGrid.tsx`: optional marquee interface and low-level pointer session.
- Modify `ui/src/components/ContentBrowser.tsx`: start-snapshot selection reducer and active/anchor updates.
- Modify `ui/src/components/ContentBrowser.test.tsx`: Viewer semantics and three-owner drag regressions.
- Modify `ui/src/styles/app.css`: marquee overlay, cursor and text-selection suppression.

### Task 1: Pure marquee geometry and virtual-layout hit testing

**Files:**
- Create: `ui/src/components/marqueeSelection.ts`
- Create: `ui/src/components/marqueeSelection.test.ts`

**Interfaces:**
- Consumes: numeric CSS-pixel points and the exact layout values already calculated by `VirtualGrid`.
- Produces:

```ts
export interface MarqueePoint { x: number; y: number }
export interface MarqueeRect { left: number; top: number; right: number; bottom: number; width: number; height: number }
export interface VirtualGridGeometry {
  itemCount: number
  columns: number
  cellWidth: number
  cellHeight: number
  columnStride: number
  rowStride: number
}
export function normalizeMarquee(start: MarqueePoint, current: MarqueePoint): MarqueeRect
export function marqueeDistance(start: MarqueePoint, current: MarqueePoint): number
export function intersectingGridIndexes(rect: MarqueeRect, geometry: VirtualGridGeometry): number[]
export function verticalAutoScrollDelta(pointerY: number, viewportTop: number, viewportBottom: number): number
```

- [ ] **Step 1: Write failing geometry tests**

Create `ui/src/components/marqueeSelection.test.ts`:

```ts
import { describe, expect, it } from 'vitest'
import {
  intersectingGridIndexes,
  marqueeDistance,
  normalizeMarquee,
  verticalAutoScrollDelta,
} from './marqueeSelection'

const geometry = {
  itemCount: 20,
  columns: 4,
  cellWidth: 100,
  cellHeight: 80,
  columnStride: 112,
  rowStride: 92,
}

describe('marqueeSelection', () => {
  it('normalizes forward and reverse pointer movement', () => {
    expect(normalizeMarquee({ x: 220, y: 180 }, { x: 10, y: 20 })).toEqual({
      left: 10, top: 20, right: 220, bottom: 180, width: 210, height: 160,
    })
  })

  it('measures the activation distance', () => {
    expect(marqueeDistance({ x: 1, y: 1 }, { x: 4, y: 5 })).toBe(5)
  })

  it('hits touching cells in stable item order and excludes gaps', () => {
    expect(intersectingGridIndexes(normalizeMarquee({ x: 100, y: 0 }, { x: 112, y: 80 }), geometry))
      .toEqual([0, 1])
    expect(intersectingGridIndexes(normalizeMarquee({ x: 105, y: 5 }, { x: 107, y: 70 }), geometry))
      .toEqual([])
  })

  it('hits virtual items outside the mounted viewport', () => {
    expect(intersectingGridIndexes(normalizeMarquee({ x: 0, y: 368 }, { x: 212, y: 448 }), geometry))
      .toEqual([16, 17])
  })

  it('clamps edge scrolling to the specified 18 pixel maximum', () => {
    expect(verticalAutoScrollDelta(100, 100, 300)).toBe(-18)
    expect(verticalAutoScrollDelta(116, 100, 300)).toBe(-9)
    expect(verticalAutoScrollDelta(284, 100, 300)).toBe(9)
    expect(verticalAutoScrollDelta(300, 100, 300)).toBe(18)
    expect(verticalAutoScrollDelta(200, 100, 300)).toBe(0)
  })
})
```

- [ ] **Step 2: Run the tests and verify RED**

Run:

```bash
pnpm --dir ui test -- src/components/marqueeSelection.test.ts
```

Expected: FAIL because `./marqueeSelection` does not exist.

- [ ] **Step 3: Implement the pure functions**

Create `ui/src/components/marqueeSelection.ts` with these exact rules:

```ts
export interface MarqueePoint { x: number; y: number }
export interface MarqueeRect {
  left: number
  top: number
  right: number
  bottom: number
  width: number
  height: number
}
export interface VirtualGridGeometry {
  itemCount: number
  columns: number
  cellWidth: number
  cellHeight: number
  columnStride: number
  rowStride: number
}

export function normalizeMarquee(start: MarqueePoint, current: MarqueePoint): MarqueeRect {
  const left = Math.min(start.x, current.x)
  const top = Math.min(start.y, current.y)
  const right = Math.max(start.x, current.x)
  const bottom = Math.max(start.y, current.y)
  return { left, top, right, bottom, width: right - left, height: bottom - top }
}

export function marqueeDistance(start: MarqueePoint, current: MarqueePoint): number {
  return Math.hypot(current.x - start.x, current.y - start.y)
}

export function intersectingGridIndexes(
  rect: MarqueeRect,
  geometry: VirtualGridGeometry,
): number[] {
  if (geometry.itemCount === 0) return []
  const rowCount = Math.ceil(geometry.itemCount / geometry.columns)
  const firstRow = Math.max(0, Math.floor(rect.top / geometry.rowStride))
  const lastRow = Math.min(rowCount - 1, Math.floor(rect.bottom / geometry.rowStride))
  const firstColumn = Math.max(0, Math.floor(rect.left / geometry.columnStride))
  const lastColumn = Math.min(geometry.columns - 1, Math.floor(rect.right / geometry.columnStride))
  const hits: number[] = []
  for (let row = firstRow; row <= lastRow; row += 1) {
    for (let column = firstColumn; column <= lastColumn; column += 1) {
      const index = row * geometry.columns + column
      if (index >= geometry.itemCount) continue
      const left = column * geometry.columnStride
      const top = row * geometry.rowStride
      const right = left + geometry.cellWidth
      const bottom = top + geometry.cellHeight
      if (rect.left <= right && rect.right >= left && rect.top <= bottom && rect.bottom >= top) {
        hits.push(index)
      }
    }
  }
  return hits
}

export function verticalAutoScrollDelta(
  pointerY: number,
  viewportTop: number,
  viewportBottom: number,
): number {
  const edge = 32
  const max = 18
  if (pointerY < viewportTop + edge) {
    return -Math.min(max, Math.max(0, ((viewportTop + edge - pointerY) / edge) * max))
  }
  if (pointerY > viewportBottom - edge) {
    return Math.min(max, Math.max(0, ((pointerY - (viewportBottom - edge)) / edge) * max))
  }
  return 0
}
```

Use inclusive intersection so touching a card border counts. Clamp candidate row/column ranges before looping so work is proportional to the marquee bounds rather than the full project.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run:

```bash
pnpm --dir ui test -- src/components/marqueeSelection.test.ts
```

Expected: 5 tests PASS.

- [ ] **Step 5: Commit Task 1**

```bash
git add ui/src/components/marqueeSelection.ts ui/src/components/marqueeSelection.test.ts
git commit -m "feat: add virtual-grid marquee geometry"
```

### Task 2: Generic VirtualGrid marquee pointer session and overlay

**Files:**
- Create: `ui/src/components/VirtualGrid.test.tsx`
- Modify: `ui/src/components/VirtualGrid.tsx`

**Interfaces:**
- Consumes: Task 1's four pure functions and the existing `getKey`, `items`, layout dimensions and scroll container.
- Produces:

```ts
export type MarqueePhase = 'start' | 'change' | 'end' | 'cancel'
export interface MarqueeSelectionChange {
  phase: MarqueePhase
  keys: string[]
  metaKey: boolean
}

interface VirtualGridProps<T> {
  // existing props remain unchanged
  ariaMultiselectable?: boolean
  onMarqueeSelectionChange?: (change: MarqueeSelectionChange) => void
}
```

- [ ] **Step 1: Write failing component tests for ownership, visibility and lifecycle**

Create `ui/src/components/VirtualGrid.test.tsx` with this harness. It renders six keyed items with `cellWidth={100}`, `cellHeight={80}`, `gap={12}` and stubs the grid bounds and pointer-capture methods:

```tsx
import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import VirtualGrid, { type MarqueeSelectionChange } from './VirtualGrid'

afterEach(() => vi.unstubAllGlobals())

function renderGrid(changed: (change: MarqueeSelectionChange) => void) {
  const items = Array.from({ length: 6 }, (_, index) => `item-${index}`)
  render(
    <VirtualGrid
      items={items}
      cellWidth={100}
      cellHeight={80}
      viewportHeight={200}
      gap={12}
      getKey={(item) => item}
      renderItem={(item) => <span>{item}</span>}
      ariaLabel="files"
      onMarqueeSelectionChange={changed}
    />,
  )
}

function installPointerSurface(grid: HTMLElement) {
  vi.spyOn(grid, 'getBoundingClientRect').mockReturnValue({
    left: 10, top: 20, right: 410, bottom: 220,
    width: 400, height: 200, x: 10, y: 20, toJSON: () => undefined,
  })
  Object.defineProperty(grid, 'setPointerCapture', { value: vi.fn(), configurable: true })
  Object.defineProperty(grid, 'releasePointerCapture', { value: vi.fn(), configurable: true })
}
```

Add these tests:

```ts
it('starts only on background, crosses the threshold, and emits ordered keys with an overlay', () => {
  const changed = vi.fn()
  renderGrid(changed)
  const grid = screen.getByRole('listbox', { name: 'files' })
  installPointerSurface(grid)
  fireEvent.pointerDown(grid, { pointerId: 7, button: 0, clientX: 122, clientY: 110 })
  fireEvent.pointerMove(grid, { pointerId: 7, clientX: 12, clientY: 22 })
  expect(changed).toHaveBeenNthCalledWith(1, { phase: 'start', keys: [], metaKey: false })
  expect(changed).toHaveBeenLastCalledWith({ phase: 'change', keys: ['item-0', 'item-1'], metaKey: false })
  expect(screen.getByTestId('marquee-selection')).toBeVisible()
  fireEvent.pointerUp(grid, { pointerId: 7, clientX: 12, clientY: 22 })
  expect(changed).toHaveBeenLastCalledWith({ phase: 'end', keys: ['item-0', 'item-1'], metaKey: false })
  expect(screen.queryByTestId('marquee-selection')).not.toBeInTheDocument()
})

it('does not start from an item wrapper or a non-primary button', () => {
  const changed = vi.fn()
  renderGrid(changed)
  fireEvent.pointerDown(screen.getByText('item-0'), { pointerId: 1, button: 0 })
  fireEvent.pointerDown(screen.getByRole('listbox', { name: 'files' }), { pointerId: 2, button: 2 })
  expect(changed).not.toHaveBeenCalled()
})

it('freezes Command at pointer-down and reports an empty end for a background click', () => {
  const changed = vi.fn()
  renderGrid(changed)
  const grid = screen.getByRole('listbox', { name: 'files' })
  installPointerSurface(grid)
  fireEvent.pointerDown(grid, { pointerId: 3, button: 0, metaKey: true, clientX: 200, clientY: 100 })
  fireEvent.pointerUp(grid, { pointerId: 3, metaKey: false, clientX: 200, clientY: 100 })
  expect(changed).toHaveBeenLastCalledWith({ phase: 'end', keys: [], metaKey: true })
})

it('emits cancel and cleans the overlay on Escape and pointercancel', () => {
  const changed = vi.fn()
  renderGrid(changed)
  const grid = screen.getByRole('listbox', { name: 'files' })
  installPointerSurface(grid)

  fireEvent.pointerDown(grid, { pointerId: 4, button: 0, clientX: 122, clientY: 110 })
  fireEvent.pointerMove(grid, { pointerId: 4, clientX: 12, clientY: 22 })
  fireEvent.keyDown(grid, { key: 'Escape' })
  expect(changed).toHaveBeenLastCalledWith({ phase: 'cancel', keys: [], metaKey: false })
  expect(screen.queryByTestId('marquee-selection')).not.toBeInTheDocument()
  expect(grid).not.toHaveAttribute('data-marquee-active')

  changed.mockClear()
  fireEvent.pointerDown(grid, { pointerId: 5, button: 0, clientX: 122, clientY: 110 })
  fireEvent.pointerMove(grid, { pointerId: 5, clientX: 12, clientY: 22 })
  fireEvent.pointerCancel(grid, { pointerId: 5 })
  expect(changed).toHaveBeenLastCalledWith({ phase: 'cancel', keys: [], metaKey: false })
  expect(screen.queryByTestId('marquee-selection')).not.toBeInTheDocument()
  expect(grid).not.toHaveAttribute('data-marquee-active')
})

it('auto-scrolls at the edge, recomputes hits, and stops after pointerup', () => {
  let frame: FrameRequestCallback | null = null
  const requestFrame = vi.fn((callback: FrameRequestCallback) => {
    frame = callback
    return requestFrame.mock.calls.length
  })
  const cancelFrame = vi.fn()
  vi.stubGlobal('requestAnimationFrame', requestFrame)
  vi.stubGlobal('cancelAnimationFrame', cancelFrame)
  const changed = vi.fn()
  renderGrid(changed)
  const grid = screen.getByRole('listbox', { name: 'files' })
  installPointerSurface(grid)
  Object.defineProperty(grid, 'scrollHeight', { value: 600, configurable: true })
  Object.defineProperty(grid, 'clientHeight', { value: 200, configurable: true })

  fireEvent.pointerDown(grid, { pointerId: 6, button: 0, clientX: 200, clientY: 180 })
  fireEvent.pointerMove(grid, { pointerId: 6, clientX: 200, clientY: 220 })
  expect(requestFrame).toHaveBeenCalledTimes(1)
  const firstFrame = frame
  act(() => firstFrame?.(0))
  expect(grid.scrollTop).toBeGreaterThan(0)
  expect(grid.scrollTop).toBeLessThanOrEqual(18)
  expect(changed).toHaveBeenLastCalledWith(expect.objectContaining({ phase: 'change' }))

  const staleFrame = frame
  fireEvent.pointerUp(grid, { pointerId: 6, clientX: 200, clientY: 220 })
  const requestCountAtEnd = requestFrame.mock.calls.length
  act(() => staleFrame?.(16))
  expect(requestFrame).toHaveBeenCalledTimes(requestCountAtEnd)
  expect(cancelFrame).toHaveBeenCalled()
})
```

- [ ] **Step 2: Run the component test and verify RED**

Run:

```bash
pnpm --dir ui test -- src/components/VirtualGrid.test.tsx
```

Expected: FAIL because the new props, overlay and pointer lifecycle do not exist.

- [ ] **Step 3: Add the low-level session to VirtualGrid**

Implement the exported types and optional props. Add `data-virtual-grid-item` to each absolute item wrapper. Treat a pointer-down as background only when:

```ts
event.button === 0 &&
onMarqueeSelectionChange !== undefined &&
!(event.target as Element).closest('[data-virtual-grid-item]')
```

Store this low-level session in a ref so pointer movement and animation frames never depend on delayed React state:

```ts
interface MarqueeSession {
  pointerId: number
  start: MarqueePoint
  current: MarqueePoint
  lastClientX: number
  lastClientY: number
  metaKey: boolean
  activated: boolean
  keys: string[]
}
```

Convert client coordinates into content coordinates from the same container that owns layout:

```ts
function contentPoint(clientX: number, clientY: number): MarqueePoint {
  const node = container.current!
  const bounds = node.getBoundingClientRect()
  return {
    x: clientX - bounds.left + node.scrollLeft,
    y: clientY - bounds.top + node.scrollTop,
  }
}
```

At pointer-down: focus the grid, freeze `event.metaKey`, save the session, optionally call `setPointerCapture`, emit `{ phase: 'start', keys: [], metaKey }`, and prevent default. At pointer-move: ignore another pointer ID; wait until `marqueeDistance >= 4`; normalize the rectangle; derive indexes with the exact current `columns`, `cellWidth`, `cellHeight`, `columnStride`, `rowStride`; map indexes through `getKey(items[index]!)`; save keys; render the rectangle; emit `change`; prevent default.

At pointer-up: emit `end` with the saved keys (empty for a background click), release capture if available, cancel the animation frame, clear the ref and overlay. At pointercancel or Escape: emit `cancel`, perform the same cleanup and do not emit `end`. An effect keyed by `items` and an unmount cleanup must silently clear the low-level session and animation frame so no stale keys are published after workspace replacement.

Render the overlay inside the grid's tall relative content element:

```tsx
{marqueeRect && (
  <div
    aria-hidden="true"
    className="virtual-grid-marquee"
    data-testid="marquee-selection"
    style={{
      left: marqueeRect.left,
      top: marqueeRect.top,
      width: marqueeRect.width,
      height: marqueeRect.height,
    }}
  />
)}
```

Set `aria-multiselectable={ariaMultiselectable || undefined}` and `data-marquee-active={marqueeRect ? 'true' : undefined}` on the listbox.

For auto-scroll, keep one animation frame ID in a ref. While an activated session exists, calculate `verticalAutoScrollDelta(lastClientY, bounds.top, bounds.bottom)`, assign a clamped `node.scrollTop`, synchronize component `scrollTop`, recompute the current content point/rectangle/keys after movement, emit `change`, and queue another frame only when delta is nonzero and scrolling can continue. Stop immediately on center movement, pointer-up, cancellation, workspace replacement and unmount.

- [ ] **Step 4: Run Task 1 and Task 2 tests and verify GREEN**

Run:

```bash
pnpm --dir ui test -- src/components/marqueeSelection.test.ts src/components/VirtualGrid.test.tsx
```

Expected: all pure and component tests PASS with no unhandled timer/`act` warnings.

- [ ] **Step 5: Commit Task 2**

```bash
git add ui/src/components/VirtualGrid.tsx ui/src/components/VirtualGrid.test.tsx
git commit -m "feat: add virtual-grid marquee sessions"
```

### Task 3: Viewer selection semantics, visual styling and drag regressions

**Files:**
- Modify: `ui/src/components/ContentBrowser.tsx`
- Modify: `ui/src/components/ContentBrowser.test.tsx`
- Modify: `ui/src/styles/app.css`

**Interfaces:**
- Consumes: Task 2's `MarqueeSelectionChange` and `VirtualGrid.onMarqueeSelectionChange`.
- Produces: visible image-only marquee selection through the existing ordered `commitSelection(Set<string>)` data path; Finder export and organization drag APIs remain byte-for-byte unchanged.

- [ ] **Step 1: Write failing Viewer-level selection and ownership tests**

Add these helpers to `ContentBrowser.test.tsx` so every Viewer-level test uses deterministic image-grid bounds and one fixed pointer session:

```ts
function selectedLabels(): string[] {
  return screen
    .getAllByRole('option')
    .filter((item) => item.getAttribute('aria-selected') === 'true')
    .map((item) => item.getAttribute('aria-label')!)
}

function imageGrid(): HTMLElement {
  const grid = screen.getByRole('listbox', { name: '图片文件' })
  vi.spyOn(grid, 'getBoundingClientRect').mockReturnValue({
    left: 0, top: 0, right: 900, bottom: 520,
    width: 900, height: 520, x: 0, y: 0, toJSON: () => undefined,
  })
  Object.defineProperty(grid, 'setPointerCapture', { value: vi.fn(), configurable: true })
  Object.defineProperty(grid, 'releasePointerCapture', { value: vi.fn(), configurable: true })
  return grid
}

function marqueeImages({
  start,
  end,
  metaKey = false,
}: {
  start: [number, number]
  end: [number, number]
  metaKey?: boolean
}) {
  const grid = imageGrid()
  fireEvent.pointerDown(grid, {
    pointerId: 41, button: 0, metaKey, clientX: start[0], clientY: start[1],
  })
  fireEvent.pointerMove(grid, {
    pointerId: 41, metaKey: false, clientX: end[0], clientY: end[1],
  })
}

function finishMarquee(end: [number, number]) {
  fireEvent.pointerUp(screen.getByRole('listbox', { name: '图片文件' }), {
    pointerId: 41, clientX: end[0], clientY: end[1],
  })
}
```

Then add these focused tests:

```ts
it('replaces image selection live with a visible background marquee', () => {
  render(<ContentBrowser workspace={workspace(4)} />)
  fireEvent.click(screen.getByRole('option', { name: '4.jpg' }))
  marqueeImages({ start: [378, 100], end: [0, 0] })
  expect(selectedLabels()).toEqual(['1.jpg', '2.jpg'])
  expect(screen.getByTestId('marquee-selection')).toBeVisible()
  finishMarquee([0, 0])
  expect(screen.queryByTestId('marquee-selection')).not.toBeInTheDocument()
})

it('toggles marquee hits against the frozen Command selection snapshot', () => {
  render(<ContentBrowser workspace={workspace(4)} />)
  fireEvent.click(screen.getByRole('option', { name: '1.jpg' }))
  fireEvent.click(screen.getByRole('option', { name: '3.jpg' }), { metaKey: true })
  marqueeImages({ start: [378, 100], end: [0, 0], metaKey: true })
  finishMarquee([0, 0])
  expect(selectedLabels()).toEqual(['2.jpg', '3.jpg'])
})

it('clears on a background click and restores the start snapshot on Escape', () => {
  render(<ContentBrowser workspace={workspace(4)} />)
  fireEvent.click(screen.getByRole('option', { name: '1.jpg' }))
  const grid = imageGrid()
  fireEvent.pointerDown(grid, { pointerId: 41, button: 0, clientX: 500, clientY: 300 })
  fireEvent.pointerUp(grid, { pointerId: 41, clientX: 500, clientY: 300 })
  expect(selectedLabels()).toEqual([])

  fireEvent.click(screen.getByRole('option', { name: '1.jpg' }))
  marqueeImages({ start: [378, 100], end: [0, 0] })
  expect(selectedLabels()).toEqual(['1.jpg', '2.jpg'])
  fireEvent.keyDown(screen.getByRole('listbox', { name: '图片文件' }), { key: 'Escape' })
  expect(selectedLabels()).toEqual(['1.jpg'])
  expect(screen.queryByTestId('marquee-selection')).not.toBeInTheDocument()
})

it('keeps background marquee, card Finder export, and handle organization drag isolated', () => {
  const exportFiles = vi.fn()
  const organize = vi.fn()
  render(<ContentBrowser workspace={workspace(3)} onFinderDragStart={exportFiles} onOrganizationDragStart={organize} />)
  fireEvent.pointerDown(screen.getByRole('option', { name: '1.jpg' }), { pointerId: 9, button: 0 })
  fireEvent.dragStart(screen.getByRole('option', { name: '1.jpg' }))
  expect(exportFiles).toHaveBeenCalledTimes(1)
  fireEvent.pointerDown(screen.getByRole('button', { name: '整理 2.jpg' }), { pointerId: 10, button: 0 })
  fireEvent.dragStart(screen.getByRole('button', { name: '整理 2.jpg' }), {
    dataTransfer: { setData: vi.fn(), effectAllowed: 'none' },
  })
  expect(organize).toHaveBeenCalledTimes(1)
  expect(screen.queryByTestId('marquee-selection')).not.toBeInTheDocument()
})
```

Retain the existing Finder multi-selection test. Change its setup to use the new marquee helper so it proves a marquee-created ordered selection is frozen into `onFinderDragStart(['image-1', 'image-2'])`.

- [ ] **Step 2: Run the Viewer test and verify RED**

Run:

```bash
pnpm --dir ui test -- src/components/ContentBrowser.test.tsx
```

Expected: the new tests FAIL because `ContentBrowser` does not consume marquee lifecycle events and the overlay CSS is absent.

- [ ] **Step 3: Connect marquee lifecycle to the existing selection truth source**

Import `MarqueeSelectionChange`. Keep a session ref in `ContentBrowser`:

```ts
const marqueeSelection = useRef<{
  baseline: Set<string>
  metaKey: boolean
} | null>(null)
```

Implement one handler:

```ts
function updateMarqueeSelection(change: MarqueeSelectionChange) {
  if (change.phase === 'start') {
    marqueeSelection.current = { baseline: new Set(selected), metaKey: change.metaKey }
    return
  }
  const session = marqueeSelection.current
  if (session === null) return
  if (change.phase === 'cancel') {
    commitSelection(new Set(session.baseline))
    marqueeSelection.current = null
    return
  }
  const hits = new Set(change.keys)
  const next = session.metaKey ? new Set(session.baseline) : new Set<string>()
  if (session.metaKey) {
    for (const id of hits) {
      if (next.has(id)) next.delete(id)
      else next.add(id)
    }
  } else {
    for (const id of hits) next.add(id)
  }
  commitSelection(next)
  if (change.phase === 'end') {
    const first = workspace.images.find((file) => hits.has(file.entityId))
    setActiveId(first?.entityId ?? null)
    anchorId.current = first?.entityId ?? null
    marqueeSelection.current = null
  }
}
```

Pass this only to the image `VirtualGrid`:

```tsx
ariaMultiselectable
onMarqueeSelectionChange={updateMarqueeSelection}
```

Do not add it to `.text-file-list`. Clear `marqueeSelection.current` when the visible workspace collection changes; the existing selection-pruning effect remains the only publisher for removed entities.

- [ ] **Step 4: Add exact visual feedback**

Modify `ui/src/styles/app.css`:

```css
.virtual-grid[data-marquee-active="true"] {
  cursor: crosshair;
  user-select: none;
}

.virtual-grid-marquee {
  background: rgb(36 119 212 / 18%);
  border: 1px solid #2477d4;
  border-radius: 3px;
  box-sizing: border-box;
  pointer-events: none;
  position: absolute;
  z-index: 2;
}
```

The overlay must stay above item wrappers while `pointer-events: none` keeps input ownership with the grid. Do not add animation, shadow, preference or platform-specific color API.

- [ ] **Step 5: Verify focused and full UI suites**

Run:

```bash
pnpm --dir ui test -- src/components/marqueeSelection.test.ts src/components/VirtualGrid.test.tsx src/components/ContentBrowser.test.tsx src/App.test.tsx
pnpm --dir ui test
pnpm --dir ui build
```

Expected: all focused tests PASS; the complete UI suite passes; TypeScript and Vite production build pass without warnings or new dependencies.

- [ ] **Step 6: Commit Task 3**

```bash
git add ui/src/components/ContentBrowser.tsx ui/src/components/ContentBrowser.test.tsx ui/src/styles/app.css
git commit -m "feat: add image marquee selection"
```

### Task 4: M3 gate, package and resumed physical acceptance

**Files:**
- No source file changes unless a failing gate identifies a real regression; any fix requires a new red-green cycle and focused commit.
- Evidence remains pending in `docs/superpowers/plans/2026-07-16-viewer-m3-organization-comparison-plan.md` and `docs/reviews/2026-07-16-m3-organization-comparison-review.md` until all physical checks pass.

**Interfaces:**
- Consumes: Tasks 1–3 and the existing M3 gate/package scripts.
- Produces: a new packaged app in which marquee-created selection successfully exports two exact files to Finder, after which the original eight-item M3 physical checklist resumes.

- [ ] **Step 1: Run the exact M3 automated gate**

```bash
pnpm gate:m3
```

Expected final line: `M3 organization and comparison gate passed`.

- [ ] **Step 2: Build the exact Apple Silicon package**

```bash
pnpm build:macos
```

Expected artifacts:

```text
target/aarch64-apple-darwin/release/bundle/macos/Viewer.app
target/aarch64-apple-darwin/release/bundle/dmg/Viewer_0.1.0_aarch64.dmg
```

- [ ] **Step 3: Re-run the failed multi-image physical check**

Use a fresh empty destination. In the packaged app, begin on image-grid whitespace, draw the visible rectangle across the JPG and PNG, verify both cards become highlighted, then drag either selected card body to Finder. Pass only when both destination files exist and these source/destination SHA-256 pairs match:

```text
acceptance-photo.jpg  ffb89121baa0aafd524f82eb0ed6d0338e596f9a5e2b20d0749348f9d22dc2c7
acceptance-alpha.png   f514f2a5563166aaa73d0549d3708a4fdf66a140d97ac57edefb050371699f77
```

- [ ] **Step 4: Resume the remaining M3 physical checklist**

Continue with Markdown/TXT export, cancelled Finder drag, organization-handle move, Option-frozen copy, Finder-folder import, packaged-app relaunch and post-relaunch export exactly as Task 3 of `2026-07-20-viewer-m3-drag-session-rearchitecture-plan.md` specifies. Do not record visual-only passes.

- [ ] **Step 5: Hand back to the existing final review and merge gate**

Only after all physical checks pass, update truthful evidence, run the complete branch review, resolve every Critical/Important finding, commit the evidence, fast-forward locally to `main`, and rerun `pnpm gate:m3` on `main` as already specified by the parent M3 plan.

---

## Plan self-review

- Spec coverage: background-only ownership, 4 px threshold, live overlay/highlight, ordinary/Command semantics, active/anchor behavior, all-item virtual hit testing, vertical auto-scroll, cancellation cleanup, accessibility, text-list exclusion and physical hash evidence map to Tasks 1–4.
- Type consistency: `MarqueeSelectionChange`, its four phases and its ordered `keys`/frozen `metaKey` fields are defined once in Task 2 and consumed unchanged in Task 3.
- Architecture consistency: `VirtualGrid` remains the only layout truth source; `ContentBrowser` remains the only business selection owner; Finder/AppKit and internal HTML drag interfaces are unchanged.
- Regression coverage: item body and handle cannot start marquee; Finder export still freezes the current selection; existing click/Command/Shift/keyboard/text tests remain in the full UI gate.
- Performance: hit testing scans only candidate rows/columns and can address unmounted items; one animation frame loop exists only while edge scrolling can continue.
- Security/privacy: no path, IPC, capability, Rust, dependency or metadata change is introduced.
- Completeness scan: every implementation step names exact files, interfaces, commands, expected results and concrete assertions; no deferred or undefined work remains.
