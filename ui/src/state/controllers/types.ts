import type { Dispatch, MutableRefObject } from 'react'
import type { ProjectSnapshot } from '../../api/types'
import type { ViewerBridge } from '../../api/viewer'
import type { ViewerAction, ViewerState } from '../viewerState'

export interface ControllerCore {
  bridge: ViewerBridge
  state: ViewerState
  stateRef: MutableRefObject<ViewerState>
  sessionEpochRef: MutableRefObject<number>
  dispatch: Dispatch<ViewerAction>
}

export type RefreshProjection = (
  project: ProjectSnapshot,
  selectedFolderId: string | null,
  selectedFolderPath: string,
  showingAggregate: boolean,
  repairMissingFolder?: boolean,
) => Promise<void>
