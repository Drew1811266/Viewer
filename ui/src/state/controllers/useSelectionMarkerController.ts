import type { MutableRefObject } from 'react'
import { useCallback, useEffect, useRef } from 'react'
import type { ProjectSnapshot, ReviewState } from '../../api/types'
import { safeUserMessage } from '../../api/viewer'
import type { ControllerCore, RefreshProjection } from './types'
import { refreshDesiredProjection } from './useProjectSessionController'

export interface SelectionMarkerController {
  setSelectedEntityIds(entityIds: string[]): void
  setReviewState(reviewState: ReviewState | null, entityIdsOverride?: string[]): Promise<void>
  toggleFavorite(entityIdsOverride?: string[]): Promise<void>
  setPreviewEntityId(entityId: string | null): void
  setCompareEntityIds(entityIds: string[]): void
  consumeContextRepair(): void
}

export interface SelectionMarkerControllerCore extends ControllerCore {
  refreshProjection: RefreshProjection
  operationRequestPendingRef: MutableRefObject<boolean>
  undoRequestPendingRef: MutableRefObject<boolean>
  activeBatchRef: MutableRefObject<string | null>
}

export function useSelectionMarkerController(
  core: SelectionMarkerControllerCore,
  sessionEpoch: number,
): SelectionMarkerController {
  const {
    activeBatchRef,
    bridge,
    dispatch,
    operationRequestPendingRef,
    refreshProjection,
    sessionEpochRef,
    stateRef,
    undoRequestPendingRef,
  } = core
  const selectionRequestRef = useRef(0)
  const renderedSessionEpochRef = useRef(sessionEpoch)

  useEffect(() => {
    if (renderedSessionEpochRef.current === sessionEpoch) return
    renderedSessionEpochRef.current = sessionEpoch
    selectionRequestRef.current = 0
  }, [sessionEpoch])

  const refreshSelectionInfo = useCallback(
    async (project: ProjectSnapshot, entityIds: string[]) => {
      const requestSessionEpoch = sessionEpochRef.current
      const request = ++selectionRequestRef.current
      try {
        const info = await bridge.selectionInfo({
          sessionId: project.sessionId,
          generation: project.generation,
          entityIds,
        })
        if (
          requestSessionEpoch !== sessionEpochRef.current ||
          request !== selectionRequestRef.current
        ) {
          return
        }
        dispatch({
          type: 'selection_info_loaded',
          sessionId: project.sessionId,
          generation: project.generation,
          entityIds,
          info,
        })
      } catch {
        // Selection summaries are supplemental; the selection itself remains usable.
      }
    },
    [bridge, dispatch, sessionEpochRef],
  )

  const setSelectedEntityIds = useCallback(
    (entityIds: string[]) => {
      dispatch({ type: 'selection_changed', entityIds })
      const project = stateRef.current.project
      if (project === null) return
      void refreshSelectionInfo(project, entityIds)
    },
    [dispatch, refreshSelectionInfo, stateRef],
  )

  const setReviewState = useCallback(
    async (reviewState: ReviewState | null, entityIdsOverride?: string[]) => {
      const current = stateRef.current
      const requestedEntityIds = uniqueEntityIds(entityIdsOverride ?? current.selectedEntityIds)
      const targetEntityIds = current.search.showResults
        ? requestedEntityIds.filter((entityId) =>
            current.search.visibleEntityIds.includes(entityId),
          )
        : requestedEntityIds
      if (
        current.status !== 'active' ||
        current.project === null ||
        current.project.access === 'read_only' ||
        operationRequestPendingRef.current ||
        undoRequestPendingRef.current ||
        activeBatchRef.current !== null ||
        targetEntityIds.length === 0
      ) {
        return
      }
      const project = current.project
      const requestSessionEpoch = sessionEpochRef.current
      try {
        const result = await bridge.setReviewState({
          sessionId: project.sessionId,
          generation: project.generation,
          entityIds: targetEntityIds,
          reviewState,
        })
        if (requestSessionEpoch !== sessionEpochRef.current) return
        dispatch({
          type: 'marker_changes_applied',
          sessionId: project.sessionId,
          generation: project.generation,
          changes: result.changes,
        })
        await Promise.all([
          refreshSelectionInfo(project, current.selectedEntityIds),
          refreshDesiredProjection(refreshProjection, project),
        ])
      } catch (error) {
        if (requestSessionEpoch !== sessionEpochRef.current) return
        dispatch({ type: 'input_rejected', message: safeUserMessage(error) })
      }
    },
    [
      activeBatchRef,
      bridge,
      dispatch,
      operationRequestPendingRef,
      refreshProjection,
      refreshSelectionInfo,
      sessionEpochRef,
      stateRef,
      undoRequestPendingRef,
    ],
  )

  const toggleFavorite = useCallback(
    async (entityIdsOverride?: string[]) => {
      const current = stateRef.current
      const requestedEntityIds = uniqueEntityIds(entityIdsOverride ?? current.selectedEntityIds)
      const targetEntityIds = current.search.showResults
        ? requestedEntityIds.filter((entityId) =>
            current.search.visibleEntityIds.includes(entityId),
          )
        : requestedEntityIds
      if (
        current.status !== 'active' ||
        current.project === null ||
        current.project.access === 'read_only' ||
        operationRequestPendingRef.current ||
        undoRequestPendingRef.current ||
        activeBatchRef.current !== null ||
        targetEntityIds.length === 0
      ) {
        return
      }
      const project = current.project
      const requestSessionEpoch = sessionEpochRef.current
      try {
        const result = await bridge.toggleFavorite({
          sessionId: project.sessionId,
          generation: project.generation,
          entityIds: targetEntityIds,
        })
        if (requestSessionEpoch !== sessionEpochRef.current) return
        dispatch({
          type: 'marker_changes_applied',
          sessionId: project.sessionId,
          generation: project.generation,
          changes: result.changes,
        })
        await Promise.all([
          refreshSelectionInfo(project, current.selectedEntityIds),
          refreshDesiredProjection(refreshProjection, project),
        ])
      } catch (error) {
        if (requestSessionEpoch !== sessionEpochRef.current) return
        dispatch({ type: 'input_rejected', message: safeUserMessage(error) })
      }
    },
    [
      activeBatchRef,
      bridge,
      dispatch,
      operationRequestPendingRef,
      refreshProjection,
      refreshSelectionInfo,
      sessionEpochRef,
      stateRef,
      undoRequestPendingRef,
    ],
  )

  const setPreviewEntityId = useCallback(
    (entityId: string | null) => {
      dispatch({ type: 'preview_context_changed', entityId })
    },
    [dispatch],
  )

  const setCompareEntityIds = useCallback(
    (entityIds: string[]) => {
      dispatch({ type: 'compare_context_changed', entityIds })
    },
    [dispatch],
  )

  const consumeContextRepair = useCallback(() => {
    dispatch({ type: 'context_repair_consumed' })
  }, [dispatch])

  return {
    setSelectedEntityIds,
    setReviewState,
    toggleFavorite,
    setPreviewEntityId,
    setCompareEntityIds,
    consumeContextRepair,
  }
}

function uniqueEntityIds(entityIds: readonly string[]): string[] {
  return [...new Set(entityIds)]
}
