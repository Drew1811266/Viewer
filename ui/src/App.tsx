import type { MutableRefObject } from 'react'
import { useCallback, useMemo, useRef } from 'react'
import type { ViewerBridge } from './api/viewer'
import { tauriViewerBridge } from './api/viewer'
import type { WorkspaceIntentSink } from './app/workspace/intents'
import { createWorkspacePorts } from './app/workspace/ports'
import { useFeedbackCoordinator } from './app/workspace/useFeedbackCoordinator'
import { useOrganizationCoordinator } from './app/workspace/useOrganizationCoordinator'
import { useViewingCoordinator } from './app/workspace/useViewingCoordinator'
import { useWorkspaceShellCoordinator } from './app/workspace/useWorkspaceShellCoordinator'
import WorkspaceProjectView from './app/workspace/WorkspaceProjectView'
import EmptyProject from './components/EmptyProject'
import type { Point } from './components/imagePreview/imageGeometry'
import { useLatestPointerClientPoint } from './components/imagePreview/useLatestPointerClientPoint'
import { useViewerSettings, ViewerSettingsProvider } from './settings/ViewerSettingsProvider'
import { useViewerController } from './state/useViewerController'

interface AppProps {
  bridge?: ViewerBridge
}

export default function App({ bridge = tauriViewerBridge }: AppProps) {
  const pointerClientPoint = useLatestPointerClientPoint()
  return (
    <ViewerSettingsProvider bridge={bridge}>
      <ViewerWorkspace bridge={bridge} pointerClientPoint={pointerClientPoint} />
    </ViewerSettingsProvider>
  )
}

function ViewerWorkspace({
  bridge,
  pointerClientPoint,
}: {
  bridge: ViewerBridge
  pointerClientPoint: MutableRefObject<Point | null>
}) {
  const settings = useViewerSettings()
  const controller = useViewerController(bridge)
  const { state } = controller
  const projectSessionId = state.project?.sessionId ?? 'no-session'
  const intentTargetRef = useRef<WorkspaceIntentSink>(() => undefined)
  const emitIntent = useCallback<WorkspaceIntentSink>((intent) => {
    intentTargetRef.current(intent)
  }, [])
  const ports = useMemo(() => createWorkspacePorts(bridge), [bridge])
  const shell = useWorkspaceShellCoordinator({
    projectSessionId,
    videoProjectSessionId: state.project?.sessionId ?? null,
    port: ports.shell,
  })
  const viewing = useViewingCoordinator({
    state,
    projectSessionId,
    port: ports.preview,
    playbackPort: ports.playback,
    commands: {
      setPreviewEntityId: controller.setPreviewEntityId,
      setCompareEntityIds: controller.setCompareEntityIds,
    },
  })
  const organization = useOrganizationCoordinator({
    state,
    commands: {
      setSelectedEntityIds: controller.setSelectedEntityIds,
      setReviewState: controller.setReviewState,
      toggleFavorite: controller.toggleFavorite,
      previewRename: controller.previewRename,
      preflightFileCommand: controller.preflightFileCommand,
      executeFileCommand: controller.executeFileCommand,
      cancelOperation: controller.cancelOperation,
      loadOperationResults: controller.loadOperationResults,
      undoLastOperation: controller.undoLastOperation,
      consumeContextRepair: controller.consumeContextRepair,
      beginFinderDrag: controller.beginFinderDrag,
    },
    emitIntent,
    compareOpen: viewing.compareOpen,
    activePreviewOpen: viewing.activePreview !== null,
    infoOpen: shell.infoOpen,
  })
  const feedback = useFeedbackCoordinator({
    projectSessionId,
    scan: state.scan,
    operation: state.operation,
    projectError: state.errorMessage,
    finderDragMessage: organization.finderDragMessage,
    workspaceActionError: shell.workspaceActionError,
  })

  intentTargetRef.current = (intent) => {
    switch (intent.kind) {
      case 'open-preview':
        viewing.openIntentPreview(intent)
        return
      case 'enter-compare':
        viewing.enterCompare(intent.files, organization.operationBusy)
        return
      case 'start-rename':
        organization.openRenameDialog(intent.files)
        return
      case 'show-operation-results':
        organization.showResults(intent.batchId)
        return
      case 'open-settings':
        shell.openSettings()
        return
      case 'open-info':
        if (shell.infoOpen) shell.closeInfo()
        else shell.openInfo()
        return
      case 'close-project':
        void controller.closeProject()
    }
  }

  if (state.project === null) {
    return (
      <EmptyProject
        bridge={ports.emptyProject}
        busy={state.status === 'opening'}
        errorMessage={state.errorMessage}
        fatalError={state.status === 'error'}
        onOpenProject={controller.openProject}
      />
    )
  }

  return (
    <WorkspaceProjectView
      state={state}
      project={state.project}
      projectSessionId={projectSessionId}
      commands={controller}
      ports={ports}
      settings={settings}
      shell={shell}
      viewing={viewing}
      organization={organization}
      feedback={feedback}
      emitIntent={emitIntent}
      pointerClientPoint={pointerClientPoint}
    />
  )
}
