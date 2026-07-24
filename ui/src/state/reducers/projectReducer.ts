import { freshViewerState, type ViewerAction, type ViewerState } from '../viewerState'
import { isCurrentProjection } from './shared'

export function reduceProjectAction(
  state: ViewerState,
  action: ViewerAction,
): ViewerState | undefined {
  switch (action.type) {
    case 'project_open_requested':
      return freshViewerState('opening')
    case 'project_opened':
      return {
        ...freshViewerState('active'),
        project: action.project,
        recoveryReport: action.project.recoveryReport ?? null,
      }
    case 'project_reconciled':
      if (state.project?.sessionId !== action.project.sessionId) return state
      return { ...state, status: 'active', project: action.project }
    case 'project_open_failed':
      return { ...freshViewerState('error'), errorMessage: action.message }
    case 'project_close_requested':
      return state.project ? { ...state, status: 'closing', errorMessage: null } : state
    case 'project_close_stayed':
      return state.project ? { ...state, status: 'active' } : state
    case 'project_close_failed':
      return state.project
        ? { ...state, status: 'active', errorMessage: action.message }
        : { ...freshViewerState('error'), errorMessage: action.message }
    case 'project_closed':
      return { ...freshViewerState('empty'), errorMessage: action.message ?? null }
    case 'input_rejected':
      return state.project
        ? { ...state, errorMessage: action.message }
        : { ...freshViewerState('error'), errorMessage: action.message }
    case 'close_blocked_received':
      if (!isCurrentProjection(state, action.event.sessionId, action.event.generation)) {
        return state
      }
      return { ...state, status: 'active', closeBlocked: action.event }
    case 'close_blocked_cleared':
      return { ...state, closeBlocked: null }
    default:
      return undefined
  }
}
