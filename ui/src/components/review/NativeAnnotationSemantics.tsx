import { type KeyboardEvent, useEffect, useRef } from 'react'
import {
  arrowFromDrag,
  type BoxHandle,
  type ImageAnchor,
  lineFromDrag,
  moveAnchor,
  movePolygonVertex,
  type PolygonVertexHandle,
  resizeBoxAnchor,
} from '../../app/review/annotationGeometry'
import type { ImageReviewWorkbenchController } from '../../app/review/useImageReviewWorkbench'
import type { ImagePreviewProjection } from '../imagePreview/ImagePreviewSurface'
import { annotationMarkerPoint, annotationOrdinalPoint } from './annotationScene'

type Handle = BoxHandle | 'tail' | 'head' | PolygonVertexHandle
export interface NativeAnnotationFocus {
  itemId: string | null
  handle?: Handle
}
const handleLabels: Record<BoxHandle | 'tail' | 'head', string> = {
  north_west: '左上角',
  north: '上边',
  north_east: '右上角',
  east: '右边',
  south_east: '右下角',
  south: '下边',
  south_west: '左下角',
  west: '左边',
  tail: '箭尾',
  head: '箭头',
}

function handleLabel(handle: Handle) {
  return isVertexHandle(handle)
    ? `顶点 ${Number(handle.slice('vertex_'.length)) + 1}`
    : handleLabels[handle]
}

function isVertexHandle(handle: Handle): handle is PolygonVertexHandle {
  return handle.startsWith('vertex_')
}

function isBoxHandle(handle: Handle): handle is BoxHandle {
  return handle !== 'tail' && handle !== 'head' && !isVertexHandle(handle)
}

/** Keyboard/accessibility only. The native renderer owns all annotation pixels and pointer input. */
export default function NativeAnnotationSemantics({
  controller,
  projection,
  focusRequest,
}: {
  controller: ImageReviewWorkbenchController
  projection: ImagePreviewProjection
  focusRequest?: NativeAnnotationFocus | null
}) {
  const root = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (focusRequest === undefined || focusRequest === null) return
    if (focusRequest.itemId === null) {
      if (
        document.activeElement instanceof HTMLElement &&
        root.current?.contains(document.activeElement)
      )
        document.activeElement.blur()
      return
    }
    for (const button of root.current?.querySelectorAll<HTMLButtonElement>('button') ?? []) {
      if (
        button.dataset.annotationId === focusRequest.itemId &&
        button.dataset.handle === focusRequest.handle
      ) {
        button.focus({ preventScroll: true })
        break
      }
    }
  }, [focusRequest])
  const editable = controller.readOnlyReason === null && controller.editor.phase.status === 'idle'
  const local = (point: { x: number; y: number }) => {
    const projected = projection.normalizedToStage(point)
    return projected === null
      ? null
      : {
          x: projected.x - projection.stageRect.left,
          y: projected.y - projection.stageRect.top,
        }
  }
  function keyDown(
    event: KeyboardEvent<HTMLButtonElement>,
    itemId: string,
    anchor: ImageAnchor,
    handle?: Handle,
  ) {
    if (event.key === 'Enter' && handle === undefined && editable) {
      event.preventDefault()
      event.stopPropagation()
      controller.beginFeedbackTextEdit(itemId)
      return
    }
    const direction =
      event.key === 'ArrowRight'
        ? { x: 1, y: 0 }
        : event.key === 'ArrowLeft'
          ? { x: -1, y: 0 }
          : event.key === 'ArrowDown'
            ? { x: 0, y: 1 }
            : event.key === 'ArrowUp'
              ? { x: 0, y: -1 }
              : null
    if (direction === null) return
    // A focused annotation owns arrows even when readonly or awaiting a save.
    event.preventDefault()
    event.stopPropagation()
    if (!editable) return
    const multiplier = event.shiftKey ? 10 : 1
    const delta = {
      x: (direction.x * multiplier) / projection.sourceSize.width,
      y: (direction.y * multiplier) / projection.sourceSize.height,
    }
    if (handle === undefined) {
      void controller.replaceFeedbackAnchor(itemId, moveAnchor(anchor, delta))
      return
    }
    const point = handlePoint(anchor, handle)
    if (point === null) return
    const moved = { x: point.x + delta.x, y: point.y + delta.y }
    const changed = anchorForHandle(anchor, handle, moved)
    if (changed !== null) void controller.replaceFeedbackAnchor(itemId, changed)
  }
  return (
    <div ref={root} className="native-annotation-semantics">
      {controller.feedback.map((feedback) => {
        if (!feedback.anchor.kind.startsWith('image_') || feedback.ordinal === null) return null
        const phase = controller.editor.phase
        const candidate =
          phase.status !== 'idle' && phase.sourceItemId === feedback.itemId
            ? phase.draftAnchor
            : feedback.anchor
        const anchor = candidate.kind.startsWith('image_')
          ? (candidate as ImageAnchor)
          : (feedback.anchor as ImageAnchor)
        const ordinal =
          annotationOrdinalPoint(
            anchor,
            { normalizedToLocal: local, localBounds: projection.stageRect },
            {
              lineWidth: anchor.kind === 'image_rect' || anchor.kind === 'image_ellipse' ? 6 : 2,
              ordinalRadius: 10,
            },
          ) ?? local(annotationMarkerPoint(anchor) ?? { x: 0.5, y: 0.5 })
        const handles: Handle[] =
          anchor.kind === 'image_arrow' || anchor.kind === 'image_line'
            ? ['tail', 'head']
            : anchor.kind === 'image_rect' || anchor.kind === 'image_ellipse'
              ? [
                  'north_west',
                  'north',
                  'north_east',
                  'east',
                  'south_east',
                  'south',
                  'south_west',
                  'west',
                ]
              : anchor.kind === 'image_polygon'
                ? anchor.points.map((_point, index) => `vertex_${index}` as const)
                : []
        const selected = controller.selectedItemId === feedback.itemId
        const selection = selected ? selectionBounds(anchor, local) : null
        return (
          <span key={feedback.itemId}>
            {selection !== null && (
              <span
                className="native-annotation-selection"
                aria-hidden="true"
                style={{
                  left: selection.left,
                  top: selection.top,
                  width: selection.width,
                  height: selection.height,
                }}
              />
            )}
            <button
              type="button"
              className="native-annotation-semantic"
              data-annotation-id={feedback.itemId}
              aria-label={`意见 ${feedback.ordinal}：${feedback.text}`}
              aria-pressed={selected}
              style={{ left: ordinal?.x ?? 0, top: ordinal?.y ?? 0 }}
              onFocus={() => {
                if (controller.editor.phase.status === 'idle')
                  controller.selectFeedback(feedback.itemId)
              }}
              onClick={() => controller.selectFeedback(feedback.itemId)}
              onKeyDown={(event) => keyDown(event, feedback.itemId, anchor)}
            />
            {selected &&
              handles.map((handle) => {
                const source = handlePoint(anchor, handle)
                const point = source === null ? null : local(source)
                return (
                  <button
                    key={handle}
                    type="button"
                    className="native-annotation-semantic"
                    data-annotation-id={feedback.itemId}
                    data-handle={handle}
                    aria-label={`调整意见 ${feedback.ordinal} ${handleLabel(handle)}`}
                    disabled={controller.readOnlyReason !== null}
                    aria-disabled={!editable}
                    style={{ left: point?.x ?? 0, top: point?.y ?? 0 }}
                    onKeyDown={(event) => keyDown(event, feedback.itemId, anchor, handle)}
                  />
                )
              })}
          </span>
        )
      })}
    </div>
  )
}

function selectionBounds(
  anchor: ImageAnchor,
  local: (point: { x: number; y: number }) => { x: number; y: number } | null,
) {
  const points: { x: number; y: number }[] = []
  if (anchor.kind === 'image_rect' || anchor.kind === 'image_ellipse') {
    points.push(
      { x: anchor.x, y: anchor.y },
      { x: anchor.x + anchor.width, y: anchor.y + anchor.height },
    )
  } else if (anchor.kind === 'image_arrow' || anchor.kind === 'image_line') {
    points.push(anchor.tail, anchor.head)
  } else if (anchor.kind === 'image_polygon' || anchor.kind === 'image_stroke') {
    points.push(...anchor.points)
  } else {
    return null
  }
  const projected = points.flatMap((point) => {
    const value = local(point)
    return value === null ? [] : [value]
  })
  if (projected.length === 0) return null
  const left = Math.min(...projected.map((point) => point.x)) - 5
  const top = Math.min(...projected.map((point) => point.y)) - 5
  const right = Math.max(...projected.map((point) => point.x)) + 5
  const bottom = Math.max(...projected.map((point) => point.y)) + 5
  return { left, top, width: Math.max(2, right - left), height: Math.max(2, bottom - top) }
}

function anchorForHandle(anchor: ImageAnchor, handle: Handle, point: { x: number; y: number }) {
  if (anchor.kind === 'image_arrow') {
    return handle === 'tail' ? arrowFromDrag(point, anchor.head) : arrowFromDrag(anchor.tail, point)
  }
  if (anchor.kind === 'image_line') {
    return handle === 'tail' ? lineFromDrag(point, anchor.head) : lineFromDrag(anchor.tail, point)
  }
  if (anchor.kind === 'image_polygon' && isVertexHandle(handle)) {
    return movePolygonVertex(anchor, Number(handle.slice('vertex_'.length)), point)
  }
  if ((anchor.kind === 'image_rect' || anchor.kind === 'image_ellipse') && isBoxHandle(handle)) {
    return resizeBoxAnchor(anchor, handle, point)
  }
  return null
}

function handlePoint(anchor: ImageAnchor, handle: Handle) {
  if (anchor.kind === 'image_arrow' || anchor.kind === 'image_line')
    return handle === 'tail' ? anchor.tail : handle === 'head' ? anchor.head : null
  if (anchor.kind === 'image_polygon') {
    return isVertexHandle(handle)
      ? (anchor.points[Number(handle.slice('vertex_'.length))] ?? null)
      : null
  }
  if (anchor.kind !== 'image_rect' && anchor.kind !== 'image_ellipse') return null
  if (!isBoxHandle(handle)) return null
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
      return null
  }
}
