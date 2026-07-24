import { useCallback, useEffect, useReducer, useRef } from 'react'
import type {
  ConflictResolution,
  FileCommandItem,
  FileCommandKind,
  OperationProgressEvent,
  ProjectSnapshot,
  RenameRules,
  ReviewState,
  SearchFilters,
  SearchLayout,
  SearchQueryModel,
  SearchSort,
} from '../api/types'
import type { ViewerBridge } from '../api/viewer'
import { safeUserMessage } from '../api/viewer'
import type { ControllerCore } from './controllers/types'
import {
  refreshDesiredProjection,
  useProjectSessionController,
} from './controllers/useProjectSessionController'
import type { SearchFilterChip } from './viewerReducer'
import { initialViewerState, viewerReducer } from './viewerReducer'
import type { ViewerAction } from './viewerState'

export function useViewerController(bridge: ViewerBridge) {
  const [state, dispatch] = useReducer(viewerReducer, initialViewerState)
  const stateRef = useRef(state)
  const sessionEpochRef = useRef(0)
  const searchRevisionRef = useRef(0)
  const selectionRequestRef = useRef(0)
  const requestedSnippetsRef = useRef(new Set<string>())
  const activeBatchRef = useRef<string | null>(null)
  const operationRequestPendingRef = useRef(false)
  const undoRequestPendingRef = useRef(false)
  const operationResultsRequestRef = useRef(0)
  const completedBatchesRef = useRef(new Set<string>())
  stateRef.current = state

  const resetLaterSessionRequests = useCallback(() => {
    searchRevisionRef.current += 1
    selectionRequestRef.current += 1
    requestedSnippetsRef.current.clear()
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

  const executeSearch = useCallback(
    async (project: ProjectSnapshot, query: SearchQueryModel, offset: number) => {
      const revision = ++searchRevisionRef.current
      requestedSnippetsRef.current.clear()
      dispatch({ type: 'search_requested', revision })
      try {
        const page = await bridge.searchProject({
          sessionId: project.sessionId,
          generation: project.generation,
          revision,
          ...query,
          offset,
          limit: 200,
        })
        dispatch({
          type: 'search_loaded',
          sessionId: project.sessionId,
          generation: project.generation,
          page,
        })
      } catch (error) {
        dispatch({
          type: 'search_failed',
          sessionId: project.sessionId,
          generation: project.generation,
          revision,
          message: safeUserMessage(error),
        })
      }
    },
    [bridge],
  )

  useEffect(() => {
    const project = state.project
    if (project === null || state.status !== 'active' || state.search.queryVersion === 0) {
      return
    }
    const delay = state.search.schedule === 'debounced' ? 120 : 0
    const query = state.search.query
    const offset = state.search.offset
    const timer = window.setTimeout(() => void executeSearch(project, query, offset), delay)
    return () => window.clearTimeout(timer)
  }, [
    executeSearch,
    state.project,
    state.search.query,
    state.search.queryVersion,
    state.search.schedule,
    state.search.offset,
    state.status,
  ])

  useEffect(() => {
    const project = state.project
    const page = state.search.page
    if (project === null || page === null || page.revision !== state.search.revision) return
    const visible = new Set(state.search.visibleEntityIds)
    for (const hit of page.hits) {
      if (hit.matchedField !== 'body' || !visible.has(hit.entityId)) continue
      const key = `${page.revision}:${hit.entityId}`
      if (requestedSnippetsRef.current.has(key)) continue
      requestedSnippetsRef.current.add(key)
      void bridge
        .searchTextSnippet({
          sessionId: project.sessionId,
          generation: project.generation,
          revision: page.revision,
          entityId: hit.entityId,
          query: state.search.query.text,
        })
        .then((result) => {
          dispatch({
            type: 'search_snippet_loaded',
            revision: result.revision,
            entityId: result.entityId,
            snippet: result.snippet,
          })
        })
        .catch(() => undefined)
    }
  }, [
    bridge,
    state.project,
    state.search.page,
    state.search.query.text,
    state.search.revision,
    state.search.visibleEntityIds,
  ])

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

  useEffect(() => {
    function requestSearchFocus(event: KeyboardEvent) {
      if (!(event.metaKey && event.key.toLowerCase() === 'f')) return
      if (stateRef.current.project === null) return
      event.preventDefault()
      dispatch({ type: 'search_focus_requested' })
    }
    window.addEventListener('keydown', requestSearchFocus)
    return () => window.removeEventListener('keydown', requestSearchFocus)
  }, [])

  const setSearchText = useCallback((text: string) => {
    dispatch({ type: 'search_text_changed', text })
  }, [])

  const setSearchScope = useCallback((folderId: string | null) => {
    dispatch({ type: 'search_scope_changed', folderId })
  }, [])

  const setSearchFilters = useCallback((filters: SearchFilters) => {
    dispatch({ type: 'search_filters_changed', filters })
  }, [])

  const setSearchSort = useCallback((sort: SearchSort) => {
    dispatch({ type: 'search_sort_changed', sort })
  }, [])

  const setSearchLayout = useCallback((layout: SearchLayout) => {
    dispatch({ type: 'search_layout_changed', layout })
  }, [])

  const removeSearchFilter = useCallback((chip: SearchFilterChip) => {
    dispatch({ type: 'search_filter_chip_removed', chip })
  }, [])

  const clearSearchFilters = useCallback(() => {
    dispatch({ type: 'search_filters_cleared' })
  }, [])

  const setVisibleSearchHits = useCallback((entityIds: string[]) => {
    dispatch({ type: 'visible_search_hits_changed', entityIds })
  }, [])

  const setSearchPage = useCallback((offset: number) => {
    dispatch({ type: 'search_page_changed', offset })
  }, [])

  const returnToFolderContext = useCallback(() => {
    searchRevisionRef.current += 1
    requestedSnippetsRef.current.clear()
    dispatch({ type: 'search_context_closed' })
  }, [])

  const refreshSelectionInfo = useCallback(
    async (project: ProjectSnapshot, entityIds: string[]) => {
      const request = ++selectionRequestRef.current
      try {
        const info = await bridge.selectionInfo({
          sessionId: project.sessionId,
          generation: project.generation,
          entityIds,
        })
        if (request !== selectionRequestRef.current) return
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
    [bridge],
  )

  const setSelectedEntityIds = useCallback(
    (entityIds: string[]) => {
      dispatch({ type: 'selection_changed', entityIds })
      const project = stateRef.current.project
      if (project === null) return
      void refreshSelectionInfo(project, entityIds)
    },
    [refreshSelectionInfo],
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
      try {
        const result = await bridge.setReviewState({
          sessionId: current.project.sessionId,
          generation: current.project.generation,
          entityIds: targetEntityIds,
          reviewState,
        })
        dispatch({
          type: 'marker_changes_applied',
          sessionId: current.project.sessionId,
          generation: current.project.generation,
          changes: result.changes,
        })
        await Promise.all([
          refreshSelectionInfo(current.project, current.selectedEntityIds),
          refreshDesiredProjection(refreshProjection, current.project),
        ])
      } catch (error) {
        dispatch({ type: 'input_rejected', message: safeUserMessage(error) })
      }
    },
    [bridge, refreshProjection, refreshSelectionInfo],
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
      try {
        const result = await bridge.toggleFavorite({
          sessionId: current.project.sessionId,
          generation: current.project.generation,
          entityIds: targetEntityIds,
        })
        dispatch({
          type: 'marker_changes_applied',
          sessionId: current.project.sessionId,
          generation: current.project.generation,
          changes: result.changes,
        })
        await Promise.all([
          refreshSelectionInfo(current.project, current.selectedEntityIds),
          refreshDesiredProjection(refreshProjection, current.project),
        ])
      } catch (error) {
        dispatch({ type: 'input_rejected', message: safeUserMessage(error) })
      }
    },
    [bridge, refreshProjection, refreshSelectionInfo],
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

  const setPreviewEntityId = useCallback((entityId: string | null) => {
    dispatch({ type: 'preview_context_changed', entityId })
  }, [])

  const setCompareEntityIds = useCallback((entityIds: string[]) => {
    dispatch({ type: 'compare_context_changed', entityIds })
  }, [])

  const consumeContextRepair = useCallback(() => {
    dispatch({ type: 'context_repair_consumed' })
  }, [])

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

function uniqueEntityIds(entityIds: readonly string[]): string[] {
  return [...new Set(entityIds)]
}
