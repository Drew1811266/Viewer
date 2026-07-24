import type { SearchFilters, SearchQueryModel } from '../../api/types'
import {
  emptySearchFilters,
  type SearchFilterChip,
  type SearchState,
  type ViewerAction,
  type ViewerState,
} from '../viewerState'
import { isCurrentProjection, unique } from './shared'

export function reduceSearchAction(
  state: ViewerState,
  action: ViewerAction,
): ViewerState | undefined {
  switch (action.type) {
    case 'search_focus_requested':
      return { ...state, search: { ...state.search, focusRequest: state.search.focusRequest + 1 } }
    case 'search_text_changed':
      return queryChanged(state, { ...state.search.query, text: action.text }, 'debounced')
    case 'search_scope_changed':
      return queryChanged(
        state,
        { ...state.search.query, scopeFolderId: action.folderId },
        'immediate',
      )
    case 'search_filters_changed':
      return queryChanged(state, { ...state.search.query, filters: action.filters }, 'immediate')
    case 'search_sort_changed':
      return queryChanged(state, { ...state.search.query, sort: action.sort }, 'immediate')
    case 'search_layout_changed':
      return queryChanged(state, { ...state.search.query, layout: action.layout }, 'immediate')
    case 'search_filter_chip_removed':
      return queryChanged(
        state,
        {
          ...state.search.query,
          filters: removeFilterChip(state.search.query.filters, action.chip),
        },
        'immediate',
      )
    case 'search_filters_cleared':
      return queryChanged(
        state,
        { ...state.search.query, filters: emptySearchFilters },
        'immediate',
      )
    case 'search_page_changed':
      return {
        ...state,
        selectedEntityIds: [],
        selectionInfo: null,
        search: {
          ...state.search,
          showResults: true,
          offset: Math.max(0, action.offset),
          queryVersion: state.search.queryVersion + 1,
          schedule: 'immediate',
        },
      }
    case 'search_context_closed':
      return {
        ...state,
        search: {
          ...state.search,
          showResults: false,
          status: 'idle',
          page: null,
          snippets: {},
          visibleEntityIds: [],
        },
      }
    case 'search_requested':
      return {
        ...state,
        selectedEntityIds: [],
        selectionInfo: null,
        search: {
          ...state.search,
          showResults: true,
          revision: action.revision,
          status: 'searching',
          page: null,
          snippets: {},
          visibleEntityIds: [],
        },
      }
    case 'search_loaded':
      if (
        !isCurrentProjection(state, action.sessionId, action.generation) ||
        action.page.revision !== state.search.revision
      ) {
        return state
      }
      return {
        ...state,
        search: { ...state.search, status: 'ready', page: action.page },
      }
    case 'search_failed':
      if (
        !isCurrentProjection(state, action.sessionId, action.generation) ||
        action.revision !== state.search.revision
      ) {
        return state
      }
      return {
        ...state,
        search: { ...state.search, status: 'error' },
        errorMessage: action.message,
      }
    case 'visible_search_hits_changed':
      return {
        ...state,
        search: { ...state.search, visibleEntityIds: unique(action.entityIds) },
      }
    case 'search_snippet_loaded':
      if (action.revision !== state.search.revision) return state
      return {
        ...state,
        search: {
          ...state.search,
          snippets: { ...state.search.snippets, [action.entityId]: action.snippet },
        },
      }
    case 'search_refresh_requested':
      if (!state.search.showResults) return state
      return {
        ...state,
        search: {
          ...state.search,
          queryVersion: state.search.queryVersion + 1,
          schedule: 'immediate',
        },
      }
    default:
      return undefined
  }
}

function queryChanged(
  state: ViewerState,
  query: SearchQueryModel,
  schedule: SearchState['schedule'],
): ViewerState {
  return {
    ...state,
    selectedEntityIds: [],
    selectionInfo: null,
    search: {
      ...state.search,
      showResults: true,
      query,
      offset: 0,
      queryVersion: state.search.queryVersion + 1,
      schedule,
    },
  }
}

function removeFilterChip(filters: SearchFilters, chip: SearchFilterChip): SearchFilters {
  switch (chip.kind) {
    case 'file_kind':
      return { ...filters, kinds: filters.kinds.filter((value) => value !== chip.value) }
    case 'review_state':
      return {
        ...filters,
        reviewStates: filters.reviewStates.filter((value) => value !== chip.value),
      }
    case 'orientation':
      return {
        ...filters,
        orientations: filters.orientations.filter((value) => value !== chip.value),
      }
    case 'favorite':
      return { ...filters, favoriteOnly: false }
    case 'unmarked':
      return { ...filters, unmarkedOnly: false }
    case 'range':
      if (chip.field === 'width') return { ...filters, widthMin: null, widthMax: null }
      if (chip.field === 'height') return { ...filters, heightMin: null, heightMax: null }
      if (chip.field === 'size') return { ...filters, sizeMin: null, sizeMax: null }
      return { ...filters, modifiedNsMin: null, modifiedNsMax: null }
  }
}
