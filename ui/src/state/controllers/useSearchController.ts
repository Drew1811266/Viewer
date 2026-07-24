import { useCallback, useEffect, useRef } from 'react'
import type {
  ProjectSnapshot,
  SearchFilters,
  SearchLayout,
  SearchQueryModel,
  SearchSort,
} from '../../api/types'
import { safeUserMessage } from '../../api/viewer'
import type { SearchFilterChip } from '../viewerReducer'
import type { ControllerCore } from './types'

export interface SearchController {
  setSearchText(text: string): void
  setSearchScope(folderId: string | null): void
  setSearchFilters(filters: SearchFilters): void
  setSearchSort(sort: SearchSort): void
  setSearchLayout(layout: SearchLayout): void
  removeSearchFilter(chip: SearchFilterChip): void
  clearSearchFilters(): void
  setVisibleSearchHits(entityIds: string[]): void
  setSearchPage(offset: number): void
  returnToFolderContext(): void
}

export function useSearchController(core: ControllerCore, sessionEpoch: number): SearchController {
  const { bridge, dispatch, sessionEpochRef, state, stateRef } = core
  const searchRevisionRef = useRef(-1)
  const requestedSnippetsRef = useRef(new Set<string>())
  const renderedSessionEpochRef = useRef(sessionEpoch)

  useEffect(() => {
    if (renderedSessionEpochRef.current === sessionEpoch) return
    renderedSessionEpochRef.current = sessionEpoch
    searchRevisionRef.current += 1
    requestedSnippetsRef.current.clear()
  }, [sessionEpoch])

  const executeSearch = useCallback(
    async (
      project: ProjectSnapshot,
      query: SearchQueryModel,
      offset: number,
      requestSessionEpoch: number,
    ) => {
      if (requestSessionEpoch !== sessionEpochRef.current) return
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
        if (requestSessionEpoch !== sessionEpochRef.current) return
        dispatch({
          type: 'search_loaded',
          sessionId: project.sessionId,
          generation: project.generation,
          page,
        })
      } catch (error) {
        if (requestSessionEpoch !== sessionEpochRef.current) return
        dispatch({
          type: 'search_failed',
          sessionId: project.sessionId,
          generation: project.generation,
          revision,
          message: safeUserMessage(error),
        })
      }
    },
    [bridge, dispatch, sessionEpochRef],
  )

  useEffect(() => {
    const project = state.project
    if (project === null || state.status !== 'active' || state.search.queryVersion === 0) {
      return
    }
    const delay = state.search.schedule === 'debounced' ? 120 : 0
    const query = state.search.query
    const offset = state.search.offset
    const requestSessionEpoch = sessionEpochRef.current
    const timer = window.setTimeout(
      () => void executeSearch(project, query, offset, requestSessionEpoch),
      delay,
    )
    return () => window.clearTimeout(timer)
  }, [
    executeSearch,
    sessionEpochRef,
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
      const requestSessionEpoch = sessionEpochRef.current
      void bridge
        .searchTextSnippet({
          sessionId: project.sessionId,
          generation: project.generation,
          revision: page.revision,
          entityId: hit.entityId,
          query: state.search.query.text,
        })
        .then((result) => {
          if (requestSessionEpoch !== sessionEpochRef.current) return
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
    dispatch,
    sessionEpochRef,
    state.project,
    state.search.page,
    state.search.query.text,
    state.search.revision,
    state.search.visibleEntityIds,
  ])

  useEffect(() => {
    function requestSearchFocus(event: KeyboardEvent) {
      if (!(event.metaKey && event.key.toLowerCase() === 'f')) return
      if (stateRef.current.project === null) return
      event.preventDefault()
      dispatch({ type: 'search_focus_requested' })
    }
    window.addEventListener('keydown', requestSearchFocus)
    return () => window.removeEventListener('keydown', requestSearchFocus)
  }, [dispatch, stateRef])

  const setSearchText = useCallback(
    (text: string) => {
      dispatch({ type: 'search_text_changed', text })
    },
    [dispatch],
  )

  const setSearchScope = useCallback(
    (folderId: string | null) => {
      dispatch({ type: 'search_scope_changed', folderId })
    },
    [dispatch],
  )

  const setSearchFilters = useCallback(
    (filters: SearchFilters) => {
      dispatch({ type: 'search_filters_changed', filters })
    },
    [dispatch],
  )

  const setSearchSort = useCallback(
    (sort: SearchSort) => {
      dispatch({ type: 'search_sort_changed', sort })
    },
    [dispatch],
  )

  const setSearchLayout = useCallback(
    (layout: SearchLayout) => {
      dispatch({ type: 'search_layout_changed', layout })
    },
    [dispatch],
  )

  const removeSearchFilter = useCallback(
    (chip: SearchFilterChip) => {
      dispatch({ type: 'search_filter_chip_removed', chip })
    },
    [dispatch],
  )

  const clearSearchFilters = useCallback(() => {
    dispatch({ type: 'search_filters_cleared' })
  }, [dispatch])

  const setVisibleSearchHits = useCallback(
    (entityIds: string[]) => {
      dispatch({ type: 'visible_search_hits_changed', entityIds })
    },
    [dispatch],
  )

  const setSearchPage = useCallback(
    (offset: number) => {
      dispatch({ type: 'search_page_changed', offset })
    },
    [dispatch],
  )

  const returnToFolderContext = useCallback(() => {
    searchRevisionRef.current += 1
    requestedSnippetsRef.current.clear()
    dispatch({ type: 'search_context_closed' })
  }, [dispatch])

  return {
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
  }
}
