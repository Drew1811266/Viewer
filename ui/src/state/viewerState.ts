import type {
  CloseBlockedEvent,
  FileCommandKind,
  FileKind,
  FolderTreeItem,
  FolderWorkspace,
  ImageOrientation,
  IndexProgressEvent,
  MarkerChange,
  OperationProgressEvent,
  OperationResultPage,
  ProjectChangedEvent,
  ProjectSnapshot,
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

export interface ProjectionTransition {
  selectedFolderId: string | null
  selectedFolderPath: string
  showingAggregate: boolean
}

export function initialSearchState(): SearchState {
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
  projectionTransition: ProjectionTransition | null
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
  finishing: boolean
  pending: boolean
}

export interface ContextRepair {
  removedEntityIds: string[]
  suggestedEntityId: string | null
  message: string
}

export function initialOperationState(): OperationState {
  return { kind: null, active: null, results: null, finishing: false, pending: false }
}

export const initialViewerState: ViewerState = {
  status: 'empty',
  project: null,
  folders: [],
  workspace: null,
  projectionTransition: null,
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

export function freshViewerState(status: ViewerStatus): ViewerState {
  return {
    ...initialViewerState,
    status,
    search: initialSearchState(),
    operation: initialOperationState(),
    selectedEntityIds: [],
    compareEntityIds: [],
  }
}

export type SearchFilterChip =
  | { kind: 'file_kind'; value: FileKind }
  | { kind: 'review_state'; value: ReviewState }
  | { kind: 'orientation'; value: ImageOrientation }
  | { kind: 'favorite' }
  | { kind: 'unmarked' }
  | {
      kind: 'range'
      field: 'width' | 'height' | 'size' | 'modified_ns'
    }

export type ViewerAction =
  | { type: 'project_open_requested' }
  | { type: 'project_opened'; project: ProjectSnapshot }
  | { type: 'project_reconciled'; project: ProjectSnapshot }
  | { type: 'project_open_failed'; message: string }
  | { type: 'project_close_requested' }
  | { type: 'project_close_stayed' }
  | { type: 'project_close_failed'; message: string }
  | { type: 'project_closed'; message?: string }
  | { type: 'input_rejected'; message: string }
  | { type: 'scan_received'; event: ScanEvent }
  | { type: 'index_progress_received'; progress: IndexProgressEvent }
  | { type: 'scan_cancelled'; taskId: string }
  | {
      type: 'projection_requested'
      sessionId: string
      generation: number
      selectedFolderId: string | null
      selectedFolderPath: string
      showingAggregate: boolean
    }
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
  | { type: 'context_repair_consumed' }
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
  | { type: 'operation_request_pending'; pending: boolean }
  | {
      type: 'operation_finish_settled'
      sessionId: string
      generation: number
      batchId: string
    }
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
