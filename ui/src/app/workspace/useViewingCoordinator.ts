import type { ViewerController } from '../../state/useViewerController'
import type { ViewerState } from '../../state/viewerState'
import type { PreviewDataPort, VideoPlaybackPort } from './ports'
import { type CompareCoordinator, useCompareCoordinator } from './useCompareCoordinator'
import { type PreviewCoordinator, usePreviewCoordinator } from './usePreviewCoordinator'
import { type PreviewDataCoordinator, usePreviewData } from './usePreviewData'

export type ViewingCommands = Pick<ViewerController, 'setPreviewEntityId' | 'setCompareEntityIds'>

export interface ViewingOptions {
  state: ViewerState
  projectSessionId: string
  port: PreviewDataPort
  playbackPort: VideoPlaybackPort
  commands: ViewingCommands
}

export interface ViewingCoordinator
  extends PreviewCoordinator,
    PreviewDataCoordinator,
    CompareCoordinator {
  playbackPort: VideoPlaybackPort
}

export function useViewingCoordinator({
  state,
  projectSessionId,
  port,
  playbackPort,
  commands,
}: ViewingOptions): ViewingCoordinator {
  const preview = usePreviewCoordinator(state, projectSessionId, commands)
  const data = usePreviewData(projectSessionId, port)
  const comparison = useCompareCoordinator({
    state,
    projectSessionId,
    commands,
    closePreview: preview.closePreview,
    openPreview: preview.openPreview,
  })

  return {
    ...preview,
    ...data,
    ...comparison,
    playbackPort,
  }
}
