import type { ComponentProps, MutableRefObject } from 'react'
import { useCallback, useMemo, useRef } from 'react'
import type { ReviewWorkspacePort } from './api/reviewWorkspaceTypes'
import type { ViewerBridge } from './api/viewer'
import { tauriReviewWorkspaceBridge, tauriViewerBridge } from './api/viewer'
import { deriveReviewScope } from './app/review/reviewModel'
import { useReviewSessionCoordinator } from './app/review/useReviewSessionCoordinator'
import { useReviewWorkspaceActivation } from './app/review/useReviewWorkspaceActivation'
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
import ReviewWorkspaceLayer, { ReviewToolbarAction } from './components/review/ReviewWorkspaceLayer'
import { useViewerSettings, ViewerSettingsProvider } from './settings/ViewerSettingsProvider'
import { useViewerController } from './state/useViewerController'

interface AppProps {
  bridge?: ViewerBridge
  reviewWorkspacePort?: ReviewWorkspacePort
}

export default function App({ bridge = tauriViewerBridge, reviewWorkspacePort }: AppProps) {
  const pointerClientPoint = useLatestPointerClientPoint()
  const activeReviewWorkspacePort =
    reviewWorkspacePort ?? (bridge === tauriViewerBridge ? tauriReviewWorkspaceBridge : null)
  return (
    <ViewerSettingsProvider bridge={bridge}>
      <ViewerWorkspace
        bridge={bridge}
        reviewWorkspacePort={activeReviewWorkspacePort}
        pointerClientPoint={pointerClientPoint}
      />
    </ViewerSettingsProvider>
  )
}

function ViewerWorkspace({
  bridge,
  reviewWorkspacePort,
  pointerClientPoint,
}: {
  bridge: ViewerBridge
  reviewWorkspacePort: ReviewWorkspacePort | null
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
  const reviewSelectedEntityIds = state.search.showResults
    ? state.selectedEntityIds
    : organization.selectedFiles.map((file) => file.entityId)
  const review = useReviewSessionCoordinator({
    port: ports.review,
    sessionId: state.project?.sessionId ?? 'no-session',
    generation: state.project?.generation ?? 0,
    selectedEntityIds: reviewSelectedEntityIds,
    enabled: state.project !== null,
  })
  const continuousReview = useReviewWorkspaceActivation({
    port: reviewWorkspacePort,
    sessionId: state.project?.sessionId,
    generation: state.project?.generation,
  })
  const reviewScope = deriveReviewScope(
    state.search.showResults
      ? { kind: 'search', selectedEntityIds: reviewSelectedEntityIds }
      : {
          kind: 'folder',
          folderId: state.projectionTransition?.selectedFolderId ?? state.selectedFolderId,
          includeDescendants:
            state.projectionTransition?.showingAggregate ?? state.showingAggregate,
          selectedEntityIds: reviewSelectedEntityIds,
        },
  )
  const feedbackCountByEntityId = useMemo(() => {
    const counts = new Map<string, number>()
    for (const feedbackItem of review.snapshot.feedback) {
      const targetEntityIds = new Set(
        feedbackItem.targets.flatMap((target) =>
          target.entityId === null ? [] : [target.entityId],
        ),
      )
      for (const entityId of targetEntityIds) {
        counts.set(entityId, (counts.get(entityId) ?? 0) + 1)
      }
    }
    return counts
  }, [review.snapshot.feedback])
  const returnToReviewMembers = useCallback(
    async (entityIds: string[]) => {
      controller.returnToFolderContext()
      await controller.selectFolder(null)
      await controller.showAllDescendants()
      controller.setSelectedEntityIds(entityIds)
    },
    [
      controller.returnToFolderContext,
      controller.selectFolder,
      controller.setSelectedEntityIds,
      controller.showAllDescendants,
    ],
  )
  const reselectProject = useCallback(() => {
    review.requestDiscard('context_replacement', () => void controller.reselectProject())
  }, [controller.reselectProject, review.requestDiscard])
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
        review.requestDiscard('project_close', () => void controller.closeProject())
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
      review={{
        coordinator: review,
        protocol: continuousReview.protocol,
        continuousCoordinator: continuousReview.presentation,
        capturedScope: reviewScope,
        feedbackCountByEntityId,
        onReturnToMembers: (entityIds) => void returnToReviewMembers(entityIds),
      }}
      reviewToolbarAction={
        <ActivatedReviewToolbar
          enabled={continuousReview.enabled}
          review={review}
          scope={reviewScope}
          projectAccess={state.project.access}
        />
      }
      reviewLayer={
        <ReviewWorkspaceLayer
          review={review}
          selectedEntityIds={reviewSelectedEntityIds}
          projectAccess={state.project.access}
          contextBarHidden={viewing.activePreview !== null}
          continuousReview={continuousReview.coordinator}
          onReturnToMembers={(entityIds) => void returnToReviewMembers(entityIds)}
        />
      }
      onReselectProject={reselectProject}
    />
  )
}

function ActivatedReviewToolbar({
  enabled,
  ...props
}: ComponentProps<typeof ReviewToolbarAction> & { enabled: boolean }) {
  if (enabled) return null
  return <ReviewToolbarAction {...props} />
}
