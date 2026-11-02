import {
  annotationColorRgba,
  annotationFillRgba,
  annotationRgb,
  annotationTextSizeCss,
  annotationTextSizeLabel,
  annotationToneRgba,
  type ReviewAnchor,
  type ReviewAnnotationStyle,
  styleForAnchor,
} from '../../app/review/annotationStyle'
import type {
  ImageReviewWorkbenchController,
  SavedImageFeedback,
} from '../../app/review/useImageReviewWorkbench'
import type { ImagePreviewProjection } from '../imagePreview/ImagePreviewSurface'

/** Native renderer draws lines and strokes; notes, ruler ticks and polygon fills stay in DOM. */
export default function AnnotationOverlayDetails({
  controller,
  projection,
}: {
  controller: ImageReviewWorkbenchController
  projection: ImagePreviewProjection
}) {
  const width = projection.stageRect.width
  const height = projection.stageRect.height
  const local = (point: { x: number; y: number }) => {
    const projected = projection.normalizedToStage(point)
    return projected === null
      ? null
      : { x: projected.x - projection.stageRect.left, y: projected.y - projection.stageRect.top }
  }
  const phase = controller.editor.phase
  // While an existing annotation is being moved, resized or retexted the
  // native stroke follows the draft anchor. These DOM-only details must follow
  // the same draft, otherwise the fill and text note stay pinned to the old
  // geometry until the save lands.
  const editing =
    phase.status === 'editing' || phase.status === 'saving' || phase.status === 'save_error'
      ? phase
      : null
  const editingItemId = editing?.sourceItemId ?? null
  const effectiveAnchor = (feedback: SavedImageFeedback): ReviewAnchor =>
    editing !== null &&
    feedback.itemId === editingItemId &&
    editing.draftAnchor.kind.startsWith('image_')
      ? editing.draftAnchor
      : feedback.anchor
  const effectiveStyle = (feedback: SavedImageFeedback): ReviewAnnotationStyle | null =>
    styleForAnchor(
      effectiveAnchor(feedback),
      editing !== null && feedback.itemId === editingItemId && editing.style !== undefined
        ? editing.style
        : feedback.style,
    )
  const effectiveText = (feedback: SavedImageFeedback): string =>
    editing !== null && feedback.itemId === editingItemId && editing.operation !== 'geometry'
      ? editing.text
      : feedback.text

  return (
    <div className="annotation-overlay-details" aria-hidden="true">
      <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`}>
        <title>标注辅助层</title>
        {controller.feedback.map((feedback) => {
          const anchor = effectiveAnchor(feedback)
          const style = effectiveStyle(feedback)
          if (style === null) return null
          if (anchor.kind === 'image_rect' || anchor.kind === 'image_ellipse') {
            const start = local({ x: anchor.x, y: anchor.y })
            const end = local({
              x: anchor.x + anchor.width,
              y: anchor.y + anchor.height,
            })
            if (start === null || end === null) return null
            const x = Math.min(start.x, end.x)
            const y = Math.min(start.y, end.y)
            const boxWidth = Math.abs(end.x - start.x)
            const boxHeight = Math.abs(end.y - start.y)
            return anchor.kind === 'image_rect' ? (
              <rect
                key={feedback.itemId}
                x={x}
                y={y}
                width={boxWidth}
                height={boxHeight}
                fill={annotationFillRgba(style)}
              />
            ) : (
              <ellipse
                key={feedback.itemId}
                cx={x + boxWidth / 2}
                cy={y + boxHeight / 2}
                rx={boxWidth / 2}
                ry={boxHeight / 2}
                fill={annotationFillRgba(style)}
              />
            )
          }
          if (anchor.kind === 'image_polygon') {
            const points = anchor.points.flatMap((point) => {
              const projected = local(point)
              return projected === null ? [] : [`${projected.x},${projected.y}`]
            })
            return points.length < 3 ? null : (
              <polygon
                key={feedback.itemId}
                points={points.join(' ')}
                fill={annotationFillRgba(style)}
              />
            )
          }
          if (anchor.kind === 'image_line' && style.tool === 'ruler') {
            return (
              <RulerDetails
                key={feedback.itemId}
                anchor={anchor}
                style={style}
                local={local}
                sourceSize={projection.sourceSize}
              />
            )
          }
          return null
        })}
      </svg>
      {controller.feedback.map((feedback) => {
        const anchor = effectiveAnchor(feedback)
        if (anchor.kind !== 'image_text') return null
        const style = effectiveStyle(feedback)
        if (style === null) return null
        const point = local(anchor)
        if (point === null || point.x < 0 || point.y < 0 || point.x > width || point.y > height)
          return null
        return (
          <div
            key={feedback.itemId}
            className="annotation-text-note"
            data-selected={controller.selectedItemId === feedback.itemId || undefined}
            style={{
              left: point.x,
              top: point.y,
              borderColor: annotationToneRgba(style.color, 0.35),
              fontSize: `${annotationTextSizeCss(style)}px`,
              color: '#3B2F13',
            }}
          >
            <span
              className="annotation-text-note__accent"
              style={{ background: annotationRgb(style) }}
            />
            {effectiveText(feedback)}
            <span className="visually-hidden">
              {' '}
              文字标注，字号 {annotationTextSizeLabel(style)}
            </span>
          </div>
        )
      })}
    </div>
  )
}

function RulerDetails({
  anchor,
  style,
  local,
  sourceSize,
}: {
  anchor: Extract<ReviewAnchor, { kind: 'image_line' }>
  style: ReviewAnnotationStyle
  local(point: { x: number; y: number }): { x: number; y: number } | null
  sourceSize: { width: number; height: number }
}) {
  const tail = local(anchor.tail)
  const head = local(anchor.head)
  if (tail === null || head === null) return null
  const delta = { x: head.x - tail.x, y: head.y - tail.y }
  const length = Math.hypot(delta.x, delta.y)
  if (length <= 0) return null
  const unit = { x: delta.x / length, y: delta.y / length }
  const perpendicular = { x: -unit.y, y: unit.x }
  const tick = 7
  const middle = { x: (tail.x + head.x) / 2, y: (tail.y + head.y) / 2 }
  const pixels = Math.round(
    Math.hypot(
      (anchor.head.x - anchor.tail.x) * sourceSize.width,
      (anchor.head.y - anchor.tail.y) * sourceSize.height,
    ),
  )
  return (
    <g stroke={annotationColorRgba(style)} strokeWidth={1.5}>
      <line
        x1={tail.x - perpendicular.x * tick}
        y1={tail.y - perpendicular.y * tick}
        x2={tail.x + perpendicular.x * tick}
        y2={tail.y + perpendicular.y * tick}
      />
      <line
        x1={head.x - perpendicular.x * tick}
        y1={head.y - perpendicular.y * tick}
        x2={head.x + perpendicular.x * tick}
        y2={head.y + perpendicular.y * tick}
      />
      <g stroke="none">
        <rect
          x={middle.x - 27}
          y={middle.y - 22}
          width={54}
          height={18}
          rx={4}
          fill="rgba(251,251,251,0.94)"
        />
        <text
          x={middle.x}
          y={middle.y - 9}
          textAnchor="middle"
          fontSize={11}
          fontWeight={600}
          fill="#42474E"
        >
          {pixels} px
        </text>
      </g>
    </g>
  )
}
