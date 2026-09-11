import type { ImageAnchor } from '../../app/review/annotationGeometry'
import type { ReviewAnchor, SavedImageFeedback } from '../../app/review/useImageReviewWorkbench'
import type { Point } from '../imagePreview/imageGeometry'
import type { MagnifierOverlayPainter } from '../imagePreview/magnifierOverlay'

export type AnnotationSceneAppearance = 'saved' | 'selected' | 'transient'

export interface AnnotationSceneItem {
  itemId: string | null
  ordinal: number | null
  anchor: ReviewAnchor
  appearance: AnnotationSceneAppearance
}

export interface AnnotationSceneProjection {
  normalizedToLocal(point: Point): Point | null
  localBounds?: { width: number; height: number }
}

export interface AnnotationPaintOptions {
  color: string
  lineWidth: number
  drawOrdinals: boolean
  ordinalRadius: number
}

export function buildAnnotationScene(input: {
  feedback: ReadonlyArray<SavedImageFeedback>
  selectedItemId: string | null
  transientAnchor: ReviewAnchor | null
}): ReadonlyArray<AnnotationSceneItem> {
  const scene: AnnotationSceneItem[] = []
  for (const feedback of input.feedback) {
    if (!isImageAnchor(feedback.anchor)) continue
    scene.push({
      itemId: feedback.itemId,
      ordinal: feedback.ordinal,
      anchor: feedback.anchor,
      appearance: feedback.itemId === input.selectedItemId ? 'selected' : 'saved',
    })
  }
  if (input.transientAnchor !== null && isImageAnchor(input.transientAnchor)) {
    scene.push({
      itemId: null,
      ordinal: null,
      anchor: input.transientAnchor,
      appearance: 'transient',
    })
  }
  return scene
}

export function paintAnnotationScene(
  context: CanvasRenderingContext2D,
  scene: ReadonlyArray<AnnotationSceneItem>,
  projection: AnnotationSceneProjection,
  options: AnnotationPaintOptions,
): number {
  let painted = 0
  context.save()
  context.lineCap = 'round'
  context.lineJoin = 'round'
  context.lineWidth = options.lineWidth
  context.strokeStyle = options.color
  for (const item of scene) {
    context.save()
    if (item.appearance === 'transient') context.setLineDash([6, 4])
    const itemPainted = paintAnchor(context, item.anchor, projection)
    if (itemPainted) {
      painted += 1
      if (options.drawOrdinals && item.ordinal !== null) {
        paintOrdinal(context, item.anchor, item.ordinal, projection, options)
      }
    }
    context.restore()
  }
  context.restore()
  return painted
}

export function annotationMarkerPoint(anchor: ReviewAnchor): Point | null {
  switch (anchor.kind) {
    case 'image_point':
      return { x: anchor.x, y: anchor.y }
    case 'image_arrow':
      return anchor.tail
    case 'image_rect':
    case 'image_ellipse':
      return { x: anchor.x + anchor.width, y: anchor.y }
    case 'image_stroke':
      return anchor.points.at(-1) ?? null
    case 'asset':
    case 'video_point':
    case 'video_range':
      return null
  }
}

export function annotationOrdinalPoint(
  anchor: ImageAnchor,
  projection: AnnotationSceneProjection,
  options: Pick<AnnotationPaintOptions, 'lineWidth' | 'ordinalRadius'>,
): Point | null {
  const marker = annotationMarkerPoint(anchor)
  if (marker === null) return null
  const projected = projection.normalizedToLocal(marker)
  if (projected === null || projection.localBounds === undefined) return projected
  const geometry = projectedGeometry(anchor, projection)
  if (geometry === null) return null
  const clearance = options.ordinalRadius + options.lineWidth + 4
  for (const candidate of ordinalCandidates(anchor, geometry, clearance)) {
    if (
      fitsBounds(candidate, projection.localBounds, options.ordinalRadius) &&
      !overlapsGeometry(candidate, geometry, clearance)
    ) {
      return candidate
    }
  }
  return null
}

export function createAnnotationMagnifierPainter(
  scene: ReadonlyArray<AnnotationSceneItem>,
): MagnifierOverlayPainter {
  return (context, frame) =>
    paintAnnotationScene(
      context,
      scene,
      {
        normalizedToLocal: frame.projection.normalizedToLens,
        localBounds: frame.projection.lensSize,
      },
      {
        color: annotationCanvasColor(context.canvas),
        lineWidth: clamp(2 * frame.magnification, 2, 5),
        drawOrdinals: true,
        ordinalRadius: 14,
      },
    )
}

function isImageAnchor(anchor: ReviewAnchor): anchor is ImageAnchor {
  return anchor.kind.startsWith('image_')
}

function paintAnchor(
  context: CanvasRenderingContext2D,
  anchor: ReviewAnchor,
  projection: AnnotationSceneProjection,
): boolean {
  switch (anchor.kind) {
    case 'image_point': {
      const point = projection.normalizedToLocal(anchor)
      if (point === null) return false
      context.beginPath()
      context.arc(point.x, point.y, 7, 0, Math.PI * 2)
      context.stroke()
      return true
    }
    case 'image_arrow':
      return paintArrow(context, anchor, projection)
    case 'image_rect': {
      const box = projectedBox(anchor, projection)
      if (box === null) return false
      context.strokeRect(box.start.x, box.start.y, box.end.x - box.start.x, box.end.y - box.start.y)
      return true
    }
    case 'image_ellipse': {
      const box = projectedBox(anchor, projection)
      if (box === null) return false
      context.beginPath()
      context.ellipse(
        (box.start.x + box.end.x) / 2,
        (box.start.y + box.end.y) / 2,
        Math.abs(box.end.x - box.start.x) / 2,
        Math.abs(box.end.y - box.start.y) / 2,
        0,
        0,
        Math.PI * 2,
      )
      context.stroke()
      return true
    }
    case 'image_stroke': {
      context.beginPath()
      let started = false
      for (const point of anchor.points) {
        const projected = projection.normalizedToLocal(point)
        if (projected === null) continue
        if (started) context.lineTo(projected.x, projected.y)
        else {
          context.moveTo(projected.x, projected.y)
          started = true
        }
      }
      if (!started) return false
      context.stroke()
      return true
    }
    case 'asset':
    case 'video_point':
    case 'video_range':
      return false
  }
}

function paintArrow(
  context: CanvasRenderingContext2D,
  anchor: Extract<ImageAnchor, { kind: 'image_arrow' }>,
  projection: AnnotationSceneProjection,
): boolean {
  const tail = projection.normalizedToLocal(anchor.tail)
  const head = projection.normalizedToLocal(anchor.head)
  if (tail === null || head === null) return false
  const delta = { x: head.x - tail.x, y: head.y - tail.y }
  const length = Math.hypot(delta.x, delta.y)
  if (length <= 0) return false
  const unit = { x: delta.x / length, y: delta.y / length }
  const perpendicular = { x: -unit.y, y: unit.x }
  const headLength = clamp(length * 0.24, 8, 16)
  const wing = headLength * 0.46
  const base = { x: head.x - unit.x * headLength, y: head.y - unit.y * headLength }
  const left = { x: base.x + perpendicular.x * wing, y: base.y + perpendicular.y * wing }
  const right = { x: base.x - perpendicular.x * wing, y: base.y - perpendicular.y * wing }
  context.beginPath()
  context.moveTo(tail.x, tail.y)
  context.lineTo(head.x, head.y)
  context.moveTo(left.x, left.y)
  context.lineTo(head.x, head.y)
  context.lineTo(right.x, right.y)
  context.stroke()
  return true
}

function paintOrdinal(
  context: CanvasRenderingContext2D,
  anchor: ReviewAnchor,
  ordinal: number,
  projection: AnnotationSceneProjection,
  options: AnnotationPaintOptions,
) {
  if (!isImageAnchor(anchor)) return
  const projected = annotationOrdinalPoint(anchor, projection, options)
  if (projected === null) return
  // White disc with a colored ring, matching the native badge style: one arc
  // path is filled (with a soft shadow) and then stroked as the ring. Stroke
  // state is restored explicitly afterwards so subsequent geometry keeps the
  // caller's line width and dash.
  const previousLineWidth = context.lineWidth
  const previousStrokeStyle = context.strokeStyle
  context.beginPath()
  context.arc(projected.x, projected.y, options.ordinalRadius, 0, Math.PI * 2)
  context.save()
  context.shadowColor = 'rgba(0, 0, 0, 0.22)'
  context.shadowBlur = 5
  context.shadowOffsetY = 1
  context.fillStyle = '#fff'
  context.fill()
  context.restore()
  context.lineWidth = 2.5
  context.strokeStyle = options.color
  context.stroke()
  context.fillStyle = options.color
  context.font = '700 12px system-ui, sans-serif'
  context.textAlign = 'center'
  context.textBaseline = 'middle'
  context.fillText(String(ordinal), projected.x, projected.y)
  context.lineWidth = previousLineWidth
  context.strokeStyle = previousStrokeStyle
}

type ProjectedGeometry =
  | { kind: 'point'; point: Point }
  | { kind: 'path'; points: ReadonlyArray<Point> }
  | { kind: 'box'; left: number; top: number; right: number; bottom: number }

function projectedGeometry(
  anchor: ImageAnchor,
  projection: AnnotationSceneProjection,
): ProjectedGeometry | null {
  switch (anchor.kind) {
    case 'image_point': {
      const point = projection.normalizedToLocal(anchor)
      return point === null ? null : { kind: 'point', point }
    }
    case 'image_arrow': {
      const tail = projection.normalizedToLocal(anchor.tail)
      const head = projection.normalizedToLocal(anchor.head)
      return tail === null || head === null ? null : { kind: 'path', points: [tail, head] }
    }
    case 'image_stroke': {
      const points = anchor.points.flatMap((point) => {
        const projected = projection.normalizedToLocal(point)
        return projected === null ? [] : [projected]
      })
      return points.length === 0 ? null : { kind: 'path', points }
    }
    case 'image_rect':
    case 'image_ellipse': {
      const box = projectedBox(anchor, projection)
      if (box === null) return null
      return {
        kind: 'box',
        left: Math.min(box.start.x, box.end.x),
        top: Math.min(box.start.y, box.end.y),
        right: Math.max(box.start.x, box.end.x),
        bottom: Math.max(box.start.y, box.end.y),
      }
    }
  }
}

function projectedBox(
  anchor: Extract<ImageAnchor, { kind: 'image_rect' | 'image_ellipse' }>,
  projection: AnnotationSceneProjection,
) {
  const start = projection.normalizedToLocal({ x: anchor.x, y: anchor.y })
  const end = projection.normalizedToLocal({
    x: anchor.x + anchor.width,
    y: anchor.y + anchor.height,
  })
  return start === null || end === null ? null : { start, end }
}

function ordinalCandidates(
  anchor: ImageAnchor,
  geometry: ProjectedGeometry,
  distance: number,
): ReadonlyArray<Point> {
  if (geometry.kind === 'box') {
    const { left, top, right, bottom } = geometry
    return [
      { x: right + distance, y: top - distance },
      { x: left - distance, y: top - distance },
      { x: right + distance, y: bottom + distance },
      { x: left - distance, y: bottom + distance },
      { x: right + distance, y: (top + bottom) / 2 },
      { x: left - distance, y: (top + bottom) / 2 },
      { x: (left + right) / 2, y: top - distance },
      { x: (left + right) / 2, y: bottom + distance },
    ]
  }
  const reference =
    geometry.kind === 'point'
      ? geometry.point
      : anchor.kind === 'image_arrow'
        ? (geometry.points[0] as Point)
        : (geometry.points.at(-1) as Point)
  return [
    { x: reference.x + distance, y: reference.y - distance },
    { x: reference.x - distance, y: reference.y - distance },
    { x: reference.x + distance, y: reference.y + distance },
    { x: reference.x - distance, y: reference.y + distance },
    { x: reference.x, y: reference.y - distance },
    { x: reference.x + distance, y: reference.y },
    { x: reference.x, y: reference.y + distance },
    { x: reference.x - distance, y: reference.y },
  ]
}

function fitsBounds(point: Point, bounds: { width: number; height: number }, radius: number) {
  return (
    point.x >= radius &&
    point.y >= radius &&
    point.x <= bounds.width - radius &&
    point.y <= bounds.height - radius
  )
}

function overlapsGeometry(point: Point, geometry: ProjectedGeometry, clearance: number) {
  if (geometry.kind === 'point') return pointDistance(point, geometry.point) < clearance + 7
  if (geometry.kind === 'box') {
    return (
      point.x > geometry.left - clearance &&
      point.x < geometry.right + clearance &&
      point.y > geometry.top - clearance &&
      point.y < geometry.bottom + clearance
    )
  }
  if (geometry.points.length === 1) {
    return pointDistance(point, geometry.points[0] as Point) < clearance
  }
  for (let index = 1; index < geometry.points.length; index += 1) {
    if (
      pointToSegmentDistance(
        point,
        geometry.points[index - 1] as Point,
        geometry.points[index] as Point,
      ) < clearance
    ) {
      return true
    }
  }
  return false
}

function pointToSegmentDistance(point: Point, start: Point, end: Point) {
  const deltaX = end.x - start.x
  const deltaY = end.y - start.y
  if (deltaX === 0 && deltaY === 0) return pointDistance(point, start)
  const progress = clamp(
    ((point.x - start.x) * deltaX + (point.y - start.y) * deltaY) /
      (deltaX * deltaX + deltaY * deltaY),
    0,
    1,
  )
  return pointDistance(point, {
    x: start.x + progress * deltaX,
    y: start.y + progress * deltaY,
  })
}

function pointDistance(left: Point, right: Point) {
  return Math.hypot(left.x - right.x, left.y - right.y)
}

function annotationCanvasColor(canvas: HTMLCanvasElement): string {
  try {
    return getComputedStyle(canvas).getPropertyValue('--review-annotation').trim() || 'CanvasText'
  } catch {
    return 'CanvasText'
  }
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.max(minimum, Math.min(maximum, value))
}
