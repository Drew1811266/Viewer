import type { PointerEvent as ReactPointerEvent } from 'react'
import { useEffect, useRef, useState } from 'react'
import {
  arrowFromDrag,
  type BoxHandle,
  ellipseFromDrag,
  type ImageAnchor,
  lineFromDrag,
  moveAnchor,
  movePolygonVertex,
  type NormalizedPoint,
  type PolygonVertexHandle,
  pointAnchor,
  polygonFromPoints,
  rectFromDrag,
  resizeBoxAnchor,
  simplifyNormalizedStroke,
  textAnchor,
} from '../../app/review/annotationGeometry'
import { type AnnotationHandle, hitTestAnnotation } from '../../app/review/annotationHitTest'
import {
  ANNOTATION_BADGE_RADIUS,
  annotationColorHex,
  defaultAnnotationStyle,
  styleForAnchor,
} from '../../app/review/annotationStyle'
import type {
  ImageReviewWorkbenchController,
  ReviewAnchor,
  SavedImageFeedback,
} from '../../app/review/useImageReviewWorkbench'
import type { ImagePreviewProjection } from '../imagePreview/ImagePreviewSurface'
import {
  type AnnotationSceneItem,
  type AnnotationSceneProjection,
  annotationMarkerPoint,
  annotationOrdinalPoint,
  buildAnnotationScene,
  paintAnnotationScene,
} from './annotationScene'

interface AnnotationCanvasProps {
  projection: ImagePreviewProjection
  controller: ImageReviewWorkbenchController
  scene?: ReadonlyArray<AnnotationSceneItem>
}

type DrawingGesture =
  | {
      kind: 'point'
      pointerId: number
      point: NormalizedPoint
      startClient: NormalizedPoint
    }
  | {
      kind: 'text'
      pointerId: number
      point: NormalizedPoint
      startClient: NormalizedPoint
    }
  | {
      kind: 'arrow'
      pointerId: number
      start: NormalizedPoint
      startClient: NormalizedPoint
    }
  | {
      kind: 'line'
      pointerId: number
      start: NormalizedPoint
      startClient: NormalizedPoint
    }
  | {
      kind: 'brush'
      pointerId: number
      points: NormalizedPoint[]
      startClient: NormalizedPoint
    }
  | {
      kind: 'polygon'
      pointerId: number
      points: NormalizedPoint[]
      preview: NormalizedPoint
      startClient: NormalizedPoint
    }
  | {
      kind: 'box'
      pointerId: number
      shape: 'rectangle' | 'ellipse'
      start: NormalizedPoint
      startClient: NormalizedPoint
    }

type GeometryEdit = 'move' | AnnotationHandle

const MINIMUM_DRAG_CSS_PX = 6
const MINIMUM_EDIT_CSS_PX = 1

export default function AnnotationCanvas({ projection, controller, scene }: AnnotationCanvasProps) {
  const canvas = useRef<HTMLCanvasElement>(null)
  const gesture = useRef<DrawingGesture | null>(null)
  const pointerCleanup = useRef<(() => void) | null>(null)
  const [polygonPointCount, setPolygonPointCount] = useState(0)
  const editorPhase = controller.editor.phase
  const candidate =
    editorPhase.status === 'drawing' ||
    (editorPhase.status !== 'idle' && editorPhase.operation === 'geometry')
      ? editorPhase.draftAnchor
      : null
  const draftAnchor =
    editorPhase.status === 'idle' || editorPhase.status === 'drawing'
      ? null
      : editorPhase.draftAnchor
  const transientAnchor = candidate ?? draftAnchor
  const transientStyle =
    editorPhase.status === 'idle' ? null : (editorPhase.style ?? defaultAnnotationStyle('brush'))
  const transientText =
    editorPhase.status !== 'idle' && editorPhase.status !== 'drawing' ? editorPhase.text : null
  const editingItemId =
    editorPhase.status === 'idle' || editorPhase.status === 'drawing'
      ? null
      : editorPhase.sourceItemId
  const editingOperation =
    editorPhase.status === 'idle' || editorPhase.status === 'drawing'
      ? null
      : (editorPhase.operation ?? null)
  const renderedScene =
    scene ??
    buildAnnotationScene({
      feedback: controller.feedback,
      selectedItemId: controller.selectedItemId,
      transientAnchor,
      transientStyle,
      transientText,
      editingItemId,
      editingOperation,
    })
  const drawingEnabled =
    controller.readOnlyReason === null &&
    (editorPhase.status === 'idle' || editorPhase.status === 'drawing') &&
    !controller.editor.temporarilyPanning &&
    controller.tool !== 'browse'
  const selectionEnabled =
    controller.readOnlyReason === null &&
    editorPhase.status === 'idle' &&
    !controller.editor.temporarilyPanning &&
    controller.tool === 'browse' &&
    controller.feedback.some((feedback) => isImageAnchor(feedback.anchor))

  useEffect(() => {
    if (editorPhase.status === 'drawing') return
    gesture.current = null
    pointerCleanup.current?.()
    pointerCleanup.current = null
  }, [editorPhase.status])

  useEffect(
    () => () => {
      pointerCleanup.current?.()
    },
    [],
  )

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
    paintAnnotationScene(context, renderedScene, stageAnnotationProjection(projection), {
      color: annotationColor(element),
      lineWidth: 2,
      drawOrdinals: false,
      ordinalRadius: ANNOTATION_BADGE_RADIUS,
      selectionColor: annotationSelectionColor(element),
    })
  }, [projection, renderedScene])

  function beginCanvasPointer(event: ReactPointerEvent<HTMLCanvasElement>) {
    if (drawingEnabled) {
      if (controller.tool === 'polygon') {
        beginPolygon(event)
        return
      }
      beginCreation(event)
      return
    }
    if (!selectionEnabled) return
    const point = projection.stageToNormalized({ x: event.clientX, y: event.clientY })
    if (point === null) return
    const hit = hitTestAnnotation({
      point,
      imageSizeCss: projectedImageSize(projection),
      items: controller.feedback.flatMap((feedback) => {
        if (!isImageAnchor(feedback.anchor) || feedback.ordinal === null) return []
        const ordinalPoint = annotationMarkerPoint(feedback.anchor)
        return [
          {
            itemId: feedback.itemId,
            ordinal: feedback.ordinal,
            selected: feedback.itemId === controller.selectedItemId,
            anchor: feedback.anchor,
            ...(ordinalPoint === null ? {} : { ordinalPoint }),
          },
        ]
      }),
    })
    if (hit === null) {
      // Clicking empty canvas clears the selection so the style bar and
      // resize handles do not stay pinned to an old annotation.
      controller.selectFeedback(null)
      return
    }
    event.preventDefault()
    event.stopPropagation()
    controller.selectFeedback(hit.itemId)
    if (hit.itemId !== controller.selectedItemId || hit.part === 'ordinal') return
    const feedback = controller.feedback.find((item) => item.itemId === hit.itemId)
    if (feedback === undefined || !isImageAnchor(feedback.anchor)) return
    const edit = geometryEditForHit(feedback.anchor, hit.handle)
    if (edit === null) return
    pointerCleanup.current = beginAnchorPointerEdit({
      event,
      anchor: feedback.anchor,
      edit,
      projection,
      onCandidate: (anchor) => controller.stageFeedbackAnchor(feedback.itemId, anchor),
      onReplace: (anchor) => controller.replaceFeedbackAnchor(feedback.itemId, anchor),
      onCancel: controller.cancelDraft,
    })
  }

  function beginCreation(event: ReactPointerEvent<HTMLCanvasElement>) {
    const point = projection.stageToNormalized({ x: event.clientX, y: event.clientY })
    if (point === null) return
    const current = creationGesture(controller.tool, event.pointerId, point, event)
    if (current === null) return
    const initial = initialAnchor(current)
    if (initial === null) return
    if (!controller.beginDrawing(initial, controller.redrawItemId ?? undefined)) return
    controller.selectFeedback(null)
    event.preventDefault()
    event.stopPropagation()
    event.currentTarget.setPointerCapture?.(event.pointerId)
    gesture.current = current
    pointerCleanup.current?.()
    pointerCleanup.current = bindWindowPointerSession(event.pointerId, {
      move(pointer) {
        const anchor = drawingAnchor(current, pointer, projection)
        if (anchor !== null) controller.updateDraftAnchor(anchor)
      },
      finish(pointer) {
        const anchor = completedDrawingAnchor(current, pointer, projection)
        gesture.current = null
        pointerCleanup.current = null
        void controller.finishDrawing(anchor)
      },
      cancel() {
        gesture.current = null
        pointerCleanup.current = null
        controller.cancelDraft()
      },
    })
  }

  function beginPolygon(event: ReactPointerEvent<HTMLCanvasElement>) {
    const point = projection.stageToNormalized({ x: event.clientX, y: event.clientY })
    if (point === null) return
    event.preventDefault()
    event.stopPropagation()
    const current = gesture.current
    if (current === null || current.kind !== 'polygon') {
      const initial = polygonPreview([point])
      if (!controller.beginDrawing(initial, controller.redrawItemId ?? undefined)) return
      controller.selectFeedback(null)
      gesture.current = {
        kind: 'polygon',
        pointerId: event.pointerId,
        points: [point],
        preview: point,
        startClient: { x: event.clientX, y: event.clientY },
      }
      setPolygonPointCount(1)
      return
    }
    const first = projection.normalizedToStage(current.points[0] as NormalizedPoint)
    const nearFirst =
      first !== null &&
      Math.hypot(event.clientX - first.x, event.clientY - first.y) <= MINIMUM_DRAG_CSS_PX * 1.8
    const doubleClick = event.detail >= 2
    if ((nearFirst || doubleClick) && current.points.length >= 3) {
      finishPolygon(current)
      return
    }
    if (current.points.length >= 64) return
    current.points.push(point)
    current.preview = point
    setPolygonPointCount(current.points.length)
    controller.updateDraftAnchor(polygonPreview(current.points))
  }

  function finishPolygon(current: Extract<DrawingGesture, { kind: 'polygon' }>) {
    gesture.current = null
    pointerCleanup.current?.()
    pointerCleanup.current = null
    setPolygonPointCount(0)
    void controller.finishDrawing(polygonFromPoints(current.points))
  }

  function removeLastPolygonPoint() {
    const current = gesture.current
    if (current === null || current.kind !== 'polygon') return
    if (current.points.length <= 1) {
      gesture.current = null
      setPolygonPointCount(0)
      controller.cancelDraft()
      return
    }
    current.points.pop()
    current.preview = current.points[current.points.length - 1] as NormalizedPoint
    setPolygonPointCount(current.points.length)
    controller.updateDraftAnchor(polygonPreview(current.points))
  }

  function handleCanvasDoubleClick(event: React.MouseEvent<HTMLCanvasElement>) {
    if (controller.tool === 'polygon') {
      const current = gesture.current
      if (current === null || current.kind !== 'polygon' || current.points.length < 3) return
      event.preventDefault()
      event.stopPropagation()
      // The second pointerdown of a double click may already have appended the
      // same vertex; remove that duplicate before closing.
      const last = current.points[current.points.length - 1]
      const previous = current.points[current.points.length - 2]
      if (
        last !== undefined &&
        previous !== undefined &&
        Math.hypot(last.x - previous.x, last.y - previous.y) < 1e-6
      ) {
        current.points.pop()
      }
      finishPolygon(current)
      return
    }
    if (
      controller.tool === 'browse' &&
      controller.selectedItemId !== null &&
      controller.readOnlyReason === null
    ) {
      event.preventDefault()
      controller.beginFeedbackTextEdit(controller.selectedItemId)
    }
  }

  useEffect(() => {
    if (editorPhase.status !== 'drawing' || controller.tool !== 'polygon') return
    function move(pointer: PointerEvent) {
      const current = gesture.current
      if (current === null || current.kind !== 'polygon' || pointer.pointerId !== current.pointerId)
        return
      const point = projection.stageToNormalized(
        { x: pointer.clientX, y: pointer.clientY },
        { allowOutsideImage: true },
      )
      if (point === null) return
      current.preview = point
      controller.updateDraftAnchor(polygonPreview([...current.points, point]))
    }
    window.addEventListener('pointermove', move)
    return () => window.removeEventListener('pointermove', move)
  }, [controller, editorPhase.status, projection])

  useEffect(() => {
    if (editorPhase.status !== 'drawing' || controller.tool !== 'polygon') {
      setPolygonPointCount(0)
      return
    }
    function keyDown(event: KeyboardEvent) {
      if (event.defaultPrevented || event.isComposing) return
      if (event.key === 'Enter') {
        const current = gesture.current
        if (current === null || current.kind !== 'polygon' || current.points.length < 3) return
        event.preventDefault()
        finishPolygon(current)
        return
      }
      if (event.key === 'Backspace' || event.key === 'Delete') {
        if (gesture.current?.kind !== 'polygon') return
        event.preventDefault()
        removeLastPolygonPoint()
      }
    }
    window.addEventListener('keydown', keyDown)
    return () => window.removeEventListener('keydown', keyDown)
  }, [controller, editorPhase.status, controller.tool])

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
        data-interactive={drawingEnabled || selectionEnabled || undefined}
        data-drawing={drawingEnabled || undefined}
        data-has-candidate={candidate !== null || undefined}
        data-has-draft-anchor={(draftAnchor !== null && draftAnchor.kind !== 'asset') || undefined}
        onPointerDown={beginCanvasPointer}
        onDoubleClick={handleCanvasDoubleClick}
      />
      {editorPhase.status === 'drawing' && controller.tool === 'polygon' && (
        <div className="annotation-polygon-hint" role="status" data-native-input-exclusion="true">
          <span>
            {polygonPointCount < 3
              ? `已放置 ${polygonPointCount} 个点 · 至少 3 个点`
              : `已放置 ${polygonPointCount} 个点`}
          </span>
          <span className="annotation-polygon-hint__keys">
            Enter / 双击 / 点起点完成 · 退格 退一点 · Esc 取消
          </span>
          <button
            type="button"
            className="annotation-polygon-hint__finish"
            disabled={polygonPointCount < 3}
            onClick={() => {
              const current = gesture.current
              if (current?.kind === 'polygon') finishPolygon(current)
            }}
          >
            完成
          </button>
          <button
            type="button"
            className="annotation-polygon-hint__secondary"
            onClick={removeLastPolygonPoint}
          >
            退一点
          </button>
          <button
            type="button"
            className="annotation-polygon-hint__secondary"
            onClick={() => {
              gesture.current = null
              setPolygonPointCount(0)
              controller.cancelDraft()
            }}
          >
            取消
          </button>
        </div>
      )}
      <div className="annotation-markers">
        {controller.feedback.map((feedback) => (
          <AnnotationMarker
            key={feedback.itemId}
            feedback={feedback}
            projection={projection}
            selected={feedback.itemId === controller.selectedItemId}
            readOnly={
              controller.readOnlyReason !== null ||
              (controller.dirty && editorPhase.status !== 'drawing')
            }
            drawing={editorPhase.status === 'drawing'}
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
  if (!isImageAnchor(feedback.anchor) || feedback.ordinal === null) return null
  const anchor = feedback.anchor
  const local = annotationOrdinalPoint(anchor, stageAnnotationProjection(projection), {
    lineWidth: 2,
    ordinalRadius: ANNOTATION_BADGE_RADIUS,
  })
  if (local === null) return null
  const ordinal = feedback.ordinal

  function beginEdit(event: ReactPointerEvent<HTMLElement>, edit: GeometryEdit) {
    if (readOnly) return
    pointerCleanup.current = beginAnchorPointerEdit({
      event,
      anchor,
      edit,
      projection,
      onCandidate,
      onReplace,
      onCancel,
    })
  }

  function keyDown(event: React.KeyboardEvent<HTMLButtonElement>) {
    if (readOnly || anchor.kind === 'image_stroke') return
    const direction = arrowDirection(event.key)
    if (direction === null) return
    event.preventDefault()
    event.stopPropagation()
    const multiplier = event.shiftKey ? 10 : 1
    void onReplace(
      moveAnchor(anchor, {
        x: (direction.x * multiplier) / projection.sourceSize.width,
        y: (direction.y * multiplier) / projection.sourceSize.height,
      }),
    )
  }

  return (
    <>
      {selected && isBoxAnchor(anchor) && (
        <BoxMoveTarget
          ordinal={ordinal}
          anchor={anchor}
          projection={projection}
          readOnly={readOnly}
          onPointerDown={(event) => beginEdit(event, 'move')}
        />
      )}
      <button
        type="button"
        className="annotation-marker"
        data-testid="annotation-marker"
        data-anchor-kind={anchor.kind}
        data-selected={selected || undefined}
        aria-label={`意见 ${ordinal}：${feedback.text}`}
        style={
          {
            left: local.x,
            top: local.y,
            '--annotation-color': annotationColorHex(
              styleForAnchor(anchor, feedback.style)?.color ?? 'red',
            ),
          } as React.CSSProperties
        }
        onClick={onSelect}
        onKeyDown={keyDown}
        onPointerDown={(event) => {
          event.stopPropagation()
          if (!selected || readOnly) return
          const edit = markerEdit(anchor)
          if (edit !== null) beginEdit(event, edit)
        }}
      >
        {ordinal}
      </button>
      {selected &&
        isBoxAnchor(anchor) &&
        (
          [
            'north_west',
            'north',
            'north_east',
            'east',
            'south_east',
            'south',
            'south_west',
            'west',
          ] as const
        ).map((handle) => (
          <GeometryHandleButton
            key={handle}
            handle={handle}
            ordinal={ordinal}
            anchor={anchor as Extract<ImageAnchor, { kind: 'image_rect' | 'image_ellipse' }>}
            projection={projection}
            readOnly={readOnly}
            drawing={drawing}
            onCandidate={onCandidate}
            onReplace={onReplace}
            onCancel={onCancel}
          />
        ))}
      {selected &&
        (anchor.kind === 'image_arrow' || anchor.kind === 'image_line') &&
        (['tail', 'head'] as const).map((handle) => (
          <GeometryHandleButton
            key={handle}
            handle={handle}
            ordinal={ordinal}
            anchor={anchor as Extract<ImageAnchor, { kind: 'image_arrow' | 'image_line' }>}
            projection={projection}
            readOnly={readOnly}
            drawing={drawing}
            onCandidate={onCandidate}
            onReplace={onReplace}
            onCancel={onCancel}
          />
        ))}
      {selected &&
        anchor.kind === 'image_polygon' &&
        anchor.points.map((point, index) => (
          <GeometryHandleButton
            key={`vertex_${point.x}-${point.y}`}
            handle={`vertex_${index}`}
            ordinal={ordinal}
            anchor={anchor}
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

function BoxMoveTarget({
  ordinal,
  anchor,
  projection,
  readOnly,
  onPointerDown,
}: {
  ordinal: number
  anchor: Extract<ImageAnchor, { kind: 'image_rect' | 'image_ellipse' }>
  projection: ImagePreviewProjection
  readOnly: boolean
  onPointerDown(event: ReactPointerEvent<HTMLButtonElement>): void
}) {
  const start = projection.normalizedToStage({ x: anchor.x, y: anchor.y })
  const end = projection.normalizedToStage({
    x: anchor.x + anchor.width,
    y: anchor.y + anchor.height,
  })
  if (start === null || end === null) return null
  return (
    <button
      type="button"
      className="annotation-box-move-target"
      aria-label={`移动意见 ${ordinal} 区域`}
      disabled={readOnly}
      style={{
        left: Math.min(start.x, end.x) - projection.stageRect.left,
        top: Math.min(start.y, end.y) - projection.stageRect.top,
        width: Math.max(24, Math.abs(end.x - start.x)),
        height: Math.max(24, Math.abs(end.y - start.y)),
      }}
      onPointerDown={onPointerDown}
    />
  )
}

function GeometryHandleButton({
  handle,
  ordinal,
  anchor,
  projection,
  readOnly,
  drawing,
  onCandidate,
  onReplace,
  onCancel,
}: {
  handle: BoxHandle | 'tail' | 'head' | PolygonVertexHandle
  ordinal: number
  anchor: Extract<
    ImageAnchor,
    { kind: 'image_rect' | 'image_ellipse' | 'image_arrow' | 'image_line' | 'image_polygon' }
  >
  projection: ImagePreviewProjection
  readOnly: boolean
  drawing: boolean
  onCandidate(anchor: ReviewAnchor): boolean
  onReplace(anchor: ReviewAnchor): Promise<void>
  onCancel(): void
}) {
  const pointerCleanup = useRef<(() => void) | null>(null)
  useEffect(() => {
    if (!drawing) pointerCleanup.current?.()
  }, [drawing])
  useEffect(() => () => pointerCleanup.current?.(), [])
  const point = handlePoint(anchor, handle)
  const projected = point === null ? null : projection.normalizedToStage(point)
  if (point === null || projected === null) return null
  return (
    <button
      type="button"
      className="annotation-geometry-handle"
      data-handle={handle}
      aria-label={`调整意见 ${ordinal} ${handleLabel(handle, anchor.kind)}`}
      disabled={readOnly}
      style={localPoint(projected, projection)}
      onPointerDown={(event) => {
        if (readOnly) return
        pointerCleanup.current = beginAnchorPointerEdit({
          event,
          anchor,
          edit: handle,
          projection,
          onCandidate,
          onReplace,
          onCancel,
        })
      }}
      onKeyDown={(event) => {
        if (readOnly) return
        const direction = arrowDirection(event.key)
        if (direction === null) return
        event.preventDefault()
        event.stopPropagation()
        const multiplier = event.shiftKey ? 10 : 1
        const movedPoint = {
          x: point.x + (direction.x * multiplier) / projection.sourceSize.width,
          y: point.y + (direction.y * multiplier) / projection.sourceSize.height,
        }
        const resized = anchorForHandle(anchor, handle, movedPoint)
        if (resized !== null) void onReplace(resized)
      }}
    />
  )
}

function beginAnchorPointerEdit({
  event,
  anchor,
  edit,
  projection,
  onCandidate,
  onReplace,
  onCancel,
}: {
  event: ReactPointerEvent<HTMLElement>
  anchor: ImageAnchor
  edit: GeometryEdit
  projection: ImagePreviewProjection
  onCandidate(anchor: ReviewAnchor): boolean
  onReplace(anchor: ReviewAnchor): Promise<void>
  onCancel(): void
}) {
  event.preventDefault()
  event.stopPropagation()
  const start = projection.stageToNormalized(
    { x: event.clientX, y: event.clientY },
    { allowOutsideImage: true },
  )
  if (start === null) return null
  const startPoint = start
  event.currentTarget.setPointerCapture?.(event.pointerId)
  const startClient = { x: event.clientX, y: event.clientY }
  let candidate = anchor
  let staged = onCandidate(anchor)
  let moved = false
  if (!staged) return null

  function update(pointer: PointerEvent) {
    if (clientDistance(startClient, pointer) < MINIMUM_EDIT_CSS_PX) return
    const point = projection.stageToNormalized(
      { x: pointer.clientX, y: pointer.clientY },
      { allowOutsideImage: true },
    )
    if (point === null) return
    const next = anchorForEdit(anchor, edit, startPoint, point)
    if (next === null) return
    if (onCandidate(next)) {
      candidate = next
      staged = true
      moved = true
    }
  }

  return bindWindowPointerSession(event.pointerId, {
    move: update,
    finish(pointer) {
      update(pointer)
      if (staged && moved) void onReplace(candidate)
      else if (staged) onCancel()
    },
    cancel() {
      if (staged) onCancel()
    },
  })
}

function creationGesture(
  tool: ImageReviewWorkbenchController['tool'],
  pointerId: number,
  point: NormalizedPoint,
  event: ReactPointerEvent<HTMLElement>,
): DrawingGesture | null {
  const startClient = { x: event.clientX, y: event.clientY }
  switch (tool) {
    case 'browse':
    case 'polygon':
      return null
    case 'point':
      return { kind: 'point', pointerId, point, startClient }
    case 'text':
      return { kind: 'text', pointerId, point, startClient }
    case 'arrow':
      return { kind: 'arrow', pointerId, start: point, startClient }
    case 'line':
    case 'ruler':
      return { kind: 'line', pointerId, start: point, startClient }
    case 'brush':
    case 'highlighter':
      return { kind: 'brush', pointerId, points: [point], startClient }
    case 'rectangle':
    case 'ellipse':
      return { kind: 'box', pointerId, shape: tool, start: point, startClient }
  }
}

function initialAnchor(gesture: DrawingGesture): ReviewAnchor | null {
  switch (gesture.kind) {
    case 'point':
      return pointAnchor(gesture.point) as NonNullable<ReturnType<typeof pointAnchor>>
    case 'text':
      return textAnchor(gesture.point)
    case 'arrow':
      return { kind: 'image_arrow', tail: gesture.start, head: gesture.start }
    case 'line':
      return { kind: 'image_line', tail: gesture.start, head: gesture.start }
    case 'brush':
      return { kind: 'image_stroke', points: [gesture.points[0] as NormalizedPoint] }
    case 'polygon':
      return polygonPreview(gesture.points)
    case 'box':
      return {
        kind: gesture.shape === 'rectangle' ? 'image_rect' : 'image_ellipse',
        x: gesture.start.x,
        y: gesture.start.y,
        width: 0,
        height: 0,
      }
  }
}

function polygonPreview(points: ReadonlyArray<NormalizedPoint>): ReviewAnchor {
  return {
    kind: 'image_polygon',
    points:
      points.length === 1
        ? [points[0] as NormalizedPoint, points[0] as NormalizedPoint, points[0] as NormalizedPoint]
        : points,
  }
}

function drawingAnchor(
  gesture: DrawingGesture,
  pointer: PointerEvent,
  projection: ImagePreviewProjection,
): ReviewAnchor | null {
  const point = projection.stageToNormalized(
    { x: pointer.clientX, y: pointer.clientY },
    { allowOutsideImage: true },
  )
  if (point === null) return null
  switch (gesture.kind) {
    case 'point':
      return pointAnchor(clampedPoint(point))
    case 'text':
      return textAnchor(clampedPoint(point))
    case 'arrow':
      return arrowFromDrag(
        gesture.start,
        pointer.shiftKey ? constrainLine(gesture.start, point) : point,
      )
    case 'line':
      return lineFromDrag(
        gesture.start,
        pointer.shiftKey ? constrainLine(gesture.start, point) : point,
      )
    case 'brush': {
      gesture.points.push(point)
      const points = simplifyNormalizedStroke(gesture.points, strokeOptions(projection))
      return points === null ? null : { kind: 'image_stroke', points }
    }
    case 'polygon':
      return polygonPreview([...gesture.points, point])
    case 'box':
      return boxAnchor(gesture, point, pointer.shiftKey, projection)
  }
}

function completedDrawingAnchor(
  gesture: DrawingGesture,
  pointer: PointerEvent,
  projection: ImagePreviewProjection,
): ReviewAnchor | null {
  if (
    gesture.kind !== 'point' &&
    gesture.kind !== 'text' &&
    gesture.kind !== 'polygon' &&
    clientDistance(gesture.startClient, pointer) < MINIMUM_DRAG_CSS_PX
  ) {
    return null
  }
  if (
    gesture.kind === 'box' &&
    (Math.abs(pointer.clientX - gesture.startClient.x) < MINIMUM_DRAG_CSS_PX ||
      Math.abs(pointer.clientY - gesture.startClient.y) < MINIMUM_DRAG_CSS_PX)
  ) {
    return null
  }
  return drawingAnchor(gesture, pointer, projection)
}

function boxAnchor(
  gesture: Extract<DrawingGesture, { kind: 'box' }>,
  point: NormalizedPoint,
  constrainCircle: boolean,
  projection: ImagePreviewProjection,
): ReviewAnchor | null {
  if (gesture.shape === 'ellipse') {
    return ellipseFromDrag(gesture.start, point, {
      constrainCircle,
      sourceWidth: projection.sourceSize.width,
      sourceHeight: projection.sourceSize.height,
    })
  }
  const rect = rectFromDrag(gesture.start, point)
  return rect === null ? null : { kind: 'image_rect', ...rect }
}

function strokeOptions(projection: ImagePreviewProjection) {
  return {
    sourceWidth: projection.sourceSize.width,
    sourceHeight: projection.sourceSize.height,
    maxPoints: 2_048,
  }
}

function geometryEditForHit(anchor: ImageAnchor, handle?: AnnotationHandle): GeometryEdit | null {
  if (handle !== undefined) return handle
  return anchor.kind === 'image_stroke' ? null : 'move'
}

function markerEdit(anchor: ImageAnchor): GeometryEdit | null {
  switch (anchor.kind) {
    case 'image_point':
    case 'image_text':
      return 'point'
    case 'image_arrow':
    case 'image_rect':
    case 'image_ellipse':
    case 'image_line':
    case 'image_polygon':
      return 'move'
    case 'image_stroke':
      return null
  }
}

function anchorForEdit(
  anchor: ImageAnchor,
  edit: GeometryEdit,
  start: NormalizedPoint,
  point: NormalizedPoint,
): ImageAnchor | null {
  if (edit === 'move' || edit === 'point') {
    return moveAnchor(anchor, { x: point.x - start.x, y: point.y - start.y })
  }
  return anchorForHandle(anchor, edit, point)
}

function anchorForHandle(
  anchor: ImageAnchor,
  handle: Exclude<AnnotationHandle, 'point'>,
  point: NormalizedPoint,
): ImageAnchor | null {
  if ((anchor.kind === 'image_arrow' || anchor.kind === 'image_line') && handle === 'tail') {
    return anchor.kind === 'image_arrow'
      ? arrowFromDrag(point, anchor.head)
      : lineFromDrag(point, anchor.head)
  }
  if ((anchor.kind === 'image_arrow' || anchor.kind === 'image_line') && handle === 'head') {
    return anchor.kind === 'image_arrow'
      ? arrowFromDrag(anchor.tail, point)
      : lineFromDrag(anchor.tail, point)
  }
  if (anchor.kind === 'image_polygon' && isPolygonVertexHandle(handle)) {
    return movePolygonVertex(anchor, Number(handle.slice('vertex_'.length)), point)
  }
  if (isBoxAnchor(anchor) && isBoxHandle(handle)) {
    return resizeBoxAnchor(anchor, handle, point)
  }
  return null
}

function bindWindowPointerSession(
  pointerId: number,
  handlers: {
    move(pointer: PointerEvent): void
    finish(pointer: PointerEvent): void
    cancel(pointer: PointerEvent): void
  },
) {
  let active = true
  function owns(pointer: PointerEvent) {
    return pointer.pointerId === pointerId
  }
  function move(pointer: PointerEvent) {
    if (active && owns(pointer)) handlers.move(pointer)
  }
  function finish(pointer: PointerEvent) {
    if (!active || !owns(pointer)) return
    cleanup()
    handlers.finish(pointer)
  }
  function cancel(pointer: PointerEvent) {
    if (!active || !owns(pointer)) return
    cleanup()
    handlers.cancel(pointer)
  }
  function cleanup() {
    if (!active) return
    active = false
    window.removeEventListener('pointermove', move)
    window.removeEventListener('pointerup', finish)
    window.removeEventListener('pointercancel', cancel)
  }
  window.addEventListener('pointermove', move)
  window.addEventListener('pointerup', finish)
  window.addEventListener('pointercancel', cancel)
  return cleanup
}

function handlePoint(
  anchor: Extract<
    ImageAnchor,
    { kind: 'image_rect' | 'image_ellipse' | 'image_arrow' | 'image_line' | 'image_polygon' }
  >,
  handle: BoxHandle | 'tail' | 'head' | PolygonVertexHandle,
): NormalizedPoint | null {
  if (anchor.kind === 'image_arrow' || anchor.kind === 'image_line')
    return handle === 'tail' ? anchor.tail : handle === 'head' ? anchor.head : null
  if (anchor.kind === 'image_polygon') {
    if (!isPolygonVertexHandle(handle)) return null
    return anchor.points[Number(handle.slice('vertex_'.length))] ?? null
  }
  if (handle === 'tail' || handle === 'head' || isPolygonVertexHandle(handle)) return null
  const left = anchor.x
  const top = anchor.y
  const right = anchor.x + anchor.width
  const bottom = anchor.y + anchor.height
  const middleX = left + anchor.width / 2
  const middleY = top + anchor.height / 2
  switch (handle) {
    case 'north_west':
      return { x: left, y: top }
    case 'north':
      return { x: middleX, y: top }
    case 'north_east':
      return { x: right, y: top }
    case 'east':
      return { x: right, y: middleY }
    case 'south_east':
      return { x: right, y: bottom }
    case 'south':
      return { x: middleX, y: bottom }
    case 'south_west':
      return { x: left, y: bottom }
    case 'west':
      return { x: left, y: middleY }
    default:
      return { x: middleX, y: middleY }
  }
}

function isImageAnchor(anchor: ReviewAnchor): anchor is ImageAnchor {
  return anchor.kind.startsWith('image_')
}

function isBoxAnchor(
  anchor: ImageAnchor,
): anchor is Extract<ImageAnchor, { kind: 'image_rect' | 'image_ellipse' }> {
  return anchor.kind === 'image_rect' || anchor.kind === 'image_ellipse'
}

function isBoxHandle(handle: AnnotationHandle): handle is BoxHandle {
  return (
    handle === 'north_west' ||
    handle === 'north' ||
    handle === 'north_east' ||
    handle === 'east' ||
    handle === 'south_east' ||
    handle === 'south' ||
    handle === 'south_west' ||
    handle === 'west'
  )
}

function isPolygonVertexHandle(handle: AnnotationHandle): handle is PolygonVertexHandle {
  return handle.startsWith('vertex_')
}

function handleLabel(
  handle: BoxHandle | 'tail' | 'head' | PolygonVertexHandle,
  anchorKind: 'image_rect' | 'image_ellipse' | 'image_arrow' | 'image_line' | 'image_polygon',
) {
  if (handle === 'tail') return anchorKind === 'image_arrow' ? '箭尾' : '起点'
  if (handle === 'head') return anchorKind === 'image_arrow' ? '箭头' : '终点'
  if (isPolygonVertexHandle(handle)) return `顶点 ${Number(handle.slice('vertex_'.length)) + 1}`
  if (handle === 'north_west') return '左上角'
  if (handle === 'north') return '上边'
  if (handle === 'north_east') return '右上角'
  if (handle === 'east') return '右边'
  if (handle === 'south_east') return '右下角'
  if (handle === 'south') return '下边'
  if (handle === 'south_west') return '左下角'
  return '左边'
}

function annotationSelectionColor(element: HTMLCanvasElement): string {
  try {
    return (
      getComputedStyle(element).getPropertyValue('--viewer-secondary-container').trim() ||
      'Highlight'
    )
  } catch {
    return 'Highlight'
  }
}

function arrowDirection(key: string) {
  if (key === 'ArrowLeft') return { x: -1, y: 0 }
  if (key === 'ArrowRight') return { x: 1, y: 0 }
  if (key === 'ArrowUp') return { x: 0, y: -1 }
  if (key === 'ArrowDown') return { x: 0, y: 1 }
  return null
}

function projectedImageSize(projection: ImagePreviewProjection) {
  const origin = projection.normalizedToStage({ x: 0, y: 0 })
  const horizontal = projection.normalizedToStage({ x: 1, y: 0 })
  const vertical = projection.normalizedToStage({ x: 0, y: 1 })
  if (origin === null || horizontal === null || vertical === null) {
    return { width: projection.stageRect.width, height: projection.stageRect.height }
  }
  return {
    width: Math.hypot(horizontal.x - origin.x, horizontal.y - origin.y),
    height: Math.hypot(vertical.x - origin.x, vertical.y - origin.y),
  }
}

function localPoint(point: NormalizedPoint, projection: ImagePreviewProjection) {
  return {
    left: point.x - projection.stageRect.left,
    top: point.y - projection.stageRect.top,
  }
}

function stageAnnotationProjection(projection: ImagePreviewProjection): AnnotationSceneProjection {
  return {
    localBounds: {
      width: projection.stageRect.width,
      height: projection.stageRect.height,
    },
    normalizedToLocal(point) {
      const projected = projection.normalizedToStage(point)
      return projected === null
        ? null
        : {
            x: projected.x - projection.stageRect.left,
            y: projected.y - projection.stageRect.top,
          }
    },
  }
}

function clientDistance(
  start: NormalizedPoint,
  pointer: Pick<PointerEvent, 'clientX' | 'clientY'>,
) {
  return Math.hypot(pointer.clientX - start.x, pointer.clientY - start.y)
}

function clampedPoint(point: NormalizedPoint): NormalizedPoint {
  return { x: clamp(point.x, 0, 1), y: clamp(point.y, 0, 1) }
}

function constrainLine(start: NormalizedPoint, end: NormalizedPoint): NormalizedPoint {
  return Math.abs(end.x - start.x) >= Math.abs(end.y - start.y)
    ? { x: end.x, y: start.y }
    : { x: start.x, y: end.y }
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
