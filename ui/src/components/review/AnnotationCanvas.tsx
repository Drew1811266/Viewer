import type { PointerEvent as ReactPointerEvent } from 'react'
import { useEffect, useMemo, useRef } from 'react'
import { rectFromDrag, simplifyNormalizedStroke } from '../../app/review/annotationGeometry'
import type {
  ImageReviewWorkbenchController,
  ReviewAnchor,
  SavedImageFeedback,
} from '../../app/review/useImageReviewWorkbench'
import type { ImagePreviewProjection } from '../imagePreview/ImagePreviewSurface'
import {
  annotationMarkerPoint,
  buildAnnotationScene,
  paintAnnotationScene,
} from './annotationScene'

interface AnnotationCanvasProps {
  projection: ImagePreviewProjection
  controller: ImageReviewWorkbenchController
}

type DrawingGesture =
  | { kind: 'rectangle'; start: { x: number; y: number } }
  | { kind: 'brush'; points: Array<{ x: number; y: number }> }

type RectHandle = 'north_west' | 'north_east' | 'south_east' | 'south_west'

export default function AnnotationCanvas({ projection, controller }: AnnotationCanvasProps) {
  const canvas = useRef<HTMLCanvasElement>(null)
  const gesture = useRef<DrawingGesture | null>(null)
  const candidate =
    controller.editor.status === 'drawing' ||
    (controller.editor.status !== 'idle' && controller.editor.operation === 'geometry')
      ? controller.editor.draftAnchor
      : null
  const draftAnchor =
    controller.editor.status === 'idle' || controller.editor.status === 'drawing'
      ? null
      : controller.editor.draftAnchor
  const transientAnchor = candidate ?? draftAnchor
  const scene = useMemo(
    () =>
      buildAnnotationScene({
        feedback: controller.feedback,
        selectedItemId: controller.selectedItemId,
        transientAnchor,
      }),
    [controller.feedback, controller.selectedItemId, transientAnchor],
  )
  const drawingEnabled =
    controller.readOnlyReason === null &&
    (controller.editor.status === 'idle' || controller.editor.status === 'drawing') &&
    !controller.editor.temporarilyPanning &&
    (controller.tool === 'brush' || controller.tool === 'rectangle')

  useEffect(() => {
    if (controller.editor.status !== 'drawing') gesture.current = null
  }, [controller.editor.status])

  useEffect(() => {
    const element = canvas.current
    if (element === null) return
    const ratio = devicePixelRatio()
    element.width = Math.max(1, Math.round(projection.stageRect.width * ratio))
    element.height = Math.max(1, Math.round(projection.stageRect.height * ratio))
    element.style.width = `${projection.stageRect.width}px`
    element.style.height = `${projection.stageRect.height}px`
    let context: CanvasRenderingContext2D | null = null
    try {
      context = element.getContext('2d')
    } catch {
      return
    }
    if (context === null) return
    context.setTransform(ratio, 0, 0, ratio, 0, 0)
    context.clearRect(0, 0, projection.stageRect.width, projection.stageRect.height)
    paintAnnotationScene(
      context,
      scene,
      {
        normalizedToLocal(point) {
          const projected = projection.normalizedToStage(point)
          return projected === null
            ? null
            : {
                x: projected.x - projection.stageRect.left,
                y: projected.y - projection.stageRect.top,
              }
        },
      },
      {
        color: annotationColor(element),
        lineWidth: 2,
        drawOrdinals: false,
        ordinalRadius: 14,
      },
    )
  }, [projection, scene])

  function beginDrawing(event: ReactPointerEvent<HTMLCanvasElement>) {
    if (!drawingEnabled) return
    event.stopPropagation()
    const point = projection.stageToNormalized({ x: event.clientX, y: event.clientY })
    if (point === null) return
    const initial: ReviewAnchor =
      controller.tool === 'rectangle'
        ? { kind: 'image_rect', x: point.x, y: point.y, width: 0, height: 0 }
        : { kind: 'image_stroke', points: [point] }
    if (!controller.beginDrawing(initial, controller.redrawItemId ?? undefined)) return
    event.currentTarget.setPointerCapture?.(event.pointerId)
    if (controller.tool === 'rectangle') {
      gesture.current = { kind: 'rectangle', start: point }
      return
    }
    gesture.current = {
      kind: 'brush',
      points: [point],
    }
  }

  function continueDrawing(event: ReactPointerEvent<HTMLCanvasElement>) {
    const current = gesture.current
    if (current === null) return
    event.stopPropagation()
    const point = projection.stageToNormalized({ x: event.clientX, y: event.clientY })
    if (point === null) return
    if (current.kind === 'rectangle') {
      const rect = rectFromDrag(current.start, point)
      if (rect !== null) controller.updateDraftAnchor({ kind: 'image_rect', ...rect })
      return
    }
    current.points.push(point)
    const points = simplifyNormalizedStroke(current.points, {
      sourceWidth: projection.sourceSize.width,
      sourceHeight: projection.sourceSize.height,
      maxPoints: 2_048,
    })
    if (points !== null) controller.updateDraftAnchor({ kind: 'image_stroke', points })
  }

  function finishDrawing(event: ReactPointerEvent<HTMLCanvasElement>) {
    continueDrawing(event)
    const current = gesture.current
    gesture.current = null
    if (current === null) return
    if (current.kind === 'rectangle') {
      const end = projection.stageToNormalized({ x: event.clientX, y: event.clientY })
      const rect = end === null ? null : rectFromDrag(current.start, end)
      void controller.finishDrawing(rect === null ? null : { kind: 'image_rect', ...rect })
      return
    }
    const points = simplifyNormalizedStroke(current.points, {
      sourceWidth: projection.sourceSize.width,
      sourceHeight: projection.sourceSize.height,
      maxPoints: 2_048,
    })
    if (points === null) {
      void controller.finishDrawing(null)
      return
    }
    const anchor: ReviewAnchor = { kind: 'image_stroke', points }
    void controller.finishDrawing(anchor)
  }

  return (
    <div
      className="annotation-canvas-layer"
      data-tool={controller.tool}
      data-temporary-pan={controller.editor.temporarilyPanning || undefined}
    >
      <canvas
        ref={canvas}
        className="annotation-canvas"
        data-testid="annotation-canvas"
        data-interactive={drawingEnabled || undefined}
        data-has-candidate={candidate !== null || undefined}
        data-has-draft-anchor={(draftAnchor !== null && draftAnchor.kind !== 'asset') || undefined}
        onPointerDown={beginDrawing}
        onPointerMove={continueDrawing}
        onPointerUp={finishDrawing}
        onPointerCancel={(event) => {
          if (gesture.current !== null) event.stopPropagation()
          gesture.current = null
          controller.cancelDraft()
        }}
      />
      <div className="annotation-markers">
        {controller.feedback.map((feedback) => (
          <AnnotationMarker
            key={feedback.itemId}
            feedback={feedback}
            projection={projection}
            selected={feedback.itemId === controller.selectedItemId}
            readOnly={
              controller.readOnlyReason !== null ||
              (controller.dirty && controller.editor.status !== 'drawing')
            }
            drawing={controller.editor.status === 'drawing'}
            onSelect={() => controller.selectFeedback(feedback.itemId)}
            onReplace={(anchor) => controller.replaceFeedbackAnchor(feedback.itemId, anchor)}
            onCandidate={(anchor) => controller.stageFeedbackAnchor(feedback.itemId, anchor)}
            onCancel={controller.cancelDraft}
          />
        ))}
      </div>
    </div>
  )
}

interface AnnotationMarkerProps {
  feedback: SavedImageFeedback
  projection: ImagePreviewProjection
  selected: boolean
  readOnly: boolean
  drawing: boolean
  onSelect(): void
  onReplace(anchor: ReviewAnchor): Promise<void>
  onCandidate(anchor: ReviewAnchor): boolean
  onCancel(): void
}

function AnnotationMarker({
  feedback,
  projection,
  selected,
  readOnly,
  drawing,
  onSelect,
  onReplace,
  onCandidate,
  onCancel,
}: AnnotationMarkerProps) {
  const pointerCleanup = useRef<(() => void) | null>(null)
  useEffect(() => {
    if (!drawing) pointerCleanup.current?.()
  }, [drawing])
  useEffect(() => () => pointerCleanup.current?.(), [])
  if (feedback.anchor.kind === 'asset' || feedback.ordinal === null) return null
  const point = annotationMarkerPoint(feedback.anchor)
  if (point === null) return null
  const projected = projection.normalizedToStage(point)
  if (projected === null) return null
  const local = {
    left: projected.x - projection.stageRect.left,
    top: projected.y - projection.stageRect.top,
  }
  const ordinal = feedback.ordinal

  function keyDown(event: React.KeyboardEvent<HTMLButtonElement>) {
    if (readOnly || feedback.anchor.kind !== 'image_rect') return
    const moved = rectFromArrow(feedback.anchor, event.key, event.shiftKey, projection.sourceSize)
    if (moved === null) return
    event.preventDefault()
    event.stopPropagation()
    void onReplace(moved)
  }

  return (
    <>
      <button
        type="button"
        className="annotation-marker"
        data-testid="annotation-marker"
        data-anchor-kind={feedback.anchor.kind}
        data-selected={selected || undefined}
        aria-label={`意见 ${ordinal}：${feedback.text}`}
        style={local}
        onClick={onSelect}
        onKeyDown={keyDown}
        onPointerDown={(event) => {
          event.stopPropagation()
          if (!selected || readOnly || feedback.anchor.kind !== 'image_rect') return
          pointerCleanup.current = beginRectPointerEdit(
            event,
            feedback.anchor,
            null,
            projection,
            onCandidate,
            onReplace,
            onCancel,
          )
        }}
      >
        {ordinal}
      </button>
      {selected &&
        feedback.anchor.kind === 'image_rect' &&
        (['north_west', 'north_east', 'south_east', 'south_west'] as const).map((handle) => (
          <RectHandleButton
            key={handle}
            handle={handle}
            ordinal={ordinal}
            anchor={feedback.anchor as Extract<ReviewAnchor, { kind: 'image_rect' }>}
            projection={projection}
            readOnly={readOnly}
            drawing={drawing}
            onCandidate={onCandidate}
            onReplace={onReplace}
            onCancel={onCancel}
          />
        ))}
    </>
  )
}

interface RectHandleButtonProps {
  handle: RectHandle
  ordinal: number
  anchor: Extract<ReviewAnchor, { kind: 'image_rect' }>
  projection: ImagePreviewProjection
  readOnly: boolean
  drawing: boolean
  onCandidate(anchor: ReviewAnchor): boolean
  onReplace(anchor: ReviewAnchor): Promise<void>
  onCancel(): void
}

function RectHandleButton({
  handle,
  ordinal,
  anchor,
  projection,
  readOnly,
  drawing,
  onCandidate,
  onReplace,
  onCancel,
}: RectHandleButtonProps) {
  const pointerCleanup = useRef<(() => void) | null>(null)
  useEffect(() => {
    if (!drawing) pointerCleanup.current?.()
  }, [drawing])
  useEffect(() => () => pointerCleanup.current?.(), [])
  const point = handlePoint(anchor, handle)
  const projected = projection.normalizedToStage(point)
  if (projected === null) return null
  return (
    <button
      type="button"
      className="annotation-rect-handle"
      data-handle={handle}
      aria-label={`调整意见 ${ordinal} ${handleLabel(handle)}`}
      disabled={readOnly}
      style={{
        left: projected.x - projection.stageRect.left,
        top: projected.y - projection.stageRect.top,
      }}
      onPointerDown={(event) => {
        if (!readOnly)
          pointerCleanup.current = beginRectPointerEdit(
            event,
            anchor,
            handle,
            projection,
            onCandidate,
            onReplace,
            onCancel,
          )
      }}
      onKeyDown={(event) => {
        const resized = resizeRectFromArrow(
          anchor,
          handle,
          event.key,
          event.shiftKey,
          projection.sourceSize,
        )
        if (resized === null) return
        event.preventDefault()
        event.stopPropagation()
        void onReplace(resized)
      }}
    />
  )
}

function beginRectPointerEdit(
  event: ReactPointerEvent<HTMLElement>,
  anchor: Extract<ReviewAnchor, { kind: 'image_rect' }>,
  handle: RectHandle | null,
  projection: ImagePreviewProjection,
  onCandidate: (anchor: ReviewAnchor) => boolean,
  onReplace: (anchor: ReviewAnchor) => Promise<void>,
  onCancel: () => void,
) {
  event.preventDefault()
  event.stopPropagation()
  const start = projection.stageToNormalized(
    { x: event.clientX, y: event.clientY },
    { allowOutsideImage: true },
  )
  if (start === null || !onCandidate(anchor)) return null
  const startPoint = start
  let candidate: Extract<ReviewAnchor, { kind: 'image_rect' }> = anchor

  function move(pointer: PointerEvent) {
    const point = projection.stageToNormalized(
      { x: pointer.clientX, y: pointer.clientY },
      { allowOutsideImage: true },
    )
    if (point === null) return
    candidate =
      handle === null
        ? moveRect(anchor, point.x - startPoint.x, point.y - startPoint.y)
        : resizeRect(anchor, handle, point)
    onCandidate(candidate)
  }

  function finish(pointer: PointerEvent) {
    move(pointer)
    cleanup()
    void onReplace(candidate)
  }

  function cleanup() {
    window.removeEventListener('pointermove', move)
    window.removeEventListener('pointerup', finish)
    window.removeEventListener('pointercancel', cancel)
  }
  function cancel() {
    cleanup()
    onCancel()
  }

  window.addEventListener('pointermove', move)
  window.addEventListener('pointerup', finish)
  window.addEventListener('pointercancel', cancel)
  return cleanup
}

function handlePoint(anchor: Extract<ReviewAnchor, { kind: 'image_rect' }>, handle: RectHandle) {
  return {
    x: handle === 'north_west' || handle === 'south_west' ? anchor.x : anchor.x + anchor.width,
    y: handle === 'north_west' || handle === 'north_east' ? anchor.y : anchor.y + anchor.height,
  }
}

function moveRect(
  anchor: Extract<ReviewAnchor, { kind: 'image_rect' }>,
  deltaX: number,
  deltaY: number,
) {
  return {
    ...anchor,
    x: clamp(anchor.x + deltaX, 0, 1 - anchor.width),
    y: clamp(anchor.y + deltaY, 0, 1 - anchor.height),
  }
}

function resizeRect(
  anchor: Extract<ReviewAnchor, { kind: 'image_rect' }>,
  handle: RectHandle,
  point: { x: number; y: number },
) {
  const opposite = handlePoint(anchor, oppositeHandle(handle))
  const rect = rectFromDrag(opposite, point)
  return rect === null ? anchor : { kind: 'image_rect' as const, ...rect }
}

function rectFromArrow(
  anchor: Extract<ReviewAnchor, { kind: 'image_rect' }>,
  key: string,
  resize: boolean,
  source: { width: number; height: number },
) {
  const direction = arrowDirection(key)
  if (direction === null) return null
  if (resize) {
    const stepX = 10 / source.width
    const stepY = 10 / source.height
    return resizeRectFromDelta(anchor, direction.x * stepX, direction.y * stepY)
  }
  return moveRect(anchor, direction.x / source.width, direction.y / source.height)
}

function resizeRectFromArrow(
  anchor: Extract<ReviewAnchor, { kind: 'image_rect' }>,
  handle: RectHandle,
  key: string,
  coarse: boolean,
  source: { width: number; height: number },
) {
  const direction = arrowDirection(key)
  if (direction === null) return null
  const multiplier = coarse ? 10 : 1
  const point = handlePoint(anchor, handle)
  return resizeRect(anchor, handle, {
    x: point.x + (direction.x * multiplier) / source.width,
    y: point.y + (direction.y * multiplier) / source.height,
  })
}

function resizeRectFromDelta(
  anchor: Extract<ReviewAnchor, { kind: 'image_rect' }>,
  deltaX: number,
  deltaY: number,
) {
  const width = clamp(anchor.width + deltaX, 1 / 65_536, 1 - anchor.x)
  const height = clamp(anchor.height + deltaY, 1 / 65_536, 1 - anchor.y)
  return { ...anchor, width, height }
}

function arrowDirection(key: string) {
  if (key === 'ArrowLeft') return { x: -1, y: 0 }
  if (key === 'ArrowRight') return { x: 1, y: 0 }
  if (key === 'ArrowUp') return { x: 0, y: -1 }
  if (key === 'ArrowDown') return { x: 0, y: 1 }
  return null
}

function oppositeHandle(handle: RectHandle): RectHandle {
  if (handle === 'north_west') return 'south_east'
  if (handle === 'north_east') return 'south_west'
  if (handle === 'south_east') return 'north_west'
  return 'north_east'
}

function handleLabel(handle: RectHandle) {
  if (handle === 'north_west') return '左上角'
  if (handle === 'north_east') return '右上角'
  if (handle === 'south_east') return '右下角'
  return '左下角'
}

function annotationColor(element: HTMLElement) {
  return getComputedStyle(element).getPropertyValue('--review-annotation').trim() || 'CanvasText'
}

function devicePixelRatio() {
  const ratio = Number.isFinite(window.devicePixelRatio) ? window.devicePixelRatio : 1
  return clamp(ratio, 1, 4)
}

function clamp(value: number, minimum: number, maximum: number) {
  return Math.max(minimum, Math.min(maximum, value))
}
