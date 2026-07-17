import type {
  FileKind,
  FolderTreeItem,
  FolderWorkspace,
  ImageOrientation,
  IndexProgressEvent,
  Marker,
  MarkerChange,
  ProjectSnapshot,
  ReviewState,
  ScanEvent,
  SearchFilters,
  SearchLayout,
  SearchPage,
  SearchQueryModel,
  SearchSort,
  SelectionInfo,
} from '../api/types'

export type ViewerStatus = 'empty' | 'opening' | 'active' | 'closing' | 'error'

export interface ScanState {
  taskId: string
  phase: 'running' | 'finished' | 'cancelled'
  publishedFolders: number
  publishedFiles: number
  failedItems: Array<{ relativePath: string; code: string }>
  totals?: { folders: number; files: number; failed: number }
}

export const emptySearchFilters: SearchFilters = {
  kinds: [],
  reviewStates: [],
  favoriteOnly: false,
  unmarkedOnly: false,
  orientations: [],
  widthMin: null,
  widthMax: null,
  heightMin: null,
  heightMax: null,
  sizeMin: null,
  sizeMax: null,
  modifiedNsMin: null,
  modifiedNsMax: null,
}

export const initialSearchQuery: SearchQueryModel = {
  text: '',
  scopeFolderId: null,
  filters: emptySearchFilters,
  sort: { key: 'natural_name', direction: 'ascending' },
  layout: 'grouped',
}

export interface SearchState {
  focusRequest: number
  showResults: boolean
  query: SearchQueryModel
  queryVersion: number
  schedule: 'debounced' | 'immediate'
  revision: number
  status: 'idle' | 'searching' | 'ready' | 'error'
  page: SearchPage | null
  snippets: Record<string, string | null>
  visibleEntityIds: string[]
  offset: number
}

function initialSearchState(): SearchState {
  return {
    focusRequest: 0,
    showResults: false,
    query: initialSearchQuery,
    queryVersion: 0,
    schedule: 'immediate',
    revision: 0,
    status: 'idle',
    page: null,
    snippets: {},
    visibleEntityIds: [],
    offset: 0,
  }
}

export interface ViewerState {
  status: ViewerStatus
  project: ProjectSnapshot | null
  folders: FolderTreeItem[]
  workspace: FolderWorkspace | null
  selectedFolderId: string | null
  selectedFolderPath: string
  showingAggregate: boolean
  scan: ScanState | null
  indexProgress: IndexProgressEvent | null
  search: SearchState
  selectedEntityIds: string[]
  selectionInfo: SelectionInfo | null
  errorMessage: string | null
}

export const initialViewerState: ViewerState = {
  status: 'empty',
  project: null,
  folders: [],
  workspace: null,
  selectedFolderId: null,
  selectedFolderPath: '',
  showingAggregate: false,
  scan: null,
  indexProgress: null,
  search: initialSearchState(),
  selectedEntityIds: [],
  selectionInfo: null,
  errorMessage: null,
}

export type SearchFilterChip =
  | { kind: 'file_kind'; value: FileKind }
  | { kind: 'review_state'; value: ReviewState }
  | { kind: 'orientation'; value: ImageOrientation }
  | { kind: 'favorite' }
  | { kind: 'unmarked' }
  | {
      kind: 'range'
      field:
        | 'width'
        | 'height'
        | 'size'
        | 'modified_ns'
    }

export type ViewerAction =
  | { type: 'project_open_requested' }
  | { type: 'project_opened'; project: ProjectSnapshot }
  | { type: 'project_reconciled'; project: ProjectSnapshot }
  | { type: 'project_open_failed'; message: string }
  | { type: 'project_close_requested' }
  | { type: 'project_close_failed'; message: string }
  | { type: 'project_closed' }
  | { type: 'input_rejected'; message: string }
  | { type: 'scan_received'; event: ScanEvent }
  | { type: 'index_progress_received'; progress: IndexProgressEvent }
  | { type: 'scan_cancelled'; taskId: string }
  | {
      type: 'projection_loaded'
      sessionId: string
      generation: number
      folders: FolderTreeItem[]
      workspace: FolderWorkspace
      selectedFolderId: string | null
      selectedFolderPath: string
      showingAggregate: boolean
    }
  | {
      type: 'projection_failed'
      sessionId: string
      generation: number
      message: string
    }
  | { type: 'search_focus_requested' }
  | { type: 'search_text_changed'; text: string }
  | { type: 'search_scope_changed'; folderId: string | null }
  | { type: 'search_filters_changed'; filters: SearchFilters }
  | { type: 'search_sort_changed'; sort: SearchSort }
  | { type: 'search_layout_changed'; layout: SearchLayout }
  | { type: 'search_filter_chip_removed'; chip: SearchFilterChip }
  | { type: 'search_filters_cleared' }
  | { type: 'search_page_changed'; offset: number }
  | { type: 'search_context_closed' }
  | { type: 'search_requested'; revision: number }
  | {
      type: 'search_loaded'
      sessionId: string
      generation: number
      page: SearchPage
    }
  | {
      type: 'search_failed'
      sessionId: string
      generation: number
      revision: number
      message: string
    }
  | { type: 'visible_search_hits_changed'; entityIds: string[] }
  | {
      type: 'search_snippet_loaded'
      revision: number
      entityId: string
      snippet: string | null
    }
  | { type: 'selection_changed'; entityIds: string[] }
  | {
      type: 'selection_info_loaded'
      sessionId: string
      generation: number
      entityIds: string[]
      info: SelectionInfo
    }
  | {
      type: 'marker_changes_applied'
      sessionId: string
      generation: number
      changes: MarkerChange[]
    }

export function viewerReducer(state: ViewerState, action: ViewerAction): ViewerState {
  switch (action.type) {
    case 'project_open_requested':
      return freshState('opening')
    case 'project_opened':
      return { ...freshState('active'), project: action.project }
    case 'project_reconciled':
      if (state.project?.sessionId !== action.project.sessionId) return state
      return { ...state, status: 'active', project: action.project }
    case 'project_open_failed':
      return { ...freshState('error'), errorMessage: action.message }
    case 'project_close_requested':
      return state.project ? { ...state, status: 'closing', errorMessage: null } : state
    case 'project_close_failed':
      return state.project
        ? { ...state, status: 'active', errorMessage: action.message }
        : { ...freshState('error'), errorMessage: action.message }
    case 'project_closed':
      return freshState('empty')
    case 'input_rejected':
      return state.project
        ? { ...state, errorMessage: action.message }
        : { ...freshState('error'), errorMessage: action.message }
    case 'scan_received':
      if (!isCurrentEvent(state, action.event)) return state
      return { ...state, scan: reduceScan(state.scan, action.event) }
    case 'index_progress_received':
      if (!isCurrentProjection(state, action.progress.sessionId, action.progress.generation)) {
        return state
      }
      return { ...state, indexProgress: action.progress }
    case 'scan_cancelled':
      if (state.scan?.taskId !== action.taskId) return state
      return { ...state, scan: { ...state.scan, phase: 'cancelled' } }
    case 'projection_loaded':
      if (!isCurrentProjection(state, action.sessionId, action.generation)) return state
      return {
        ...state,
        folders: action.folders,
        workspace: action.workspace,
        selectedFolderId: action.selectedFolderId,
        selectedFolderPath: action.selectedFolderPath,
        showingAggregate: action.showingAggregate,
        errorMessage: null,
      }
    case 'projection_failed':
      if (!isCurrentProjection(state, action.sessionId, action.generation)) return state
      return { ...state, errorMessage: action.message }
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
      return queryChanged(
        state,
        { ...state.search.query, filters: action.filters },
        'immediate',
      )
    case 'search_sort_changed':
      return queryChanged(
        state,
        { ...state.search.query, sort: action.sort },
        'immediate',
      )
    case 'search_layout_changed':
      return queryChanged(
        state,
        { ...state.search.query, layout: action.layout },
        'immediate',
      )
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
    case 'selection_changed':
      return {
        ...state,
        selectedEntityIds: unique(action.entityIds),
        selectionInfo: null,
      }
    case 'selection_info_loaded':
      if (
        !isCurrentProjection(state, action.sessionId, action.generation) ||
        !sameStrings(state.selectedEntityIds, action.entityIds)
      ) {
        return state
      }
      return { ...state, selectionInfo: action.info }
    case 'marker_changes_applied':
      if (!isCurrentProjection(state, action.sessionId, action.generation)) return state
      return applyMarkerChanges(state, action.changes)
  }
}

function freshState(status: ViewerStatus): ViewerState {
  return {
    ...initialViewerState,
    status,
    search: initialSearchState(),
    selectedEntityIds: [],
  }
}

function queryChanged(
  state: ViewerState,
  query: SearchQueryModel,
  schedule: SearchState['schedule'],
): ViewerState {
  return {
    ...state,
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

function applyMarkerChanges(state: ViewerState, changes: MarkerChange[]): ViewerState {
  const markers = new Map(changes.map((change) => [change.entityId, change.marker]))
  const markerFor = (entityId: string, current: Marker) => markers.get(entityId) ?? current
  const folders = state.folders.map((folder) => ({
    ...folder,
    marker: markerFor(folder.entityId, folder.marker),
  }))
  const workspace = state.workspace && updateWorkspaceMarkers(state.workspace, markers)
  const page = state.search.page && {
    ...state.search.page,
    hits: state.search.page.hits.map((hit) => ({
      ...hit,
      marker: markerFor(hit.entityId, hit.marker),
    })),
  }
  return {
    ...state,
    folders,
    workspace,
    search: { ...state.search, page },
  }
}

function updateWorkspaceMarkers(
  workspace: FolderWorkspace,
  markers: Map<string, Marker>,
): FolderWorkspace {
  if (workspace.workspace === 'empty') return workspace
  if (workspace.workspace === 'category') {
    return {
      ...workspace,
      folders: workspace.folders.map((folder) => ({
        ...folder,
        marker: markers.get(folder.entityId) ?? folder.marker,
        representativeImages: folder.representativeImages.map((file) => ({
          ...file,
          marker: markers.get(file.entityId) ?? file.marker,
        })),
      })),
    }
  }
  return {
    ...workspace,
    images: workspace.images.map((file) => ({
      ...file,
      marker: markers.get(file.entityId) ?? file.marker,
    })),
    textFiles: workspace.textFiles.map((file) => ({
      ...file,
      marker: markers.get(file.entityId) ?? file.marker,
    })),
  }
}

function unique(values: string[]): string[] {
  return [...new Set(values)]
}

function sameStrings(left: string[], right: string[]): boolean {
  return left.length === right.length && left.every((value, index) => value === right[index])
}

function isCurrentEvent(state: ViewerState, event: ScanEvent): boolean {
  return (
    state.project !== null &&
    state.project.sessionId === event.sessionId &&
    state.project.generation === event.generation
  )
}

function isCurrentProjection(
  state: ViewerState,
  sessionId: string,
  generation: number,
): boolean {
  return state.project?.sessionId === sessionId && state.project.generation === generation
}

function reduceScan(previous: ScanState | null, event: ScanEvent): ScanState {
  const current =
    previous?.taskId === event.taskId
      ? previous
      : {
          taskId: event.taskId,
          phase: 'running' as const,
          publishedFolders: 0,
          publishedFiles: 0,
          failedItems: [],
        }
  switch (event.type) {
    case 'folders':
      return {
        ...current,
        phase: 'running',
        publishedFolders: current.publishedFolders + event.nodes.length,
      }
    case 'files':
      return {
        ...current,
        phase: 'running',
        publishedFiles: current.publishedFiles + event.nodes.length,
      }
    case 'failed_item':
      return {
        ...current,
        phase: 'running',
        failedItems: [
          ...current.failedItems,
          { relativePath: event.relativePath, code: event.code },
        ],
      }
    case 'finished':
      return { ...current, phase: 'finished', totals: event.totals }
  }
}
