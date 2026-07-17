import type {
  FolderTreeItem,
  FolderWorkspace,
  ProjectSnapshot,
  ScanEvent,
} from '../api/types'

export type ViewerStatus = 'empty' | 'opening' | 'active' | 'closing' | 'error'

export interface ScanState {
  taskId: string
  phase: 'running' | 'finished'
  publishedFolders: number
  publishedFiles: number
  failedItems: Array<{ relativePath: string; code: string }>
  totals?: { folders: number; files: number; failed: number }
}

export interface ViewerState {
  status: ViewerStatus
  project: ProjectSnapshot | null
  folders: FolderTreeItem[]
  workspace: FolderWorkspace | null
  scan: ScanState | null
  errorMessage: string | null
}

export const initialViewerState: ViewerState = {
  status: 'empty',
  project: null,
  folders: [],
  workspace: null,
  scan: null,
  errorMessage: null,
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
  | {
      type: 'projection_loaded'
      sessionId: string
      generation: number
      folders: FolderTreeItem[]
      workspace: FolderWorkspace
    }
  | {
      type: 'projection_failed'
      sessionId: string
      generation: number
      message: string
    }

export function viewerReducer(state: ViewerState, action: ViewerAction): ViewerState {
  switch (action.type) {
    case 'project_open_requested':
      return { ...initialViewerState, status: 'opening' }
    case 'project_opened':
      return {
        ...initialViewerState,
        status: 'active',
        project: action.project,
      }
    case 'project_reconciled':
      if (state.project?.sessionId !== action.project.sessionId) return state
      return { ...state, status: 'active', project: action.project }
    case 'project_open_failed':
      return { ...initialViewerState, status: 'error', errorMessage: action.message }
    case 'project_close_requested':
      return state.project ? { ...state, status: 'closing', errorMessage: null } : state
    case 'project_close_failed':
      return state.project
        ? { ...state, status: 'active', errorMessage: action.message }
        : { ...initialViewerState, status: 'error', errorMessage: action.message }
    case 'project_closed':
      return initialViewerState
    case 'input_rejected':
      return state.project
        ? { ...state, errorMessage: action.message }
        : { ...initialViewerState, status: 'error', errorMessage: action.message }
    case 'scan_received':
      if (!isCurrentEvent(state, action.event)) return state
      return { ...state, scan: reduceScan(state.scan, action.event) }
    case 'projection_loaded':
      if (!isCurrentProjection(state, action.sessionId, action.generation)) return state
      return {
        ...state,
        folders: action.folders,
        workspace: action.workspace,
        errorMessage: null,
      }
    case 'projection_failed':
      if (!isCurrentProjection(state, action.sessionId, action.generation)) return state
      return { ...state, errorMessage: action.message }
  }
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
  return (
    state.project?.sessionId === sessionId && state.project.generation === generation
  )
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
