import type {
  FileKind,
  FolderTreeItem,
  FolderWorkspace,
  ImageOrientation,
  IndexProgressEvent,
  Marker,
  MarkerChange,
  OperationProgressEvent,
  OperationResultPage,
  FileCommandKind,
  CloseBlockedEvent,
  ProjectSnapshot,
  ProjectChangedEvent,
  RecoveryReport,
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
  operation: OperationState
  recoveryReport: RecoveryReport | null
  pendingProjectChange: ProjectChangedEvent | null
  previewEntityId: string | null
  compareEntityIds: string[]
  contextRepair: ContextRepair | null
  closeBlocked: CloseBlockedEvent | null
  errorMessage: string | null
}

export interface OperationState {
  kind: FileCommandKind | null
  active: OperationProgressEvent | null
  results: OperationResultPage | null
}

export interface ContextRepair {
  removedEntityIds: string[]
  suggestedEntityId: string | null
  message: string
}

function initialOperationState(): OperationState {
  return { kind: null, active: null, results: null }
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
  operation: initialOperationState(),
  recoveryReport: null,
  pendingProjectChange: null,
  previewEntityId: null,
  compareEntityIds: [],
  contextRepair: null,
  closeBlocked: null,
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
  | { type: 'preview_context_changed'; entityId: string | null }
  | { type: 'compare_context_changed'; entityIds: string[] }
  | {
      type: 'operation_started'
      sessionId: string
      generation: number
      batchId: string
      kind: FileCommandKind
    }
  | { type: 'operation_progress_received'; progress: OperationProgressEvent }
  | {
      type: 'operation_results_loaded'
      sessionId: string
      generation: number
      batchId: string
      page: OperationResultPage
    }
  | {
      type: 'operation_failed'
      sessionId: string
      generation: number
      message: string
    }
  | { type: 'search_refresh_requested' }
  | { type: 'project_changed_received'; change: ProjectChangedEvent }
  | { type: 'close_blocked_received'; event: CloseBlockedEvent }
  | { type: 'close_blocked_cleared' }
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
      return {
        ...freshState('active'),
        project: action.project,
        recoveryReport: action.project.recoveryReport ?? null,
      }
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
      return repairContextAfterProjection(state, {
        ...state,
        folders: action.folders,
        workspace: action.workspace,
        selectedFolderId: action.selectedFolderId,
        selectedFolderPath: action.selectedFolderPath,
        showingAggregate: action.showingAggregate,
        errorMessage: null,
      })
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
    case 'preview_context_changed':
      return { ...state, previewEntityId: action.entityId }
    case 'compare_context_changed':
      return { ...state, compareEntityIds: unique(action.entityIds).slice(0, 4) }
    case 'operation_started':
      if (!isCurrentProjection(state, action.sessionId, action.generation)) return state
      return {
        ...state,
        operation: {
          kind: action.kind,
          active: {
            sessionId: action.sessionId,
            generation: action.generation,
            batchId: action.batchId,
            lifecycle: 'queued',
            requested: 0,
            completed: 0,
            failed: 0,
            skipped: 0,
            cancelled: 0,
            activeEntityId: null,
          },
          results: null,
        },
        errorMessage: null,
      }
    case 'operation_progress_received':
      if (
        !isCurrentProjection(
          state,
          action.progress.sessionId,
          action.progress.generation,
        ) ||
        state.operation.active?.batchId !== action.progress.batchId
      ) {
        return state
      }
      return {
        ...state,
        operation: { ...state.operation, active: action.progress },
      }
    case 'operation_results_loaded':
      if (
        !isCurrentProjection(state, action.sessionId, action.generation) ||
        state.operation.active?.batchId !== action.batchId
      ) {
        return state
      }
      return {
        ...state,
        operation: { ...state.operation, results: action.page },
      }
    case 'operation_failed':
      if (!isCurrentProjection(state, action.sessionId, action.generation)) return state
      return { ...state, errorMessage: action.message }
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
    case 'project_changed_received':
      if (!isCurrentProjection(state, action.change.sessionId, action.change.generation)) {
        return state
      }
      return {
        ...state,
        pendingProjectChange: action.change,
        search: state.search.showResults
          ? {
              ...state.search,
              queryVersion: state.search.queryVersion + 1,
              schedule: 'immediate',
            }
          : state.search,
      }
    case 'close_blocked_received':
      if (!isCurrentProjection(state, action.event.sessionId, action.event.generation)) {
        return state
      }
      return { ...state, status: 'active', closeBlocked: action.event }
    case 'close_blocked_cleared':
      return { ...state, closeBlocked: null }
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
    operation: initialOperationState(),
    selectedEntityIds: [],
    compareEntityIds: [],
  }
}

function repairContextAfterProjection(
  previous: ViewerState,
  next: ViewerState,
): ViewerState {
  if (previous.pendingProjectChange === null) return next
  const previousOrder = orderedWorkspaceEntityIds(previous.workspace)
  const nextOrder = orderedWorkspaceEntityIds(next.workspace)
  const previouslyKnown = new Set([
    ...previous.folders.map((folder) => folder.entityId),
    ...previousOrder,
  ])
  const currentlyLive = new Set([
    ...next.folders.map((folder) => folder.entityId),
    ...nextOrder,
  ])
  const active = unique([
    ...previous.selectedEntityIds,
    ...(previous.previewEntityId ? [previous.previewEntityId] : []),
    ...previous.compareEntityIds,
  ])
  const removedEntityIds = active.filter(
    (entityId) => previouslyKnown.has(entityId) && !currentlyLive.has(entityId),
  )
  if (removedEntityIds.length === 0) {
    return {
      ...next,
      pendingProjectChange: null,
      contextRepair: null,
    }
  }
  const removed = new Set(removedEntityIds)
  const firstRemovedIndex = previousOrder.findIndex((entityId) => removed.has(entityId))
  const suggestedEntityId =
    firstRemovedIndex < 0 || nextOrder.length === 0
      ? null
      : (nextOrder[Math.min(firstRemovedIndex, nextOrder.length - 1)] ?? null)
  return {
    ...next,
    pendingProjectChange: null,
    selectedEntityIds: previous.selectedEntityIds.filter((id) => !removed.has(id)),
    selectionInfo: null,
    previewEntityId:
      previous.previewEntityId && removed.has(previous.previewEntityId)
        ? null
        : previous.previewEntityId,
    compareEntityIds: previous.compareEntityIds.filter((id) => !removed.has(id)),
    contextRepair: {
      removedEntityIds,
      suggestedEntityId,
      message: '部分正在查看的文件已在项目外发生变化。',
    },
  }
}

function orderedWorkspaceEntityIds(workspace: FolderWorkspace | null): string[] {
  if (workspace === null || workspace.workspace === 'empty') return []
  if (workspace.workspace === 'category') {
    return workspace.folders.map((folder) => folder.entityId)
  }
  return [...workspace.images, ...workspace.textFiles].map((file) => file.entityId)
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
