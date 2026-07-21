import { useCallback, useEffect, useRef, useState } from 'react'
import {
  pointDistance,
  verticalEdgeScrollDelta,
} from '../components/pointerGeometry'

export type OrganizationDragMode = 'move' | 'copy'

export type OrganizationPointerInput =
  | {
      type: 'start'
      pointerId: number
      entityIds: string[]
      mode: OrganizationDragMode
      clientX: number
      clientY: number
      captureNode: HTMLElement
    }
  | { type: 'move' | 'end'; pointerId: number; clientX: number; clientY: number }
  | { type: 'cancel'; pointerId: number }

export interface OrganizationDropTarget {
  entityId: string
  mode: OrganizationDragMode
  valid: boolean
}

export interface OrganizationDragView {
  clientX: number
  clientY: number
  itemCount: number
  mode: OrganizationDragMode
}

export interface UseOrganizationPointerDragOptions {
  disabled: boolean
  resetKey: string
  isDropTargetValid: (
    entityIds: readonly string[],
    destinationId: string,
    mode: OrganizationDragMode,
  ) => boolean
  onDrop: (
    entityIds: string[],
    destinationId: string,
    mode: OrganizationDragMode,
  ) => void
}

export interface OrganizationPointerDragResult {
  dragView: OrganizationDragView | null
  dropTarget: OrganizationDropTarget | null
  handlePointerInput: (input: OrganizationPointerInput) => void
  cancel: () => void
}

export const ORGANIZATION_FOLDER_ATTRIBUTE = 'data-organization-folder-id'
export const ORGANIZATION_DROP_SURFACE_ATTRIBUTE = 'data-organization-drop-surface'
const ACTIVATION_DISTANCE = 4

interface DragSession {
  phase: 'armed' | 'dragging'
  pointerId: number
  entityIds: readonly string[]
  mode: OrganizationDragMode
  startX: number
  startY: number
  clientX: number
  clientY: number
  captureNode: HTMLElement
  dropTarget: OrganizationDropTarget | null
  rafId: number | null
}

export function useOrganizationPointerDrag({
  disabled,
  resetKey,
  isDropTargetValid,
  onDrop,
}: UseOrganizationPointerDragOptions): OrganizationPointerDragResult {
  const [dragView, setDragView] = useState<OrganizationDragView | null>(null)
  const [dropTarget, setDropTarget] = useState<OrganizationDropTarget | null>(null)
  const sessionRef = useRef<DragSession | null>(null)
  const disabledRef = useRef(disabled)
  const validationRef = useRef(isDropTargetValid)
  const dropRef = useRef(onDrop)
  disabledRef.current = disabled
  validationRef.current = isDropTargetValid
  dropRef.current = onDrop

  const cancel = useCallback(() => {
    const session = sessionRef.current
    if (session?.rafId !== null && session?.rafId !== undefined) {
      cancelAnimationFrame(session.rafId)
      session.rafId = null
    }
    sessionRef.current = null
    setDragView(null)
    setDropTarget(null)
    if (!session) return
    try {
      if (session.captureNode.hasPointerCapture(session.pointerId)) {
        session.captureNode.releasePointerCapture(session.pointerId)
      }
    } catch {
      // Capture can disappear when the node or browsing context is detached.
    }
  }, [])

  const resolveDropTarget = useCallback(
    (session: DragSession): OrganizationDropTarget | null => {
      const hit = document.elementFromPoint(session.clientX, session.clientY)
      const row =
        hit?.closest<HTMLElement>(`[${ORGANIZATION_FOLDER_ATTRIBUTE}]`) ?? null
      const surface =
        row?.closest<HTMLElement>(`[${ORGANIZATION_DROP_SURFACE_ATTRIBUTE}]`) ??
        null
      const destinationId = surface
        ? (row?.dataset.organizationFolderId ?? null)
        : null
      const nextTarget = destinationId
        ? {
            entityId: destinationId,
            mode: session.mode,
            valid: validationRef.current(
              session.entityIds,
              destinationId,
              session.mode,
            ),
          }
        : null
      session.dropTarget = nextTarget
      setDropTarget(nextTarget)
      return nextTarget
    },
    [],
  )

  const scheduleEdgeScroll = useCallback(
    (session: DragSession) => {
      const stopFrame = () => {
        if (session.rafId === null) return
        cancelAnimationFrame(session.rafId)
        session.rafId = null
      }
      const surface = document.querySelector<HTMLElement>(
        `[${ORGANIZATION_DROP_SURFACE_ATTRIBUTE}]`,
      )
      if (!surface) {
        stopFrame()
        return
      }
      const bounds = surface.getBoundingClientRect()
      if (
        verticalEdgeScrollDelta(session.clientY, bounds.top, bounds.bottom) === 0
      ) {
        stopFrame()
        return
      }
      if (session.rafId !== null) return

      session.rafId = requestAnimationFrame(() => {
        session.rafId = null
        const current = sessionRef.current
        if (current !== session || current.phase !== 'dragging') return
        const currentSurface = document.querySelector<HTMLElement>(
          `[${ORGANIZATION_DROP_SURFACE_ATTRIBUTE}]`,
        )
        if (!currentSurface) return
        const currentBounds = currentSurface.getBoundingClientRect()
        const delta = verticalEdgeScrollDelta(
          current.clientY,
          currentBounds.top,
          currentBounds.bottom,
        )
        if (delta === 0) return

        const previousScrollTop = currentSurface.scrollTop
        currentSurface.scrollBy({ top: delta })
        resolveDropTarget(current)
        if (currentSurface.scrollTop === previousScrollTop) return
        if (sessionRef.current === current && current.phase === 'dragging') {
          scheduleEdgeScroll(current)
        }
      })
    },
    [resolveDropTarget],
  )

  const handlePointerInput = useCallback(
    (input: OrganizationPointerInput) => {
      if (input.type === 'start') {
        if (
          disabledRef.current ||
          sessionRef.current !== null ||
          input.entityIds.length === 0 ||
          new Set(input.entityIds).size !== input.entityIds.length
        ) {
          return
        }
        const session: DragSession = {
          phase: 'armed',
          pointerId: input.pointerId,
          entityIds: [...input.entityIds],
          mode: input.mode,
          startX: input.clientX,
          startY: input.clientY,
          clientX: input.clientX,
          clientY: input.clientY,
          captureNode: input.captureNode,
          dropTarget: null,
          rafId: null,
        }
        sessionRef.current = session
        try {
          input.captureNode.setPointerCapture(input.pointerId)
        } catch {
          // Capture is an optimization; lifecycle cleanup remains idempotent without it.
        }
        return
      }

      const session = sessionRef.current
      if (!session || session.pointerId !== input.pointerId) return
      if (input.type === 'cancel') {
        cancel()
        return
      }
      if (input.type === 'end') {
        const submission =
          session.phase === 'dragging' && session.dropTarget?.valid
            ? {
                entityIds: [...session.entityIds],
                destinationId: session.dropTarget.entityId,
                mode: session.mode,
              }
            : null
        cancel()
        if (submission) {
          dropRef.current(
            submission.entityIds,
            submission.destinationId,
            submission.mode,
          )
        }
        return
      }

      session.clientX = input.clientX
      session.clientY = input.clientY
      if (
        session.phase === 'armed' &&
        pointDistance(
          { x: session.startX, y: session.startY },
          { x: session.clientX, y: session.clientY },
        ) < ACTIVATION_DISTANCE
      ) {
        return
      }
      session.phase = 'dragging'
      setDragView({
        clientX: session.clientX,
        clientY: session.clientY,
        itemCount: session.entityIds.length,
        mode: session.mode,
      })
      resolveDropTarget(session)
      scheduleEdgeScroll(session)
    },
    [cancel, resolveDropTarget, scheduleEdgeScroll],
  )

  useEffect(() => {
    cancel()
  }, [cancel, disabled, resetKey])

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') cancel()
    }
    const onBlur = () => cancel()
    window.addEventListener('keydown', onKeyDown)
    window.addEventListener('blur', onBlur)
    return () => {
      window.removeEventListener('keydown', onKeyDown)
      window.removeEventListener('blur', onBlur)
      cancel()
    }
  }, [cancel])

  return { dragView, dropTarget, handlePointerInput, cancel }
}
