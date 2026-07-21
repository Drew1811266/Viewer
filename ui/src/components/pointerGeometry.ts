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
