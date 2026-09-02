import type { PointerEventHandler, RefObject } from 'react'
import { useCallback, useEffect, useRef } from 'react'
import type { Point } from './imageGeometry'

const LINE_HEIGHT = 16
const PINCH_SENSITIVITY = 0.004
const WHEEL_LISTENER_OPTIONS = { passive: false, capture: false } as const

interface PreviewGestureOptions {
  stage: RefObject<HTMLElement | null>
  disabled: boolean
  panBounds: Point
  zoomBy: (factor: number, anchor: Point) => void
  panBy: (delta: Point) => void
}

interface PreviewPointerHandlers {
  onPointerDown: PointerEventHandler<HTMLDivElement>
  onPointerMove: PointerEventHandler<HTMLDivElement>
  onPointerUp: PointerEventHandler<HTMLDivElement>
  onPointerCancel: PointerEventHandler<HTMLDivElement>
  onLostPointerCapture: PointerEventHandler<HTMLDivElement>
}

type PendingWheelAction =
  | { kind: 'pan'; delta: Point }
  | { kind: 'pinch'; deltaY: number; anchor: Point }

export function normalizeWheelDelta(event: WheelEvent, pageSize: number): Point {
  const factor =
    event.deltaMode === WheelEvent.DOM_DELTA_LINE
      ? LINE_HEIGHT
      : event.deltaMode === WheelEvent.DOM_DELTA_PAGE
        ? pageSize
        : 1
  return { x: event.deltaX * factor, y: event.deltaY * factor }
}

export function usePreviewGestures({
  stage,
  disabled,
  panBounds,
  zoomBy,
  panBy,
}: PreviewGestureOptions): PreviewPointerHandlers {
  usePreviewWheel({ stage, disabled, panBounds, zoomBy, panBy })
  return usePreviewPointerPan({ stage, disabled, panBounds, zoomBy, panBy })
}

function usePreviewWheel({ stage, disabled, zoomBy, panBy }: PreviewGestureOptions) {
  const pendingWheel = useRef<PendingWheelAction | null>(null)
  const frame = useRef<number | null>(null)

  useEffect(() => {
    const element = stage.current
    if (element === null) return

    const flush = () => {
      frame.current = null
      const action = pendingWheel.current
      pendingWheel.current = null
      executeWheelAction(action, zoomBy, panBy)
    }
    const schedule = () => {
      if (frame.current === null) frame.current = requestAnimationFrame(flush)
    }
    const onWheel = (event: WheelEvent) => {
      if (disabled || !event.cancelable || isToolbarTarget(event.target)) return
      event.preventDefault()
      const delta = normalizeWheelDelta(event, element.clientHeight || 1)
      pendingWheel.current = accumulateWheelAction(event, element, delta, pendingWheel.current)
      schedule()
    }

    element.addEventListener('wheel', onWheel, WHEEL_LISTENER_OPTIONS)
    return () => {
      element.removeEventListener('wheel', onWheel, WHEEL_LISTENER_OPTIONS.capture)
      if (frame.current !== null) cancelAnimationFrame(frame.current)
      frame.current = null
      pendingWheel.current = null
    }
  }, [disabled, panBy, stage, zoomBy])
}

function usePreviewPointerPan({ disabled, panBounds, panBy }: PreviewGestureOptions) {
  const drag = useRef<{ pointerId: number; point: Point } | null>(null)
  const finishDrag = useCallback<PointerEventHandler<HTMLDivElement>>((event) => {
    if (drag.current?.pointerId !== event.pointerId) return
    drag.current = null
    event.currentTarget.releasePointerCapture?.(event.pointerId)
  }, [])

  const onPointerDown = useCallback<PointerEventHandler<HTMLDivElement>>(
    (event) => {
      if (
        disabled ||
        event.button !== 0 ||
        (panBounds.x === 0 && panBounds.y === 0) ||
        isInteractiveTarget(event.target)
      )
        return
      drag.current = {
        pointerId: event.pointerId,
        point: { x: event.clientX, y: event.clientY },
      }
      event.currentTarget.setPointerCapture?.(event.pointerId)
    },
    [disabled, panBounds.x, panBounds.y],
  )

  const onPointerMove = useCallback<PointerEventHandler<HTMLDivElement>>(
    (event) => {
      const current = drag.current
      if (current?.pointerId !== event.pointerId) return
      const next = { x: event.clientX, y: event.clientY }
      panBy({ x: next.x - current.point.x, y: next.y - current.point.y })
      drag.current = { pointerId: current.pointerId, point: next }
    },
    [panBy],
  )

  const onLostPointerCapture = useCallback<PointerEventHandler<HTMLDivElement>>((event) => {
    if (drag.current?.pointerId === event.pointerId) drag.current = null
  }, [])

  return {
    onPointerDown,
    onPointerMove,
    onPointerUp: finishDrag,
    onPointerCancel: finishDrag,
    onLostPointerCapture,
  }
}

function executeWheelAction(
  action: PendingWheelAction | null,
  zoomBy: PreviewGestureOptions['zoomBy'],
  panBy: PreviewGestureOptions['panBy'],
) {
  if (action?.kind === 'pan') panBy(action.delta)
  if (action?.kind === 'pinch') {
    zoomBy(Math.exp(-action.deltaY * PINCH_SENSITIVITY), action.anchor)
  }
}

function accumulateWheelAction(
  event: WheelEvent,
  element: HTMLElement,
  delta: Point,
  previous: PendingWheelAction | null,
): PendingWheelAction {
  if (event.ctrlKey) {
    const bounds = element.getBoundingClientRect()
    return {
      kind: 'pinch',
      deltaY: (previous?.kind === 'pinch' ? previous.deltaY : 0) + delta.y,
      anchor: { x: event.clientX - bounds.left, y: event.clientY - bounds.top },
    }
  }
  const accumulated = previous?.kind === 'pan' ? previous.delta : { x: 0, y: 0 }
  return {
    kind: 'pan',
    delta: { x: accumulated.x - delta.x, y: accumulated.y - delta.y },
  }
}

function isToolbarTarget(target: EventTarget | null): boolean {
  return target instanceof Element && target.closest('[role="toolbar"]') !== null
}

function isInteractiveTarget(target: EventTarget | null): boolean {
  return (
    target instanceof Element &&
    target.closest(
      'a[href], button, input, select, textarea, [contenteditable]:not([contenteditable="false"]), [role="button"], [role="link"], [role="textbox"]',
    ) !== null
  )
}
