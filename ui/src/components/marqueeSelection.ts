export interface MarqueePoint {
  x: number
  y: number
}

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
