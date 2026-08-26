export interface NormalizedPoint {
  x: number
  y: number
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

function clamp01(value: number): number {
  return Math.max(0, Math.min(1, value))
}

function roundNormalized(value: number): number {
  return Number(value.toFixed(12))
}
