import type { ReviewAnchor, SavedImageFeedback } from '../../app/review/useImageReviewWorkbench'
import type { Point } from '../imagePreview/imageGeometry'

export type AnnotationSceneAppearance = 'saved' | 'selected' | 'transient'

export interface AnnotationSceneItem {
  itemId: string | null
  ordinal: number | null
  anchor: ReviewAnchor
  appearance: AnnotationSceneAppearance
}

export interface AnnotationSceneProjection {
  normalizedToLocal(point: Point): Point | null
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
  if (anchor.kind === 'image_rect') return { x: anchor.x + anchor.width, y: anchor.y }
  if (anchor.kind === 'image_stroke') return anchor.points.at(-1) ?? null
  return null
}

function isImageAnchor(
  anchor: ReviewAnchor,
): anchor is Extract<ReviewAnchor, { kind: 'image_rect' | 'image_stroke' }> {
  return anchor.kind === 'image_rect' || anchor.kind === 'image_stroke'
}

function paintAnchor(
  context: CanvasRenderingContext2D,
  anchor: ReviewAnchor,
  projection: AnnotationSceneProjection,
): boolean {
  if (anchor.kind === 'image_rect') {
    const start = projection.normalizedToLocal({ x: anchor.x, y: anchor.y })
    const end = projection.normalizedToLocal({
      x: anchor.x + anchor.width,
      y: anchor.y + anchor.height,
    })
    if (start === null || end === null) return false
    context.strokeRect(start.x, start.y, end.x - start.x, end.y - start.y)
    return true
  }
  if (anchor.kind !== 'image_stroke') return false
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

function paintOrdinal(
  context: CanvasRenderingContext2D,
  anchor: ReviewAnchor,
  ordinal: number,
  projection: AnnotationSceneProjection,
  options: AnnotationPaintOptions,
) {
  const marker = annotationMarkerPoint(anchor)
  const projected = marker === null ? null : projection.normalizedToLocal(marker)
  if (projected === null) return
  context.beginPath()
  context.fillStyle = options.color
  context.arc(projected.x, projected.y, options.ordinalRadius, 0, Math.PI * 2)
  context.fill()
  context.fillStyle = '#fff'
  context.font = '700 12px system-ui, sans-serif'
  context.textAlign = 'center'
  context.textBaseline = 'middle'
  context.fillText(String(ordinal), projected.x, projected.y)
}
