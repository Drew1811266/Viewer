import type { ImageAnchor } from '../../app/review/annotationGeometry'
import {
  ANNOTATION_BADGE_FONT_PX,
  ANNOTATION_BADGE_RING_WIDTH,
  ANNOTATION_CORNER_RADIUS,
  ANNOTATION_HALO_COLOR,
  ANNOTATION_HALO_PER_SIDE,
  ANNOTATION_SELECTED_RING_RADIUS,
  ANNOTATION_SELECTED_RING_WIDTH,
  ANNOTATION_SELECTION_OFFSET,
  ANNOTATION_SELECTION_WIDTH,
  annotationArrowHead,
  annotationColorRgba,
  annotationFillRgba,
  annotationLineWidthCss,
  annotationRgb,
  annotationTextSizeCss,
  type ReviewAnchor,
  type ReviewAnnotationStyle,
  styleForAnchor,
  TEXT_NOTE_ACCENT_WIDTH,
  TEXT_NOTE_MAX_WIDTH,
  TEXT_NOTE_PADDING,
  TEXT_NOTE_RADIUS,
} from '../../app/review/annotationStyle'
import type { SavedImageFeedback } from '../../app/review/useImageReviewWorkbench'
import type { Point } from '../imagePreview/imageGeometry'
import type { MagnifierOverlayPainter } from '../imagePreview/magnifierOverlay'

export type AnnotationSceneAppearance = 'saved' | 'selected' | 'transient'

export interface AnnotationSceneItem {
  itemId: string | null
  ordinal: number | null
  anchor: ReviewAnchor
  style?: ReviewAnnotationStyle
  text?: string
  appearance: AnnotationSceneAppearance
}

export interface AnnotationSceneProjection {
  normalizedToLocal(point: Point): Point | null
  localBounds?: { width: number; height: number }
  sourceSize?: { width: number; height: number }
}

export interface AnnotationPaintOptions {
  color: string
  lineWidth: number
  drawOrdinals: boolean
  ordinalRadius: number
  selectionColor?: string
  scale?: number
}

export function buildAnnotationScene(input: {
  feedback: ReadonlyArray<SavedImageFeedback>
  selectedItemId: string | null
  transientAnchor: ReviewAnchor | null
  transientStyle?: ReviewAnnotationStyle | null
  transientText?: string | null
  /** Saved item whose draft is currently replacing its geometry or text. */
  editingItemId?: string | null
  editingOperation?: 'text' | 'geometry' | null
}): ReadonlyArray<AnnotationSceneItem> {
  const scene: AnnotationSceneItem[] = []
  const editingItemId = input.editingItemId ?? null
  for (const feedback of input.feedback) {
    if (!isImageAnchor(feedback.anchor)) continue
    const editing = editingItemId !== null && feedback.itemId === editingItemId
    const replacingGeometry =
      editing &&
      input.editingOperation === 'geometry' &&
      input.transientAnchor !== null &&
      isImageAnchor(input.transientAnchor)
    const anchor = replacingGeometry ? (input.transientAnchor as ReviewAnchor) : feedback.anchor
    const style = replacingGeometry ? (input.transientStyle ?? feedback.style) : feedback.style
    scene.push({
      itemId: feedback.itemId,
      ordinal: feedback.ordinal,
      anchor,
      ...(style === undefined || style === null ? {} : { style }),
      ...(anchor.kind === 'image_text'
        ? { text: editing ? (input.transientText ?? feedback.text) : feedback.text }
        : {}),
      appearance: replacingGeometry
        ? 'transient'
        : feedback.itemId === input.selectedItemId
          ? 'selected'
          : 'saved',
    })
  }
  // A transient item is only a new annotation; editing an existing one is
  // already represented by the replacement above, so it must not be drawn twice.
  if (
    editingItemId === null &&
    input.transientAnchor !== null &&
    isImageAnchor(input.transientAnchor)
  ) {
    scene.push({
      itemId: null,
      ordinal: null,
      anchor: input.transientAnchor,
      ...(input.transientStyle === undefined || input.transientStyle === null
        ? {}
        : { style: input.transientStyle }),
      ...(input.transientAnchor.kind === 'image_text' && input.transientText
        ? { text: input.transientText }
        : {}),
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
    const style = resolvedStyle(item)
    if (style !== null) {
      context.lineWidth = annotationLineWidthCss(style) * (options.scale ?? 1)
      context.strokeStyle = annotationColorRgba(style)
    }
    const transient = item.appearance === 'transient'
    if (!transient) paintAnnotationHalo(context, item, projection, style, options.scale ?? 1)
    context.setLineDash(transient ? [6, 4] : style?.dashed ? [8, 6] : [])
    if (item.appearance === 'selected' && options.selectionColor !== undefined) {
      paintSelectionFrame(context, item.anchor, projection, options.selectionColor)
    }
    if (paintAnchor(context, item.anchor, projection, style, options.scale ?? 1, transient))
      painted += 1
    context.restore()
  }
  for (const item of scene) {
    if (!isTextAnnotation(item)) continue
    context.save()
    paintTextNote(context, item, projection, options)
    context.restore()
  }
  if (options.drawOrdinals) {
    for (const item of scene) {
      if (item.ordinal === null || !isImageAnchor(item.anchor)) continue
      context.save()
      const style = resolvedStyle(item)
      if (style !== null) {
        context.lineWidth = annotationLineWidthCss(style) * (options.scale ?? 1)
        context.strokeStyle = annotationColorRgba(style)
        context.setLineDash(style.dashed ? [8, 6] : [])
      }
      paintOrdinal(
        context,
        item.anchor,
        item.ordinal,
        projection,
        options,
        item.appearance === 'selected',
        style === null ? options.color : annotationRgb(style),
      )
      context.restore()
    }
  }
  context.restore()
  return painted
}

function resolvedStyle(item: AnnotationSceneItem): ReviewAnnotationStyle | null {
  if (item.style !== undefined) return item.style
  return styleForAnchor(item.anchor)
}

export function annotationMarkerPoint(anchor: ReviewAnchor): Point | null {
  switch (anchor.kind) {
    case 'image_point':
    case 'image_text':
      return { x: anchor.x, y: anchor.y }
    case 'image_arrow':
    case 'image_line':
      return anchor.tail
    case 'image_rect':
    case 'image_ellipse':
      return { x: anchor.x + anchor.width, y: anchor.y }
    case 'image_stroke':
      return anchor.points.at(-1) ?? null
    case 'image_polygon':
      return anchor.points[0] ?? null
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
  if (projected === null) return projected
  if (anchor.kind === 'image_point') return projected
  if (projection.localBounds === undefined) return projected
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
        ordinalRadius: 10,
        scale: Math.min(frame.magnification, 2.5),
        selectionColor: annotationSelectionColor(context.canvas),
      },
    )
}

type ProjectedGeometry =
  | { kind: 'box'; left: number; top: number; right: number; bottom: number }
  | { kind: 'point'; point: Point }
  | { kind: 'path'; points: Point[] }

function isImageAnchor(anchor: ReviewAnchor): anchor is ImageAnchor {
  return anchor.kind.startsWith('image_')
}

function roundedRectPath(
  context: CanvasRenderingContext2D,
  x: number,
  y: number,
  width: number,
  height: number,
  radius: number,
) {
  const r = Math.max(0, Math.min(radius, Math.min(width, height) / 2))
  if (typeof context.roundRect === 'function') {
    context.roundRect(x, y, width, height, r)
    return
  }
  context.moveTo(x + r, y)
  context.lineTo(x + width - r, y)
  context.arcTo(x + width, y, x + width, y + r, r)
  context.lineTo(x + width, y + height - r)
  context.arcTo(x + width, y + height, x + width - r, y + height, r)
  context.lineTo(x + r, y + height)
  context.arcTo(x, y + height, x, y + height - r, r)
  context.lineTo(x, y + r)
  context.arcTo(x, y, x + r, y, r)
  context.closePath()
}

function paintSelectionFrame(
  context: CanvasRenderingContext2D,
  anchor: ReviewAnchor,
  projection: AnnotationSceneProjection,
  color: string,
) {
  if (!isImageAnchor(anchor)) return
  const geometry = projectedGeometry(anchor, projection)
  if (geometry === null) return
  const bounds =
    geometry.kind === 'box'
      ? { left: geometry.left, top: geometry.top, right: geometry.right, bottom: geometry.bottom }
      : geometry.kind === 'point'
        ? {
            left: geometry.point.x,
            top: geometry.point.y,
            right: geometry.point.x,
            bottom: geometry.point.y,
          }
        : {
            left: Math.min(...geometry.points.map((point) => point.x)),
            top: Math.min(...geometry.points.map((point) => point.y)),
            right: Math.max(...geometry.points.map((point) => point.x)),
            bottom: Math.max(...geometry.points.map((point) => point.y)),
          }
  const offset = ANNOTATION_SELECTION_OFFSET
  const previousLineWidth = context.lineWidth
  const previousStrokeStyle = context.strokeStyle
  context.save()
  context.setLineDash([])
  context.lineWidth = ANNOTATION_SELECTION_WIDTH
  context.strokeStyle = color
  context.beginPath()
  roundedRectPath(
    context,
    bounds.left - offset,
    bounds.top - offset,
    bounds.right - bounds.left + offset * 2,
    bounds.bottom - bounds.top + offset * 2,
    2,
  )
  context.stroke()
  context.lineWidth = previousLineWidth
  context.strokeStyle = previousStrokeStyle
  context.restore()
}

function paintAnnotationHalo(
  context: CanvasRenderingContext2D,
  item: AnnotationSceneItem,
  projection: AnnotationSceneProjection,
  style: ReviewAnnotationStyle | null,
  scale: number,
) {
  if (style === null || style.tool === 'highlighter') return
  const anchor = item.anchor
  if (!isImageAnchor(anchor)) return
  if (anchor.kind === 'image_point' || anchor.kind === 'image_text') return
  const width = annotationLineWidthCss(style) + ANNOTATION_HALO_PER_SIDE * 2
  const previousLineWidth = context.lineWidth
  const previousStrokeStyle = context.strokeStyle
  const previousFillStyle = context.fillStyle
  context.save()
  context.strokeStyle = ANNOTATION_HALO_COLOR
  context.fillStyle = ANNOTATION_HALO_COLOR
  context.lineWidth = width
  context.setLineDash([])
  context.lineJoin = 'round'
  context.lineCap = 'round'
  if (anchor.kind === 'image_rect') {
    const box = projectedBox(anchor, projection)
    if (box !== null) {
      context.beginPath()
      roundedRectPath(
        context,
        Math.min(box.start.x, box.end.x),
        Math.min(box.start.y, box.end.y),
        Math.abs(box.end.x - box.start.x),
        Math.abs(box.end.y - box.start.y),
        ANNOTATION_CORNER_RADIUS * scale,
      )
      context.stroke()
    }
  } else if (anchor.kind === 'image_ellipse') {
    const box = projectedBox(anchor, projection)
    if (box !== null) {
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
    }
  } else if (anchor.kind === 'image_polygon') {
    if (anchor.points.length >= 3) {
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
      if (started) {
        context.closePath()
        context.stroke()
      }
    }
  } else if (anchor.kind === 'image_line') {
    const tail = projection.normalizedToLocal(anchor.tail)
    const head = projection.normalizedToLocal(anchor.head)
    if (tail !== null && head !== null) {
      context.beginPath()
      context.moveTo(tail.x, tail.y)
      context.lineTo(head.x, head.y)
      context.stroke()
      if (style.tool === 'ruler') {
        const delta = { x: head.x - tail.x, y: head.y - tail.y }
        const length = Math.hypot(delta.x, delta.y)
        if (length > 0) {
          const perpendicular = { x: -delta.y / length, y: delta.x / length }
          const tick = 7
          context.beginPath()
          for (const point of [tail, head]) {
            context.moveTo(point.x - perpendicular.x * tick, point.y - perpendicular.y * tick)
            context.lineTo(point.x + perpendicular.x * tick, point.y + perpendicular.y * tick)
          }
          context.stroke()
        }
      }
    }
  } else if (anchor.kind === 'image_stroke') {
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
    if (started) context.stroke()
  } else if (anchor.kind === 'image_arrow') {
    const tail = projection.normalizedToLocal(anchor.tail)
    const head = projection.normalizedToLocal(anchor.head)
    const shape = tail === null || head === null ? null : arrowShape(tail, head, width)
    if (shape !== null) {
      context.beginPath()
      context.moveTo(shape.tail.x, shape.tail.y)
      context.lineTo(shape.notch.x, shape.notch.y)
      context.stroke()
      context.beginPath()
      context.moveTo(shape.tip.x, shape.tip.y)
      context.lineTo(shape.p1.x, shape.p1.y)
      context.lineTo(shape.notch.x, shape.notch.y)
      context.lineTo(shape.p2.x, shape.p2.y)
      context.closePath()
      context.fill()
    }
  }
  context.lineWidth = previousLineWidth
  context.strokeStyle = previousStrokeStyle
  context.fillStyle = previousFillStyle
  context.restore()
}

function arrowShape(tail: Point, head: Point, lineWidth: number) {
  const deltaX = head.x - tail.x
  const deltaY = head.y - tail.y
  const length = Math.hypot(deltaX, deltaY)
  if (length <= 0) return null
  const nominal = annotationArrowHead(lineWidth)
  const headLength = Math.min(nominal.length, length * 0.72)
  const half = nominal.halfWidth * (headLength / nominal.length)
  const unit = { x: deltaX / length, y: deltaY / length }
  const perpendicular = { x: -unit.y, y: unit.x }
  const base = { x: head.x - unit.x * headLength, y: head.y - unit.y * headLength }
  const notch = {
    x: head.x - unit.x * headLength * 0.72,
    y: head.y - unit.y * headLength * 0.72,
  }
  return {
    tail,
    tip: head,
    notch,
    p1: { x: base.x + perpendicular.x * half, y: base.y + perpendicular.y * half },
    p2: { x: base.x - perpendicular.x * half, y: base.y - perpendicular.y * half },
  }
}

function paintAnchor(
  context: CanvasRenderingContext2D,
  anchor: ReviewAnchor,
  projection: AnnotationSceneProjection,
  style: ReviewAnnotationStyle | null,
  scale: number,
  transient: boolean,
): boolean {
  switch (anchor.kind) {
    case 'image_point': {
      if (!transient) return true
      const point = projection.normalizedToLocal(anchor)
      if (point === null) return false
      context.fillStyle = context.strokeStyle
      context.beginPath()
      context.arc(point.x, point.y, 4, 0, Math.PI * 2)
      context.fill()
      return true
    }
    case 'image_arrow':
      return paintArrow(context, anchor, projection, style, scale, transient)
    case 'image_rect': {
      const box = projectedBox(anchor, projection)
      if (box === null) return false
      const x = Math.min(box.start.x, box.end.x)
      const y = Math.min(box.start.y, box.end.y)
      const width = Math.abs(box.end.x - box.start.x)
      const height = Math.abs(box.end.y - box.start.y)
      context.beginPath()
      roundedRectPath(context, x, y, width, height, ANNOTATION_CORNER_RADIUS * scale)
      if (style !== null) context.fillStyle = annotationFillRgba(style)
      if (style !== null) context.fill()
      context.stroke()
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
      if (style !== null) context.fillStyle = annotationFillRgba(style)
      if (style !== null) context.fill()
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
      if (style?.tool === 'highlighter') {
        context.save()
        context.globalCompositeOperation = 'multiply'
        context.stroke()
        context.restore()
      } else {
        context.stroke()
      }
      return true
    }
    case 'image_text':
      return transient ? paintTextAnchor(context, anchor, projection, style, scale) : true
    case 'image_line': {
      const tail = projection.normalizedToLocal(anchor.tail)
      const head = projection.normalizedToLocal(anchor.head)
      if (tail === null || head === null) return false
      context.beginPath()
      context.moveTo(tail.x, tail.y)
      context.lineTo(head.x, head.y)
      context.stroke()
      if (style?.tool === 'ruler') paintRulerDetails(context, anchor, projection, style)
      return true
    }
    case 'image_polygon': {
      if (anchor.points.length < 3) return false
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
      context.closePath()
      if (style !== null) context.fillStyle = annotationFillRgba(style)
      if (style !== null) context.fill()
      context.stroke()
      return true
    }
    case 'asset':
    case 'video_point':
    case 'video_range':
      return false
  }
}

function paintTextAnchor(
  context: CanvasRenderingContext2D,
  anchor: Extract<ImageAnchor, { kind: 'image_text' }>,
  projection: AnnotationSceneProjection,
  style: ReviewAnnotationStyle | null,
  scale: number,
): boolean {
  const point = projection.normalizedToLocal(anchor)
  if (point === null) return false
  const resolved = style ?? styleForAnchor(anchor)
  if (resolved === null) return false
  context.fillStyle = annotationColorRgba(resolved)
  context.beginPath()
  context.arc(point.x, point.y, 4 * Math.max(1, scale), 0, Math.PI * 2)
  context.fill()
  return true
}

function paintRulerDetails(
  context: CanvasRenderingContext2D,
  anchor: Extract<ImageAnchor, { kind: 'image_line' }>,
  projection: AnnotationSceneProjection,
  style: ReviewAnnotationStyle,
) {
  const tail = projection.normalizedToLocal(anchor.tail)
  const head = projection.normalizedToLocal(anchor.head)
  if (tail === null || head === null) return
  const delta = { x: head.x - tail.x, y: head.y - tail.y }
  const length = Math.hypot(delta.x, delta.y)
  if (length <= 0) return
  const unit = { x: delta.x / length, y: delta.y / length }
  const perpendicular = { x: -unit.y, y: unit.x }
  const tick = 7
  context.save()
  context.setLineDash([])
  context.strokeStyle = annotationColorRgba(style)
  context.lineWidth = annotationLineWidthCss(style)
  context.beginPath()
  for (const point of [tail, head]) {
    context.moveTo(point.x - perpendicular.x * tick, point.y - perpendicular.y * tick)
    context.lineTo(point.x + perpendicular.x * tick, point.y + perpendicular.y * tick)
  }
  context.stroke()

  const source =
    projection.sourceSize === undefined
      ? Math.round(length)
      : Math.round(
          Math.hypot(
            (anchor.head.x - anchor.tail.x) * projection.sourceSize.width,
            (anchor.head.y - anchor.tail.y) * projection.sourceSize.height,
          ),
        )
  const label = `${source} px`
  context.font = '600 11px system-ui, sans-serif'
  context.textAlign = 'center'
  context.textBaseline = 'middle'
  const middle = { x: (tail.x + head.x) / 2, y: (tail.y + head.y) / 2 }
  const width = context.measureText(label).width + 12
  const chipHeight = 18
  const left = middle.x - width / 2
  const top = middle.y - 24
  context.beginPath()
  roundedRectPath(context, left, top, width, chipHeight, 4)
  context.fillStyle = 'rgba(255, 255, 255, 0.94)'
  context.fill()
  context.strokeStyle = '#CDD7E7'
  context.lineWidth = 1
  context.stroke()
  context.fillStyle = '#171A21'
  context.fillText(label, middle.x, top + chipHeight / 2 + 0.5)
  context.restore()
}

function isTextAnnotation(item: AnnotationSceneItem): boolean {
  return item.anchor.kind === 'image_text' || item.style?.tool === 'text'
}

function paintTextNote(
  context: CanvasRenderingContext2D,
  item: AnnotationSceneItem,
  projection: AnnotationSceneProjection,
  options: AnnotationPaintOptions,
) {
  const anchor = item.anchor
  if (anchor.kind !== 'image_text') return
  const text = item.text?.trim()
  if (text === undefined || text.length === 0) return
  const point = projection.normalizedToLocal(anchor)
  if (point === null) return
  const style = item.style ?? styleForAnchor(anchor)
  if (style === null) return
  const scale = options.scale ?? 1
  const fontSize = annotationTextSizeCss(style) * scale
  context.font = `600 ${fontSize}px system-ui, sans-serif`
  const padding = TEXT_NOTE_PADDING * scale
  const accent = TEXT_NOTE_ACCENT_WIDTH * scale
  const maxTextWidth = TEXT_NOTE_MAX_WIDTH * scale - padding * 2 - accent
  const words = text.split(/\s+/)
  const lines: string[] = []
  let current = ''
  for (const word of words) {
    const candidate = current.length === 0 ? word : `${current} ${word}`
    if (context.measureText(candidate).width > maxTextWidth && current.length > 0) {
      lines.push(current)
      current = word
    } else {
      current = candidate
    }
    while (context.measureText(current).width > maxTextWidth && current.length > 1) {
      let cut = current.length - 1
      while (cut > 1 && context.measureText(current.slice(0, cut)).width > maxTextWidth) cut -= 1
      lines.push(current.slice(0, cut))
      current = current.slice(cut)
    }
  }
  if (current.length > 0) lines.push(current)
  const lineHeight = fontSize * 1.35
  const width =
    Math.min(
      maxTextWidth,
      Math.max(...lines.map((line) => context.measureText(line).width), fontSize * 4),
    ) +
    padding * 2 +
    accent
  const height = lines.length * lineHeight + padding * 2
  const left = point.x
  const top = point.y
  context.save()
  context.beginPath()
  roundedRectPath(context, left, top, width, height, TEXT_NOTE_RADIUS * scale)
  context.shadowColor = 'rgba(23, 26, 33, 0.12)'
  context.shadowBlur = 6 * scale
  context.shadowOffsetY = 2 * scale
  context.fillStyle = '#FFFFFF'
  context.fill()
  context.shadowColor = 'transparent'
  context.shadowBlur = 0
  context.shadowOffsetY = 0
  context.beginPath()
  roundedRectPath(
    context,
    left,
    top,
    accent,
    height,
    Math.min(TEXT_NOTE_RADIUS * scale, accent / 2),
  )
  context.fillStyle = annotationRgb(style)
  context.fill()
  context.beginPath()
  roundedRectPath(context, left, top, width, height, TEXT_NOTE_RADIUS * scale)
  context.strokeStyle = annotationRgb(style)
  context.globalAlpha = 0.35
  context.lineWidth = Math.max(1, scale)
  context.stroke()
  context.globalAlpha = 1
  context.fillStyle = '#3B2F13'
  context.textAlign = 'left'
  context.textBaseline = 'top'
  for (const [index, line] of lines.entries()) {
    context.fillText(line, left + accent + padding, top + padding + index * lineHeight)
  }
  context.restore()
}

function paintArrow(
  context: CanvasRenderingContext2D,
  anchor: Extract<ImageAnchor, { kind: 'image_arrow' }>,
  projection: AnnotationSceneProjection,
  style: ReviewAnnotationStyle | null,
  scale: number,
  transient: boolean,
): boolean {
  const tail = projection.normalizedToLocal(anchor.tail)
  const head = projection.normalizedToLocal(anchor.head)
  if (tail === null || head === null) return false
  const width = style === null ? context.lineWidth : annotationLineWidthCss(style) * scale
  const shape = arrowShape(tail, head, width)
  if (shape === null) return false
  context.beginPath()
  context.moveTo(shape.tail.x, shape.tail.y)
  context.lineTo(shape.notch.x, shape.notch.y)
  context.stroke()
  context.save()
  context.setLineDash([])
  context.beginPath()
  context.moveTo(shape.tip.x, shape.tip.y)
  context.lineTo(shape.p1.x, shape.p1.y)
  context.lineTo(shape.notch.x, shape.notch.y)
  context.lineTo(shape.p2.x, shape.p2.y)
  context.closePath()
  if (transient) {
    context.stroke()
  } else {
    context.fillStyle = context.strokeStyle
    context.fill()
  }
  context.restore()
  return true
}

function paintOrdinal(
  context: CanvasRenderingContext2D,
  anchor: ImageAnchor,
  ordinal: number,
  projection: AnnotationSceneProjection,
  options: AnnotationPaintOptions,
  selected: boolean,
  color: string,
) {
  const point = annotationOrdinalPoint(anchor, projection, options)
  if (point === null) return
  const radius = options.ordinalRadius
  const previousLineWidth = context.lineWidth
  const previousStrokeStyle = context.strokeStyle
  const previousFillStyle = context.fillStyle
  context.save()
  context.setLineDash([])
  if (selected) {
    context.beginPath()
    context.arc(point.x, point.y, ANNOTATION_SELECTED_RING_RADIUS, 0, Math.PI * 2)
    context.strokeStyle = options.selectionColor ?? '#C5BAFF'
    context.lineWidth = ANNOTATION_SELECTED_RING_WIDTH
    context.stroke()
  }
  context.beginPath()
  context.arc(point.x, point.y, radius, 0, Math.PI * 2)
  if (selected) {
    context.fillStyle = color
    context.fill()
  } else {
    context.shadowColor = 'rgba(23, 26, 33, 0.14)'
    context.shadowBlur = 2
    context.shadowOffsetY = 1
    context.fillStyle = '#FFFFFF'
    context.fill()
    context.shadowColor = 'transparent'
    context.shadowBlur = 0
    context.shadowOffsetY = 0
    context.strokeStyle = color
    context.lineWidth = ANNOTATION_BADGE_RING_WIDTH
    context.stroke()
  }
  context.fillStyle = selected ? '#FFFFFF' : color
  context.font = `600 ${ANNOTATION_BADGE_FONT_PX}px system-ui, sans-serif`
  context.textAlign = 'center'
  context.textBaseline = 'middle'
  context.fillText(String(ordinal), point.x, point.y + 0.5)
  context.lineWidth = previousLineWidth
  context.strokeStyle = previousStrokeStyle
  context.fillStyle = previousFillStyle
  context.restore()
}

function projectedGeometry(
  anchor: ImageAnchor,
  projection: AnnotationSceneProjection,
): ProjectedGeometry | null {
  switch (anchor.kind) {
    case 'image_point':
    case 'image_text': {
      const point = projection.normalizedToLocal(anchor)
      return point === null ? null : { kind: 'point', point }
    }
    case 'image_arrow':
    case 'image_line': {
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
    case 'image_polygon': {
      const points = anchor.points.flatMap((point) => {
        const projected = projection.normalizedToLocal(point)
        return projected === null ? [] : [projected]
      })
      return points.length === 0 ? null : { kind: 'path', points: [...points, points[0] as Point] }
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
      : anchor.kind === 'image_arrow' ||
          anchor.kind === 'image_line' ||
          anchor.kind === 'image_polygon'
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

function annotationSelectionColor(canvas: HTMLCanvasElement): string {
  try {
    return (
      getComputedStyle(canvas).getPropertyValue('--viewer-secondary-container').trim() ||
      'Highlight'
    )
  } catch {
    return 'Highlight'
  }
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
