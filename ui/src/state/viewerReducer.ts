import { reduceOperationAction } from './reducers/operationReducer'
import { reduceProjectAction } from './reducers/projectReducer'
import { reduceSearchAction } from './reducers/searchReducer'
import { reduceWorkspaceAction } from './reducers/workspaceReducer'
import type { ViewerAction, ViewerState } from './viewerState'

export {
  type ContextRepair,
  emptySearchFilters,
  freshViewerState,
  initialOperationState,
  initialSearchQuery,
  initialSearchState,
  initialViewerState,
  type OperationState,
  type ScanState,
  type SearchFilterChip,
  type SearchState,
  type ViewerAction,
  type ViewerState,
  type ViewerStatus,
} from './viewerState'

export function viewerReducer(state: ViewerState, action: ViewerAction): ViewerState {
  return (
    reduceProjectAction(state, action) ??
    reduceWorkspaceAction(state, action) ??
    reduceSearchAction(state, action) ??
    reduceOperationAction(state, action) ??
    state
  )
}
