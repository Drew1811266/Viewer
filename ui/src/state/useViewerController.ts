import { useCallback, useEffect, useReducer, useRef } from 'react'
import type {
  ConflictResolution,
  FileCommandItem,
  FileCommandKind,
  OperationProgressEvent,
  RenameRules,
} from '../api/types'
import type { ViewerBridge } from '../api/viewer'
import { safeUserMessage } from '../api/viewer'
import type { ControllerCore } from './controllers/types'
import {
  refreshDesiredProjection,
  useProjectSessionController,
} from './controllers/useProjectSessionController'
import { useSearchController } from './controllers/useSearchController'
import { useSelectionMarkerController } from './controllers/useSelectionMarkerController'
import { initialViewerState, viewerReducer } from './viewerReducer'
import type { ViewerAction } from './viewerState'

export function useViewerController(bridge: ViewerBridge) {
  const [state, dispatch] = useReducer(viewerReducer, initialViewerState)
  const stateRef = useRef(state)
  const sessionEpochRef = useRef(0)
  const activeBatchRef = useRef<string | null>(null)
  const operationRequestPendingRef = useRef(false)
  const undoRequestPendingRef = useRef(false)
  const operationResultsRequestRef = useRef(0)
  const completedBatchesRef = useRef(new Set<string>())
  stateRef.current = state

  const resetLaterSessionRequests = useCallback(() => {
    activeBatchRef.current = null
    operationRequestPendingRef.current = false
    undoRequestPendingRef.current = false
    operationResultsRequestRef.current += 1
    completedBatchesRef.current.clear()
  }, [])

  const dispatchControllerAction = useCallback(
    (action: ViewerAction) => {
      if (action.type === 'project_closed') {
        resetLaterSessionRequests()
      }
      dispatch(action)
    },
    [resetLaterSessionRequests],
  )

  const core: ControllerCore = {
    bridge,
    state,
    stateRef,
    sessionEpochRef,
    dispatch: dispatchControllerAction,
  }
  const projectSession = useProjectSessionController(core)
  const refreshProjection = projectSession.refreshProjection
  const projectSessionCommands = {
    openProject: projectSession.openProject,
    closeProject: projectSession.closeProject,
    reselectProject: projectSession.reselectProject,
    selectFolder: projectSession.selectFolder,
    showAllDescendants: projectSession.showAllDescendants,
    cancelTask: projectSession.cancelTask,
  }
  const {
    setSearchText,
    setSearchScope,
    setSearchFilters,
    setSearchSort,
    setSearchLayout,
    removeSearchFilter,
    clearSearchFilters,
    setVisibleSearchHits,
    setSearchPage,
    returnToFolderContext,
  } = useSearchController(core, projectSession.sessionEpoch)
  const {
    setSelectedEntityIds,
    setReviewState,
    toggleFavorite,
    setPreviewEntityId,
    setCompareEntityIds,
    consumeContextRepair,
  } = useSelectionMarkerController(
    {
      ...core,
      refreshProjection,
      operationRequestPendingRef,
      undoRequestPendingRef,
      activeBatchRef,
    },
    projectSession.sessionEpoch,
  )

  const previewRename = useCallback(
    async (entityIds: string[], rules: RenameRules) => {
      const current = stateRef.current
      if (
        current.status !== 'active' ||
        current.project === null ||
        current.project.access === 'read_only'
      )
        return null
      const project = current.project
      const sessionEpoch = sessionEpochRef.current
      try {
        const preview = await bridge.previewRename({
          sessionId: project.sessionId,
          generation: project.generation,
          entityIds,
          rules,
        })
        const latest = stateRef.current
        if (
          sessionEpoch !== sessionEpochRef.current ||
          latest.status !== 'active' ||
          latest.project?.sessionId !== project.sessionId ||
          latest.project.generation !== project.generation
        ) {
          return null
        }
        return preview
      } catch (error) {
        if (sessionEpoch !== sessionEpochRef.current) return null
        dispatch({ type: 'input_rejected', message: safeUserMessage(error) })
        return null
      }
    },
    [bridge],
  )

  const preflightFileCommand = useCallback(
    async (kind: FileCommandKind, items: FileCommandItem[]) => {
      const current = stateRef.current
      if (
        current.status !== 'active' ||
        current.project === null ||
        current.project.access === 'read_only'
      )
        return null
      const project = current.project
      const sessionEpoch = sessionEpochRef.current
      try {
        const preflight = await bridge.preflightFileCommand({
          sessionId: project.sessionId,
          generation: project.generation,
          kind,
          items,
        })
        const latest = stateRef.current
        if (
          sessionEpoch !== sessionEpochRef.current ||
          latest.status !== 'active' ||
          latest.project?.sessionId !== project.sessionId ||
          latest.project.generation !== project.generation
        ) {
          return null
        }
        return preflight
      } catch (error) {
        if (sessionEpoch !== sessionEpochRef.current) return null
        dispatch({ type: 'input_rejected', message: safeUserMessage(error) })
        return null
      }
    },
    [bridge],
  )

  const finishOperation = useCallback(
    async (progress: OperationProgressEvent) => {
      const project = stateRef.current.project
      if (
        project === null ||
        project.sessionId !== progress.sessionId ||
        project.generation !== progress.generation ||
        activeBatchRef.current !== progress.batchId ||
        completedBatchesRef.current.has(progress.batchId)
      ) {
        return
      }
      const sessionEpoch = sessionEpochRef.current
      const projectIsCurrent = () => {
        const latestProject = stateRef.current.project
        return (
          sessionEpoch === sessionEpochRef.current &&
          latestProject?.sessionId === project.sessionId &&
          latestProject.generation === project.generation
        )
      }
      completedBatchesRef.current.add(progress.batchId)
      const loadResults = bridge
        .operationResults({
          sessionId: project.sessionId,
          generation: project.generation,
          batchId: progress.batchId,
          offset: 0,
          limit: 200,
        })
        .then((page) => {
          if (!projectIsCurrent()) return
          dispatch({
            type: 'operation_results_loaded',
            sessionId: project.sessionId,
            generation: project.generation,
            batchId: progress.batchId,
            page,
          })
        })
        .catch((error: unknown) => {
          if (!projectIsCurrent()) return
          dispatch({
            type: 'operation_failed',
            sessionId: project.sessionId,
            generation: project.generation,
            message: safeUserMessage(error),
          })
        })
      const refreshAfterTerminalBatch = (async () => {
        await refreshDesiredProjection(refreshProjection, project, true)
        if (!projectIsCurrent()) return
        dispatch({ type: 'search_refresh_requested' })
      })()
      try {
        await Promise.all([loadResults, refreshAfterTerminalBatch])
      } finally {
        const latestProject = stateRef.current.project
        if (
          sessionEpoch === sessionEpochRef.current &&
          latestProject?.sessionId === project.sessionId &&
          activeBatchRef.current === progress.batchId
        ) {
          activeBatchRef.current = null
          dispatch({
            type: 'operation_finish_settled',
            sessionId: latestProject.sessionId,
            generation: latestProject.generation,
            batchId: progress.batchId,
          })
        }
      }
    },
    [bridge, refreshProjection],
  )

  const receiveOperationProgress = useCallback(
    (progress: OperationProgressEvent) => {
      const project = stateRef.current.project
      if (
        project === null ||
        project.sessionId !== progress.sessionId ||
        project.generation !== progress.generation ||
        activeBatchRef.current !== progress.batchId
      ) {
        return
      }
      dispatch({ type: 'operation_progress_received', progress })
      if (progress.lifecycle === 'completed') void finishOperation(progress)
    },
    [finishOperation],
  )

  const executeFileCommand = useCallback(
    async (
      kind: FileCommandKind,
      items: FileCommandItem[],
      conflicts: ConflictResolution[] = [],
    ) => {
      const current = stateRef.current
      if (
        current.status !== 'active' ||
        current.project === null ||
        current.project.access === 'read_only' ||
        operationRequestPendingRef.current ||
        undoRequestPendingRef.current ||
        activeBatchRef.current !== null
      ) {
        return null
      }
      const project = current.project
      operationRequestPendingRef.current = true
      try {
        const started = await bridge.executeFileCommand({
          sessionId: project.sessionId,
          generation: project.generation,
          kind,
          items,
          conflicts,
        })
        if (
          stateRef.current.project?.sessionId !== project.sessionId ||
          stateRef.current.project.generation !== project.generation
        ) {
          return null
        }
        activeBatchRef.current = started.batchId
        operationResultsRequestRef.current += 1
        dispatch({
          type: 'operation_started',
          sessionId: project.sessionId,
          generation: project.generation,
          batchId: started.batchId,
          kind,
        })
        void bridge
          .operationStatus({
            sessionId: project.sessionId,
            generation: project.generation,
            batchId: started.batchId,
          })
          .then((progress) => {
            if (progress) receiveOperationProgress(progress)
          })
          .catch(() => undefined)
        return started
      } catch (error) {
        dispatch({ type: 'input_rejected', message: safeUserMessage(error) })
        return null
      } finally {
        operationRequestPendingRef.current = false
      }
    },
    [bridge, receiveOperationProgress],
  )

  const cancelOperation = useCallback(async () => {
    const project = stateRef.current.project
    const batchId = activeBatchRef.current
    if (project === null || batchId === null) return false
    try {
      return await bridge.cancelOperation({
        sessionId: project.sessionId,
        generation: project.generation,
        batchId,
      })
    } catch (error) {
      dispatch({ type: 'input_rejected', message: safeUserMessage(error) })
      return false
    }
  }, [bridge])

  const loadOperationResults = useCallback(
    async (offset: number) => {
      const current = stateRef.current
      const active = current.operation.active
      if (current.project === null || active === null) return null
      const requestId = ++operationResultsRequestRef.current
      const project = current.project
      const batchId = active.batchId
      try {
        const page = await bridge.operationResults({
          sessionId: project.sessionId,
          generation: project.generation,
          batchId,
          offset: Math.max(0, offset),
          limit: 200,
        })
        if (
          requestId !== operationResultsRequestRef.current ||
          stateRef.current.project?.sessionId !== project.sessionId ||
          stateRef.current.project.generation !== project.generation ||
          stateRef.current.operation.active?.batchId !== batchId
        )
          return null
        dispatch({
          type: 'operation_results_loaded',
          sessionId: project.sessionId,
          generation: project.generation,
          batchId,
          page,
        })
        return page
      } catch (error) {
        if (requestId !== operationResultsRequestRef.current) return null
        dispatch({
          type: 'operation_failed',
          sessionId: project.sessionId,
          generation: project.generation,
          message: safeUserMessage(error),
        })
        return null
      }
    },
    [bridge],
  )

  const undoLastOperation = useCallback(async () => {
    const current = stateRef.current
    if (
      current.status !== 'active' ||
      current.project === null ||
      current.project.access === 'read_only' ||
      operationRequestPendingRef.current ||
      undoRequestPendingRef.current ||
      activeBatchRef.current !== null
    )
      return null
    undoRequestPendingRef.current = true
    dispatch({ type: 'operation_request_pending', pending: true })
    try {
      const receipt = await bridge.undoLastOperation({
        sessionId: current.project.sessionId,
        generation: current.project.generation,
      })
      if (receipt) {
        await refreshDesiredProjection(refreshProjection, current.project, true)
        dispatch({ type: 'search_refresh_requested' })
      }
      return receipt
    } catch (error) {
      dispatch({ type: 'input_rejected', message: safeUserMessage(error) })
      return null
    } finally {
      undoRequestPendingRef.current = false
      dispatch({ type: 'operation_request_pending', pending: false })
    }
  }, [bridge, refreshProjection])

  const clearCloseBlocked = useCallback(() => {
    dispatch({ type: 'close_blocked_cleared' })
  }, [])

  const openPermissionSettings = useCallback(async () => {
    try {
      await bridge.openPermissionSettings()
    } catch (error) {
      dispatch({ type: 'input_rejected', message: safeUserMessage(error) })
    }
  }, [bridge])

  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined
    void Promise.resolve()
      .then(() => bridge.listenOperationProgress(receiveOperationProgress))
      .then((cleanup) => {
        if (disposed) cleanup()
        else unlisten = cleanup
      })
      .catch(() => undefined)
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [bridge, receiveOperationProgress])

  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined
    void Promise.resolve()
      .then(() =>
        bridge.listenProjectChanged((change) => {
          const project = stateRef.current.project
          if (
            project === null ||
            project.sessionId !== change.sessionId ||
            project.generation !== change.generation
          ) {
            return
          }
          if (change.reason === 'expected_viewer_change') return
          dispatch({ type: 'project_changed_received', change })
          void refreshDesiredProjection(refreshProjection, project, true)
        }),
      )
      .then((cleanup) => {
        if (disposed) cleanup()
        else unlisten = cleanup
      })
      .catch(() => undefined)
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [bridge, refreshProjection])

  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined
    void Promise.resolve()
      .then(() =>
        bridge.listenCloseBlocked((event) => {
          dispatch({ type: 'close_blocked_received', event })
        }),
      )
      .then((cleanup) => {
        if (disposed) cleanup()
        else unlisten = cleanup
      })
      .catch(() => undefined)
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [bridge])

  return {
    state,
    ...projectSessionCommands,
    setSearchText,
    setSearchScope,
    setSearchFilters,
    setSearchSort,
    setSearchLayout,
    removeSearchFilter,
    clearSearchFilters,
    setVisibleSearchHits,
    setSearchPage,
    returnToFolderContext,
    setSelectedEntityIds,
    setReviewState,
    toggleFavorite,
    previewRename,
    preflightFileCommand,
    executeFileCommand,
    cancelOperation,
    loadOperationResults,
    undoLastOperation,
    setPreviewEntityId,
    setCompareEntityIds,
    consumeContextRepair,
    clearCloseBlocked,
    openPermissionSettings,
  }
}

export type ViewerController = ReturnType<typeof useViewerController>
