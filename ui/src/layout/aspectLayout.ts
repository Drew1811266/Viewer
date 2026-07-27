import type { MarqueeRect } from '../components/marqueeSelection'

export interface ImageDimensions {
  width: number
  height: number
}

export interface AspectSource {
  key: string
  dimensions: ImageDimensions | null
}

export interface AspectRect {
  key: string
  index: number
  row: number
  left: number
  top: number
  width: number
  height: number
  imageWidth: number
  imageHeight: number
}

export interface AspectRow {
  index: number
  start: number
  end: number
  top: number
  height: number
}

export interface AspectGeometry {
  items: AspectRect[]
  rows: AspectRow[]
  indexByKey: Map<string, number>
  totalWidth: number
  totalHeight: number
}

export function validDimensions(value: ImageDimensions | null): value is ImageDimensions {
  if (value === null || !Number.isFinite(value.width) || !Number.isFinite(value.height)) {
    return false
  }
  if (value.width <= 0 || value.height <= 0) return false
  const ratio = value.width / value.height
  return Number.isFinite(ratio) && ratio > 0
}

export function proportionalWidth(imageHeight: number, dimensions: ImageDimensions | null): number {
  if (!validDimensions(dimensions)) return imageHeight
  const width = (imageHeight / dimensions.height) * dimensions.width
  return Number.isFinite(width) && width > 0 ? width : imageHeight
}

export function buildFilmstripGeometry(
  sources: readonly AspectSource[],
  imageHeight: number,
  gap: number,
  inlinePadding: number,
): AspectGeometry {
  const lengthLimit = geometryLengthLimit(sources.length)
  const safeImageHeight = positiveLength(imageHeight, lengthLimit)
  const safeGap = nonnegativeLength(gap, lengthLimit)
  const safePadding = nonnegativeLength(inlinePadding, lengthLimit)
  const items: AspectRect[] = []
  const indexByKey = new Map<string, number>()
  let left = safePadding
  let occupiedRight = safePadding

  for (const [index, source] of sources.entries()) {
    const remaining = sources.length - index - 1
    const tail = filmstripTail(remaining, safeImageHeight, safeGap, safePadding)
    const width = fittingWidth(
      safeProportionalWidth(safeImageHeight, source.dimensions),
      safeImageHeight,
      left,
      tail,
    )
    occupiedRight = finiteAdd(left, width) ?? left
    items.push({
      key: source.key,
      index,
      row: 0,
      left,
      top: 0,
      width,
      height: safeImageHeight,
      imageWidth: width,
      imageHeight: safeImageHeight,
    })
    indexByKey.set(source.key, index)
    if (remaining > 0) left = finiteAdd(occupiedRight, safeGap) ?? occupiedRight
  }

  const totalWidth =
    items.length === 0
      ? (finiteAdd(safePadding, safePadding) ?? safePadding)
      : (finiteAdd(occupiedRight, safePadding) ?? occupiedRight)
  return {
    items,
    rows:
      items.length === 0
        ? []
        : [{ index: 0, start: 0, end: items.length, top: 0, height: safeImageHeight }],
    indexByKey,
    totalWidth,
    totalHeight: items.length === 0 ? 0 : safeImageHeight,
  }
}

export function buildFlowGeometry(
  sources: readonly AspectSource[],
  availableWidth: number,
  imageHeight: number,
  captionHeight: number,
  gap: number,
): AspectGeometry {
  const lengthLimit = geometryLengthLimit(sources.length)
  const safeAvailableWidth = nonnegativeLength(availableWidth)
  const safeImageHeight = positiveLength(imageHeight, lengthLimit)
  const safeCaptionHeight = nonnegativeLength(captionHeight, lengthLimit)
  const rowHeight = finiteAdd(safeImageHeight, safeCaptionHeight) ?? safeImageHeight
  const safeGap = nonnegativeLength(gap, lengthLimit)
  const items: AspectRect[] = []
  const rows: AspectRow[] = []
  const indexByKey = new Map<string, number>()
  let row = 0
  let rowStart = 0
  let left = 0
  let top = 0
  let widestRight = 0

  for (const [index, source] of sources.entries()) {
    const proportional = safeProportionalWidth(safeImageHeight, source.dimensions)
    const candidateLeft = left === 0 ? 0 : finiteAdd(left, safeGap)
    const candidateRight = candidateLeft === null ? null : finiteAdd(candidateLeft, proportional)
    const needsNewRow = left > 0 && (candidateRight === null || candidateRight > safeAvailableWidth)
    if (needsNewRow) {
      rows.push({ index: row, start: rowStart, end: index, top, height: rowHeight })
      row += 1
      rowStart = index
      left = 0
      top = finiteAdd(top, rowHeight, safeGap) ?? top
    }

    const itemLeft = left === 0 ? 0 : (finiteAdd(left, safeGap) ?? 0)
    const width = fittingWidth(proportional, safeImageHeight, itemLeft, 0)
    const itemRight = finiteAdd(itemLeft, width) ?? itemLeft
    items.push({
      key: source.key,
      index,
      row,
      left: itemLeft,
      top,
      width,
      height: rowHeight,
      imageWidth: width,
      imageHeight: safeImageHeight,
    })
    indexByKey.set(source.key, index)
    left = itemRight
    widestRight = Math.max(widestRight, itemRight)
  }

  if (items.length > 0) {
    rows.push({ index: row, start: rowStart, end: items.length, top, height: rowHeight })
  }

  return {
    items,
    rows,
    indexByKey,
    totalWidth: Math.max(safeAvailableWidth, widestRight),
    totalHeight: items.length === 0 ? 0 : (finiteAdd(top, rowHeight) ?? top),
  }
}

export function horizontalVisibleIndexes(
  geometry: AspectGeometry,
  scrollLeft: number,
  viewportWidth: number,
  overscanPixels: number,
): { start: number; end: number } {
  const lowerBound = nonnegativeLength(scrollLeft) - nonnegativeLength(overscanPixels)
  const upperBound =
    nonnegativeLength(scrollLeft) +
    nonnegativeLength(viewportWidth) +
    nonnegativeLength(overscanPixels)
  const start = firstIndex(geometry.items, (item) => item.left + item.width >= lowerBound)
  const end = firstIndex(geometry.items, (item) => item.left > upperBound)
  return { start, end }
}

export function verticalVisibleRows(
  geometry: AspectGeometry,
  scrollTop: number,
  viewportHeight: number,
  overscanRows: number,
): { start: number; end: number } {
  const safeScrollTop = nonnegativeLength(scrollTop)
  const visibleBottom = safeScrollTop + nonnegativeLength(viewportHeight)
  const firstVisible = firstIndex(geometry.rows, (row) => row.top + row.height >= safeScrollTop)
  if (firstVisible === geometry.rows.length) return { start: firstVisible, end: firstVisible }
  const endVisible = firstIndex(geometry.rows, (row) => row.top > visibleBottom)
  const extraRows = Math.floor(nonnegativeLength(overscanRows))
  return {
    start: Math.max(0, firstVisible - extraRows),
    end: Math.min(geometry.rows.length, endVisible + extraRows),
  }
}

export function intersectingAspectIndexes(geometry: AspectGeometry, rect: MarqueeRect): number[] {
  return geometry.items
    .filter(
      (item) =>
        rect.left <= item.left + item.width &&
        rect.right >= item.left &&
        rect.top <= item.top + item.height &&
        rect.bottom >= item.top,
    )
    .map((item) => item.index)
}

export function directionalNeighbor(
  geometry: AspectGeometry,
  activeIndex: number,
  direction: 'left' | 'right' | 'up' | 'down',
): number {
  const active = geometry.items[activeIndex]
  if (active === undefined) return activeIndex
  if (direction === 'left') return activeIndex > 0 ? activeIndex - 1 : activeIndex
  if (direction === 'right')
    return activeIndex + 1 < geometry.items.length ? activeIndex + 1 : activeIndex

  const rowIndex = active.row + (direction === 'up' ? -1 : 1)
  const row = geometry.rows[rowIndex]
  if (row === undefined) return activeIndex

  const activeCenter = active.left + active.width / 2
  let nearest = activeIndex
  let nearestDistance = Number.POSITIVE_INFINITY
  for (let index = row.start; index < row.end; index += 1) {
    const candidate = geometry.items[index]
    if (candidate === undefined) continue
    const distance = Math.abs(candidate.left + candidate.width / 2 - activeCenter)
    if (distance < nearestDistance) {
      nearest = index
      nearestDistance = distance
    }
  }
  return nearest
}

export function anchoredScrollOffset(
  previous: AspectGeometry,
  next: AspectGeometry,
  key: string,
  previousScroll: number,
  axis: 'horizontal' | 'vertical',
): number {
  const previousIndex = previous.indexByKey.get(key)
  const nextIndex = next.indexByKey.get(key)
  if (previousIndex === undefined || nextIndex === undefined) return previousScroll
  const previousItem = previous.items[previousIndex]
  const nextItem = next.items[nextIndex]
  if (previousItem === undefined || nextItem === undefined) return previousScroll
  const previousPosition = axis === 'horizontal' ? previousItem.left : previousItem.top
  const nextPosition = axis === 'horizontal' ? nextItem.left : nextItem.top
  const offset = nextPosition + (previousScroll - previousPosition)
  return Number.isFinite(offset) ? Math.max(0, offset) : previousScroll
}

function safeProportionalWidth(imageHeight: number, dimensions: ImageDimensions | null): number {
  const width = proportionalWidth(imageHeight, dimensions)
  return Number.isFinite(width) && width > 0 ? width : imageHeight
}

function geometryLengthLimit(sourceCount: number): number {
  return Number.MAX_VALUE / (4 * Math.max(1, sourceCount) + 4)
}

function positiveLength(value: number, limit = Number.MAX_VALUE): number {
  return Number.isFinite(value) && value > 0 ? Math.min(value, limit) : 1
}

function nonnegativeLength(value: number, limit = Number.MAX_VALUE): number {
  return Number.isFinite(value) && value > 0 ? Math.min(value, limit) : 0
}

function filmstripTail(
  remaining: number,
  imageHeight: number,
  gap: number,
  padding: number,
): number {
  return finiteAdd(remaining * imageHeight, remaining * gap, padding) ?? padding
}

function fittingWidth(
  candidate: number,
  squareFallback: number,
  left: number,
  tail: number,
): number {
  if (finiteAdd(left, candidate, tail) !== null) return candidate
  if (finiteAdd(left, squareFallback, tail) !== null) return squareFallback
  const available = Number.MAX_VALUE - left - tail
  return Number.isFinite(available) && available > 0
    ? Math.min(squareFallback, available)
    : Number.MIN_VALUE
}

function finiteAdd(...values: number[]): number | null {
  let total = 0
  for (const value of values) {
    total += value
    if (!Number.isFinite(total)) return null
  }
  return total
}

function firstIndex<T>(values: readonly T[], includes: (value: T) => boolean): number {
  let low = 0
  let high = values.length
  while (low < high) {
    const middle = low + Math.floor((high - low) / 2)
    const value = values[middle]
    if (value !== undefined && includes(value)) high = middle
    else low = middle + 1
  }
  return low
}
