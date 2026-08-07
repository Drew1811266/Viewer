import type { PointerEventHandler, RefObject } from 'react'
import { useCallback, useEffect, useRef } from 'react'
import type { Point } from './imageGeometry'

const LINE_HEIGHT = 16
const PINCH_SENSITIVITY = 0.002
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
  const pendingWheel = useRef<PendingWheelAction | null>(null)
  const frame = useRef<number | null>(null)
  const drag = useRef<{ pointerId: number; point: Point } | null>(null)

  useEffect(() => {
    const element = stage.current
    if (element === null) return

    const flush = () => {
      frame.current = null
      const action = pendingWheel.current
      pendingWheel.current = null
      if (action?.kind === 'pan') panBy(action.delta)
      if (action?.kind === 'pinch') {
        zoomBy(Math.exp(-action.deltaY * PINCH_SENSITIVITY), action.anchor)
      }
    }
    const schedule = () => {
      if (frame.current === null) frame.current = requestAnimationFrame(flush)
    }
    const onWheel = (event: WheelEvent) => {
      if (disabled || !event.cancelable || isToolbarTarget(event.target)) return
      event.preventDefault()
      const delta = normalizeWheelDelta(event, element.clientHeight || 1)
      if (event.ctrlKey) {
        const bounds = element.getBoundingClientRect()
        const anchor = { x: event.clientX - bounds.left, y: event.clientY - bounds.top }
        const previous = pendingWheel.current
        pendingWheel.current = {
          kind: 'pinch',
          deltaY: (previous?.kind === 'pinch' ? previous.deltaY : 0) + delta.y,
          anchor,
        }
      } else {
        const previous = pendingWheel.current
        const accumulated = previous?.kind === 'pan' ? previous.delta : { x: 0, y: 0 }
        pendingWheel.current = {
          kind: 'pan',
          delta: { x: accumulated.x - delta.x, y: accumulated.y - delta.y },
        }
      }
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

  const finishDrag = useCallback<PointerEventHandler<HTMLDivElement>>((event) => {
    if (drag.current?.pointerId !== event.pointerId) return
    drag.current = null
    event.currentTarget.releasePointerCapture?.(event.pointerId)
  }, [])

  const onPointerDown = useCallback<PointerEventHandler<HTMLDivElement>>(
    (event) => {
      if (disabled || event.button !== 0 || (panBounds.x === 0 && panBounds.y === 0)) return
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

function isToolbarTarget(target: EventTarget | null): boolean {
  return target instanceof Element && target.closest('[role="toolbar"]') !== null
}
