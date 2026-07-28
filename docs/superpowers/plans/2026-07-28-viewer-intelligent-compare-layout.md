# Viewer Intelligent Compare Layout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Viewer’s fixed 2–4 image comparison grid with a performant, aspect-aware comparison system for 2–20 JPG/PNG images that chooses readable no-scroll layouts and virtualized scrolling fallbacks.

**Architecture:** A shared compare policy owns capacity and validation. A pure layout engine scores bounded equal-weight candidates and builds scroll geometry from metadata; a measurement hook adds ResizeObserver batching and hysteresis; a specialized virtual viewport mounts only visible and overscanned compare panes. `CompareWorkspace` composes those pieces while the existing compare model remains responsible for transforms, rotation, review state and favorites.

**Tech Stack:** React 19, TypeScript 6, Vitest 4, Testing Library, CSS, existing Viewer image-representation APIs; no new dependency.

## Global Constraints

- Comparison accepts exactly 2–20 unique JPG or PNG images; `MIN_COMPARE_IMAGES = 2` and `MAX_COMPARE_IMAGES = 20`.
- Selecting more than 20 items disables the radial `并排对比` action with reason `最多同时对比 20 张图片`; it must not dispatch an action or silently truncate.
- Preserve source/selection order in every layout.
- Base fit never crops or stretches an image.
- Remove the asymmetric three-pane comparison; every no-scroll pane has equal visual weight.
- Initial readability thresholds are 120,000 CSS px² display area and 360 CSS px displayed long edge.
- Candidate score weights are minimum normalized area 45%, mean normalized area 25%, image fill 20% and layout continuity 10%.
- Generate at most six candidates and switch layout family only when the current family is invalid or the new score is at least 12% better.
- Portrait threshold is ratio `< 0.90`; neutral is `0.90–1.10`; landscape is `> 1.10`.
- Landscape/square scrolling uses two columns at 900 CSS px or wider and one column below 900 CSS px.
- Scroll virtualization mounts the visible interval, one viewport of overscan before and after it, and at most one additional focused pane.
- Layout solving must not read image pixels, wait for decode or measure every pane.
- Preserve current light preview tokens, synchronized/independent transforms, 100% safety fallback, review/favorite controls and read-only behavior.
- Do not add a manual layout picker or third-party layout/virtualization package.
- Do not modify `ui/src/components/VirtualList.tsx`; it has unrelated user changes and its fixed vertical-row contract does not fit comparison geometry.
- Preserve all unrelated dirty worktree changes and stage only files named by the current task.

---

## File Structure

### New files

- `ui/src/state/comparePolicy.ts` — shared min/max capacity, supported-kind validation and user-facing validation copy.
- `ui/src/state/comparePolicy.test.ts` — capacity, duplicate and supported-kind policy tests.
- `ui/src/components/compareLayoutEngine.ts` — pure orientation, candidate scoring, no-scroll and scroll geometry, visibility and anchor math.
- `ui/src/components/compareLayoutEngine.test.ts` — solver, threshold, hysteresis, geometry and visibility matrix.
- `ui/src/components/useCompareLayout.ts` — outer-workspace measurement, frame batching and previous-plan retention.
- `ui/src/components/useCompareLayout.test.tsx` — ResizeObserver, rAF batching, metadata and rotation hook tests.
- `ui/src/components/CompareVirtualViewport.tsx` — horizontal/vertical compare virtualization, wheel behavior, keyboard navigation and focus anchoring.
- `ui/src/components/CompareVirtualViewport.test.tsx` — mounted-window, scrolling, focus, cancellation boundary and accessibility tests.
- `ui/src/components/compareLayoutBenchmark.test.ts` — opt-in/current-machine solver benchmark plus deterministic candidate-count assertions.

### Modified files

- `ui/src/state/compareModel.ts` — consume shared policy, accept up to 20, remove fixed layout types/function.
- `ui/src/state/compareModel.test.ts` — 2–20 creation and transform/reconciliation regression tests.
- `ui/src/state/reducers/workspaceReducer.ts` — remove `.slice(0, 4)` and refuse over-capacity compare context without truncation.
- `ui/src/state/viewerReducer.test.ts` — reducer capacity and no-truncation coverage.
- `ui/src/components/radialMenuModel.ts` — 20-image gate and disabled-reason precedence.
- `ui/src/components/radialMenuModel.test.ts` — enabled-at-20 and disabled-at-21 tests.
- `ui/src/components/RadialFileMenu.test.tsx` — grey/semantic disabled rendering and no-dispatch test.
- `ui/src/App.tsx` — shared validation for keyboard/radial/direct comparison entry.
- `ui/src/App.test.tsx` — 20-image entry, 21-image disabled radial action and validation-copy tests.
- `ui/src/components/ComparePane.tsx` — treat virtualization-driven request aborts as silent cleanup.
- `ui/src/components/ComparePane.test.tsx` — abort cleanup and failed-image geometry regression tests.
- `ui/src/components/CompareWorkspace.tsx` — use the layout hook and render fit or virtual scrolling plans.
- `ui/src/components/CompareWorkspace.test.tsx` — aspect-aware layouts, scrolling integration, bounded requests and transform regressions.
- `ui/src/styles/app.css` — replace fixed comparison classes with plan-driven fit/scroll surfaces.
- `ui/src/styles/app.test.ts` — assert new layout, containment, overflow and light-token contracts.

---

### Task 1: Unify the 2–20 Image Capacity Contract

**Files:**
- Create: `ui/src/state/comparePolicy.ts`
- Create: `ui/src/state/comparePolicy.test.ts`
- Modify: `ui/src/state/compareModel.ts`
- Modify: `ui/src/state/compareModel.test.ts`
- Modify: `ui/src/state/reducers/workspaceReducer.ts`
- Modify: `ui/src/state/viewerReducer.test.ts`
- Modify: `ui/src/components/radialMenuModel.ts`
- Modify: `ui/src/components/radialMenuModel.test.ts`
- Modify: `ui/src/components/RadialFileMenu.test.tsx`
- Modify: `ui/src/App.tsx`
- Modify: `ui/src/App.test.tsx`
- Modify: `ui/src/components/CompareWorkspace.tsx`
- Test: `ui/src/state/comparePolicy.test.ts`
- Test: `ui/src/state/compareModel.test.ts`
- Test: `ui/src/components/radialMenuModel.test.ts`
- Test: `ui/src/components/RadialFileMenu.test.tsx`
- Test: `ui/src/App.test.tsx`
- Test: `ui/src/state/viewerReducer.test.ts`

**Interfaces:**
- Produces:
  - `MIN_COMPARE_IMAGES: 2`
  - `MAX_COMPARE_IMAGES: 20`
  - `CompareCandidateLike { entityId: string; kind: string }`
  - `CompareValidationReason = 'invalid_cardinality' | 'duplicate_entity' | 'unsupported_type'`
  - `validateCompareCandidates(candidates): { ok: true } | { ok: false; reason: CompareValidationReason }`
  - `compareValidationMessage(reason): string`
- Consumes: `FileKind` values already present on `BrowserFile` and `CompareCandidate`.

- [ ] **Step 1: Write the failing shared-policy tests**

Add `ui/src/state/comparePolicy.test.ts`:

```ts
import { describe, expect, it } from 'vitest'
import {
  MAX_COMPARE_IMAGES,
  MIN_COMPARE_IMAGES,
  compareValidationMessage,
  validateCompareCandidates,
} from './comparePolicy'

const image = (index: number) => ({ entityId: `image-${index}`, kind: 'jpeg' })

describe('comparePolicy', () => {
  it('accepts exactly 2 through 20 supported unique images', () => {
    expect(MIN_COMPARE_IMAGES).toBe(2)
    expect(MAX_COMPARE_IMAGES).toBe(20)
    expect(validateCompareCandidates([image(1), image(2)])).toEqual({ ok: true })
    expect(
      validateCompareCandidates(Array.from({ length: 20 }, (_, index) => image(index))),
    ).toEqual({ ok: true })
  })

  it('rejects invalid cardinality without truncating', () => {
    expect(validateCompareCandidates([image(1)])).toEqual({
      ok: false,
      reason: 'invalid_cardinality',
    })
    expect(
      validateCompareCandidates(Array.from({ length: 21 }, (_, index) => image(index))),
    ).toEqual({ ok: false, reason: 'invalid_cardinality' })
    expect(compareValidationMessage('invalid_cardinality')).toBe(
      '请选择 2–20 张 JPG 或 PNG 图片进行对比。',
    )
  })

  it('rejects duplicates and unsupported kinds', () => {
    expect(validateCompareCandidates([image(1), image(1)])).toEqual({
      ok: false,
      reason: 'duplicate_entity',
    })
    expect(
      validateCompareCandidates([image(1), { entityId: 'note', kind: 'text' }]),
    ).toEqual({ ok: false, reason: 'unsupported_type' })
  })
})
```

- [ ] **Step 2: Run the policy test and verify the missing module failure**

Run:

```bash
pnpm --dir ui test -- src/state/comparePolicy.test.ts
```

Expected: FAIL because `./comparePolicy` does not exist.

- [ ] **Step 3: Implement the shared policy**

Create `ui/src/state/comparePolicy.ts`:

```ts
export const MIN_COMPARE_IMAGES = 2
export const MAX_COMPARE_IMAGES = 20

export interface CompareCandidateLike {
  entityId: string
  kind: string
}

export type CompareValidationReason =
  | 'invalid_cardinality'
  | 'duplicate_entity'
  | 'unsupported_type'

export type CompareValidationResult =
  | { ok: true }
  | { ok: false; reason: CompareValidationReason }

export function isSupportedCompareKind(kind: string): boolean {
  return kind === 'jpeg' || kind === 'png'
}

export function validateCompareCandidates(
  candidates: readonly CompareCandidateLike[],
): CompareValidationResult {
  if (candidates.length < MIN_COMPARE_IMAGES || candidates.length > MAX_COMPARE_IMAGES) {
    return { ok: false, reason: 'invalid_cardinality' }
  }
  if (new Set(candidates.map((candidate) => candidate.entityId)).size !== candidates.length) {
    return { ok: false, reason: 'duplicate_entity' }
  }
  if (candidates.some((candidate) => !isSupportedCompareKind(candidate.kind))) {
    return { ok: false, reason: 'unsupported_type' }
  }
  return { ok: true }
}

export function compareValidationMessage(_reason: CompareValidationReason): string {
  return '请选择 2–20 张 JPG 或 PNG 图片进行对比。'
}
```

- [ ] **Step 4: Run the policy test and verify it passes**

Run:

```bash
pnpm --dir ui test -- src/state/comparePolicy.test.ts
```

Expected: PASS.

- [ ] **Step 5: Write failing model, reducer and radial capacity tests**

Update `ui/src/state/compareModel.test.ts` so the creation test accepts a
20-image array and rejects a 21-image array. Remove imports and expectations
for the fixed `compareLayout` function.

Add this reducer case to `ui/src/state/viewerReducer.test.ts`:

```ts
it('keeps up to 20 compare IDs and refuses 21 without truncating', () => {
  const twenty = Array.from({ length: 20 }, (_, index) => `image-${index}`)
  const accepted = viewerReducer(initialViewerState, {
    type: 'compare_context_changed',
    entityIds: twenty,
  })
  expect(accepted.compareEntityIds).toEqual(twenty)

  const rejected = viewerReducer(accepted, {
    type: 'compare_context_changed',
    entityIds: [...twenty, 'image-20'],
  })
  expect(rejected.compareEntityIds).toEqual(twenty)
})
```

Extend `ui/src/components/radialMenuModel.test.ts`:

```ts
it('enables compare at 20 and disables it above 20 with exact copy', () => {
  const twenty = buildRadialMenuModel(
    context({ selectedCount: 20, selectedImageCount: 20 }),
  )[4]
  expect(twenty).toMatchObject({ id: 'compare', disabled: false })

  const twentyOne = buildRadialMenuModel(
    context({ selectedCount: 21, selectedImageCount: 21 }),
  )[4]
  expect(twentyOne).toMatchObject({
    id: 'compare',
    disabled: true,
    disabledReason: '最多同时对比 20 张图片',
  })
})
```

Add a `RadialFileMenu` test that renders the 21-image model, asserts
`aria-disabled="true"` and the title, clicks it, presses Enter while another
enabled item owns focus, and verifies `onAction` never receives `compare`.

- [ ] **Step 6: Run the focused tests and verify the old four-image gates fail**

Run:

```bash
pnpm --dir ui test -- \
  src/state/compareModel.test.ts \
  src/state/viewerReducer.test.ts \
  src/components/radialMenuModel.test.ts \
  src/components/RadialFileMenu.test.tsx
```

Expected: FAIL on the existing `> 4`, `.slice(0, 4)` and `请选择 2–4 张图片`
behavior.

- [ ] **Step 7: Wire the shared policy through model, reducer and radial menu**

In `compareModel.ts`, replace the local cardinality/kind checks with
`validateCompareCandidates`. Keep transform creation unchanged.

In `workspaceReducer.ts`, remove `.slice(0, 4)` and refuse only over-capacity
context actions while still allowing `[]` and one-ID repair states:

```ts
case 'compare_context_changed': {
  const entityIds = unique(action.entityIds)
  if (entityIds.length > MAX_COMPARE_IMAGES) return state
  return { ...state, compareEntityIds: entityIds }
}
```

In `radialMenuModel.ts`, compute count reasons before the unsupported-kind
reason:

```ts
const compareDisabled =
  context.busy ||
  !context.compareContextAvailable ||
  context.selectedCount < MIN_COMPARE_IMAGES ||
  context.selectedCount > MAX_COMPARE_IMAGES ||
  context.selectedImageCount !== context.selectedCount

const compareDisabledReason = context.busy
  ? '请等待当前文件操作完成'
  : !context.compareContextAvailable
    ? '请先返回文件夹内容，再选择图片进行对比'
    : context.selectedCount > MAX_COMPARE_IMAGES
      ? '最多同时对比 20 张图片'
      : context.selectedCount < MIN_COMPARE_IMAGES
        ? '请选择 2–20 张图片'
        : context.selectedImageCount !== context.selectedCount
          ? '仅支持 JPG 或 PNG 图片'
          : undefined
```

Use `compareDisabledReason` on the `compare` menu item. Existing
`RadialFileMenu` `aria-disabled`, no-op `execute`, disabled shape and grayscale
styles remain the rendering mechanism.

- [ ] **Step 8: Write failing application-entry tests**

In `ui/src/App.test.tsx`:

1. Update existing invalid-copy assertions from `2–4` to `2–20`.
2. Add a 20-image content fixture, select all with the existing
   `全选当前文件夹` control or Meta+A, and verify keyboard `c` opens comparison.
3. Add a 21-image fixture, select all, open the radial menu and verify the
   compare item is disabled with title `最多同时对比 20 张图片`; clicking it
   must not open the compare region.

Use a count-parameterized fixture:

```ts
function compareContentWorkspaceWithCount(count: number) {
  const source = defined(contentWorkspace().images[0], 'Expected source image')
  return {
    workspace: 'content' as const,
    images: Array.from({ length: count }, (_, index) => ({
      ...source,
      entityId: `image-${index + 1}`,
      relativePath: `id/image-${index + 1}.jpg`,
      name: `image-${index + 1}.jpg`,
      modifiedNs: String(index + 1),
    })),
    textFiles: [],
  }
}
```

- [ ] **Step 9: Use shared validation in application and fallback copy**

Replace `matchesImage` cardinality logic in `openComparison` with
`validateCompareCandidates(files)`. On failure, call
`compareValidationMessage(validation.reason)`.

Update `CompareWorkspace` invalid feedback to use the same copy. Keep
`matchesImage` for selection/image counting where still needed.

- [ ] **Step 10: Run capacity and application tests**

Run:

```bash
pnpm --dir ui test -- \
  src/state/comparePolicy.test.ts \
  src/state/compareModel.test.ts \
  src/state/viewerReducer.test.ts \
  src/components/radialMenuModel.test.ts \
  src/components/RadialFileMenu.test.tsx \
  src/App.test.tsx
```

Expected: PASS.

- [ ] **Step 11: Commit the capacity contract**

```bash
git add \
  ui/src/state/comparePolicy.ts \
  ui/src/state/comparePolicy.test.ts \
  ui/src/state/compareModel.ts \
  ui/src/state/compareModel.test.ts \
  ui/src/state/reducers/workspaceReducer.ts \
  ui/src/state/viewerReducer.test.ts \
  ui/src/components/radialMenuModel.ts \
  ui/src/components/radialMenuModel.test.ts \
  ui/src/components/RadialFileMenu.test.tsx \
  ui/src/App.tsx \
  ui/src/App.test.tsx \
  ui/src/components/CompareWorkspace.tsx
git commit -m "feat: raise comparison capacity to twenty"
```

---

### Task 2: Build the Pure Intelligent Layout Engine

**Files:**
- Create: `ui/src/components/compareLayoutEngine.ts`
- Create: `ui/src/components/compareLayoutEngine.test.ts`
- Test: `ui/src/components/compareLayoutEngine.test.ts`

**Interfaces:**
- Consumes:
  - ordered `{ entityId, aspectRatio }` items;
  - quarter-rotation-adjusted ratios supplied by the caller;
  - workspace width/height.
- Produces:
  - `CompareLayoutKind`
  - `CompareLayoutPlan`
  - `solveCompareLayout(input)`
  - `visibleCompareIndexes(plan, scrollLeft, scrollTop, retainedEntityId?)`
  - `anchoredCompareScrollOffset(previous, next, entityId, previousOffset)`
  - exported exact constants from the approved design.

- [ ] **Step 1: Write the failing public-contract and representative-layout tests**

Create `compareLayoutEngine.test.ts` with fixture builders:

```ts
import { describe, expect, it } from 'vitest'
import {
  MAX_LAYOUT_CANDIDATES,
  solveCompareLayout,
  type CompareLayoutInput,
} from './compareLayoutEngine'

const items = (count: number, ratio: number) =>
  Array.from({ length: count }, (_, index) => ({
    entityId: `image-${index}`,
    aspectRatio: ratio,
  }))

const input = (
  comparedItems: CompareLayoutInput['items'],
  overrides: Partial<CompareLayoutInput> = {},
): CompareLayoutInput => ({
  width: 1_700,
  height: 900,
  gap: 6,
  padding: 6,
  paneChromeHeight: 70,
  items: comparedItems,
  previous: null,
  ...overrides,
})

describe('solveCompareLayout', () => {
  it('puts four portrait images in one readable row', () => {
    const plan = solveCompareLayout(input(items(4, 0.75)))
    expect(plan.kind).toBe('fit-row')
    expect(plan.scrollAxis).toBe('none')
    expect(plan.rects).toHaveLength(4)
    expect(plan.candidateCount).toBeLessThanOrEqual(MAX_LAYOUT_CANDIDATES)
  })

  it('uses a readable grid for four landscapes', () => {
    const plan = solveCompareLayout(input(items(4, 1.5)))
    expect(plan.kind).toBe('fit-grid')
    expect(plan.columns).toBe(2)
  })

  it('falls back to a horizontal strip for six portraits', () => {
    const plan = solveCompareLayout(input(items(6, 0.75)))
    expect(plan.kind).toBe('horizontal-strip')
    expect(plan.scrollAxis).toBe('horizontal')
    expect(plan.totalWidth).toBeGreaterThan(1_700)
  })

  it('uses a two-column vertical flow for overflowing landscapes', () => {
    const plan = solveCompareLayout(input(items(8, 1.5)))
    expect(plan.kind).toBe('vertical-flow')
    expect(plan.columns).toBe(2)
    expect(plan.totalHeight).toBeGreaterThan(900)
  })
})
```

- [ ] **Step 2: Run the solver test and verify the missing module failure**

Run:

```bash
pnpm --dir ui test -- src/components/compareLayoutEngine.test.ts
```

Expected: FAIL because the engine does not exist.

- [ ] **Step 3: Define exact engine types and constants**

Create `compareLayoutEngine.ts` with these public definitions:

```ts
export const MIN_READABLE_AREA = 120_000
export const MIN_READABLE_LONG_EDGE = 360
export const LAYOUT_SWITCH_GAIN = 0.12
export const LANDSCAPE_SINGLE_COLUMN_BREAKPOINT = 900
export const MAX_LAYOUT_CANDIDATES = 6
export const PORTRAIT_RATIO_MAX = 0.9
export const LANDSCAPE_RATIO_MIN = 1.1

export type CompareLayoutKind =
  | 'fit-row'
  | 'fit-column'
  | 'fit-grid'
  | 'horizontal-strip'
  | 'vertical-flow'
  | 'safe-column'

export interface CompareLayoutSource {
  entityId: string
  aspectRatio: number
}

export interface CompareLayoutRect {
  entityId: string
  index: number
  left: number
  top: number
  width: number
  height: number
  stageWidth: number
  stageHeight: number
}

export interface PreviousCompareLayout {
  key: string
  kind: CompareLayoutKind
  score: number
  eligible: boolean
}

export interface CompareLayoutInput {
  width: number
  height: number
  gap: number
  padding: number
  paneChromeHeight: number
  items: readonly CompareLayoutSource[]
  previous: PreviousCompareLayout | null
}

export interface CompareLayoutPlan extends PreviousCompareLayout {
  scrollAxis: 'none' | 'horizontal' | 'vertical'
  columns: number
  rows: number
  rects: CompareLayoutRect[]
  viewportWidth: number
  viewportHeight: number
  totalWidth: number
  totalHeight: number
  candidateCount: number
  retainedPrevious: boolean
}
```

Use `safeRatio(value)` to map missing/non-finite/non-positive ratios to `1`.
Keep every arithmetic result finite and nonnegative.

- [ ] **Step 4: Implement fit simulation and bounded candidates**

Generate deduplicated column counts `[1, 2, 3, 4, itemCount]`, discard values
outside `1..itemCount`, and therefore keep the candidate count at five or less.
For each candidate:

```ts
const rows = Math.ceil(items.length / columns)
const cellWidth = (width - 2 * padding - (columns - 1) * gap) / columns
const cellHeight = (height - 2 * padding - (rows - 1) * gap) / rows
const stageHeight = Math.max(1, cellHeight - paneChromeHeight)

const displayWidth = Math.min(cellWidth, stageHeight * ratio)
const displayHeight = Math.min(stageHeight, cellWidth / ratio)
const displayArea = displayWidth * displayHeight
```

Center the final partial row using:

```ts
const rowItemCount = Math.min(columns, items.length - rowStart)
const rowWidth = rowItemCount * cellWidth + Math.max(0, rowItemCount - 1) * gap
const rowLeft = padding + Math.max(0, (width - 2 * padding - rowWidth) / 2)
```

Classify `columns === items.length` as `fit-row`, `columns === 1` as
`fit-column`, and the rest as `fit-grid`.

- [ ] **Step 5: Implement readability, normalized scoring and hysteresis**

Simulate each source once in a single full-workspace stage. Calculate:

```ts
const normalizedAreas = displays.map(
  (display, index) => display.area / singlePaneDisplays[index]!.area,
)
const minimumNormalizedArea = Math.min(...normalizedAreas)
const meanNormalizedArea =
  normalizedAreas.reduce((sum, value) => sum + value, 0) / normalizedAreas.length
const fill =
  displays.reduce((sum, display) => sum + display.area, 0) /
  displays.reduce((sum, display) => sum + display.stageArea, 0)
const continuity = previous?.key === candidate.key ? 1 : 0
const score =
  0.45 * minimumNormalizedArea +
  0.25 * meanNormalizedArea +
  0.20 * fill +
  0.10 * continuity
```

A candidate is eligible only when every display has area at least
`MIN_READABLE_AREA` and long edge at least `MIN_READABLE_LONG_EDGE`.

Choose the highest eligible score. Retain the previous no-scroll candidate key
only when it remains eligible and the new score is less than
`previous.score * (1 + LAYOUT_SWITCH_GAIN)`. Geometry for the retained key must
still be rebuilt at the new width/height. A valid no-scroll candidate always
wins over a scrolling fallback; when none is eligible, the hard orientation
rules below choose exactly one scrolling family, so no score comparison is
performed across the fit/scroll hard-constraint boundary.

- [ ] **Step 6: Implement scroll-family selection and geometry**

Use these exact rules:

```ts
const classes = items.map(({ aspectRatio }) => classifyRatio(aspectRatio))
const allPortrait = classes.every((value) => value === 'portrait')
const allNonPortrait = classes.every((value) => value !== 'portrait')

if (allPortrait) return buildHorizontalStrip(...)
if (allNonPortrait) return buildVerticalFlow(...)
return buildHorizontalStrip(...)
```

For a horizontal strip:

- common stage height is `height - 2 * padding - paneChromeHeight`;
- card width is `clamp(stageHeight * ratio, 320, width * 0.75)`;
- item offsets preserve source order and fractional CSS pixels;
- `totalWidth` includes both inline paddings.

For a vertical flow:

- columns are `width < 900 ? 1 : 2`;
- consecutive items form each row;
- column width is equal and the last item does not span;
- each row stage height is the maximum contained-image height needed by its
  row items, capped at `height - 2 * padding - paneChromeHeight`;
- row offsets remain monotonic and `totalHeight` includes block padding.

If the workspace is invalid, return finite `safe-column` geometry with one CSS
pixel minimum dimensions.

- [ ] **Step 7: Add threshold, rotation-input, hysteresis and invalid-number tests**

Extend the engine tests with:

```ts
it('switches only after a twelve-percent score gain', () => {
  const first = solveCompareLayout(input(items(4, 0.75)))
  const retained = solveCompareLayout(
    input(items(4, 0.75), {
      width: 1_660,
      previous: first,
    }),
  )
  expect(retained.key).toBe(first.key)
  expect(retained.retainedPrevious).toBe(true)
})

it.each([Number.NaN, Number.POSITIVE_INFINITY, 0, -1])(
  'returns finite safe geometry for invalid dimension %s',
  (width) => {
    const plan = solveCompareLayout(input(items(2, 1), { width }))
    expect(plan.kind).toBe('safe-column')
    expect(Number.isFinite(plan.totalWidth)).toBe(true)
    expect(Number.isFinite(plan.totalHeight)).toBe(true)
  },
)
```

Also cover:

- ratios `0.89`, `0.90`, `1.10`, `1.11`;
- exact 120,000-area and 360-long-edge boundaries;
- mixed ratios retaining entity order;
- below-900 single-column vertical flow;
- 20 items with `candidateCount <= 6`;
- current ineligible candidate switching without hysteresis.

- [ ] **Step 8: Add visible-range and anchor tests before implementation**

Define expected behavior for:

```ts
visibleCompareIndexes(plan, scrollLeft, scrollTop, retainedEntityId?)
anchoredCompareScrollOffset(previous, next, entityId, previousOffset)
```

Tests must cover start/middle/end, one viewport overscan, one retained
off-window entity, horizontal and vertical axes, and a missing anchor returning
the prior offset.

- [ ] **Step 9: Implement binary-search visibility and anchoring**

Use monotonic `left/right` bounds for horizontal plans and `top/bottom` bounds
for vertical plans. The mounted array is sorted by source index after adding
the retained entity. It may contain only one index outside the visible plus
overscan interval.

Anchor using:

```ts
nextOffset = nextPosition + (previousOffset - previousPosition)
```

Clamp to `0..max(0, nextExtent - viewportExtent)`.

- [ ] **Step 10: Run solver tests and UI static checks**

Run:

```bash
pnpm --dir ui test -- src/components/compareLayoutEngine.test.ts
pnpm --dir ui check
```

Expected: PASS.

- [ ] **Step 11: Commit the pure engine**

```bash
git add \
  ui/src/components/compareLayoutEngine.ts \
  ui/src/components/compareLayoutEngine.test.ts
git commit -m "feat: add intelligent compare layout solver"
```

---

### Task 3: Measure the Workspace and Stabilize Resize Updates

**Files:**
- Create: `ui/src/components/useCompareLayout.ts`
- Create: `ui/src/components/useCompareLayout.test.tsx`
- Test: `ui/src/components/useCompareLayout.test.tsx`

**Interfaces:**
- Consumes:
  - `files: readonly BrowserFile[]`
  - `rotations: Readonly<Record<string, QuarterRotation>>`
  - recovered dimensions keyed by file source revision
  - `solveCompareLayout`
- Produces:
  - `UseCompareLayoutResult { containerRef: RefObject<HTMLDivElement | null>; plan: CompareLayoutPlan }`
  - `effectiveCompareRatio(file, rotation, recovered): number`
  - `compareSourceRevision(file): string`
- State guarantee: preserves the previous `CompareLayoutPlan` across resize
  notifications.

- [ ] **Step 1: Write failing ratio and frame-coalescing hook tests**

Create `useCompareLayout.test.tsx` using `renderHook` and a local
ResizeObserver/rAF harness:

```tsx
it('uses metadata and quarter rotation to solve the layout', () => {
  const resize = installResizeObserver()
  const { result } = renderHook(() =>
    useCompareLayout({
      files: portraitFiles(4),
      rotations: { a: 0, b: 0, c: 0, d: 0 },
      recoveredDimensions: {},
    }),
  )
  attachRef(result.current.containerRef)
  act(() => resize(1_700, 900))
  expect(result.current.plan.kind).toBe('fit-row')
})

it('coalesces multiple ResizeObserver notifications into one frame', () => {
  const frames = installAnimationFrameQueue()
  const resize = installResizeObserver()
  const { result } = renderHook(() =>
    useCompareLayout({
      files: portraitFiles(4),
      rotations: {},
      recoveredDimensions: {},
    }),
  )
  attachRef(result.current.containerRef)
  act(() => {
    resize(1_600, 900)
    resize(1_650, 900)
    resize(1_700, 900)
  })
  expect(frames.pending()).toBe(1)
  act(() => frames.flush())
  expect(result.current.plan.totalWidth).toBe(1_700)
})
```

Add tests for:

- missing metadata without recovered dimensions using ratio 1;
- matching recovered dimensions replacing the ratio-1 fallback;
- a stale recovered source revision being ignored;
- rotation 90 inverting metadata or recovered `width/height`;
- a recovered-ratio update causing exactly one controlled re-solve.

- [ ] **Step 2: Run the hook test and verify the missing module failure**

Run:

```bash
pnpm --dir ui test -- src/components/useCompareLayout.test.tsx
```

Expected: FAIL because `useCompareLayout` does not exist.

- [ ] **Step 3: Implement the measurement hook**

Use this public shape:

```ts
export interface UseCompareLayoutInput {
  files: readonly BrowserFile[]
  rotations: Readonly<Record<string, QuarterRotation>>
  recoveredDimensions: Readonly<Record<string, RecoveredCompareDimensions | undefined>>
}

export interface RecoveredCompareDimensions {
  sourceRevision: string
  width: number
  height: number
}

export interface UseCompareLayoutResult {
  containerRef: RefObject<HTMLDivElement | null>
  plan: CompareLayoutPlan
}

export function compareSourceRevision(file: BrowserFile): string {
  return `${file.entityId}:${file.modifiedNs}:${file.size}`
}
```

Implementation rules:

1. Observe only `containerRef.current`.
2. Store the latest finite positive size in a ref.
3. Schedule at most one `requestAnimationFrame`.
4. Build sources with the current file order.
5. Call `solveCompareLayout` with the previous valid plan.
6. Commit state only when `kind`, rect geometry, extent or score changed.
7. Cancel a pending frame and disconnect the observer on unmount.
8. When ResizeObserver is absent, read the one outer
   `getBoundingClientRect()` and use safe solver geometry if it is zero.

Build effective ratios with:

```ts
export function effectiveCompareRatio(
  file: BrowserFile,
  rotation: QuarterRotation,
  recovered: RecoveredCompareDimensions | undefined,
): number {
  const sourceRevision = compareSourceRevision(file)
  const recoveredMatches =
    recovered !== undefined && recovered.sourceRevision === sourceRevision
  const width = file.imageMetadata?.width ?? (recoveredMatches ? recovered.width : undefined)
  const height =
    file.imageMetadata?.height ?? (recoveredMatches ? recovered.height : undefined)
  if (
    width === undefined ||
    height === undefined ||
    !Number.isFinite(width) ||
    !Number.isFinite(height) ||
    width <= 0 ||
    height <= 0
  ) {
    return 1
  }
  const ratio = width / height
  return rotation === 90 || rotation === 270 ? 1 / ratio : ratio
}
```

Use the exact geometry tokens `gap: 6`, `padding: 6` and
`paneChromeHeight: 70` in one exported constants object so CSS integration
tests can guard against drift.

- [ ] **Step 4: Run hook and engine tests**

Run:

```bash
pnpm --dir ui test -- \
  src/components/compareLayoutEngine.test.ts \
  src/components/useCompareLayout.test.tsx
```

Expected: PASS.

- [ ] **Step 5: Commit the measurement hook**

```bash
git add \
  ui/src/components/useCompareLayout.ts \
  ui/src/components/useCompareLayout.test.tsx
git commit -m "feat: measure intelligent compare layouts"
```

---

### Task 4: Virtualize Horizontal and Vertical Comparison Scrolling

**Files:**
- Create: `ui/src/components/CompareVirtualViewport.tsx`
- Create: `ui/src/components/CompareVirtualViewport.test.tsx`
- Test: `ui/src/components/CompareVirtualViewport.test.tsx`

**Interfaces:**
- Consumes:
  - `plan: CompareLayoutPlan` whose axis is horizontal or vertical;
  - ordered entity IDs already represented by `plan.rects`;
  - `activeEntityId`;
  - `renderItem(entityId, index): ReactNode`;
  - `onActivate(entityId)`.
- Produces:
  - a `role="list"` virtual scroll region;
  - `role="listitem"` wrappers with `aria-posinset`/`aria-setsize`;
  - bounded calls to `renderItem`;
  - active-item focus and scroll anchoring.

- [ ] **Step 1: Write failing bounded-mount tests**

Create `CompareVirtualViewport.test.tsx`:

```tsx
it('mounts only the horizontal visible window, overscan and active item', () => {
  const plan = horizontalPlanWithTwentyItems()
  const renderItem = vi.fn((entityId: string) => <button>{entityId}</button>)
  render(
    <CompareVirtualViewport
      plan={plan}
      activeEntityId="image-19"
      renderItem={renderItem}
      onActivate={vi.fn()}
    />,
  )

  expect(renderItem.mock.calls.length).toBeLessThan(20)
  expect(screen.getByRole('listitem', { name: 'image-19' })).toBeInTheDocument()
  expect(screen.getByRole('listitem', { name: 'image-19' })).toHaveAttribute(
    'aria-setsize',
    '20',
  )
})
```

Add a vertical-flow equivalent and a rerender test proving an item leaving the
window unmounts.

- [ ] **Step 2: Run the component test and verify the missing module failure**

Run:

```bash
pnpm --dir ui test -- src/components/CompareVirtualViewport.test.tsx
```

Expected: FAIL because the component does not exist.

- [ ] **Step 3: Implement the virtual track and bounded render window**

Use this exact prop contract:

```tsx
export interface CompareVirtualViewportProps {
  plan: CompareLayoutPlan
  activeEntityId: string
  renderItem: (entityId: string, index: number) => ReactNode
  onActivate: (entityId: string) => void
}
```

Render:

```tsx
<div
  ref={viewportRef}
  role="list"
  aria-label="滚动图片对比"
  className="compare-scroll-viewport"
  data-axis={plan.scrollAxis}
  onScroll={scrolled}
  onWheel={wheel}
  onKeyDown={keyDown}
>
  <div
    className="compare-scroll-track"
    style={{ width: plan.totalWidth, height: plan.totalHeight }}
  >
    {mountedIndexes.map((index) => {
      const rect = plan.rects[index]
      if (rect === undefined) throw new Error(`Missing compare rect ${index}`)
      return (
        <div
          key={rect.entityId}
          role="listitem"
          aria-label={rect.entityId}
          aria-posinset={index + 1}
          aria-setsize={plan.rects.length}
          data-compare-entity-id={rect.entityId}
          className="compare-layout-item"
          style={{ left: rect.left, top: rect.top, width: rect.width, height: rect.height }}
          onFocusCapture={() => onActivate(rect.entityId)}
        >
          {renderItem(rect.entityId, index)}
        </div>
      )
    })}
  </div>
</div>
```

Calculate `mountedIndexes` with `visibleCompareIndexes`, current scroll offsets
and `activeEntityId`.

- [ ] **Step 4: Write failing wheel, keyboard and anchor tests**

Cover:

- plain `deltaY` moves `scrollLeft` in a horizontal plan and calls
  `preventDefault` only when more scrolling is possible;
- wheel at the start/end edge is released;
- vertical plans do not translate the wheel;
- ArrowLeft/Right selects neighbors in a strip;
- ArrowUp/Down moves by `plan.columns` in a vertical flow;
- rerendering a changed plan keeps the active image at its prior visual offset.

- [ ] **Step 5: Implement wheel, keyboard and plan-change anchoring**

Horizontal wheel logic:

```ts
const desired = event.deltaX !== 0 ? event.deltaX : event.deltaY
const maximum = Math.max(0, node.scrollWidth - node.clientWidth)
const next = clamp(node.scrollLeft + desired, 0, maximum)
if (next !== node.scrollLeft) {
  event.preventDefault()
  node.scrollLeft = next
  setScrollLeft(next)
}
```

On plan change, call `anchoredCompareScrollOffset` for the active entity inside
`useLayoutEffect`, assign the appropriate scroll property and update local
state.

Keyboard navigation computes the next source index, calls `onActivate`, and
after rerender focuses:

```ts
viewportRef.current
  ?.querySelector<HTMLElement>(`[data-compare-entity-id="${CSS.escape(entityId)}"] article`)
  ?.focus()
```

If `CSS.escape` is unavailable in jsdom, use a lookup over
`[data-compare-entity-id]` attributes rather than interpolating an unsafe
selector.

- [ ] **Step 6: Run virtual viewport tests and static checks**

Run:

```bash
pnpm --dir ui test -- \
  src/components/compareLayoutEngine.test.ts \
  src/components/CompareVirtualViewport.test.tsx
pnpm --dir ui check
```

Expected: PASS.

- [ ] **Step 7: Commit scrolling virtualization**

```bash
git add \
  ui/src/components/CompareVirtualViewport.tsx \
  ui/src/components/CompareVirtualViewport.test.tsx
git commit -m "feat: virtualize scrolling comparisons"
```

---

### Task 5: Integrate Plans into CompareWorkspace and Replace Fixed CSS

**Files:**
- Modify: `ui/src/components/CompareWorkspace.tsx`
- Modify: `ui/src/components/CompareWorkspace.test.tsx`
- Modify: `ui/src/components/ComparePane.tsx`
- Modify: `ui/src/components/ComparePane.test.tsx`
- Modify: `ui/src/state/compareModel.ts`
- Modify: `ui/src/state/compareModel.test.ts`
- Modify: `ui/src/styles/app.css`
- Modify: `ui/src/styles/app.test.ts`
- Test: `ui/src/components/CompareWorkspace.test.tsx`
- Test: `ui/src/components/ComparePane.test.tsx`
- Test: `ui/src/state/compareModel.test.ts`
- Test: `ui/src/styles/app.test.ts`

**Interfaces:**
- Consumes:
  - `useCompareLayout({ files, rotations, recoveredDimensions })`
  - `CompareVirtualViewport`
  - `CompareLayoutPlan.rects`
  - existing `ComparePane` props and compare reducer actions.
- Produces:
  - `data-layout` with the new `CompareLayoutKind`;
  - plan-driven fit surfaces;
  - virtualized scrolling surfaces;
  - no remaining `compareLayout(state)` dependency.

- [ ] **Step 1: Replace fixed-layout tests with failing aspect-aware workspace tests**

In `CompareWorkspace.test.tsx`, replace
`uses two-column, asymmetric-three and four-grid layouts` with:

```tsx
it('uses one row for four portraits in a wide workspace', () => {
  const resize = installCompareResizeObserver()
  renderWorkspace({ files: portraitFiles(4) })
  act(() => resize.workspace(1_700, 900))
  expect(screen.getByRole('region', { name: '图片对比' })).toHaveAttribute(
    'data-layout',
    'fit-row',
  )
})

it('uses a grid for four landscapes', () => {
  const resize = installCompareResizeObserver()
  renderWorkspace({ files: landscapeFiles(4) })
  act(() => resize.workspace(1_700, 900))
  expect(screen.getByRole('region', { name: '图片对比' })).toHaveAttribute(
    'data-layout',
    'fit-grid',
  )
})

it('virtualizes a scrolling portrait set', () => {
  const resize = installCompareResizeObserver()
  const requestImage = vi.fn(() => new Promise<ImageRepresentation>(() => undefined))
  renderWorkspace({ files: portraitFiles(20), requestImage })
  act(() => resize.workspace(1_700, 900))
  expect(screen.getByRole('list', { name: '滚动图片对比' })).toHaveAttribute(
    'data-axis',
    'horizontal',
  )
  expect(requestImage.mock.calls.length).toBeLessThan(20)
})
```

The ResizeObserver harness must identify the observed outer layout region
separately from pane stages instead of assuming only one observer exists.

- [ ] **Step 2: Run workspace tests and verify fixed layout failures**

Run:

```bash
pnpm --dir ui test -- src/components/CompareWorkspace.test.tsx
```

Expected: FAIL because the workspace still emits `two_columns`,
`three_asymmetric` and `four_grid`.

- [ ] **Step 3: Remove the fixed layout function from the compare model**

Delete:

```ts
export type CompareLayout = 'two_columns' | 'three_asymmetric' | 'four_grid'
export function compareLayout(state: CompareState): CompareLayout
```

Remove model tests that assert fixed cardinality layouts. Keep all transform,
rotation, pan, zoom, actual-size and reconciliation tests. Extend the
reconciliation fixture to 20 items and verify surviving entity state is
retained. Add a focused removal test: when the active entity is removed,
choose the surviving entity with the smallest distance from the removed
entity's prior source index; on an equal-distance tie choose the following
entity, then the preceding entity. Implement that rule in
`reconcileComparePanes` using the previous `state.entityIds` before rebuilding
the remaining entity map.

- [ ] **Step 4: Add the layout hook to CompareWorkspace**

Build rotations without changing transform ownership:

```ts
const rotations = Object.fromEntries(
  model.entityIds.map((entityId) => [
    entityId,
    model.transforms[entityId]?.rotation ?? 0,
  ]),
)
const [recoveredDimensions, setRecoveredDimensions] = useState<
  Record<string, RecoveredCompareDimensions | undefined>
>({})
const { containerRef, plan } = useCompareLayout({
  files,
  rotations,
  recoveredDimensions,
})
```

Precompute `filesById` once with `useMemo`. Extract one `renderPane(entityId)`
function that contains the existing `ComparePane` props so fit and scrolling
paths cannot diverge. Extend the existing `updateMetrics(entityId, metrics)`
path so it also stores natural image dimensions with the current source
revision:

```ts
const file = filesById.get(entityId)
if (file !== undefined && metrics.imageWidth > 0 && metrics.imageHeight > 0) {
  const sourceRevision = compareSourceRevision(file)
  setRecoveredDimensions((current) => {
    const previous = current[entityId]
    if (
      previous?.sourceRevision === sourceRevision &&
      previous.width === metrics.imageWidth &&
      previous.height === metrics.imageHeight
    ) {
      return current
    }
    return {
      ...current,
      [entityId]: {
        sourceRevision,
        width: metrics.imageWidth,
        height: metrics.imageHeight,
      },
    }
  })
}
```

The hook ignores entries whose `sourceRevision` does not match the current
file. Prune entries for removed entity IDs in the same reconciliation effect.
This permits at most one ratio correction after a mounted image reports its
natural size and prevents stale metadata from moving a replaced file.

Replace the old `.compare-pane-grid` with:

```tsx
<div ref={containerRef} className="compare-layout-region">
  {plan.scrollAxis === 'none' ? (
    <div
      role="list"
      aria-label="全部图片对比"
      className="compare-fit-layout"
      data-testid="compare-fit-layout"
    >
      {plan.rects.map((rect, index) => (
        <div
          key={rect.entityId}
          role="listitem"
          aria-posinset={index + 1}
          aria-setsize={plan.rects.length}
          className="compare-layout-item"
          style={{
            left: rect.left,
            top: rect.top,
            width: rect.width,
            height: rect.height,
          }}
        >
          {renderPane(rect.entityId)}
        </div>
      ))}
    </div>
  ) : (
    <CompareVirtualViewport
      plan={plan}
      activeEntityId={activeEntityId}
      onActivate={(entityId) => update({ type: 'active_changed', entityId })}
      renderItem={(entityId) => renderPane(entityId)}
    />
  )}
</div>
```

Set `data-layout={plan.kind}` on the workspace. The toolbar stays outside the
measured region.

- [ ] **Step 5: Preserve original requests and active panes across virtualization**

Keep `activeEntityId` in the virtual mounted set. When an active pane changes:

- clear `originalEntityId` only if the existing code already does so;
- keep the original request lane serialization;
- rely on `ComparePane` cleanup to abort an unmounted fit/original request;
- ignore `AbortError`-equivalent virtual unmounts rather than showing
  `无法预览该图片`.

Add this `ComparePane` helper and use it in both proxy and original request
rejection handlers:

```ts
function requestWasAborted(caught: unknown, signal: AbortSignal): boolean {
  return signal.aborted || (caught instanceof DOMException && caught.name === 'AbortError')
}
```

An aborted effect cleanup must not set `proxyError` or call
`onOriginalUnavailable`.

Add a workspace regression test where one mounted image request rejects with a
non-abort error. Assert the pane keeps its planned width/height, renders the
existing error state, and neighboring pane rectangles do not move. Add a pane
test where unmount aborts both pending request kinds and produces neither
error UI nor `onOriginalUnavailable`.

- [ ] **Step 6: Replace fixed comparison CSS**

Remove:

```css
.compare-pane-grid
.compare-layout-two_columns .compare-pane-grid
.compare-layout-three_asymmetric .compare-pane-grid
.compare-layout-four_grid .compare-pane-grid
.compare-layout-three_asymmetric .compare-pane:first-child
```

Add:

```css
.compare-layout-region {
  background: var(--preview-surface);
  flex: 1;
  min-height: 0;
  overflow: hidden;
  position: relative;
}

.compare-fit-layout,
.compare-scroll-viewport {
  height: 100%;
  position: relative;
  width: 100%;
}

.compare-fit-layout {
  overflow: hidden;
}

.compare-scroll-viewport {
  overscroll-behavior: contain;
}

.compare-scroll-viewport[data-axis="horizontal"] {
  overflow-x: auto;
  overflow-y: hidden;
}

.compare-scroll-viewport[data-axis="vertical"] {
  overflow-x: hidden;
  overflow-y: auto;
}

.compare-scroll-track {
  min-height: 100%;
  min-width: 100%;
  position: relative;
}

.compare-layout-item {
  contain: layout paint;
  position: absolute;
}

.compare-layout-item > .compare-pane {
  height: 100%;
  width: 100%;
}
```

Keep all existing light preview surface, chrome, panel, border, accent and
control tokens.

- [ ] **Step 7: Update CSS contract tests before running them**

In `app.test.ts`, remove fixed class assertions and add exact checks for:

- `.compare-layout-region` `overflow: hidden` and `position: relative`;
- horizontal/vertical axis overflow;
- `.compare-layout-item` `contain: layout paint`;
- `.compare-pane-stage` remains `object-fit: contain`/`overflow: hidden`;
- existing light preview token use remains unchanged;
- radial disabled rules still use grayscale/opacity in light mode.

- [ ] **Step 8: Run workspace, pane, model and CSS tests**

Run:

```bash
pnpm --dir ui test -- \
  src/components/ComparePane.test.tsx \
  src/components/CompareWorkspace.test.tsx \
  src/components/CompareVirtualViewport.test.tsx \
  src/state/compareModel.test.ts \
  src/styles/app.test.ts
```

Expected: PASS.

- [ ] **Step 9: Run the complete UI check and test suite**

Run:

```bash
pnpm --dir ui check
pnpm --dir ui test
pnpm --dir ui build
```

Expected: all commands PASS.

- [ ] **Step 10: Commit workspace integration**

```bash
git add \
  ui/src/components/ComparePane.tsx \
  ui/src/components/ComparePane.test.tsx \
  ui/src/components/CompareWorkspace.tsx \
  ui/src/components/CompareWorkspace.test.tsx \
  ui/src/state/compareModel.ts \
  ui/src/state/compareModel.test.ts
git add -p ui/src/styles/app.css ui/src/styles/app.test.ts
git diff --cached --stat
git diff --cached -- ui/src/styles/app.css ui/src/styles/app.test.ts
git commit -m "feat: integrate intelligent comparison layouts"
```

For the two pre-existing dirty style files, answer `y` only for the new
comparison-layout hunks and `n` for all unrelated user hunks. Before committing,
the cached style diff must contain the fixed-to-intelligent comparison CSS/test
replacement only; `FolderTree`, `VirtualList`, and unrelated style changes must
remain unstaged.

---

### Task 6: Verify Performance, Accessibility and End-to-End Acceptance

**Files:**
- Create: `ui/src/components/compareLayoutBenchmark.test.ts`
- Modify: `ui/src/App.test.tsx`
- Modify: `ui/src/components/CompareWorkspace.test.tsx`
- Modify: `ui/src/components/CompareVirtualViewport.test.tsx`
- Modify: `ui/src/components/RadialFileMenu.test.tsx`
- Test: `ui/src/components/compareLayoutBenchmark.test.ts`
- Test: `ui/src/App.test.tsx`
- Test: all UI and repository quality suites.

**Interfaces:**
- Consumes all production interfaces from Tasks 1–5.
- Produces no new production API; it locks performance and acceptance behavior.

- [ ] **Step 1: Add deterministic complexity and opt-in timing tests**

Create `compareLayoutBenchmark.test.ts`:

```ts
import { describe, expect, it } from 'vitest'
import { solveCompareLayout } from './compareLayoutEngine'

const twenty = (ratio: number) =>
  Array.from({ length: 20 }, (_, index) => ({
    entityId: `image-${index}`,
    aspectRatio: ratio,
  }))

const solve = (
  items: ReadonlyArray<{ entityId: string; aspectRatio: number }>,
) =>
  solveCompareLayout({
    width: 1_700,
    height: 900,
    gap: 6,
    padding: 6,
    paneChromeHeight: 70,
    items,
    previous: null,
  })

describe('compare layout performance', () => {
  it.each([0.75, 1, 1.5])('keeps candidate work bounded for ratio %s', (ratio) => {
    const plan = solve(twenty(ratio))
    expect(plan.candidateCount).toBeLessThanOrEqual(6)
    expect(plan.rects).toHaveLength(20)
  })

  it.runIf(process.env.VIEWER_COMPARE_BENCH === '1')(
    'solves a mixed 20-image set below the current-machine 2 ms target',
    () => {
      const mixed = Array.from({ length: 20 }, (_, index) => ({
        entityId: `image-${index}`,
        aspectRatio: index % 2 === 0 ? 0.75 : 1.5,
      }))
      for (let index = 0; index < 100; index += 1) solve(mixed)
      const start = performance.now()
      for (let index = 0; index < 1_000; index += 1) solve(mixed)
      const average = (performance.now() - start) / 1_000
      expect(average).toBeLessThan(2)
    },
  )
})
```

The timing test is opt-in because CI hardware is not a stable wall-clock
contract.

- [ ] **Step 2: Add final accessibility and resource-bound assertions**

Extend component tests to assert:

- compare fit/scroll collections have accessible names;
- every mounted wrapper exposes correct `aria-posinset` and `aria-setsize`;
- Arrow navigation activates and focuses the correct logical neighbor;
- a 20-image horizontal set mounts fewer than 20 panes at the representative
  viewport;
- request count equals mounted panes, not selected count;
- scrolling to the end unmounts old panes and aborts their pending request;
- the active pane remains mounted as at most one extra item;
- failed/corrupt image content keeps its planned rectangle and does not
  collapse or move neighboring panes;
- removing the active image transfers activity to the nearest surviving
  source-order neighbor;
- the 21-image radial action has `aria-disabled="true"`, a disabled title,
  grey CSS contract and never calls `onAction`.

- [ ] **Step 3: Add final application acceptance cases**

In `App.test.tsx`, retain the existing compare shortcut, read-only, marker,
search close and surviving-single-preview cases. Add:

1. 4 portrait files produce `data-layout="fit-row"`.
2. 20 supported images can enter comparison.
3. 21 selected images leave the radial action inactive.
4. Closing comparison restores the underlying 20-image selection.

- [ ] **Step 4: Run the focused performance and acceptance suites**

Run:

```bash
pnpm --dir ui test -- \
  src/components/compareLayoutBenchmark.test.ts \
  src/components/compareLayoutEngine.test.ts \
  src/components/useCompareLayout.test.tsx \
  src/components/CompareVirtualViewport.test.tsx \
  src/components/CompareWorkspace.test.tsx \
  src/components/RadialFileMenu.test.tsx \
  src/App.test.tsx
```

Expected: PASS with the timing test skipped unless
`VIEWER_COMPARE_BENCH=1`.

- [ ] **Step 5: Run the current-machine benchmark**

Run:

```bash
VIEWER_COMPARE_BENCH=1 pnpm --dir ui test -- \
  src/components/compareLayoutBenchmark.test.ts
```

Expected: PASS with average mixed 20-image solve time below 2 ms. If this
specific machine misses the target, capture the measured average, profile the
pure solver, remove repeated allocation from candidate loops, and rerun until
the target passes without weakening functional assertions.

- [ ] **Step 6: Run repository-wide verification**

Run:

```bash
pnpm quality
```

Expected:

- repository policy tests PASS;
- Biome and TypeScript checks PASS;
- complete UI suite PASS;
- UI production build PASS;
- Rust formatting, clippy and workspace tests PASS.

- [ ] **Step 7: Start the latest development build for visual acceptance**

Run:

```bash
pnpm start:viewer
```

Using the real fixture project, verify:

1. 4 portrait images fill one row and are visibly larger than the former 2×2
   layout.
2. 4 landscape images choose a width-efficient grid.
3. 6 portrait images use the expected fit or horizontal scroll behavior at the
   actual window size according to the readability threshold.
4. a landscape/square overflow set uses two-column vertical scrolling;
5. a mixed set keeps order in an equal-height variable-width strip;
6. zoom, synchronized mode, rotation, review/favorite and remove controls work;
7. 21 selected images show a grey inactive `并排对比` radial action.

- [ ] **Step 8: Commit verification coverage**

```bash
git add \
  ui/src/components/compareLayoutBenchmark.test.ts \
  ui/src/components/CompareWorkspace.test.tsx \
  ui/src/components/CompareVirtualViewport.test.tsx \
  ui/src/components/RadialFileMenu.test.tsx \
  ui/src/App.test.tsx
git commit -m "test: verify intelligent compare performance"
```

- [ ] **Step 9: Inspect the final diff and preserve unrelated work**

Run:

```bash
git status --short
git diff --check HEAD~6..HEAD
git log --oneline -6
```

Expected:

- only files named in this plan appear in the six feature commits;
- the pre-existing `FolderTree`, `VirtualList`, `app.css`/test and fixture
  changes remain exactly as owned by the user, except the intentional compare
  CSS hunks in `app.css`/`app.test.ts`;
- no untracked build artifact or installed application is added.
