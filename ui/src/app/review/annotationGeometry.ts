import type { ReviewAnchor, ReviewPoint } from '../../api/types'

export type NormalizedPoint = ReviewPoint
export type ImageAnchor = Extract<ReviewAnchor, { kind: `image_${string}` }>
export type ImagePointAnchor = Extract<ReviewAnchor, { kind: 'image_point' }>
export type ImageArrowAnchor = Extract<ReviewAnchor, { kind: 'image_arrow' }>
export type ImageRectAnchor = Extract<ReviewAnchor, { kind: 'image_rect' }>
export type ImageEllipseAnchor = Extract<ReviewAnchor, { kind: 'image_ellipse' }>
export type BoxHandle = 'north_west' | 'north_east' | 'south_east' | 'south_west'

export interface EllipseOptions {
  constrainCircle: boolean
  sourceWidth: number
  sourceHeight: number
}

export interface StrokeSimplificationOptions {
  sourceWidth: number
  sourceHeight: number
  maxPoints: number
}

export interface NormalizedRect {
  x: number
  y: number
  width: number
  height: number
}

export function pointAnchor(point: NormalizedPoint): ImagePointAnchor | null {
  if (!isNormalizedPoint(point)) return null
  return { kind: 'image_point', x: roundNormalized(point.x), y: roundNormalized(point.y) }
}

export function arrowFromDrag(
  start: NormalizedPoint,
  end: NormalizedPoint,
): ImageArrowAnchor | null {
  if (!isFinitePoint(start) || !isFinitePoint(end)) return null
  const tail = clampedPoint(start)
  const head = clampedPoint(end)
  if (tail.x === head.x && tail.y === head.y) return null
  return { kind: 'image_arrow', tail, head }
}

export function ellipseFromDrag(
  start: NormalizedPoint,
  end: NormalizedPoint,
  options: EllipseOptions,
): ImageEllipseAnchor | null {
  if (!isFinitePoint(start) || !isFinitePoint(end)) return null
  const origin = clampedPoint(start)
  let destination = clampedPoint(end)
  if (options.constrainCircle) {
    if (
      !Number.isFinite(options.sourceWidth) ||
      !Number.isFinite(options.sourceHeight) ||
      options.sourceWidth <= 0 ||
      options.sourceHeight <= 0
    ) {
      return null
    }
    const deltaX = destination.x - origin.x
    const deltaY = destination.y - origin.y
    const sidePixels = Math.min(
      Math.abs(deltaX) * options.sourceWidth,
      Math.abs(deltaY) * options.sourceHeight,
    )
    if (sidePixels <= 0) return null
    destination = {
      x: origin.x + Math.sign(deltaX) * (sidePixels / options.sourceWidth),
      y: origin.y + Math.sign(deltaY) * (sidePixels / options.sourceHeight),
    }
  }
  const rect = rectFromDrag(origin, destination)
  return rect === null ? null : { kind: 'image_ellipse', ...rect }
}

export function moveAnchor(anchor: ImageAnchor, delta: NormalizedPoint): ImageAnchor {
  if (!isFinitePoint(delta)) return cloneAnchor(anchor)
  switch (anchor.kind) {
    case 'image_point': {
      const point = clampedPoint({ x: anchor.x + delta.x, y: anchor.y + delta.y })
      return { kind: anchor.kind, ...point }
    }
    case 'image_arrow': {
      const movement = constrainedMovement([anchor.tail, anchor.head], delta)
      return {
        kind: anchor.kind,
        tail: translatedPoint(anchor.tail, movement),
        head: translatedPoint(anchor.head, movement),
      }
    }
    case 'image_stroke': {
      const movement = constrainedMovement(anchor.points, delta)
      return {
        kind: anchor.kind,
        points: anchor.points.map((point) => translatedPoint(point, movement)),
      }
    }
    case 'image_rect':
    case 'image_ellipse': {
      const movement = constrainedMovement(
        [
          { x: anchor.x, y: anchor.y },
          { x: anchor.x + anchor.width, y: anchor.y + anchor.height },
        ],
        delta,
      )
      return {
        ...anchor,
        x: roundNormalized(anchor.x + movement.x),
        y: roundNormalized(anchor.y + movement.y),
      }
    }
  }
}

export function resizeBoxAnchor<T extends ImageRectAnchor | ImageEllipseAnchor>(
  anchor: T,
  handle: BoxHandle,
  point: NormalizedPoint,
): T | null {
  if (!isFinitePoint(point)) return null
  const active = clampedPoint(point)
  const opposite = oppositeCorner(anchor, handle)
  const rect = rectFromDrag(opposite, active)
  return rect === null ? null : ({ kind: anchor.kind, ...rect } as T)
}

export function rectFromDrag(start: NormalizedPoint, end: NormalizedPoint): NormalizedRect | null {
  if (!isFinitePoint(start) || !isFinitePoint(end)) return null
  const left = clamp01(Math.min(start.x, end.x))
  const top = clamp01(Math.min(start.y, end.y))
  const right = clamp01(Math.max(start.x, end.x))
  const bottom = clamp01(Math.max(start.y, end.y))
  const width = roundNormalized(right - left)
  const height = roundNormalized(bottom - top)
  if (width <= 0 || height <= 0) return null
  return {
    x: roundNormalized(left),
    y: roundNormalized(top),
    width,
    height,
  }
}

export function simplifyNormalizedStroke(
  input: ReadonlyArray<NormalizedPoint>,
  options: StrokeSimplificationOptions,
): ReadonlyArray<NormalizedPoint> | null {
  if (
    !Number.isFinite(options.sourceWidth) ||
    !Number.isFinite(options.sourceHeight) ||
    options.sourceWidth <= 0 ||
    options.sourceHeight <= 0 ||
    !Number.isInteger(options.maxPoints) ||
    options.maxPoints < 2 ||
    input.length < 2 ||
    input.some((point) => !isFinitePoint(point))
  ) {
    return null
  }

  const normalized = removeAdjacentDuplicates(
    input.map((point) => ({ x: clamp01(point.x), y: clamp01(point.y) })),
  )
  if (normalized.length < 2) return null

  const pixels = normalized.map((point) => ({
    x: point.x * options.sourceWidth,
    y: point.y * options.sourceHeight,
  }))
  const tolerance = Math.max(1.5, Math.max(options.sourceWidth, options.sourceHeight) / 4_096)
  const radial = radialDistanceFilter(pixels, tolerance)
  let simplified = douglasPeucker(radial, tolerance)
  if (simplified.length > options.maxPoints) {
    simplified = evenlySpaced(simplified, options.maxPoints)
  }

  const result = simplified.map((point) => ({
    x: clamp01(point.x / options.sourceWidth),
    y: clamp01(point.y / options.sourceHeight),
  }))
  result[0] = normalized[0] as NormalizedPoint
  result[result.length - 1] = normalized.at(-1) as NormalizedPoint
  return hasStrokeArea(result) ? result : null
}

function radialDistanceFilter(points: ReadonlyArray<NormalizedPoint>, tolerance: number) {
  const squareTolerance = tolerance * tolerance
  const result: NormalizedPoint[] = [points[0] as NormalizedPoint]
  let previous = points[0] as NormalizedPoint
  for (let index = 1; index < points.length - 1; index += 1) {
    const point = points[index] as NormalizedPoint
    if (squaredDistance(point, previous) > squareTolerance) {
      result.push(point)
      previous = point
    }
  }
  const last = points.at(-1) as NormalizedPoint
  if (last.x !== previous.x || last.y !== previous.y) result.push(last)
  return result
}

function douglasPeucker(points: ReadonlyArray<NormalizedPoint>, tolerance: number) {
  if (points.length <= 2) return [...points]
  const squareTolerance = tolerance * tolerance
  const keep = new Uint8Array(points.length)
  keep[0] = 1
  keep[points.length - 1] = 1
  const segments: Array<[number, number]> = [[0, points.length - 1]]

  while (segments.length > 0) {
    const [start, end] = segments.pop() as [number, number]
    let farthestIndex = -1
    let farthestDistance = squareTolerance
    for (let index = start + 1; index < end; index += 1) {
      const distance = squaredSegmentDistance(
        points[index] as NormalizedPoint,
        points[start] as NormalizedPoint,
        points[end] as NormalizedPoint,
      )
      if (distance > farthestDistance) {
        farthestDistance = distance
        farthestIndex = index
      }
    }
    if (farthestIndex >= 0) {
      keep[farthestIndex] = 1
      segments.push([start, farthestIndex], [farthestIndex, end])
    }
  }

  return points.filter((_, index) => keep[index] === 1)
}

function evenlySpaced(points: ReadonlyArray<NormalizedPoint>, maximum: number) {
  return Array.from({ length: maximum }, (_, index) => {
    const sourceIndex = Math.round((index * (points.length - 1)) / (maximum - 1))
    return points[sourceIndex] as NormalizedPoint
  })
}

function removeAdjacentDuplicates(points: ReadonlyArray<NormalizedPoint>) {
  return points.filter(
    (point, index) =>
      index === 0 ||
      point.x !== (points[index - 1] as NormalizedPoint).x ||
      point.y !== (points[index - 1] as NormalizedPoint).y,
  )
}

function hasStrokeArea(points: ReadonlyArray<NormalizedPoint>): boolean {
  if (points.length < 2) return false
  let minimumX = 1
  let minimumY = 1
  let maximumX = 0
  let maximumY = 0
  for (const point of points) {
    minimumX = Math.min(minimumX, point.x)
    minimumY = Math.min(minimumY, point.y)
    maximumX = Math.max(maximumX, point.x)
    maximumY = Math.max(maximumY, point.y)
  }
  return maximumX > minimumX && maximumY > minimumY
}

function squaredSegmentDistance(
  point: NormalizedPoint,
  start: NormalizedPoint,
  end: NormalizedPoint,
): number {
  let x = start.x
  let y = start.y
  let deltaX = end.x - x
  let deltaY = end.y - y

  if (deltaX !== 0 || deltaY !== 0) {
    const progress = ((point.x - x) * deltaX + (point.y - y) * deltaY) / (deltaX ** 2 + deltaY ** 2)
    if (progress > 1) {
      x = end.x
      y = end.y
    } else if (progress > 0) {
      x += deltaX * progress
      y += deltaY * progress
    }
  }
  deltaX = point.x - x
  deltaY = point.y - y
  return deltaX ** 2 + deltaY ** 2
}

function squaredDistance(left: NormalizedPoint, right: NormalizedPoint): number {
  return (left.x - right.x) ** 2 + (left.y - right.y) ** 2
}

function isFinitePoint(point: NormalizedPoint): boolean {
  return Number.isFinite(point.x) && Number.isFinite(point.y)
}

function isNormalizedPoint(point: NormalizedPoint): boolean {
  return isFinitePoint(point) && point.x >= 0 && point.x <= 1 && point.y >= 0 && point.y <= 1
}

function clampedPoint(point: NormalizedPoint): NormalizedPoint {
  return { x: roundNormalized(clamp01(point.x)), y: roundNormalized(clamp01(point.y)) }
}

function translatedPoint(point: NormalizedPoint, delta: NormalizedPoint): NormalizedPoint {
  return {
    x: roundNormalized(point.x + delta.x),
    y: roundNormalized(point.y + delta.y),
  }
}

function constrainedMovement(
  points: ReadonlyArray<NormalizedPoint>,
  delta: NormalizedPoint,
): NormalizedPoint {
  const minimumX = Math.min(...points.map((point) => point.x))
  const maximumX = Math.max(...points.map((point) => point.x))
  const minimumY = Math.min(...points.map((point) => point.y))
  const maximumY = Math.max(...points.map((point) => point.y))
  return {
    x: roundNormalized(Math.max(-minimumX, Math.min(1 - maximumX, delta.x))),
    y: roundNormalized(Math.max(-minimumY, Math.min(1 - maximumY, delta.y))),
  }
}

function oppositeCorner(
  anchor: ImageRectAnchor | ImageEllipseAnchor,
  handle: BoxHandle,
): NormalizedPoint {
  const left = anchor.x
  const top = anchor.y
  const right = anchor.x + anchor.width
  const bottom = anchor.y + anchor.height
  switch (handle) {
    case 'north_west':
      return { x: right, y: bottom }
    case 'north_east':
      return { x: left, y: bottom }
    case 'south_east':
      return { x: left, y: top }
    case 'south_west':
      return { x: right, y: top }
  }
}

function cloneAnchor(anchor: ImageAnchor): ImageAnchor {
  switch (anchor.kind) {
    case 'image_arrow':
      return { kind: anchor.kind, tail: { ...anchor.tail }, head: { ...anchor.head } }
    case 'image_stroke':
      return { kind: anchor.kind, points: anchor.points.map((point) => ({ ...point })) }
    default:
      return { ...anchor }
  }
}

function clamp01(value: number): number {
  return Math.max(0, Math.min(1, value))
}

function roundNormalized(value: number): number {
  return Number(value.toFixed(12))
}
