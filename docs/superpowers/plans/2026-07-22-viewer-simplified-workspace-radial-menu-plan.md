# Viewer Simplified Workspace and Radial Menu Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Viewer’s persistent selection toolbars with an accessible right-click radial menu and compress the workspace chrome so project content becomes the dominant surface.

**Architecture:** Keep every file operation and controller API unchanged. Add pure menu-model and geometry modules, render them through one `RadialFileMenu` component, and let `App.tsx` translate semantic radial actions into existing preview, marker, compare, dialog, and task flows. Compact the existing React presentation layer without changing Rust, Tauri commands, persistence, or file-transaction boundaries.

**Tech Stack:** React 19, TypeScript 6, Vitest 4, Testing Library, CSS, Tauri 2 development shell.

## Global Constraints

- macOS 13+ and Apple Silicon remain the release target; do not add Windows implementation work.
- Do not add npm or Rust dependencies.
- Do not modify Rust crates, Tauri commands, file transaction protocols, `.viewer` metadata, or persistence schemas.
- Preserve every existing preview, search, marker, compare, rename, copy, move, Trash, undo, read-only, Finder export, and project-internal drag capability.
- Search results do not receive the radial menu in this change; only `ContentBrowser` image cards and text rows do.
- Disk mutations must continue through existing preflight, confirmation, conflict, task, and transaction flows.
- The six primary radial positions remain stable when actions are disabled.
- Use TDD for every task and commit after each independently passing deliverable.

## File Structure

### New files

- `ui/src/components/radialMenuModel.ts` — stable action identifiers, availability rules, labels, checked state, and submenu structure.
- `ui/src/components/radialMenuModel.test.ts` — single/multi/read-only/busy model matrix.
- `ui/src/components/radialMenuGeometry.ts` — annular SVG paths, angle hit testing, motion threshold, viewport fitting, and submenu rotation.
- `ui/src/components/radialMenuGeometry.test.ts` — deterministic geometry and edge-placement tests.
- `ui/src/components/RadialFileMenu.tsx` — pointer gesture, click fallback, keyboard navigation, ARIA menu semantics, and SVG rendering.
- `ui/src/components/RadialFileMenu.test.tsx` — pointer, keyboard, submenu, disabled action, and destructive handoff tests.
- `ui/src/state/useReviewShortcuts.ts` — headless ownership of `1/2/3/0/F` review shortcuts after the persistent marker toolbar is removed.
- `ui/src/state/useReviewShortcuts.test.tsx` — editable, modal, selected-text, read-only, and busy shortcut suppression tests.

### Modified files

- `ui/src/App.tsx` — radial session state, semantic action mapping, compact project header, sidebar separator, close conditions, and removal of persistent selection toolbars.
- `ui/src/App.test.tsx` — integrated radial action, read-only, project-menu, sidebar-resize, shortcut, and lifecycle coverage.
- `ui/src/components/ContentBrowser.tsx` — right-click request boundary, compact content bar, conditional marker badges, and progressive drag-handle visibility.
- `ui/src/components/ContentBrowser.test.tsx` — context selection, request snapshots, compact view menu, marker badges, Finder drag, and internal drag regression.
- `ui/src/components/SearchToolbar.tsx` — compact search plus combined filter/sort and view popovers.
- `ui/src/components/SearchToolbar.test.tsx` — compact popover behavior while preserving every existing search control.
- `ui/src/components/MarkerControls.tsx` — retain reusable `MarkerButtons` for comparison panes; delegate shortcut behavior to the new hook while the default toolbar leaves `App`.
- `ui/src/components/MarkerControls.test.tsx` — narrow to visible `MarkerButtons` behavior, with global shortcuts moved to the hook tests.
- `ui/src/components/TaskBar.tsx` — floating compact task capsule and automatic dismissal of clean successes.
- `ui/src/components/TaskBar.test.tsx` — timers, sticky failures/results, expansion, and cancellation.
- `ui/src/styles/app.css` — two-row workspace layout, radial SVG/buttons, popovers, separator, conditional badges/handles, and floating task capsule.

### Deleted files

- `ui/src/components/FileActionToolbar.tsx` — its actions move to the radial menu.
- `ui/src/components/FileActionToolbar.test.tsx` — its availability matrix moves to `radialMenuModel.test.ts` and App integration tests.

---

### Task 1: Define the stable radial menu model

**Files:**
- Create: `ui/src/components/radialMenuModel.ts`
- Create: `ui/src/components/radialMenuModel.test.ts`

**Interfaces:**
- Consumes: `ReviewState` from `ui/src/api/types.ts`.
- Produces: `RadialLeafAction`, `RadialMenuContext`, `RadialMenuItem`, and `buildRadialMenuModel(context): RadialMenuItem[]`.

- [ ] **Step 1: Write the failing menu-model tests**

```ts
import { describe, expect, it } from 'vitest'
import { buildRadialMenuModel } from './radialMenuModel'
import type { RadialMenuContext } from './radialMenuModel'

function context(overrides: Partial<RadialMenuContext> = {}): RadialMenuContext {
  return {
    selectedCount: 1,
    selectedImageCount: 1,
    readOnly: false,
    busy: false,
    compareContextAvailable: true,
    commonReview: null,
    commonFavorite: false,
    ...overrides,
  }
}

describe('buildRadialMenuModel', () => {
  it('keeps the six primary positions stable', () => {
    expect(buildRadialMenuModel(context()).map((item) => item.id)).toEqual([
      'preview',
      'mark',
      'organize',
      'trash',
      'compare',
      'info',
    ])
  })

  it('builds the confirmed local fan submenus', () => {
    const model = buildRadialMenuModel(context())
    expect(model[1]?.children?.map((item) => item.id)).toEqual([
      'mark.keep',
      'mark.pending',
      'mark.reject',
      'mark.clear',
      'mark.favorite',
    ])
    expect(model[2]?.children?.map((item) => item.id)).toEqual([
      'organize.rename',
      'organize.copy',
      'organize.move',
    ])
  })

  it('gates preview and compare by selection shape without moving them', () => {
    const single = buildRadialMenuModel(context())
    expect(single[0]).toMatchObject({ id: 'preview', disabled: false })
    expect(single[4]).toMatchObject({ id: 'compare', disabled: true })

    const threeImages = buildRadialMenuModel(
      context({ selectedCount: 3, selectedImageCount: 3 }),
    )
    expect(threeImages[0]).toMatchObject({ id: 'preview', disabled: true })
    expect(threeImages[4]).toMatchObject({ id: 'compare', disabled: false })

    const mixed = buildRadialMenuModel(
      context({ selectedCount: 3, selectedImageCount: 2 }),
    )
    expect(mixed[4]).toMatchObject({ id: 'compare', disabled: true })
  })

  it('disables writes in read-only and all competing actions while busy', () => {
    const readOnly = buildRadialMenuModel(context({ readOnly: true }))
    expect(readOnly[1]).toMatchObject({ disabled: true, disabledReason: '只读项目不可标记' })
    expect(readOnly[2]).toMatchObject({ disabled: true, disabledReason: '只读项目不可整理' })
    expect(readOnly[3]).toMatchObject({ disabled: true, disabledReason: '只读项目不可删除' })
    expect(readOnly[5]).toMatchObject({ disabled: false })

    const busy = buildRadialMenuModel(
      context({ selectedCount: 3, selectedImageCount: 3, busy: true }),
    )
    expect(
      busy
        .filter((item) => ['mark', 'organize', 'trash', 'compare'].includes(item.id))
        .every((item) => item.disabled),
    ).toBe(true)
    expect(busy[0]).toMatchObject({ disabled: false })
    expect(busy[5]).toMatchObject({ disabled: false })
  })

  it('reflects common marker state and batch rename copy', () => {
    const model = buildRadialMenuModel(
      context({
        selectedCount: 2,
        selectedImageCount: 2,
        commonReview: 'pending',
        commonFavorite: true,
      }),
    )
    expect(model[1]?.children?.find((item) => item.id === 'mark.pending')).toMatchObject({
      checked: true,
    })
    expect(model[1]?.children?.find((item) => item.id === 'mark.favorite')).toMatchObject({
      label: '取消收藏',
      checked: true,
    })
    expect(model[2]?.children?.[0]).toMatchObject({ label: '批量重命名' })
  })
})
```

- [ ] **Step 2: Run the model test and verify it fails**

Run: `pnpm --dir ui exec vitest run src/components/radialMenuModel.test.ts`

Expected: FAIL because `./radialMenuModel` does not exist.

- [ ] **Step 3: Implement the complete pure menu model**

```ts
import type { ReviewState } from '../api/types'

export type RadialLeafAction =
  | 'preview'
  | 'mark.keep'
  | 'mark.pending'
  | 'mark.reject'
  | 'mark.clear'
  | 'mark.favorite'
  | 'organize.rename'
  | 'organize.copy'
  | 'organize.move'
  | 'trash'
  | 'compare'
  | 'info'

export type RadialPrimaryId =
  | 'preview'
  | 'mark'
  | 'organize'
  | 'trash'
  | 'compare'
  | 'info'

export interface RadialMenuContext {
  selectedCount: number
  selectedImageCount: number
  readOnly: boolean
  busy: boolean
  compareContextAvailable: boolean
  commonReview: ReviewState | null | 'mixed'
  commonFavorite: boolean | 'mixed'
}

export interface RadialMenuItem {
  id: RadialPrimaryId | RadialLeafAction
  label: string
  symbol: string
  disabled: boolean
  disabledReason?: string
  tone?: 'normal' | 'destructive'
  checked?: boolean | 'mixed'
  children?: RadialMenuItem[]
}

export function buildRadialMenuModel(context: RadialMenuContext): RadialMenuItem[] {
  const noSelection = context.selectedCount === 0
  const writesDisabled = noSelection || context.readOnly || context.busy
  const compareDisabled =
    context.busy ||
    !context.compareContextAvailable ||
    context.selectedCount < 2 ||
    context.selectedCount > 4 ||
    context.selectedImageCount !== context.selectedCount
  const writeReason = (action: '标记' | '整理' | '删除') =>
    noSelection
      ? '未选择文件'
      : context.readOnly
        ? `只读项目不可${action}`
        : context.busy
          ? '请等待当前文件操作完成'
          : undefined

  const markChildren: RadialMenuItem[] = [
    markerItem('mark.keep', '保留', '✓', context.commonReview === 'keep', writesDisabled),
    markerItem('mark.pending', '待定', '•', context.commonReview === 'pending', writesDisabled),
    markerItem('mark.reject', '淘汰', '×', context.commonReview === 'reject', writesDisabled),
    markerItem('mark.clear', '清除', '○', context.commonReview === null, writesDisabled),
    {
      id: 'mark.favorite',
      label: context.commonFavorite === true ? '取消收藏' : '收藏',
      symbol: '★',
      disabled: writesDisabled,
      checked: context.commonFavorite,
    },
  ]

  const organizeChildren: RadialMenuItem[] = [
    {
      id: 'organize.rename',
      label: context.selectedCount > 1 ? '批量重命名' : '重命名',
      symbol: '✎',
      disabled: writesDisabled,
    },
    { id: 'organize.copy', label: '复制到', symbol: '⧉', disabled: writesDisabled },
    { id: 'organize.move', label: '移动到', symbol: '→', disabled: writesDisabled },
  ]

  return [
    {
      id: 'preview',
      label: '预览',
      symbol: '◉',
      disabled: context.selectedCount !== 1,
      disabledReason: context.selectedCount !== 1 ? '预览仅适用于单个文件' : undefined,
    },
    {
      id: 'mark',
      label: '标记',
      symbol: '★',
      disabled: writesDisabled,
      disabledReason: writeReason('标记'),
      children: markChildren,
    },
    {
      id: 'organize',
      label: '整理',
      symbol: '⇄',
      disabled: writesDisabled,
      disabledReason: writeReason('整理'),
      children: organizeChildren,
    },
    {
      id: 'trash',
      label: '移到废纸篓',
      symbol: '⌫',
      disabled: writesDisabled,
      disabledReason: writeReason('删除'),
      tone: 'destructive',
    },
    {
      id: 'compare',
      label: '并排对比',
      symbol: '▣',
      disabled: compareDisabled,
      disabledReason: context.busy ? '请等待当前文件操作完成' : '请选择 2–4 张图片',
    },
    {
      id: 'info',
      label: '信息',
      symbol: 'ⓘ',
      disabled: noSelection,
      disabledReason: noSelection ? '未选择文件' : undefined,
    },
  ]
}

function markerItem(
  id: RadialLeafAction,
  label: string,
  symbol: string,
  checked: boolean,
  disabled: boolean,
): RadialMenuItem {
  return { id, label, symbol, checked, disabled }
}
```

- [ ] **Step 4: Run the model tests and fix type-only assertion syntax if Vitest reports it**

Run: `pnpm --dir ui exec vitest run src/components/radialMenuModel.test.ts`

Expected: PASS with 5 tests.

- [ ] **Step 5: Commit the model**

```bash
git add ui/src/components/radialMenuModel.ts ui/src/components/radialMenuModel.test.ts
git commit -m "feat(ui): define radial file menu model"
```

### Task 2: Implement radial geometry and viewport placement

**Files:**
- Create: `ui/src/components/radialMenuGeometry.ts`
- Create: `ui/src/components/radialMenuGeometry.test.ts`

**Interfaces:**
- Consumes: primary index and submenu child count from `RadialMenuItem`.
- Produces: `Point`, `Viewport`, constants, `annularSectorPath`, `primaryIndexAt`, `secondaryIndexAt`, `primaryCenterAngle`, `fitMenuOrigin`, and `chooseSecondaryAnchor`.

- [ ] **Step 1: Write failing deterministic geometry tests**

```ts
import { describe, expect, it } from 'vitest'
import {
  MOTION_THRESHOLD,
  annularSectorPath,
  chooseSecondaryAnchor,
  fitMenuOrigin,
  primaryCenterAngle,
  primaryIndexAt,
  secondaryIndexAt,
} from './radialMenuGeometry'

describe('radial menu geometry', () => {
  it('maps the confirmed six primary directions', () => {
    const origin = { x: 200, y: 200 }
    expect(primaryIndexAt({ x: 200, y: 110 }, origin)).toBe(0)
    expect(primaryIndexAt({ x: 280, y: 154 }, origin)).toBe(1)
    expect(primaryIndexAt({ x: 280, y: 246 }, origin)).toBe(2)
    expect(primaryIndexAt({ x: 200, y: 290 }, origin)).toBe(3)
    expect(primaryIndexAt({ x: 120, y: 246 }, origin)).toBe(4)
    expect(primaryIndexAt({ x: 120, y: 154 }, origin)).toBe(5)
    expect(primaryIndexAt(origin, origin)).toBeNull()
  })

  it('maps a five-item local fan around the selected direction', () => {
    const origin = { x: 200, y: 200 }
    expect(secondaryIndexAt({ x: 200, y: 28 }, origin, -30, 5)).toBe(0)
    expect(secondaryIndexAt({ x: 349, y: 114 }, origin, -30, 5)).toBe(2)
    expect(secondaryIndexAt({ x: 349, y: 286 }, origin, -30, 5)).toBe(4)
  })

  it('creates a closed annular sector path', () => {
    expect(annularSectorPath({ x: 0, y: 0 }, 40, 80, -30, 30)).toMatch(
      /^M .* A 80 80 .* L .* A 40 40 .* Z$/,
    )
  })

  it('fits the full expanded menu inside the viewport', () => {
    expect(fitMenuOrigin({ x: 10, y: 790 }, { width: 1280, height: 800 })).toEqual({
      x: 180,
      y: 620,
    })
  })

  it('keeps the local direction when it fits and rotates toward free space at an edge', () => {
    const viewport = { width: 1280, height: 800 }
    expect(chooseSecondaryAnchor(-30, { x: 640, y: 400 }, viewport)).toBe(-30)
    expect(chooseSecondaryAnchor(-30, { x: 1210, y: 400 }, viewport)).toBe(180)
    expect(chooseSecondaryAnchor(30, { x: 640, y: 760 }, viewport)).toBe(-90)
    expect(primaryCenterAngle(1)).toBe(-30)
    expect(MOTION_THRESHOLD).toBe(12)
  })
})
```

- [ ] **Step 2: Run geometry tests and verify the module is missing**

Run: `pnpm --dir ui exec vitest run src/components/radialMenuGeometry.test.ts`

Expected: FAIL because `./radialMenuGeometry` does not exist.

- [ ] **Step 3: Implement the geometry module**

```ts
export interface Point {
  x: number
  y: number
}

export interface Viewport {
  width: number
  height: number
}

export const PRIMARY_INNER_RADIUS = 42
export const PRIMARY_OUTER_RADIUS = 108
export const SECONDARY_INNER_RADIUS = 112
export const SECONDARY_OUTER_RADIUS = 168
export const MOTION_THRESHOLD = 12
export const PRIMARY_SECTOR_DEGREES = 60
export const SECONDARY_SECTOR_DEGREES = 30

export function primaryCenterAngle(index: number): number {
  return -90 + index * PRIMARY_SECTOR_DEGREES
}

export function polarPoint(origin: Point, radius: number, degrees: number): Point {
  const radians = (degrees * Math.PI) / 180
  return {
    x: origin.x + radius * Math.cos(radians),
    y: origin.y + radius * Math.sin(radians),
  }
}

export function annularSectorPath(
  origin: Point,
  innerRadius: number,
  outerRadius: number,
  startDegrees: number,
  endDegrees: number,
): string {
  const outerStart = polarPoint(origin, outerRadius, startDegrees)
  const outerEnd = polarPoint(origin, outerRadius, endDegrees)
  const innerEnd = polarPoint(origin, innerRadius, endDegrees)
  const innerStart = polarPoint(origin, innerRadius, startDegrees)
  const largeArc = endDegrees - startDegrees > 180 ? 1 : 0
  return [
    `M ${format(outerStart.x)} ${format(outerStart.y)}`,
    `A ${outerRadius} ${outerRadius} 0 ${largeArc} 1 ${format(outerEnd.x)} ${format(outerEnd.y)}`,
    `L ${format(innerEnd.x)} ${format(innerEnd.y)}`,
    `A ${innerRadius} ${innerRadius} 0 ${largeArc} 0 ${format(innerStart.x)} ${format(innerStart.y)}`,
    'Z',
  ].join(' ')
}

export function primaryIndexAt(point: Point, origin: Point): number | null {
  const radius = distance(point, origin)
  if (radius < PRIMARY_INNER_RADIUS || radius > PRIMARY_OUTER_RADIUS) return null
  const degrees = normalizeDegrees(angle(point, origin) - -120)
  return Math.floor(degrees / PRIMARY_SECTOR_DEGREES) % 6
}

export function secondaryIndexAt(
  point: Point,
  origin: Point,
  anchorDegrees: number,
  itemCount: number,
): number | null {
  const radius = distance(point, origin)
  if (radius < SECONDARY_INNER_RADIUS || radius > SECONDARY_OUTER_RADIUS) return null
  const start = anchorDegrees - (itemCount * SECONDARY_SECTOR_DEGREES) / 2
  const relative = normalizeDegrees(angle(point, origin) - start)
  const span = itemCount * SECONDARY_SECTOR_DEGREES
  if (relative >= span) return null
  return Math.floor(relative / SECONDARY_SECTOR_DEGREES)
}

export function fitMenuOrigin(point: Point, viewport: Viewport): Point {
  const margin = SECONDARY_OUTER_RADIUS + 12
  return {
    x: clamp(point.x, margin, viewport.width - margin),
    y: clamp(point.y, margin, viewport.height - margin),
  }
}

export function chooseSecondaryAnchor(
  preferredDegrees: number,
  origin: Point,
  viewport: Viewport,
): number {
  const required = SECONDARY_OUTER_RADIUS + 12
  if (directionalSpace(preferredDegrees, origin, viewport) >= required) {
    return preferredDegrees
  }
  return [0, 90, 180, -90].reduce((best, candidate) =>
    directionalSpace(candidate, origin, viewport) > directionalSpace(best, origin, viewport)
      ? candidate
      : best,
  )
}

function angle(point: Point, origin: Point): number {
  return (Math.atan2(point.y - origin.y, point.x - origin.x) * 180) / Math.PI
}

function distance(point: Point, origin: Point): number {
  return Math.hypot(point.x - origin.x, point.y - origin.y)
}

function normalizeDegrees(value: number): number {
  return ((value % 360) + 360) % 360
}

function directionalSpace(degrees: number, origin: Point, viewport: Viewport): number {
  const radians = (degrees * Math.PI) / 180
  const dx = Math.cos(radians)
  const dy = Math.sin(radians)
  const horizontal = dx > 0 ? (viewport.width - origin.x) / dx : dx < 0 ? -origin.x / dx : Infinity
  const vertical = dy > 0 ? (viewport.height - origin.y) / dy : dy < 0 ? -origin.y / dy : Infinity
  return Math.min(horizontal, vertical)
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.max(minimum, Math.min(maximum, value))
}

function format(value: number): string {
  return Number(value.toFixed(2)).toString()
}
```

- [ ] **Step 4: Run the geometry test**

Run: `pnpm --dir ui exec vitest run src/components/radialMenuGeometry.test.ts`

Expected: PASS with 5 tests.

- [ ] **Step 5: Commit geometry**

```bash
git add ui/src/components/radialMenuGeometry.ts ui/src/components/radialMenuGeometry.test.ts
git commit -m "feat(ui): add radial menu geometry"
```

### Task 3: Render an accessible pointer-and-keyboard radial menu

**Files:**
- Create: `ui/src/components/RadialFileMenu.tsx`
- Create: `ui/src/components/RadialFileMenu.test.tsx`
- Modify: `ui/src/styles/app.css`

**Interfaces:**
- Consumes: `RadialMenuItem[]`, geometry functions, and semantic `RadialLeafAction` identifiers from Tasks 1–2.
- Produces: `RadialMenuRequest` and `RadialFileMenu` props used by `ContentBrowser` and `App`.

- [ ] **Step 1: Write failing component tests for the real interaction contract**

```tsx
import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import RadialFileMenu from './RadialFileMenu'
import { buildRadialMenuModel } from './radialMenuModel'

const model = buildRadialMenuModel({
  selectedCount: 1,
  selectedImageCount: 1,
  readOnly: false,
  busy: false,
  compareContextAvailable: true,
  commonReview: null,
  commonFavorite: false,
})

describe('RadialFileMenu', () => {
  it('exposes six stable primary menuitems and a selection-count center', () => {
    render(
      <RadialFileMenu
        origin={{ x: 320, y: 240 }}
        pointerId={null}
        selectionCount={1}
        model={model}
        onAction={vi.fn()}
        onClose={vi.fn()}
      />,
    )
    expect(screen.getByRole('menu', { name: '文件操作' })).toBeVisible()
    expect(screen.getAllByRole('menuitem')).toHaveLength(6)
    expect(screen.getByText('1 个文件')).toBeVisible()
    expect(screen.getByRole('menuitem', { name: '并排对比' })).toHaveAttribute('aria-disabled', 'true')
  })

  it('opens the local marker fan and executes a leaf', () => {
    const action = vi.fn()
    render(
      <RadialFileMenu
        origin={{ x: 320, y: 240 }}
        pointerId={null}
        selectionCount={1}
        model={model}
        onAction={action}
        onClose={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByRole('menuitem', { name: '标记' }))
    expect(screen.getByRole('menuitemcheckbox', { name: '保留' })).toBeVisible()
    fireEvent.click(screen.getByRole('menuitemcheckbox', { name: '保留' }))
    expect(action).toHaveBeenCalledWith('mark.keep')
  })

  it('uses ArrowRight/ArrowLeft and ArrowUp/ArrowDown for radial keyboard navigation', () => {
    const action = vi.fn()
    render(
      <RadialFileMenu
        origin={{ x: 320, y: 240 }}
        pointerId={null}
        selectionCount={1}
        model={model}
        onAction={action}
        onClose={vi.fn()}
      />,
    )
    const menu = screen.getByRole('menu', { name: '文件操作' })
    fireEvent.keyDown(menu, { key: 'ArrowRight' })
    expect(screen.getByRole('menuitem', { name: '标记' })).toHaveFocus()
    fireEvent.keyDown(menu, { key: 'ArrowUp' })
    expect(screen.getByRole('menuitemcheckbox', { name: '保留' })).toHaveFocus()
    fireEvent.keyDown(menu, { key: 'ArrowDown' })
    expect(screen.getByRole('menuitem', { name: '标记' })).toHaveFocus()
  })

  it('does not execute a short right-button release and closes on Escape', () => {
    const action = vi.fn()
    const close = vi.fn()
    render(
      <RadialFileMenu
        origin={{ x: 320, y: 240 }}
        pointerId={7}
        selectionCount={1}
        model={model}
        onAction={action}
        onClose={close}
      />,
    )
    fireEvent.pointerUp(window, { pointerId: 7, clientX: 324, clientY: 243 })
    expect(action).not.toHaveBeenCalled()
    fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' })
    expect(close).toHaveBeenCalledOnce()
  })

  it('hands Trash to the confirmation owner instead of mutating directly', () => {
    const action = vi.fn()
    render(
      <RadialFileMenu
        origin={{ x: 320, y: 240 }}
        pointerId={null}
        selectionCount={1}
        model={model}
        onAction={action}
        onClose={vi.fn()}
      />,
    )
    fireEvent.click(screen.getByRole('menuitem', { name: '移到废纸篓' }))
    expect(action).toHaveBeenCalledWith('trash')
  })
})
```

- [ ] **Step 2: Run the component tests and verify the component is missing**

Run: `pnpm --dir ui exec vitest run src/components/RadialFileMenu.test.tsx`

Expected: FAIL because `./RadialFileMenu` does not exist.

- [ ] **Step 3: Implement `RadialFileMenu.tsx` with the exact public API**

Use these exported request and prop types:

```tsx
import { useEffect, useMemo, useRef, useState } from 'react'
import type { CSSProperties, KeyboardEvent } from 'react'
import type { BrowserFile } from '../api/types'
import {
  MOTION_THRESHOLD,
  PRIMARY_INNER_RADIUS,
  PRIMARY_OUTER_RADIUS,
  SECONDARY_INNER_RADIUS,
  SECONDARY_OUTER_RADIUS,
  annularSectorPath,
  chooseSecondaryAnchor,
  fitMenuOrigin,
  polarPoint,
  primaryCenterAngle,
  primaryIndexAt,
  secondaryIndexAt,
} from './radialMenuGeometry'
import type { Point, Viewport } from './radialMenuGeometry'
import type { RadialLeafAction, RadialMenuItem } from './radialMenuModel'

export interface RadialMenuRequest {
  files: BrowserFile[]
  origin: Point
  pointerId: number | null
}

interface RadialFileMenuProps {
  origin: Point
  pointerId: number | null
  selectionCount: number
  model: RadialMenuItem[]
  viewport?: Viewport
  onAction: (action: RadialLeafAction) => void
  onClose: () => void
}

export default function RadialFileMenu({
  origin,
  pointerId,
  selectionCount,
  model,
  viewport = { width: window.innerWidth, height: window.innerHeight },
  onAction,
  onClose,
}: RadialFileMenuProps) {
  const fittedOrigin = useMemo(() => fitMenuOrigin(origin, viewport), [origin, viewport])
  const firstEnabledIndex = Math.max(0, model.findIndex((item) => !item.disabled))
  const [primaryIndex, setPrimaryIndex] = useState(firstEnabledIndex)
  const [pointerPrimaryIndex, setPointerPrimaryIndex] = useState<number | null>(null)
  const [expandedIndex, setExpandedIndex] = useState<number | null>(null)
  const [secondaryIndex, setSecondaryIndex] = useState<number | null>(null)
  const [clickMode, setClickMode] = useState(pointerId === null)
  const expandTimer = useRef<number | null>(null)
  const closeTimer = useRef<number | null>(null)
  const rootRef = useRef<HTMLDivElement>(null)
  const startPoint = useRef(origin)
  const previousFocus = useRef(document.activeElement as HTMLElement | null)
  const expandedItem = expandedIndex === null ? null : model[expandedIndex] ?? null
  const secondaryAnchor =
    expandedIndex === null
      ? 0
      : chooseSecondaryAnchor(primaryCenterAngle(expandedIndex), fittedOrigin, viewport)
  const displayedPrimaryIndex = clickMode ? primaryIndex : pointerPrimaryIndex

  useEffect(() => {
    rootRef.current
      ?.querySelector<HTMLButtonElement>('[data-level="primary"]:not([aria-disabled="true"])')
      ?.focus()
    return () => {
      if (expandTimer.current !== null) window.clearTimeout(expandTimer.current)
      if (closeTimer.current !== null) window.clearTimeout(closeTimer.current)
      previousFocus.current?.focus()
    }
  }, [])

  useEffect(() => {
    if (pointerId === null || clickMode) return
    const move = (event: PointerEvent) => {
      if (event.pointerId !== pointerId) return
      const point = { x: event.clientX, y: event.clientY }
      const child =
        expandedItem?.children === undefined
          ? null
          : secondaryIndexAt(
              point,
              fittedOrigin,
              secondaryAnchor,
              expandedItem.children.length,
            )
      if (child !== null) {
        setPointerPrimaryIndex(expandedIndex)
        setSecondaryIndex(child)
        return
      }
      setSecondaryIndex(null)
      const nextPrimary = primaryIndexAt(point, fittedOrigin)
      setPointerPrimaryIndex(nextPrimary)
      if (nextPrimary === null) return
      setPrimaryIndex(nextPrimary)
      scheduleExpansion(nextPrimary)
    }
    const up = (event: PointerEvent) => {
      if (event.pointerId !== pointerId) return
      const travelled = Math.hypot(
        event.clientX - startPoint.current.x,
        event.clientY - startPoint.current.y,
      )
      if (travelled < MOTION_THRESHOLD) {
        setClickMode(true)
        return
      }
      const child =
        secondaryIndex === null ? null : expandedItem?.children?.[secondaryIndex] ?? null
      if (child !== null && !child.disabled && isLeaf(child)) {
        onAction(child.id)
        return
      }
      const primary = pointerPrimaryIndex === null ? undefined : model[pointerPrimaryIndex]
      if (primary !== undefined && !primary.disabled && isLeaf(primary)) onAction(primary.id)
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', up)
    return () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', up)
    }
  }, [
    clickMode,
    expandedItem,
    fittedOrigin,
    model,
    onAction,
    pointerId,
    pointerPrimaryIndex,
    primaryIndex,
    secondaryAnchor,
    secondaryIndex,
  ])

  useEffect(() => {
    if (!clickMode) return
    const outside = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) onClose()
    }
    window.addEventListener('pointerdown', outside)
    return () => window.removeEventListener('pointerdown', outside)
  }, [clickMode, onClose])

  function scheduleExpansion(index: number) {
    if (expandTimer.current !== null) window.clearTimeout(expandTimer.current)
    const item = model[index]
    if (item?.disabled || item?.children === undefined) {
      setExpandedIndex(null)
      return
    }
    expandTimer.current = window.setTimeout(() => {
      setExpandedIndex(index)
      setSecondaryIndex(null)
    }, 120)
  }

  function expandImmediately(index: number) {
    const item = model[index]
    if (item?.disabled || item?.children === undefined) return
    setExpandedIndex(index)
    setSecondaryIndex(null)
    window.setTimeout(() => focusItem('secondary', 0), 0)
  }

  function execute(item: RadialMenuItem, index: number) {
    if (item.disabled) return
    if (item.children !== undefined) {
      expandImmediately(index)
      return
    }
    onAction(item.id as RadialLeafAction)
  }

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === 'Escape') {
      event.preventDefault()
      onClose()
      return
    }
    const level = expandedIndex !== null && document.activeElement?.getAttribute('data-level') === 'secondary'
      ? 'secondary'
      : 'primary'
    const items = level === 'primary' ? model : expandedItem?.children ?? []
    const current = level === 'primary' ? primaryIndex : secondaryIndex ?? 0
    if (event.key === 'ArrowRight' || event.key === 'ArrowLeft') {
      event.preventDefault()
      const delta = event.key === 'ArrowRight' ? 1 : -1
      const next = nextEnabled(items, current, delta)
      if (level === 'primary') setPrimaryIndex(next)
      else setSecondaryIndex(next)
      focusItem(level, next)
      return
    }
    if (event.key === 'ArrowUp' && level === 'primary') {
      event.preventDefault()
      expandImmediately(primaryIndex)
      return
    }
    if (event.key === 'ArrowDown' && level === 'secondary') {
      event.preventDefault()
      setSecondaryIndex(null)
      focusItem('primary', expandedIndex ?? primaryIndex)
      return
    }
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault()
      const item = items[current]
      if (item !== undefined) execute(item, current)
    }
  }

  function focusItem(level: 'primary' | 'secondary', index: number) {
    rootRef.current
      ?.querySelector<HTMLButtonElement>(`[data-level="${level}"][data-index="${index}"]`)
      ?.focus()
  }

  function scheduleClose() {
    if (!clickMode) return
    closeTimer.current = window.setTimeout(onClose, 250)
  }

  function cancelClose() {
    if (closeTimer.current !== null) window.clearTimeout(closeTimer.current)
  }

  return (
    <div
      ref={rootRef}
      className="radial-file-menu"
      role="menu"
      aria-label="文件操作"
      style={{
        '--radial-origin-x': `${fittedOrigin.x}px`,
        '--radial-origin-y': `${fittedOrigin.y}px`,
      } as CSSProperties}
      onKeyDown={handleKeyDown}
      onPointerEnter={cancelClose}
      onPointerLeave={scheduleClose}
    >
      <svg className="radial-file-menu-shapes" viewBox="0 0 336 336" aria-hidden="true">
        {expandedItem?.children?.map((item, index) => {
          const start = secondaryAnchor - (expandedItem.children!.length * 30) / 2 + index * 30
          return (
            <path
              key={item.id}
              className="radial-secondary-shape"
              data-active={secondaryIndex === index || undefined}
              d={annularSectorPath({ x: 168, y: 168 }, SECONDARY_INNER_RADIUS, SECONDARY_OUTER_RADIUS, start, start + 30)}
            />
          )
        })}
        {model.map((item, index) => (
          <path
            key={item.id}
            className="radial-primary-shape"
            data-active={displayedPrimaryIndex === index || undefined}
            data-disabled={item.disabled || undefined}
            d={annularSectorPath({ x: 168, y: 168 }, PRIMARY_INNER_RADIUS, PRIMARY_OUTER_RADIUS, -120 + index * 60, -60 + index * 60)}
          />
        ))}
      </svg>
      {model.map((item, index) => (
        <RadialButton
          key={item.id}
          item={item}
          level="primary"
          index={index}
          point={polarPoint({ x: 0, y: 0 }, 78, primaryCenterAngle(index))}
          onFocus={() => setPrimaryIndex(index)}
          onClick={() => execute(item, index)}
        />
      ))}
      {expandedItem?.children?.map((item, index) => {
        const start = secondaryAnchor - (expandedItem.children!.length * 30) / 2
        return (
          <RadialButton
            key={item.id}
            item={item}
            level="secondary"
            index={index}
            point={polarPoint({ x: 0, y: 0 }, 140, start + index * 30 + 15)}
            onFocus={() => setSecondaryIndex(index)}
            onClick={() => execute(item, index)}
          />
        )
      })}
      <button type="button" className="radial-menu-center" onClick={onClose} aria-label="关闭文件操作">
        <strong>{selectionCount} 个文件</strong>
        <span>回到中心取消</span>
      </button>
    </div>
  )
}

function RadialButton({
  item,
  level,
  index,
  point,
  onFocus,
  onClick,
}: {
  item: RadialMenuItem
  level: 'primary' | 'secondary'
  index: number
  point: Point
  onFocus: () => void
  onClick: () => void
}) {
  const checkbox = item.checked !== undefined
  return (
    <button
      type="button"
      role={checkbox ? 'menuitemcheckbox' : 'menuitem'}
      aria-checked={checkbox ? item.checked : undefined}
      aria-disabled={item.disabled}
      aria-haspopup={item.children === undefined ? undefined : 'menu'}
      title={item.disabled ? item.disabledReason : item.label}
      className="radial-menu-button"
      data-level={level}
      data-index={index}
      data-tone={item.tone}
      style={{ '--radial-x': `${point.x}px`, '--radial-y': `${point.y}px` } as CSSProperties}
      onFocus={onFocus}
      onClick={onClick}
    >
      <span aria-hidden="true">{item.symbol}</span>
      <span>{item.label}</span>
    </button>
  )
}

function isLeaf(item: RadialMenuItem): item is RadialMenuItem & { id: RadialLeafAction } {
  return item.children === undefined
}

function nextEnabled(items: RadialMenuItem[], current: number, delta: number): number {
  for (let offset = 1; offset <= items.length; offset += 1) {
    const index = (current + delta * offset + items.length) % items.length
    if (!items[index]?.disabled) return index
  }
  return current
}
```

- [ ] **Step 4: Add the component’s complete presentation block to `app.css`**

```css
.radial-file-menu {
  height: 336px;
  left: var(--radial-origin-x);
  position: fixed;
  top: var(--radial-origin-y);
  transform: translate(-168px, -168px);
  width: 336px;
  z-index: 25;
}

.radial-file-menu-shapes {
  filter: drop-shadow(0 18px 30px rgb(31 41 55 / 22%));
  inset: 0;
  overflow: visible;
  position: absolute;
}

.radial-primary-shape,
.radial-secondary-shape {
  fill: rgb(253 254 255 / 98%);
  stroke: #cbd3de;
  stroke-width: 1.5;
}

.radial-primary-shape[data-active="true"],
.radial-secondary-shape[data-active="true"] {
  fill: #dceaff;
  stroke: #4c8dde;
  stroke-width: 2;
}

.radial-primary-shape[data-disabled="true"] {
  fill: #f3f4f6;
  opacity: 0.62;
}

.radial-secondary-shape {
  fill: #e7f0ff;
  stroke: #8db9ef;
}

.radial-menu-button {
  align-items: center;
  background: transparent;
  border: 0;
  color: #3e4857;
  display: flex;
  flex-direction: column;
  font-size: 10px;
  font-weight: 650;
  height: 46px;
  justify-content: center;
  left: calc(168px + var(--radial-x));
  padding: 2px;
  position: absolute;
  top: calc(168px + var(--radial-y));
  transform: translate(-50%, -50%);
  width: 66px;
}

.radial-menu-button > span:first-child {
  font-size: 18px;
  line-height: 1;
}

.radial-menu-button[data-level="secondary"] {
  color: #175fc4;
  font-size: 9px;
  width: 58px;
}

.radial-menu-button[data-tone="destructive"] {
  color: #a12720;
}

.radial-menu-button[aria-disabled="true"] {
  filter: grayscale(1);
  opacity: 0.38;
}

.radial-menu-button:focus-visible,
.radial-menu-center:focus-visible {
  border-radius: 9px;
  box-shadow: 0 0 0 3px rgb(36 119 212 / 42%);
  outline: none;
}

.radial-menu-center {
  align-items: center;
  background: #fff;
  border: 1px solid #d4dae3;
  border-radius: 50%;
  box-shadow: 0 4px 14px rgb(31 41 55 / 17%);
  color: #727c89;
  display: flex;
  flex-direction: column;
  font-size: 9px;
  height: 78px;
  justify-content: center;
  left: 129px;
  position: absolute;
  top: 129px;
  width: 78px;
}

.radial-menu-center strong {
  color: #202733;
  font-size: 12px;
}
```

- [ ] **Step 5: Run the focused component, model, and geometry tests**

Run: `pnpm --dir ui exec vitest run src/components/RadialFileMenu.test.tsx src/components/radialMenuModel.test.ts src/components/radialMenuGeometry.test.ts`

Expected: PASS.

- [ ] **Step 6: Commit the rendered radial menu**

```bash
git add ui/src/components/RadialFileMenu.tsx ui/src/components/RadialFileMenu.test.tsx ui/src/styles/app.css
git commit -m "feat(ui): render accessible radial file menu"
```

### Task 4: Add the ContentBrowser right-click boundary and clean card affordances

**Files:**
- Modify: `ui/src/components/ContentBrowser.tsx`
- Modify: `ui/src/components/ContentBrowser.test.tsx`
- Modify: `ui/src/styles/app.css`

**Interfaces:**
- Consumes: `RadialMenuRequest` from `RadialFileMenu.tsx`.
- Produces: `onRadialMenuRequest?: (request: RadialMenuRequest) => void` for `App.tsx`.

- [ ] **Step 1: Add failing ContentBrowser tests for right-click selection and compact content controls**

Append these cases to `ContentBrowser.test.tsx`:

```tsx
it('opens the radial request on an unselected image and replaces selection first', () => {
  const request = vi.fn()
  render(<ContentBrowser workspace={workspace(3)} onRadialMenuRequest={request} />)
  fireEvent.click(screen.getByRole('option', { name: '1.jpg' }))
  fireEvent.pointerDown(screen.getByRole('option', { name: '2.jpg' }), {
    pointerId: 70,
    button: 2,
    clientX: 210,
    clientY: 160,
  })
  expect(selectedLabels()).toEqual(['2.jpg'])
  expect(request).toHaveBeenCalledWith({
    files: [expect.objectContaining({ entityId: 'image-2' })],
    origin: { x: 210, y: 160 },
    pointerId: 70,
  })
})

it('preserves a multi-selection when right-clicking one of its files', () => {
  const request = vi.fn()
  render(<ContentBrowser workspace={workspace(3)} onRadialMenuRequest={request} />)
  fireEvent.click(screen.getByRole('option', { name: '1.jpg' }))
  fireEvent.click(screen.getByRole('option', { name: '2.jpg' }), { metaKey: true })
  fireEvent.pointerDown(screen.getByRole('option', { name: '2.jpg' }), {
    pointerId: 71,
    button: 2,
    clientX: 220,
    clientY: 170,
  })
  expect(selectedLabels()).toEqual(['1.jpg', '2.jpg'])
  expect(request.mock.calls[0]?.[0].files.map((file: BrowserFile) => file.entityId)).toEqual([
    'image-1',
    'image-2',
  ])
})

it('suppresses the native context menu for image and text options', () => {
  render(<ContentBrowser workspace={workspace(1)} onRadialMenuRequest={vi.fn()} />)
  for (const name of ['1.jpg', 'prompt.md']) {
    const event = createEvent.contextMenu(screen.getByRole('option', { name }))
    fireEvent(screen.getByRole('option', { name }), event)
    expect(event.defaultPrevented).toBe(true)
  }
})

it('hides unmarked copy and keeps marked badges plus select-all in the view menu', () => {
  const data = workspace(2)
  data.images[1] = {
    ...data.images[1]!,
    marker: { reviewState: 'keep', favorite: true },
  }
  render(<ContentBrowser workspace={data} currentPath="项目根目录" />)
  expect(screen.queryByText('未标记')).not.toBeInTheDocument()
  expect(screen.getByText('保留 · 收藏')).toBeVisible()
  expect(screen.getByText('项目根目录')).toBeVisible()
  fireEvent.click(screen.getByText('视图'))
  fireEvent.click(screen.getByRole('button', { name: '全选当前文件夹' }))
  expect(selectedLabels()).toHaveLength(3)
})
```

- [ ] **Step 2: Run the focused test and verify prop/query failures**

Run: `pnpm --dir ui exec vitest run src/components/ContentBrowser.test.tsx`

Expected: FAIL because `onRadialMenuRequest`, `currentPath`, and the compact view menu do not exist.

- [ ] **Step 3: Add the request prop and right-button selection snapshot**

Add imports and props:

```tsx
import type { RadialMenuRequest } from './RadialFileMenu'

interface ContentBrowserProps {
  currentPath?: string
  onRadialMenuRequest?: (request: RadialMenuRequest) => void
}
```

Add this helper beside `freezeDragSelection`:

```tsx
function openRadialMenu(file: BrowserFile, event: PointerEvent<HTMLElement>) {
  if (event.button !== 2 || onRadialMenuRequest === undefined) return
  event.preventDefault()
  event.stopPropagation()
  const contextSelection = selected.has(file.entityId) ? selected : new Set([file.entityId])
  if (!selected.has(file.entityId)) {
    anchorId.current = file.entityId
    setActiveId(file.entityId)
    commitSelection(contextSelection)
  }
  onRadialMenuRequest({
    files: allFiles.filter((candidate) => contextSelection.has(candidate.entityId)),
    origin: { x: event.clientX, y: event.clientY },
    pointerId: event.pointerId,
  })
}
```

Pass `onPointerDown={(event) => openRadialMenu(file, event)}` and `onContextMenu={(event) => event.preventDefault()}` to both `.image-cell` and `.text-file-row`. Do not attach the radial handler to `.organization-drag-handle`.

- [ ] **Step 4: Replace the content toolbar and conditionalize marker copy**

Replace the current `grid-toolbar` with:

```tsx
<div className="content-toolbar">
  <div>
    <strong>{currentPath ?? '当前文件夹'}</strong>
    <span>· {workspace.images.length} 张图片</span>
    {workspace.textFiles.length > 0 && <span>· {workspace.textFiles.length} 个文本文件</span>}
  </div>
  <details className="content-view-menu">
    <summary>视图</summary>
    <div>
      <label>
        缩略图大小
        <select value={gridSize} onChange={(event) => setGridSize(event.target.value as GridSize)}>
          <option value="small">小</option>
          <option value="medium">中</option>
          <option value="large">大</option>
        </select>
      </label>
      <button type="button" onClick={selectAllFiles} disabled={allFiles.length === 0}>
        全选当前文件夹
      </button>
    </div>
  </details>
</div>
```

Change `markerLabel` to return `string | null`:

```ts
function markerLabel(marker: BrowserFile['marker']): string | null {
  const review =
    marker.reviewState === 'keep'
      ? '保留'
      : marker.reviewState === 'pending'
        ? '待定'
        : marker.reviewState === 'reject'
          ? '淘汰'
          : null
  if (review === null && !marker.favorite) return null
  if (review === null) return '收藏'
  return marker.favorite ? `${review} · 收藏` : review
}
```

At both marker render sites, use:

```tsx
{markerLabel(file.marker) && <span className="file-marker">{markerLabel(file.marker)}</span>}
```

- [ ] **Step 5: Make the internal drag handle progressive without changing its events**

Add this CSS and retain the existing button in the DOM:

```css
.organization-drag-handle {
  opacity: 0;
  pointer-events: none;
  transition: opacity 120ms ease;
}

.image-cell:hover > .organization-drag-handle,
.image-cell:focus-within > .organization-drag-handle,
.text-file-row:hover > .organization-drag-handle,
.text-file-row:focus-within > .organization-drag-handle,
.viewer-shell[data-organization-drag-active="true"] .organization-drag-handle {
  opacity: 1;
  pointer-events: auto;
}

.organization-drag-handle:disabled {
  pointer-events: none;
}
```

Also reduce `VirtualGrid`’s `cellHeight` from `cellPixels + 54` to `cellPixels + 42` after the unmarked line disappears.

- [ ] **Step 6: Run ContentBrowser tests**

Run: `pnpm --dir ui exec vitest run src/components/ContentBrowser.test.tsx`

Expected: PASS, including existing Finder export and internal pointer-drag isolation tests.

- [ ] **Step 7: Commit the content boundary**

```bash
git add ui/src/components/ContentBrowser.tsx ui/src/components/ContentBrowser.test.tsx ui/src/styles/app.css
git commit -m "feat(ui): open radial menu from content files"
```

### Task 5: Wire radial actions into App and preserve global review shortcuts

**Files:**
- Create: `ui/src/state/useReviewShortcuts.ts`
- Create: `ui/src/state/useReviewShortcuts.test.tsx`
- Modify: `ui/src/components/MarkerControls.tsx`
- Modify: `ui/src/components/MarkerControls.test.tsx`
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/App.test.tsx`
- Delete: `ui/src/components/FileActionToolbar.tsx`
- Delete: `ui/src/components/FileActionToolbar.test.tsx`

**Interfaces:**
- Consumes: `RadialMenuRequest`, `RadialLeafAction`, `buildRadialMenuModel`, existing App callbacks, and `organizationShortcutIsOwned`.
- Produces: one integrated selection-action surface with no persistent `MarkerControls` or `FileActionToolbar` mount.

- [ ] **Step 1: Write failing hook tests by moving the global shortcut cases out of `MarkerControls.test.tsx`**

Create `useReviewShortcuts.test.tsx`:

```tsx
import { fireEvent, render, screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import useReviewShortcuts from './useReviewShortcuts'

function Harness({ disabled = false }: { disabled?: boolean }) {
  useReviewShortcuts({
    disabled,
    onSetReview: review,
    onToggleFavorite: favorite,
  })
  return <input aria-label="编辑名称" />
}

const review = vi.fn()
const favorite = vi.fn()

describe('useReviewShortcuts', () => {
  beforeEach(() => {
    review.mockClear()
    favorite.mockClear()
  })

  it('routes 1/2/3/0/F while the workspace owns the event', () => {
    render(<Harness />)
    for (const [key, value] of [['1', 'keep'], ['2', 'pending'], ['3', 'reject'], ['0', null]] as const) {
      fireEvent.keyDown(window, { key })
      expect(review).toHaveBeenLastCalledWith(value)
    }
    fireEvent.keyDown(window, { key: 'f' })
    expect(favorite).toHaveBeenCalledOnce()
  })

  it('does not steal editable, modal, selected-text, prevented, or disabled events', () => {
    const rendered = render(<Harness />)
    fireEvent.keyDown(screen.getByLabelText('编辑名称'), { key: '1' })
    const modal = document.createElement('div')
    modal.setAttribute('aria-modal', 'true')
    document.body.append(modal)
    fireEvent.keyDown(window, { key: '2' })
    modal.remove()
    const selection = vi.spyOn(window, 'getSelection').mockReturnValue({
      isCollapsed: false,
      toString: () => 'selected text',
    } as Selection)
    fireEvent.keyDown(window, { key: '3' })
    selection.mockRestore()
    rendered.rerender(<Harness disabled />)
    fireEvent.keyDown(window, { key: 'f' })
    expect(review).not.toHaveBeenCalled()
    expect(favorite).not.toHaveBeenCalled()
  })
})
```

- [ ] **Step 2: Run the hook test and verify it fails**

Run: `pnpm --dir ui exec vitest run src/state/useReviewShortcuts.test.tsx`

Expected: FAIL because the hook does not exist.

- [ ] **Step 3: Extract the shortcut hook**

```ts
import { useEffect } from 'react'
import type { ReviewState } from '../api/types'
import { organizationShortcutIsOwned } from './organizationShortcutOwnership'

export default function useReviewShortcuts({
  disabled,
  onSetReview,
  onToggleFavorite,
}: {
  disabled: boolean
  onSetReview: (review: ReviewState | null) => void
  onToggleFavorite: () => void
}) {
  useEffect(() => {
    function shortcut(event: KeyboardEvent) {
      if (
        organizationShortcutIsOwned(event, disabled) ||
        event.metaKey ||
        event.ctrlKey ||
        event.altKey ||
        event.shiftKey
      ) return
      const review =
        event.key === '1'
          ? 'keep'
          : event.key === '2'
            ? 'pending'
            : event.key === '3'
              ? 'reject'
              : event.key === '0'
                ? null
                : undefined
      if (review !== undefined) {
        event.preventDefault()
        onSetReview(review)
      } else if (event.key.toLowerCase() === 'f') {
        event.preventDefault()
        onToggleFavorite()
      }
    }
    window.addEventListener('keydown', shortcut)
    return () => window.removeEventListener('keydown', shortcut)
  }, [disabled, onSetReview, onToggleFavorite])
}
```

Use this hook inside the existing default `MarkerControls` until App stops mounting it. Keep the exported `MarkerButtons` unchanged for `ComparePane.tsx`. Narrow `MarkerControls.test.tsx` to button rendering/click state and remove the global shortcut cases now covered by the hook.

- [ ] **Step 4: Add failing App integration tests for radial actions and persistent-toolbar removal**

Add a helper and cases to `App.test.tsx`:

```tsx
function openRadialMenu(file: HTMLElement, pointerId = 90) {
  fireEvent.pointerDown(file, {
    pointerId,
    button: 2,
    clientX: 420,
    clientY: 260,
  })
}

it('replaces persistent selection toolbars with the right-click radial menu', async () => {
  const viewer = bridge()
  vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
  render(<App bridge={viewer} />)
  fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
  const file = await screen.findByRole('option', { name: 'front.jpg' })
  expect(screen.queryByLabelText('批量标记')).not.toBeInTheDocument()
  expect(screen.queryByLabelText('文件操作')).not.toBeInTheDocument()
  openRadialMenu(file)
  expect(screen.getByRole('menu', { name: '文件操作' })).toBeVisible()
  fireEvent.click(screen.getByRole('menuitem', { name: '信息' }))
  expect(screen.getByRole('complementary', { name: '文件信息' })).toBeVisible()
})

it('routes marker, organize, compare, and Trash leaves through existing safe owners', async () => {
  const viewer = bridge()
  vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
  render(<App bridge={viewer} />)
  fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
  const front = await screen.findByRole('option', { name: 'front.jpg' })
  openRadialMenu(front, 91)
  fireEvent.click(screen.getByRole('menuitem', { name: '标记' }))
  fireEvent.click(screen.getByRole('menuitemcheckbox', { name: '保留' }))
  await waitFor(() => expect(viewer.setReviewState).toHaveBeenCalled())

  openRadialMenu(front, 92)
  fireEvent.click(screen.getByRole('menuitem', { name: '整理' }))
  fireEvent.click(screen.getByRole('menuitem', { name: '重命名' }))
  expect(screen.getByRole('dialog', { name: '重命名文件' })).toBeVisible()
  fireEvent.click(screen.getByRole('button', { name: '取消' }))

  openRadialMenu(front, 93)
  fireEvent.click(screen.getByRole('menuitem', { name: '移到废纸篓' }))
  expect(screen.getByRole('dialog', { name: '将文件移到废纸篓？' })).toBeVisible()
})
```

- [ ] **Step 5: Add App radial session state and close conditions**

Add imports:

```tsx
import RadialFileMenu from './components/RadialFileMenu'
import type { RadialMenuRequest } from './components/RadialFileMenu'
import { buildRadialMenuModel } from './components/radialMenuModel'
import type { RadialLeafAction } from './components/radialMenuModel'
import useReviewShortcuts from './state/useReviewShortcuts'
```

Add state and reset it with the existing project-session reset effect:

```tsx
const [radialMenu, setRadialMenu] = useState<RadialMenuRequest | null>(null)
```

Build the model from the request snapshot:

```tsx
const radialModel = useMemo(() => {
  const files = radialMenu?.files ?? []
  const reviews = new Set(files.map((file) => file.marker.reviewState))
  const favorites = new Set(files.map((file) => file.marker.favorite))
  return buildRadialMenuModel({
    selectedCount: files.length,
    selectedImageCount: files.filter(matchesImage).length,
    readOnly: state.project?.access === 'read_only',
    busy: operationBusy,
    compareContextAvailable: compareEntryAvailable,
    commonReview: reviews.size === 1 ? files[0]?.marker.reviewState ?? null : 'mixed',
    commonFavorite: favorites.size === 1 ? files[0]?.marker.favorite ?? false : 'mixed',
  })
}, [compareEntryAvailable, operationBusy, radialMenu, state.project?.access])
```

Close the radial session in effects when any of these values change: project session, workspace identity, search-result visibility, active preview, compare mode, operation dialog, info overlay, results overlay, close-blocked state, or context repair.

- [ ] **Step 6: Map every radial leaf to the existing App owner**

Add this callback in `App.tsx`:

```tsx
const runRadialAction = useCallback(
  (action: RadialLeafAction) => {
    const files = radialMenu?.files ?? []
    if (files.length === 0) return
    setRadialMenu(null)
    const ids = files.map((file) => file.entityId)
    if (action === 'preview' && files.length === 1) openPreview(files[0]!)
    else if (action === 'mark.keep') void setReviewState('keep', ids)
    else if (action === 'mark.pending') void setReviewState('pending', ids)
    else if (action === 'mark.reject') void setReviewState('reject', ids)
    else if (action === 'mark.clear') void setReviewState(null, ids)
    else if (action === 'mark.favorite') void toggleFavorite(ids)
    else if (action === 'organize.rename') {
      setOperationDialog(
        files.length === 1
          ? { kind: 'rename', file: files[0]! }
          : { kind: 'batch_rename', files },
      )
    } else if (action === 'organize.copy') {
      setOperationDialog({ kind: 'destination', mode: 'copy', files })
    } else if (action === 'organize.move') {
      setOperationDialog({ kind: 'destination', mode: 'move', files })
    } else if (action === 'trash') setOperationDialog({ kind: 'trash', files })
    else if (action === 'compare') setCompareEntityIds(ids)
    else if (action === 'info') setInfoOpen(true)
  },
  [openPreview, radialMenu, setCompareEntityIds, setReviewState, toggleFavorite],
)
```

Pass `onRadialMenuRequest={setRadialMenu}` to `ContentBrowser` and render before modal overlays:

```tsx
{radialMenu && (
  <RadialFileMenu
    origin={radialMenu.origin}
    pointerId={radialMenu.pointerId}
    selectionCount={radialMenu.files.length}
    model={radialModel}
    onAction={runRadialAction}
    onClose={() => setRadialMenu(null)}
  />
)}
```

- [ ] **Step 7: Remove persistent toolbars without losing shortcuts**

Call `useReviewShortcuts` once in `App` with the exact old disabled predicate and callbacks. Remove the mounted `<MarkerControls>` and `<FileActionToolbar>` blocks and their imports. Delete `FileActionToolbar.tsx` and its test. Do not delete `MarkerControls.tsx`, because `ComparePane.tsx` imports `MarkerButtons` from it.

- [ ] **Step 8: Run focused App, radial, marker, and shortcut tests**

Run: `pnpm --dir ui exec vitest run src/App.test.tsx src/components/RadialFileMenu.test.tsx src/components/MarkerControls.test.tsx src/state/useReviewShortcuts.test.tsx`

Expected: PASS.

- [ ] **Step 9: Commit App integration**

```bash
git add ui/src/App.tsx ui/src/App.test.tsx ui/src/components/MarkerControls.tsx ui/src/components/MarkerControls.test.tsx ui/src/state/useReviewShortcuts.ts ui/src/state/useReviewShortcuts.test.tsx ui/src/components/FileActionToolbar.tsx ui/src/components/FileActionToolbar.test.tsx
git commit -m "feat(ui): route file actions through radial menu"
```

### Task 6: Compact the project header, search controls, content bar, and sidebar resize

**Files:**
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/App.test.tsx`
- Modify: `ui/src/components/SearchToolbar.tsx`
- Modify: `ui/src/components/SearchToolbar.test.tsx`
- Modify: `ui/src/components/ContentBrowser.tsx`
- Modify: `ui/src/styles/app.css`

**Interfaces:**
- Consumes: existing search callbacks, project close callback, `sidebarWidth` state, and ContentBrowser `currentPath`.
- Produces: one 48px header, one 44px content bar, project menu, compact filter/sort popover, and pointer/keyboard sidebar separator.

- [ ] **Step 1: Update SearchToolbar tests to require combined popovers**

Keep the existing callback assertions, but replace direct control discovery with this flow:

```tsx
expect(screen.getByRole('searchbox', { name: '搜索项目' })).toBeVisible()
expect(screen.queryByRole('combobox', { name: '搜索范围' })).not.toBeInTheDocument()
fireEvent.click(screen.getByText('筛选与排序'))
expect(screen.getByRole('combobox', { name: '搜索范围' })).toBeVisible()
expect(screen.getByRole('combobox', { name: '排序方式' })).toBeVisible()
fireEvent.click(screen.getByText('结果视图'))
fireEvent.click(screen.getByRole('button', { name: '展平结果' }))
expect(onLayoutChange).toHaveBeenCalledWith('flat')
```

Also assert that active filter chips remain outside the closed popover and that `Command+F` focus still works.

- [ ] **Step 2: Run SearchToolbar tests and verify the old always-visible controls fail the new expectations**

Run: `pnpm --dir ui exec vitest run src/components/SearchToolbar.test.tsx`

Expected: FAIL because range/sort/layout are still always visible.

- [ ] **Step 3: Replace `SearchToolbar`’s top-level structure while preserving every existing field callback**

Keep the existing search `<label>` as the first child of `<section className="search-toolbar">`, add the search icon immediately before its `<input>`, and add the shortcut hint immediately after the input:

```tsx
<span aria-hidden="true">⌕</span>
<input
  ref={searchRef}
  type="search"
  aria-label="搜索项目"
  placeholder="搜索名称、路径或文本"
  value={query.text}
  onChange={(event) => onTextChange(event.currentTarget.value)}
/>
<kbd>⌘F</kbd>
```

Immediately before the existing label containing `<span>范围</span>`, open the combined popover:

```tsx
<details className="search-options-panel">
  <summary>筛选与排序</summary>
  <div className="search-options-popover">
```

Keep the current range label and all four filter fieldsets. Replace `<details className="search-filter-panel">` plus its “筛选” summary with `<div className="search-filter-section">`, and replace that details element’s closing tag with `</div>` so one click exposes all options. Move the existing sort label and direction button directly after `search-filter-section`, then close the combined popover immediately after the direction button:

```tsx
  </div>
</details>
```

Replace the current `<div className="search-layout-toggle">` wrapper with this exact wrapper while keeping its two existing buttons and callbacks unchanged:

```tsx
<details className="search-view-panel">
  <summary>结果视图</summary>
  <div className="search-view-popover search-layout-toggle">
    <button
      type="button"
      aria-label="按文件夹分组"
      aria-pressed={query.layout === 'grouped'}
      onClick={() => onLayoutChange('grouped')}
    >
      分组
    </button>
    <button
      type="button"
      aria-label="展平结果"
      aria-pressed={query.layout === 'flat'}
      onClick={() => onLayoutChange('flat')}
    >
      展平
    </button>
  </div>
</details>
```

Keep the existing `chips.length > 0` block as the final child of `search-toolbar` so active filters stay visible while both details elements are closed.

- [ ] **Step 4: Put search inside the project header and move close into a project menu**

Replace the current header plus separate SearchToolbar mount with:

```tsx
<header className="workspace-header">
  <h1>{state.project.displayName}</h1>
  <SearchToolbar
    query={state.search.query}
    folders={state.folders}
    focusRequest={state.search.focusRequest}
    onTextChange={setSearchText}
    onScopeChange={setSearchScope}
    onFiltersChange={setSearchFilters}
    onSortChange={setSearchSort}
    onLayoutChange={setSearchLayout}
    onRemoveFilter={removeSearchFilter}
    onClearFilters={clearSearchFilters}
  />
  <details className="project-menu">
    <summary aria-label="项目菜单">•••</summary>
    <div>
      <button
        type="button"
        disabled={state.status === 'closing'}
        onClick={() => void closeProject()}
      >
        {state.status === 'closing' ? '正在关闭…' : '关闭项目'}
      </button>
    </div>
  </details>
</header>
```

Pass `currentPath={state.selectedFolderPath || state.project.displayName}` to `ContentBrowser`.

- [ ] **Step 5: Replace the sidebar range with a pointer-and-keyboard separator**

Add `PointerEvent as ReactPointerEvent` to the existing React type import, then add this handler in `App.tsx`:

```tsx
const startSidebarResize = useCallback((event: ReactPointerEvent<HTMLButtonElement>) => {
  if (sidebarCollapsed || event.button !== 0) return
  event.preventDefault()
  const startX = event.clientX
  const startWidth = sidebarWidth
  const move = (next: PointerEvent) => {
    setSidebarWidth(Math.max(200, Math.min(420, startWidth + next.clientX - startX)))
  }
  const stop = () => {
    window.removeEventListener('pointermove', move)
    window.removeEventListener('pointerup', stop)
  }
  window.addEventListener('pointermove', move)
  window.addEventListener('pointerup', stop)
}, [sidebarCollapsed, sidebarWidth])
```

Replace the range label with:

```tsx
<button
  type="button"
  className="sidebar-separator"
  role="separator"
  aria-label="调整文件夹栏宽度"
  aria-orientation="vertical"
  aria-valuemin={200}
  aria-valuemax={420}
  aria-valuenow={sidebarWidth}
  onPointerDown={startSidebarResize}
  onKeyDown={(event) => {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return
    event.preventDefault()
    const delta = event.key === 'ArrowLeft' ? -16 : 16
    setSidebarWidth((width) => Math.max(200, Math.min(420, width + delta)))
  }}
/>
```

- [ ] **Step 6: Add App tests for the project menu and separator**

Add this helper near `openRadialMenu` and replace every existing direct `screen.getByRole('button', { name: '关闭项目' })` click with it:

```tsx
function closeProjectFromMenu() {
  fireEvent.click(screen.getByRole('button', { name: '项目菜单' }))
  fireEvent.click(screen.getByRole('button', { name: '关闭项目' }))
}
```

```tsx
it('moves close-project into the compact project menu', async () => {
  const viewer = bridge()
  render(<App bridge={viewer} />)
  fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
  await screen.findByText('Catalog')
  expect(screen.queryByRole('button', { name: '关闭项目' })).not.toBeInTheDocument()
  closeProjectFromMenu()
  await waitFor(() => expect(viewer.closeProject).toHaveBeenCalled())
})

it('resizes the sidebar through pointer and keyboard separator input', async () => {
  const viewer = bridge()
  render(<App bridge={viewer} />)
  fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
  await screen.findByText('Catalog')
  const separator = screen.getByRole('separator', { name: '调整文件夹栏宽度' })
  fireEvent.pointerDown(separator, { pointerId: 101, button: 0, clientX: 260 })
  fireEvent.pointerMove(window, { pointerId: 101, clientX: 300 })
  fireEvent.pointerUp(window, { pointerId: 101, clientX: 300 })
  expect(screen.getByLabelText('文件夹栏')).toHaveStyle({ width: '300px' })
  fireEvent.keyDown(separator, { key: 'ArrowLeft' })
  expect(screen.getByLabelText('文件夹栏')).toHaveStyle({ width: '284px' })
})
```

- [ ] **Step 7: Replace the old toolbar CSS with compact chrome CSS**

Use these governing dimensions and layout rules:

```css
.workspace-header {
  align-items: center;
  background: #fbfcfd;
  border-bottom: 1px solid #d8dce2;
  display: flex;
  flex: 0 0 48px;
  gap: 12px;
  min-height: 48px;
  padding: 0 12px;
  position: relative;
  z-index: 10;
}

.workspace-header h1 {
  flex: 0 0 auto;
  font-size: 14px;
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.search-toolbar {
  align-items: center;
  display: flex;
  flex: 1 1 auto;
  gap: 7px;
  min-width: 0;
  position: relative;
}

.search-field {
  align-items: center;
  background: #fff;
  border: 1px solid #c8ced6;
  border-radius: 8px;
  display: flex;
  flex: 1 1 360px;
  gap: 6px;
  max-width: 640px;
  padding: 0 8px;
}

.search-field input {
  border: 0;
  min-width: 100px;
  outline: 0;
  padding: 6px 0;
  width: 100%;
}

.search-field kbd {
  color: #737d8a;
  font-size: 10px;
}

.search-options-panel,
.search-view-panel,
.project-menu,
.content-view-menu {
  position: relative;
}

.search-options-panel > summary,
.search-view-panel > summary,
.project-menu > summary,
.content-view-menu > summary {
  border-radius: 7px;
  cursor: default;
  list-style: none;
  padding: 6px 8px;
}

.search-options-popover,
.search-view-popover,
.project-menu > div,
.content-view-menu > div {
  background: #fff;
  border: 1px solid #c8ced6;
  border-radius: 10px;
  box-shadow: 0 12px 34px rgb(34 42 53 / 20%);
  padding: 10px;
  position: absolute;
  right: 0;
  top: calc(100% + 6px);
  z-index: 20;
}

.content-toolbar {
  align-items: center;
  border-bottom: 1px solid #edf0f3;
  display: flex;
  height: 44px;
  justify-content: space-between;
  margin-bottom: 12px;
}

.sidebar-separator {
  background: transparent;
  border: 0;
  cursor: col-resize;
  height: 100%;
  padding: 0;
  position: absolute;
  right: -4px;
  top: 0;
  width: 8px;
  z-index: 3;
}

.sidebar-separator:focus-visible,
.sidebar-separator:hover {
  background: rgb(36 119 212 / 28%);
  outline: none;
}
```

Remove the old standalone `.marker-controls`, `.marker-selection-summary`, `.file-action-toolbar`, and `.sidebar-resize` blocks after no mounted element uses them. Keep `.marker-buttons` styles because compare panes still use `MarkerButtons`.

- [ ] **Step 8: Run compact-chrome tests and the UI build**

Run: `pnpm --dir ui exec vitest run src/components/SearchToolbar.test.tsx src/components/ContentBrowser.test.tsx src/App.test.tsx`

Expected: PASS.

Run: `pnpm --dir ui build`

Expected: TypeScript and Vite build succeed.

- [ ] **Step 9: Commit compact chrome**

```bash
git add ui/src/App.tsx ui/src/App.test.tsx ui/src/components/SearchToolbar.tsx ui/src/components/SearchToolbar.test.tsx ui/src/components/ContentBrowser.tsx ui/src/styles/app.css
git commit -m "feat(ui): compact viewer workspace chrome"
```

### Task 7: Make task feedback progressive and preserve sticky results

**Files:**
- Modify: `ui/src/components/TaskBar.tsx`
- Modify: `ui/src/components/TaskBar.test.tsx`
- Modify: `ui/src/styles/app.css`

**Interfaces:**
- Consumes: existing `TaskFeedback[]` and callbacks.
- Produces: the same public props plus optional `successDismissMs?: number`, defaulting to 2000ms.

- [ ] **Step 1: Add failing timer and stickiness tests**

Append to `TaskBar.test.tsx`:

```tsx
it('auto-hides a clean completed task after the success interval', () => {
  vi.useFakeTimers()
  render(
    <TaskBar
      task={{
        ...failedScanTask,
        status: 'complete',
        completed: 12,
        failed: 0,
        failures: [],
      }}
      successDismissMs={2000}
    />,
  )
  expect(screen.getByText('扫描项目')).toBeVisible()
  vi.advanceTimersByTime(1999)
  expect(screen.getByText('扫描项目')).toBeVisible()
  vi.advanceTimersByTime(1)
  expect(screen.queryByText('扫描项目')).not.toBeInTheDocument()
  vi.useRealTimers()
})

it('keeps failed, cancelled, and result-bearing tasks until explicit dismissal', () => {
  vi.useFakeTimers()
  const dismiss = vi.fn()
  render(
    <TaskBar
      tasks={[
        failedScanTask,
        { ...failedScanTask, id: 'cancelled', status: 'cancelled', failed: 0, failures: [] },
        { ...failedScanTask, id: 'results', status: 'complete', failed: 0, failures: [], hasResults: true },
      ]}
      successDismissMs={1}
      onDismiss={dismiss}
    />,
  )
  vi.runAllTimers()
  expect(screen.getByText('2 项失败')).toBeVisible()
  expect(screen.getAllByText('扫描项目')).toHaveLength(3)
  fireEvent.click(screen.getAllByRole('button', { name: '关闭任务' })[0]!)
  expect(dismiss).toHaveBeenCalled()
  vi.useRealTimers()
})

it('shows the same task id again when it returns to running', () => {
  vi.useFakeTimers()
  const complete = { ...failedScanTask, status: 'complete' as const, completed: 12, failed: 0, failures: [] }
  const rendered = render(<TaskBar task={complete} successDismissMs={1} />)
  vi.runAllTimers()
  expect(screen.queryByText('扫描项目')).not.toBeInTheDocument()
  rendered.rerender(
    <TaskBar task={{ ...complete, status: 'running', completed: 0 }} successDismissMs={1} />,
  )
  expect(screen.getByText('扫描项目')).toBeVisible()
  vi.useRealTimers()
})
```

- [ ] **Step 2: Run TaskBar tests and verify clean success remains visible**

Run: `pnpm --dir ui exec vitest run src/components/TaskBar.test.tsx`

Expected: FAIL because `successDismissMs` and local auto-hide do not exist.

- [ ] **Step 3: Implement clean-success local hiding**

Add the prop, hidden set, and timer effect:

```tsx
interface TaskBarProps {
  task?: TaskFeedback | null
  tasks?: TaskFeedback[]
  onCancel?: (taskId: string) => void
  onDismiss?: (taskId: string) => void
  onShowResults?: (taskId: string) => void
  successDismissMs?: number
}

export default function TaskBar({
  task = null,
  tasks,
  onCancel,
  onDismiss,
  onShowResults,
  successDismissMs = 2000,
}: TaskBarProps) {
  const candidates = tasks ?? (task ? [task] : [])
  const [hiddenTaskIds, setHiddenTaskIds] = useState<Set<string>>(() => new Set())
  const visibleTasks = candidates.filter((candidate) => !hiddenTaskIds.has(candidate.id))

  useEffect(() => {
    const mustReappear = new Set(
      candidates
        .filter(
          (candidate) =>
            candidate.status !== 'complete' ||
            candidate.failed > 0 ||
            Boolean(candidate.hasResults),
        )
        .map((candidate) => candidate.id),
    )
    setHiddenTaskIds((current) => {
      const next = new Set([...current].filter((id) => !mustReappear.has(id)))
      return next.size === current.size ? current : next
    })
    const clean = candidates.filter(
      (candidate) =>
        candidate.status === 'complete' &&
        candidate.failed === 0 &&
        !candidate.hasResults,
    )
    const timers = clean.map((candidate) =>
      window.setTimeout(() => {
        setHiddenTaskIds((current) => new Set([...current, candidate.id]))
      }, successDismissMs),
    )
    return () => timers.forEach((timer) => window.clearTimeout(timer))
  }, [candidates, successDismissMs])
```

Retain the current expanded-task auto-collapse, failure list, cancellation, result, and explicit-dismiss behavior.

- [ ] **Step 4: Convert TaskBar CSS to a floating bottom capsule**

```css
.task-bar {
  align-items: center;
  background: rgb(35 42 51 / 94%);
  border: 1px solid rgb(255 255 255 / 18%);
  border-radius: 18px;
  bottom: 12px;
  box-shadow: 0 8px 24px rgb(0 0 0 / 20%);
  color: #fff;
  display: flex;
  gap: 10px;
  left: 50%;
  max-width: calc(100vw - 32px);
  min-height: 34px;
  padding: 4px 10px;
  position: fixed;
  transform: translateX(-50%);
  z-index: 18;
}

.task-details {
  background: #fff;
  border: 1px solid #d8dce2;
  border-radius: 9px;
  bottom: 42px;
  color: #1f2328;
  left: 0;
  max-height: 240px;
  min-width: 360px;
  overflow: auto;
  padding: 8px;
  position: absolute;
}
```

- [ ] **Step 5: Run TaskBar and App task-flow tests**

Run: `pnpm --dir ui exec vitest run src/components/TaskBar.test.tsx src/App.test.tsx`

Expected: PASS.

- [ ] **Step 6: Commit progressive task feedback**

```bash
git add ui/src/components/TaskBar.tsx ui/src/components/TaskBar.test.tsx ui/src/styles/app.css
git commit -m "feat(ui): show task feedback on demand"
```

### Task 8: Complete integration, accessibility, and regression verification

**Files:**
- Modify: `ui/src/App.test.tsx`
- Modify: `ui/src/components/RadialFileMenu.test.tsx`
- Modify: `ui/src/components/ContentBrowser.test.tsx`
- Modify: `ui/src/styles/app.css`

**Interfaces:**
- Consumes: all completed UI tasks.
- Produces: release-gate evidence that the redesign preserves the Viewer 0.1 capability matrix.

- [ ] **Step 1: Add missing App regression cases for read-only, busy, multi-select compare, and close conditions**

Add these concrete assertions to the existing read-only and operation-busy App tests:

```tsx
openRadialMenu(front, 120)
expect(screen.getByRole('menuitem', { name: '标记' })).toHaveAttribute('aria-disabled', 'true')
expect(screen.getByRole('menuitem', { name: '整理' })).toHaveAttribute('aria-disabled', 'true')
expect(screen.getByRole('menuitem', { name: '移到废纸篓' })).toHaveAttribute('aria-disabled', 'true')
expect(screen.getByRole('menuitem', { name: '信息' })).toHaveAttribute('aria-disabled', 'false')
```

Add a multi-select compare case:

```tsx
fireEvent.click(front)
fireEvent.click(back, { metaKey: true })
openRadialMenu(back, 121)
expect(screen.getByRole('menuitem', { name: '并排对比' })).toHaveAttribute('aria-disabled', 'false')
fireEvent.click(screen.getByRole('menuitem', { name: '并排对比' }))
expect(screen.getByRole('region', { name: '图片对比' })).toBeVisible()
```

Add close assertions after opening a radial menu for each existing transition: folder selection, search results, preview, context repair, operation dialog, compare, project close, and project-session reset.

- [ ] **Step 2: Add an edge-placement and label-orientation component case**

```tsx
it('rotates only the local fan at the right edge and keeps accessible labels unchanged', () => {
  render(
    <RadialFileMenu
      origin={{ x: 1260, y: 400 }}
      pointerId={null}
      selectionCount={1}
      viewport={{ width: 1280, height: 800 }}
      model={model}
      onAction={vi.fn()}
      onClose={vi.fn()}
    />,
  )
  fireEvent.click(screen.getByRole('menuitem', { name: '标记' }))
  expect(screen.getByRole('menuitemcheckbox', { name: '保留' })).toBeVisible()
  expect(screen.getByRole('menuitemcheckbox', { name: '取消收藏' })).not.toBeInTheDocument()
  const menu = screen.getByRole('menu', { name: '文件操作' })
  expect(menu).toHaveStyle({ '--radial-origin-x': '1100px' })
})
```

- [ ] **Step 3: Add system dark-appearance and reduced-motion rules for every new surface**

Append to `app.css`:

```css
@media (prefers-color-scheme: dark) {
  .workspace-header,
  .search-field,
  .search-options-popover,
  .search-view-popover,
  .project-menu > div,
  .content-view-menu > div,
  .radial-menu-center {
    background: #24282f;
    border-color: #4d5663;
    color: #f3f5f7;
  }

  .radial-primary-shape {
    fill: #2f343d;
    stroke: #596474;
  }

  .radial-secondary-shape,
  .radial-primary-shape[data-active="true"],
  .radial-secondary-shape[data-active="true"] {
    fill: #244d7d;
    stroke: #5d9ee8;
  }

  .radial-menu-button,
  .radial-menu-center strong {
    color: #f3f5f7;
  }
}

@media (prefers-reduced-motion: reduce) {
  .organization-drag-handle,
  .radial-primary-shape,
  .radial-secondary-shape {
    transition: none;
  }
}
```

- [ ] **Step 4: Run the full UI suite**

Run: `pnpm --dir ui test`

Expected: all Vitest files pass with no unhandled promise rejection or timer warning.

- [ ] **Step 5: Run TypeScript and production UI build**

Run: `pnpm --dir ui build`

Expected: `tsc -b` and Vite production build succeed.

- [ ] **Step 6: Run repository formatting, lint, policy, Rust, and security regression gate**

Run: `pnpm verify`

Expected: repository policy, UI tests/build, `cargo fmt --check`, locked workspace Clippy with `-D warnings`, and locked workspace tests all pass.

- [ ] **Step 7: Manually verify the running Tauri app at 1280×800**

Run: `pnpm tauri dev`

Verify all of the following before stopping the process:

- Project header plus content bar occupy two persistent rows; marker and file-action rows are absent.
- Right-click an unselected image: it becomes the only selection and the radial menu opens at the pointer.
- Right-click one image in a multi-selection: selection is preserved.
- Hover “标记”: the five continuous outer annular sectors grow only from that primary direction.
- Hover “整理”: rename/copy/move grow from that direction.
- Near every window edge: the full menu remains visible and labels stay upright.
- A short right-click release leaves click mode open; a deliberate radial stroke executes on release.
- Trash opens the existing confirmation and never deletes immediately.
- Read-only mode disables marker/organize/Trash but preserves preview/info/valid compare.
- Finder export and project-internal drag remain distinct; the internal handle appears on hover/focus.
- Clean task success disappears after 2 seconds; failures and results remain actionable.
- `1/2/3/0/F/C/Delete/Command+I/Command+Z/Command+F` retain their ownership rules.

- [ ] **Step 8: Inspect the final diff for unintended backend or dependency changes**

Run: `git diff --stat HEAD~7..HEAD`

Expected: changes are limited to `ui/src`, UI tests/styles, and no lockfile, Rust crate, Tauri command, or metadata-schema file appears.

Run: `git status --short`

Expected: only pre-existing unrelated `.DS_Store` or fixture artifacts may remain untracked; no implementation file is unstaged.

- [ ] **Step 9: Commit final regression adjustments**

```bash
git add ui/src/App.test.tsx ui/src/components/RadialFileMenu.test.tsx ui/src/components/ContentBrowser.test.tsx ui/src/styles/app.css
git commit -m "test(ui): verify simplified radial workspace"
```
