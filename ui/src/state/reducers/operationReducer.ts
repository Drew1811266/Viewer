import type { ViewerAction, ViewerState } from '../viewerState'
import { isCurrentProjection } from './shared'

export function reduceOperationAction(
  state: ViewerState,
  action: ViewerAction,
): ViewerState | undefined {
  switch (action.type) {
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
          finishing: false,
          pending: false,
        },
        errorMessage: null,
      }
    case 'operation_progress_received':
      if (
        !isCurrentProjection(state, action.progress.sessionId, action.progress.generation) ||
        state.operation.active?.batchId !== action.progress.batchId
      ) {
        return state
      }
      if (
        state.operation.active.lifecycle === 'completed' &&
        action.progress.lifecycle !== 'completed'
      ) {
        return state
      }
      return {
        ...state,
        operation: {
          ...state.operation,
          active: action.progress,
          finishing:
            action.progress.lifecycle === 'completed' &&
            state.operation.active?.lifecycle !== 'completed'
              ? true
              : state.operation.finishing,
        },
      }
    case 'operation_request_pending':
      return {
        ...state,
        operation: { ...state.operation, pending: action.pending },
      }
    case 'operation_finish_settled':
      if (
        !isCurrentProjection(state, action.sessionId, action.generation) ||
        state.operation.active?.batchId !== action.batchId
      ) {
        return state
      }
      return {
        ...state,
        operation: { ...state.operation, finishing: false },
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
    default:
      return undefined
  }
}
