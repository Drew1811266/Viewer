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

interface DisplayMetrics {
  area: number
  longEdge: number
}

interface FitCandidate {
  key: string
  kind: Extract<CompareLayoutKind, 'fit-row' | 'fit-column' | 'fit-grid'>
  columns: number
  rows: number
  rects: CompareLayoutRect[]
  displays: DisplayMetrics[]
  score: number
  eligible: boolean
}

const safeRatio = (value: number) => (Number.isFinite(value) && value > 0 ? value : 1)

const finiteNonnegative = (value: number) => (Number.isFinite(value) ? Math.max(0, value) : 0)

const positiveDimension = (value: number) => Math.max(1, finiteNonnegative(value))

const finiteSaturatingSum = (...values: readonly number[]) => {
  let sum = 0
  for (const rawValue of values) {
    const value = finiteNonnegative(rawValue)
    if (sum > Number.MAX_VALUE - value) return Number.MAX_VALUE
    sum += value
  }
  return sum
}

const workspaceIsValid = ({ width, height, gap, padding, paneChromeHeight }: CompareLayoutInput) =>
  Number.isFinite(width) &&
  width > 0 &&
  Number.isFinite(height) &&
  height > 0 &&
  Number.isFinite(gap) &&
  gap >= 0 &&
  Number.isFinite(padding) &&
  padding >= 0 &&
  Number.isFinite(paneChromeHeight) &&
  paneChromeHeight >= 0

const displayMetrics = (stageWidth: number, stageHeight: number, ratio: number): DisplayMetrics => {
  const safeStageWidth = positiveDimension(stageWidth)
  const safeStageHeight = positiveDimension(stageHeight)
  const displayWidth = Math.min(safeStageWidth, safeStageHeight * safeRatio(ratio))
  const displayHeight = Math.min(safeStageHeight, safeStageWidth / safeRatio(ratio))

  return {
    area: finiteNonnegative(displayWidth * displayHeight),
    longEdge: finiteNonnegative(Math.max(displayWidth, displayHeight)),
  }
}

const candidateKind = (columns: number, itemCount: number): FitCandidate['kind'] => {
  if (columns === itemCount) return 'fit-row'
  if (columns === 1) return 'fit-column'
  return 'fit-grid'
}

const buildFitCandidate = (
  input: CompareLayoutInput,
  columns: number,
  singlePaneDisplays: readonly DisplayMetrics[],
): FitCandidate => {
  const { width, height, gap, padding, paneChromeHeight, items, previous } = input
  const rows = Math.ceil(items.length / columns)
  const cellWidth = positiveDimension((width - 2 * padding - (columns - 1) * gap) / columns)
  const cellHeight = positiveDimension((height - 2 * padding - (rows - 1) * gap) / rows)
  const stageHeight = positiveDimension(cellHeight - paneChromeHeight)
  const displays = items.map(({ aspectRatio }) =>
    displayMetrics(cellWidth, stageHeight, aspectRatio),
  )
  const rects = items.map(({ entityId }, index) => {
    const row = Math.floor(index / columns)
    const rowStart = row * columns
    const rowItemCount = Math.min(columns, items.length - rowStart)
    const rowWidth = rowItemCount * cellWidth + Math.max(0, rowItemCount - 1) * gap
    const rowLeft = padding + Math.max(0, (width - 2 * padding - rowWidth) / 2)
    const column = index - rowStart

    return {
      entityId,
      index,
      left: finiteNonnegative(rowLeft + column * (cellWidth + gap)),
      top: finiteNonnegative(padding + row * (cellHeight + gap)),
      width: cellWidth,
      height: cellHeight,
      stageWidth: cellWidth,
      stageHeight,
    }
  })
  const normalizedAreas = displays.map((display, index) => {
    const singleArea = singlePaneDisplays[index]?.area ?? 1
    return finiteNonnegative(display.area / Math.max(1, singleArea))
  })
  const minimumNormalizedArea = Math.min(...normalizedAreas)
  const meanNormalizedArea =
    normalizedAreas.reduce((sum, value) => sum + value, 0) / normalizedAreas.length
  const viewportArea = Math.max(1, (width - 2 * padding) * (height - 2 * padding))
  const fill = displays.reduce((sum, display) => sum + display.area, 0) / viewportArea
  const kind = candidateKind(columns, items.length)
  const key = `${kind}:${columns}`
  const continuity = previous?.key === key ? 1 : 0
  const score =
    0.45 * minimumNormalizedArea + 0.25 * meanNormalizedArea + 0.2 * fill + 0.1 * continuity
  const eligible = displays.every(
    ({ area, longEdge }) => area >= MIN_READABLE_AREA && longEdge >= MIN_READABLE_LONG_EDGE,
  )

  return {
    key,
    kind,
    columns,
    rows,
    rects,
    displays,
    score: finiteNonnegative(score),
    eligible,
  }
}

const planFromFitCandidate = (
  candidate: FitCandidate,
  input: CompareLayoutInput,
  candidateCount: number,
  retainedPrevious: boolean,
): CompareLayoutPlan => ({
  key: candidate.key,
  kind: candidate.kind,
  score: candidate.score,
  eligible: candidate.eligible,
  scrollAxis: 'none',
  columns: candidate.columns,
  rows: candidate.rows,
  rects: candidate.rects,
  viewportWidth: input.width,
  viewportHeight: input.height,
  totalWidth: input.width,
  totalHeight: input.height,
  candidateCount,
  retainedPrevious,
})

const classifyRatio = (value: number) => {
  const ratio = safeRatio(value)
  if (ratio < PORTRAIT_RATIO_MAX) return 'portrait'
  if (ratio > LANDSCAPE_RATIO_MIN) return 'landscape'
  return 'neutral'
}

const buildHorizontalStrip = (
  input: CompareLayoutInput,
  candidateCount: number,
): CompareLayoutPlan => {
  const { width, height, gap, padding, paneChromeHeight, items } = input
  const stageHeight = positiveDimension(height - 2 * padding - paneChromeHeight)
  let left = padding
  const rects = items.map(({ entityId, aspectRatio }, index) => {
    const cardWidth = positiveDimension(
      Math.max(320, Math.min(stageHeight * safeRatio(aspectRatio), width * 0.75)),
    )
    const rect = {
      entityId,
      index,
      left,
      top: padding,
      width: cardWidth,
      height: positiveDimension(stageHeight + paneChromeHeight),
      stageWidth: cardWidth,
      stageHeight,
    }
    left = finiteSaturatingSum(left, cardWidth, gap)
    return rect
  })
  const finalRect = rects.at(-1)
  const totalWidth = finalRect
    ? finiteSaturatingSum(finalRect.left, finalRect.width, padding)
    : finiteSaturatingSum(padding, padding)

  return {
    key: 'horizontal-strip',
    kind: 'horizontal-strip',
    score: 0,
    eligible: true,
    scrollAxis: 'horizontal',
    columns: items.length,
    rows: items.length === 0 ? 0 : 1,
    rects,
    viewportWidth: width,
    viewportHeight: height,
    totalWidth,
    totalHeight: height,
    candidateCount,
    retainedPrevious: false,
  }
}

const buildVerticalFlow = (
  input: CompareLayoutInput,
  candidateCount: number,
): CompareLayoutPlan => {
  const { width, height, gap, padding, paneChromeHeight, items } = input
  const columns = width < LANDSCAPE_SINGLE_COLUMN_BREAKPOINT ? 1 : 2
  const rows = Math.ceil(items.length / columns)
  const columnWidth = positiveDimension((width - 2 * padding - (columns - 1) * gap) / columns)
  const maximumStageHeight = positiveDimension(height - 2 * padding - paneChromeHeight)
  const rects: CompareLayoutRect[] = []
  let top = padding

  for (let row = 0; row < rows; row += 1) {
    const rowStart = row * columns
    const rowItems = items.slice(rowStart, rowStart + columns)
    const rowStageHeight = positiveDimension(
      Math.min(
        maximumStageHeight,
        Math.max(
          ...rowItems.map(({ aspectRatio }) =>
            finiteNonnegative(columnWidth / safeRatio(aspectRatio)),
          ),
        ),
      ),
    )

    rowItems.forEach(({ entityId }, column) => {
      rects.push({
        entityId,
        index: rowStart + column,
        left: finiteNonnegative(padding + column * (columnWidth + gap)),
        top,
        width: columnWidth,
        height: positiveDimension(rowStageHeight + paneChromeHeight),
        stageWidth: columnWidth,
        stageHeight: rowStageHeight,
      })
    })
    top += rowStageHeight + paneChromeHeight + (row < rows - 1 ? gap : 0)
  }

  return {
    key: `vertical-flow:${columns}`,
    kind: 'vertical-flow',
    score: 0,
    eligible: true,
    scrollAxis: 'vertical',
    columns,
    rows,
    rects,
    viewportWidth: width,
    viewportHeight: height,
    totalWidth: width,
    totalHeight: finiteNonnegative(top + padding),
    candidateCount,
    retainedPrevious: false,
  }
}

const buildSafeColumn = (input: CompareLayoutInput): CompareLayoutPlan => {
  const gap = finiteNonnegative(input.gap)
  const padding = finiteNonnegative(input.padding)
  const paneChromeHeight = finiteNonnegative(input.paneChromeHeight)
  const width = positiveDimension(input.width)
  const height = positiveDimension(input.height)
  const stageWidth = positiveDimension(width - 2 * padding)
  const stageHeight = 1
  const rectHeight = positiveDimension(stageHeight + paneChromeHeight)
  const rects = input.items.map(({ entityId }, index) => ({
    entityId,
    index,
    left: padding,
    top: finiteNonnegative(padding + index * (rectHeight + gap)),
    width: stageWidth,
    height: rectHeight,
    stageWidth,
    stageHeight,
  }))
  const totalHeight =
    rects.length === 0
      ? 2 * padding
      : 2 * padding + rects.length * rectHeight + (rects.length - 1) * gap

  return {
    key: 'safe-column',
    kind: 'safe-column',
    score: 0,
    eligible: false,
    scrollAxis: 'vertical',
    columns: 1,
    rows: input.items.length,
    rects,
    viewportWidth: width,
    viewportHeight: height,
    totalWidth: width,
    totalHeight: positiveDimension(totalHeight),
    candidateCount: 1,
    retainedPrevious: false,
  }
}

export const solveCompareLayout = (input: CompareLayoutInput): CompareLayoutPlan => {
  if (!workspaceIsValid(input) || input.items.length === 0) {
    return buildSafeColumn(input)
  }

  const { items, width, height, padding, paneChromeHeight } = input
  const singleStageWidth = positiveDimension(width - 2 * padding)
  const singleStageHeight = positiveDimension(height - 2 * padding - paneChromeHeight)
  const singlePaneDisplays = items.map(({ aspectRatio }) =>
    displayMetrics(singleStageWidth, singleStageHeight, aspectRatio),
  )
  const columnCounts = [...new Set([1, 2, 3, 4, items.length])].filter(
    (columns) => columns >= 1 && columns <= items.length,
  )
  const candidates = columnCounts.map((columns) =>
    buildFitCandidate(input, columns, singlePaneDisplays),
  )
  const eligible = candidates.filter((candidate) => candidate.eligible)

  if (eligible.length > 0) {
    const best = eligible.reduce((current, candidate) =>
      candidate.score > current.score ? candidate : current,
    )
    const previousCandidate = input.previous
      ? candidates.find(({ key }) => key === input.previous?.key)
      : undefined
    const retainPrevious =
      previousCandidate?.eligible === true &&
      (best.key === previousCandidate.key ||
        best.score < (input.previous?.score ?? 0) * (1 + LAYOUT_SWITCH_GAIN))
    return planFromFitCandidate(
      retainPrevious ? previousCandidate : best,
      input,
      candidates.length,
      retainPrevious,
    )
  }

  const candidateCount = candidates.length + 1
  const classes = items.map(({ aspectRatio }) => classifyRatio(aspectRatio))
  const allPortrait = classes.every((value) => value === 'portrait')
  const allNonPortrait = classes.every((value) => value !== 'portrait')

  if (allPortrait) return buildHorizontalStrip(input, candidateCount)
  if (allNonPortrait) return buildVerticalFlow(input, candidateCount)
  return buildHorizontalStrip(input, candidateCount)
}

const firstRectWithEndAtOrAfter = (
  rects: readonly CompareLayoutRect[],
  axis: 'horizontal' | 'vertical',
  position: number,
) => {
  let low = 0
  let high = rects.length

  while (low < high) {
    const middle = Math.floor((low + high) / 2)
    const rect = rects[middle]
    if (!rect) return low
    const end = axis === 'horizontal' ? rect.left + rect.width : rect.top + rect.height
    if (end < position) low = middle + 1
    else high = middle
  }

  return low
}

const firstRectStartingAfter = (
  rects: readonly CompareLayoutRect[],
  axis: 'horizontal' | 'vertical',
  position: number,
) => {
  let low = 0
  let high = rects.length

  while (low < high) {
    const middle = Math.floor((low + high) / 2)
    const rect = rects[middle]
    if (!rect) return low
    const start = axis === 'horizontal' ? rect.left : rect.top
    if (start <= position) low = middle + 1
    else high = middle
  }

  return low
}

export const visibleCompareIndexes = (
  plan: CompareLayoutPlan,
  scrollLeft: number,
  scrollTop: number,
  retainedEntityId?: string,
): number[] => {
  if (plan.scrollAxis === 'none') {
    return plan.rects.map(({ index }) => index).sort((left, right) => left - right)
  }

  const axis = plan.scrollAxis
  const offset = finiteNonnegative(axis === 'horizontal' ? scrollLeft : scrollTop)
  const viewportExtent = positiveDimension(
    axis === 'horizontal' ? plan.viewportWidth : plan.viewportHeight,
  )
  const overscanStart = Math.max(0, offset - viewportExtent)
  const overscanEnd = finiteNonnegative(offset + 2 * viewportExtent)
  const first = firstRectWithEndAtOrAfter(plan.rects, axis, overscanStart)
  const lastExclusive = firstRectStartingAfter(plan.rects, axis, overscanEnd)
  const mounted = new Set(plan.rects.slice(first, lastExclusive).map(({ index }) => index))
  const retained = retainedEntityId
    ? plan.rects.find(({ entityId }) => entityId === retainedEntityId)
    : undefined
  if (retained) mounted.add(retained.index)

  return [...mounted].sort((left, right) => left - right)
}

const rectPosition = (plan: CompareLayoutPlan, rect: CompareLayoutRect) =>
  plan.scrollAxis === 'horizontal' ? rect.left : rect.top

export const anchoredCompareScrollOffset = (
  previous: CompareLayoutPlan,
  next: CompareLayoutPlan,
  entityId: string,
  previousOffset: number,
): number => {
  const previousRect = previous.rects.find((rect) => rect.entityId === entityId)
  const nextRect = next.rects.find((rect) => rect.entityId === entityId)
  if (!previousRect || !nextRect) return previousOffset

  const previousPosition = rectPosition(previous, previousRect)
  const nextPosition = rectPosition(next, nextRect)
  const nextExtent = next.scrollAxis === 'horizontal' ? next.totalWidth : next.totalHeight
  const viewportExtent = next.scrollAxis === 'horizontal' ? next.viewportWidth : next.viewportHeight
  const maximumOffset = Math.max(
    0,
    finiteNonnegative(nextExtent) - finiteNonnegative(viewportExtent),
  )
  const nextOffset = nextPosition + (finiteNonnegative(previousOffset) - previousPosition)

  return Math.max(0, Math.min(maximumOffset, finiteNonnegative(nextOffset)))
}
