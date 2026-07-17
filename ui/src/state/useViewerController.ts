import { useCallback, useEffect, useReducer, useRef } from 'react'
import type { ViewerBridge } from '../api/viewer'
import { safeUserMessage } from '../api/viewer'
import type {
  ProjectSnapshot,
  ReviewState,
  ScanEvent,
  SearchFilters,
  SearchLayout,
  SearchQueryModel,
  SearchSort,
} from '../api/types'
import type { SearchFilterChip } from './viewerReducer'
import { initialViewerState, viewerReducer } from './viewerReducer'

export function useViewerController(bridge: ViewerBridge) {
  const [state, dispatch] = useReducer(viewerReducer, initialViewerState)
  const stateRef = useRef(state)
  const projectionRequestRef = useRef(0)
  const searchRevisionRef = useRef(0)
  const selectionRequestRef = useRef(0)
  const requestedSnippetsRef = useRef(new Set<string>())
  const reconcilingGenerationRef = useRef<number | null>(null)
  const desiredProjectionRef = useRef({
    selectedFolderId: null as string | null,
    selectedFolderPath: '',
    showingAggregate: false,
  })
  stateRef.current = state

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
    if (
      project === null ||
      state.status !== 'active' ||
      state.search.queryVersion === 0
    ) {
      return
    }
    const delay = state.search.schedule === 'debounced' ? 120 : 0
    const query = state.search.query
    const offset = state.search.offset
    const timer = window.setTimeout(
      () => void executeSearch(project, query, offset),
      delay,
    )
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

  const refreshProjection = useCallback(
    async (
      project: ProjectSnapshot,
      selectedFolderId: string | null,
      selectedFolderPath: string,
      showingAggregate: boolean,
    ) => {
      desiredProjectionRef.current = {
        selectedFolderId,
        selectedFolderPath,
        showingAggregate,
      }
      const requestId = ++projectionRequestRef.current
      try {
        const [folders, workspace] = await Promise.all([
          bridge.folderTree(),
          bridge.queryFolder(selectedFolderId, showingAggregate),
        ])
        if (requestId !== projectionRequestRef.current) return
        dispatch({
          type: 'projection_loaded',
          sessionId: project.sessionId,
          generation: project.generation,
          folders,
          workspace,
          selectedFolderId,
          selectedFolderPath,
          showingAggregate,
        })
      } catch (error) {
        if (requestId !== projectionRequestRef.current) return
        dispatch({
          type: 'projection_failed',
          sessionId: project.sessionId,
          generation: project.generation,
          message: safeUserMessage(error),
        })
      }
    },
    [bridge],
  )

  const openProject = useCallback(
    async (path: string) => {
      if (!path || !['empty', 'error'].includes(stateRef.current.status)) return
      dispatch({ type: 'project_open_requested' })
      try {
        const project = await bridge.openProject(path)
        dispatch({ type: 'project_opened', project })
        await refreshProjection(project, null, '', false)
      } catch (error) {
        dispatch({ type: 'project_open_failed', message: safeUserMessage(error) })
      }
    },
    [bridge, refreshProjection],
  )

  const closeProject = useCallback(async () => {
    if (stateRef.current.project === null || stateRef.current.status === 'closing') return
    dispatch({ type: 'project_close_requested' })
    projectionRequestRef.current += 1
    searchRevisionRef.current += 1
    selectionRequestRef.current += 1
    requestedSnippetsRef.current.clear()
    desiredProjectionRef.current = {
      selectedFolderId: null,
      selectedFolderPath: '',
      showingAggregate: false,
    }
    try {
      await bridge.closeProject()
      dispatch({ type: 'project_closed' })
    } catch (error) {
      dispatch({ type: 'project_close_failed', message: safeUserMessage(error) })
    }
  }, [bridge])

  const receiveScan = useCallback(
    (event: ScanEvent) => {
      const project = stateRef.current.project
      if (project === null || event.sessionId !== project.sessionId) return
      if (event.generation < project.generation) return
      if (event.generation === project.generation) {
        const desired = desiredProjectionRef.current
        dispatch({ type: 'scan_received', event })
        void refreshProjection(
          project,
          desired.selectedFolderId,
          desired.selectedFolderPath,
          desired.showingAggregate,
        )
        return
      }
      if (reconcilingGenerationRef.current === event.generation) return
      reconcilingGenerationRef.current = event.generation
      void bridge
        .projectSnapshot()
        .then((snapshot) => {
          if (
            snapshot === null ||
            snapshot.sessionId !== event.sessionId ||
            snapshot.generation !== event.generation
          ) {
            return
          }
          dispatch({ type: 'project_reconciled', project: snapshot })
          dispatch({ type: 'scan_received', event })
          const desired = desiredProjectionRef.current
          void refreshProjection(
            snapshot,
            desired.selectedFolderId,
            desired.selectedFolderPath,
            desired.showingAggregate,
          )
        })
        .catch(() => undefined)
        .finally(() => {
          if (reconcilingGenerationRef.current === event.generation) {
            reconcilingGenerationRef.current = null
          }
        })
    },
    [bridge, refreshProjection],
  )

  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined
    void Promise.resolve()
      .then(() => bridge.listenScan(receiveScan))
      .then((cleanup) => {
        if (disposed) cleanup()
        else unlisten = cleanup
      })
      .catch(() => undefined)
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [bridge, receiveScan])

  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined
    void Promise.resolve()
      .then(() =>
        bridge.listenIndexProgress((progress) => {
          dispatch({ type: 'index_progress_received', progress })
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

  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined
    void Promise.resolve()
      .then(() =>
        bridge.listenProjectClosed(() => {
          projectionRequestRef.current += 1
          searchRevisionRef.current += 1
          selectionRequestRef.current += 1
          requestedSnippetsRef.current.clear()
          reconcilingGenerationRef.current = null
          desiredProjectionRef.current = {
            selectedFolderId: null,
            selectedFolderPath: '',
            showingAggregate: false,
          }
          dispatch({ type: 'project_closed' })
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

  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | undefined
    void Promise.resolve()
      .then(() =>
        bridge.listenProjectDrops((paths) => {
          if (!['empty', 'error'].includes(stateRef.current.status)) return
          if (paths.length !== 1) {
            dispatch({ type: 'input_rejected', message: '一次只能导入一个项目文件夹。' })
            return
          }
          void openProject(paths[0])
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
  }, [bridge, openProject])

  const selectFolder = useCallback(
    async (entityId: string | null) => {
      const project = stateRef.current.project
      if (project === null || stateRef.current.status !== 'active') return
      const selected = entityId
        ? stateRef.current.folders.find((folder) => folder.entityId === entityId)
        : undefined
      await refreshProjection(project, entityId, selected?.relativePath ?? '', false)
    },
    [refreshProjection],
  )

  const showAllDescendants = useCallback(async () => {
    const current = stateRef.current
    if (current.project === null || current.status !== 'active') return
    const desired = desiredProjectionRef.current
    await refreshProjection(
      current.project,
      desired.selectedFolderId,
      desired.selectedFolderPath,
      true,
    )
  }, [refreshProjection])

  const cancelTask = useCallback(
    async (taskId: string) => {
      try {
        if (await bridge.cancelTask(taskId)) {
          dispatch({ type: 'scan_cancelled', taskId })
        }
      } catch (error) {
        dispatch({ type: 'input_rejected', message: safeUserMessage(error) })
      }
    },
    [bridge],
  )

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
    async (reviewState: ReviewState | null) => {
      const current = stateRef.current
      if (
        current.project === null ||
        current.project.access === 'read_only' ||
        current.selectedEntityIds.length === 0
      ) {
        return
      }
      try {
        const result = await bridge.setReviewState({
          sessionId: current.project.sessionId,
          generation: current.project.generation,
          entityIds: current.selectedEntityIds,
          reviewState,
        })
        dispatch({
          type: 'marker_changes_applied',
          sessionId: current.project.sessionId,
          generation: current.project.generation,
          changes: result.changes,
        })
        const desired = desiredProjectionRef.current
        await Promise.all([
          refreshSelectionInfo(current.project, current.selectedEntityIds),
          refreshProjection(
            current.project,
            desired.selectedFolderId,
            desired.selectedFolderPath,
            desired.showingAggregate,
          ),
        ])
      } catch (error) {
        dispatch({ type: 'input_rejected', message: safeUserMessage(error) })
      }
    },
    [bridge, refreshProjection, refreshSelectionInfo],
  )

  const toggleFavorite = useCallback(async () => {
    const current = stateRef.current
    if (
      current.project === null ||
      current.project.access === 'read_only' ||
      current.selectedEntityIds.length === 0
    ) {
      return
    }
    try {
      const result = await bridge.toggleFavorite({
        sessionId: current.project.sessionId,
        generation: current.project.generation,
        entityIds: current.selectedEntityIds,
      })
      dispatch({
        type: 'marker_changes_applied',
        sessionId: current.project.sessionId,
        generation: current.project.generation,
        changes: result.changes,
      })
      const desired = desiredProjectionRef.current
      await Promise.all([
        refreshSelectionInfo(current.project, current.selectedEntityIds),
        refreshProjection(
          current.project,
          desired.selectedFolderId,
          desired.selectedFolderPath,
          desired.showingAggregate,
        ),
      ])
    } catch (error) {
      dispatch({ type: 'input_rejected', message: safeUserMessage(error) })
    }
  }, [bridge, refreshProjection, refreshSelectionInfo])

  return {
    state,
    openProject,
    closeProject,
    selectFolder,
    showAllDescendants,
    cancelTask,
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
  }
}
