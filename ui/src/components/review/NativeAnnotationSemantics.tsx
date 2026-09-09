import { type KeyboardEvent, useEffect, useRef } from 'react'
import {
  arrowFromDrag,
  type BoxHandle,
  type ImageAnchor,
  moveAnchor,
  resizeBoxAnchor,
} from '../../app/review/annotationGeometry'
import type { ImageReviewWorkbenchController } from '../../app/review/useImageReviewWorkbench'
import type { ImagePreviewProjection } from '../imagePreview/ImagePreviewSurface'
import { annotationMarkerPoint, annotationOrdinalPoint } from './annotationScene'

type Handle = BoxHandle | 'tail' | 'head'
export interface NativeAnnotationFocus {
  itemId: string | null
  handle?: Handle
}
const handleLabels: Record<Handle, string> = {
  north_west: '左上角',
  north_east: '右上角',
  south_east: '右下角',
  south_west: '左下角',
  tail: '箭尾',
  head: '箭头',
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
    if (!editable || anchor.kind === 'image_stroke') return
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
    const changed =
      anchor.kind === 'image_arrow'
        ? handle === 'tail'
          ? arrowFromDrag(moved, anchor.head)
          : arrowFromDrag(anchor.tail, moved)
        : (anchor.kind === 'image_rect' || anchor.kind === 'image_ellipse') &&
            handle !== 'tail' &&
            handle !== 'head'
          ? resizeBoxAnchor(anchor, handle, moved)
          : null
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
              ordinalRadius: 14,
            },
          ) ?? local(annotationMarkerPoint(anchor) ?? { x: 0.5, y: 0.5 })
        const handles: Handle[] =
          anchor.kind === 'image_arrow'
            ? ['tail', 'head']
            : anchor.kind === 'image_rect' || anchor.kind === 'image_ellipse'
              ? ['north_west', 'north_east', 'south_east', 'south_west']
              : []
        const selected = controller.selectedItemId === feedback.itemId
        return (
          <span key={feedback.itemId}>
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
                    aria-label={`调整意见 ${feedback.ordinal} ${handleLabels[handle]}`}
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

function handlePoint(anchor: ImageAnchor, handle: Handle) {
  if (anchor.kind === 'image_arrow') return handle === 'tail' ? anchor.tail : anchor.head
  if (anchor.kind !== 'image_rect' && anchor.kind !== 'image_ellipse') return null
  return {
    x: handle === 'north_west' || handle === 'south_west' ? anchor.x : anchor.x + anchor.width,
    y: handle === 'north_west' || handle === 'north_east' ? anchor.y : anchor.y + anchor.height,
  }
}
