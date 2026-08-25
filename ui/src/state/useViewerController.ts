import { useCallback, useReducer, useRef } from 'react'
import type { ViewerBridge } from '../api/viewer'
import { safeUserMessage } from '../api/viewer'
import type { ControllerCore } from './controllers/types'
import { useLifecycleSubscriptions } from './controllers/useLifecycleSubscriptions'
import {
  getOperationControllerInternals,
  useOperationController,
} from './controllers/useOperationController'
import { useProjectSessionController } from './controllers/useProjectSessionController'
import { useSearchController } from './controllers/useSearchController'
import { useSelectionMarkerController } from './controllers/useSelectionMarkerController'
import { initialViewerState, viewerReducer } from './viewerReducer'
import type { ViewerAction } from './viewerState'

export function useViewerController(bridge: ViewerBridge) {
  const [state, dispatch] = useReducer(viewerReducer, initialViewerState)
  const stateRef = useRef(state)
  const sessionEpochRef = useRef(0)
  const resetOperationSessionRequestsRef = useRef<(sessionEpoch: number) => void>(() => undefined)
  stateRef.current = state

  const dispatchControllerAction = useCallback((action: ViewerAction) => {
    if (action.type === 'project_closed') {
      resetOperationSessionRequestsRef.current(sessionEpochRef.current)
    }
    dispatch(action)
  }, [])

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
  const operationController = useOperationController(
    core,
    projectSession.sessionEpoch,
    refreshProjection,
  )
  const {
    activeBatchRef,
    operationRequestPendingRef,
    resetSessionRequests,
    undoRequestPendingRef,
  } = getOperationControllerInternals(operationController)
  resetOperationSessionRequestsRef.current = resetSessionRequests
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
  const {
    previewRename,
    preflightFileCommand,
    executeFileCommand,
    cancelOperation,
    loadOperationResults,
    undoLastOperation,
    receiveOperationProgress,
  } = operationController

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

  const beginFinderDrag = useCallback(
    async (entityIds: readonly string[]) => {
      const project = state.project
      if (project === null || state.status !== 'active' || entityIds.length === 0) return
      await bridge.beginFinderDrag({
        sessionId: project.sessionId,
        generation: project.generation,
        entityIds: [...entityIds],
      })
    },
    [bridge, state.project, state.status],
  )

  useLifecycleSubscriptions(core, { receiveOperationProgress, refreshProjection })

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
    beginFinderDrag,
    setPreviewEntityId,
    setCompareEntityIds,
    consumeContextRepair,
    clearCloseBlocked,
    openPermissionSettings,
  }
}

export type ViewerController = ReturnType<typeof useViewerController>
