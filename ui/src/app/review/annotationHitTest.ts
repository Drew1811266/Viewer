import type { BoxHandle, ImageAnchor, NormalizedPoint } from './annotationGeometry'

export type AnnotationHandle = BoxHandle | 'point' | 'tail' | 'head'
export type AnnotationHitPart = 'handle' | 'ordinal' | 'outline' | 'interior'

export interface AnnotationHitTestItem {
  itemId: string
  ordinal: number
  selected: boolean
  anchor: ImageAnchor
  ordinalPoint?: NormalizedPoint
}

export interface AnnotationHitTestInput {
  point: NormalizedPoint
  imageSizeCss: { width: number; height: number }
  items: ReadonlyArray<AnnotationHitTestItem>
  toleranceCssPx?: number
}

export interface AnnotationHit {
  itemId: string
  ordinal: number
  part: AnnotationHitPart
  handle?: AnnotationHandle
  distanceCssPx: number
}

interface Candidate extends AnnotationHit {
  priority: number
  itemIndex: number
}

export function hitTestAnnotation(input: AnnotationHitTestInput): AnnotationHit | null {
  const tolerance = input.toleranceCssPx ?? 8
  if (
    !isFinitePoint(input.point) ||
    !Number.isFinite(input.imageSizeCss.width) ||
    !Number.isFinite(input.imageSizeCss.height) ||
    input.imageSizeCss.width <= 0 ||
    input.imageSizeCss.height <= 0 ||
    !Number.isFinite(tolerance) ||
    tolerance < 0
  ) {
    return null
  }

  const candidates = input.items.flatMap((item, itemIndex) =>
    candidatesForItem(input.point, input.imageSizeCss, tolerance, item, itemIndex),
  )
  candidates.sort(
    (left, right) =>
      left.priority - right.priority ||
      left.distanceCssPx - right.distanceCssPx ||
      right.itemIndex - left.itemIndex,
  )
  const winner = candidates[0]
  if (winner === undefined) return null
  const { priority: _priority, itemIndex: _itemIndex, ...hit } = winner
  return hit
}

function candidatesForItem(
  point: NormalizedPoint,
  size: { width: number; height: number },
  tolerance: number,
  item: AnnotationHitTestItem,
  itemIndex: number,
): Candidate[] {
  const candidates: Candidate[] = []
  for (const [handle, handlePoint] of handles(item.anchor)) {
    const distance = distanceCss(point, handlePoint, size)
    if (distance <= tolerance) {
      candidates.push(candidate(item, itemIndex, 'handle', distance, 0, handle))
    }
  }

  if (item.ordinalPoint !== undefined) {
    const distance = distanceCss(point, item.ordinalPoint, size)
    if (distance <= tolerance) {
      candidates.push(candidate(item, itemIndex, 'ordinal', distance, item.selected ? 1 : 2))
    }
  }

  const geometry = geometryHit(point, item.anchor, size, tolerance)
  if (geometry !== null) {
    const basePriority = geometry.part === 'outline' ? 3 : 4
    candidates.push(
      candidate(
        item,
        itemIndex,
        geometry.part,
        geometry.distanceCssPx,
        item.selected ? 1 : basePriority,
      ),
    )
  }
  return candidates
}

function candidate(
  item: AnnotationHitTestItem,
  itemIndex: number,
  part: AnnotationHitPart,
  distanceCssPx: number,
  priority: number,
  handle?: AnnotationHandle,
): Candidate {
  return {
    itemId: item.itemId,
    ordinal: item.ordinal,
    part,
    handle,
    distanceCssPx,
    priority,
    itemIndex,
  }
}

function handles(anchor: ImageAnchor): ReadonlyArray<readonly [AnnotationHandle, NormalizedPoint]> {
  switch (anchor.kind) {
    case 'image_point':
      return [['point', { x: anchor.x, y: anchor.y }]]
    case 'image_arrow':
      return [
        ['tail', anchor.tail],
        ['head', anchor.head],
      ]
    case 'image_rect':
    case 'image_ellipse':
      return [
        ['north_west', { x: anchor.x, y: anchor.y }],
        ['north_east', { x: anchor.x + anchor.width, y: anchor.y }],
        ['south_east', { x: anchor.x + anchor.width, y: anchor.y + anchor.height }],
        ['south_west', { x: anchor.x, y: anchor.y + anchor.height }],
      ]
    case 'image_stroke':
      return []
  }
}

function geometryHit(
  point: NormalizedPoint,
  anchor: ImageAnchor,
  size: { width: number; height: number },
  tolerance: number,
): Pick<AnnotationHit, 'part' | 'distanceCssPx'> | null {
  switch (anchor.kind) {
    case 'image_point': {
      const distance = distanceCss(point, { x: anchor.x, y: anchor.y }, size)
      return distance <= tolerance ? { part: 'outline', distanceCssPx: distance } : null
    }
    case 'image_arrow': {
      const distance = segmentDistanceCss(point, anchor.tail, anchor.head, size)
      return distance <= tolerance ? { part: 'outline', distanceCssPx: distance } : null
    }
    case 'image_stroke': {
      const distance = minimumPathDistanceCss(point, anchor.points, size)
      return distance <= tolerance ? { part: 'outline', distanceCssPx: distance } : null
    }
    case 'image_rect':
      return rectangleHit(point, anchor, size, tolerance)
    case 'image_ellipse':
      return ellipseHit(point, anchor, size, tolerance)
  }
}

function rectangleHit(
  point: NormalizedPoint,
  anchor: Extract<ImageAnchor, { kind: 'image_rect' }>,
  size: { width: number; height: number },
  tolerance: number,
): Pick<AnnotationHit, 'part' | 'distanceCssPx'> | null {
  const left = anchor.x
  const right = anchor.x + anchor.width
  const top = anchor.y
  const bottom = anchor.y + anchor.height
  const inside = point.x >= left && point.x <= right && point.y >= top && point.y <= bottom
  const edgeDistance = Math.min(
    Math.abs(point.x - left) * size.width,
    Math.abs(point.x - right) * size.width,
    Math.abs(point.y - top) * size.height,
    Math.abs(point.y - bottom) * size.height,
  )
  const withinExpanded =
    point.x >= left - tolerance / size.width &&
    point.x <= right + tolerance / size.width &&
    point.y >= top - tolerance / size.height &&
    point.y <= bottom + tolerance / size.height
  if (withinExpanded && edgeDistance <= tolerance)
    return { part: 'outline', distanceCssPx: edgeDistance }
  return inside ? { part: 'interior', distanceCssPx: edgeDistance } : null
}

function ellipseHit(
  point: NormalizedPoint,
  anchor: Extract<ImageAnchor, { kind: 'image_ellipse' }>,
  size: { width: number; height: number },
  tolerance: number,
): Pick<AnnotationHit, 'part' | 'distanceCssPx'> | null {
  const center = { x: anchor.x + anchor.width / 2, y: anchor.y + anchor.height / 2 }
  const radiusX = anchor.width / 2
  const radiusY = anchor.height / 2
  if (radiusX <= 0 || radiusY <= 0) return null
  const radial = Math.hypot((point.x - center.x) / radiusX, (point.y - center.y) / radiusY)
  const boundaryDistance =
    Math.abs(radial - 1) * Math.min(radiusX * size.width, radiusY * size.height)
  if (boundaryDistance <= tolerance) return { part: 'outline', distanceCssPx: boundaryDistance }
  return radial < 1 ? { part: 'interior', distanceCssPx: boundaryDistance } : null
}

function minimumPathDistanceCss(
  point: NormalizedPoint,
  path: ReadonlyArray<NormalizedPoint>,
  size: { width: number; height: number },
): number {
  if (path.length < 2) return Number.POSITIVE_INFINITY
  let minimum = Number.POSITIVE_INFINITY
  for (let index = 1; index < path.length; index += 1) {
    minimum = Math.min(
      minimum,
      segmentDistanceCss(
        point,
        path[index - 1] as NormalizedPoint,
        path[index] as NormalizedPoint,
        size,
      ),
    )
  }
  return minimum
}

function segmentDistanceCss(
  point: NormalizedPoint,
  start: NormalizedPoint,
  end: NormalizedPoint,
  size: { width: number; height: number },
): number {
  const p = toCss(point, size)
  const a = toCss(start, size)
  const b = toCss(end, size)
  const deltaX = b.x - a.x
  const deltaY = b.y - a.y
  const denominator = deltaX ** 2 + deltaY ** 2
  const progress =
    denominator === 0
      ? 0
      : Math.max(0, Math.min(1, ((p.x - a.x) * deltaX + (p.y - a.y) * deltaY) / denominator))
  return Math.hypot(p.x - (a.x + deltaX * progress), p.y - (a.y + deltaY * progress))
}

function distanceCss(
  left: NormalizedPoint,
  right: NormalizedPoint,
  size: { width: number; height: number },
): number {
  return Math.hypot((left.x - right.x) * size.width, (left.y - right.y) * size.height)
}

function toCss(point: NormalizedPoint, size: { width: number; height: number }) {
  return { x: point.x * size.width, y: point.y * size.height }
}

function isFinitePoint(point: NormalizedPoint) {
  return Number.isFinite(point.x) && Number.isFinite(point.y)
}
