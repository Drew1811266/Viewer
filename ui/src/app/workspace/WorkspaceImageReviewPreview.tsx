import type { MutableRefObject } from 'react'
import { useCallback, useLayoutEffect, useMemo, useState } from 'react'
import type { BrowserFile, ReviewScopeRequest, ReviewSessionSnapshot } from '../../api/types'
import ImagePreview from '../../components/ImagePreview'
import type { Point } from '../../components/imagePreview/imageGeometry'
import ModalSheet from '../../components/ModalSheet'
import ImageReviewWorkspace from '../../components/review/ImageReviewWorkspace'
import { ReviewAbandonDialog } from '../../components/review/ReviewWorkspaceLayer'
import ViewerButton from '../../components/ui/ViewerButton'
import { isPreviewableImage } from '../../fileKinds'
import type { ImageRendererPort } from '../../rendering/imageRendererTypes'
import type { ViewerSettingsContextValue } from '../../settings/ViewerSettingsProvider'
import type { ViewerState } from '../../state/viewerState'
import {
  continuousImageReviewWorkbenchAdapter,
  legacyImageReviewWorkbenchAdapter,
} from '../review/imageReviewWorkbenchAdapter'
import type { ContinuousReviewCoordinator } from '../review/useContinuousReviewCoordinator'
import {
  type ImageReviewWorkbenchController,
  useImageReviewWorkbench,
} from '../review/useImageReviewWorkbench'
import type { ReviewSessionCoordinator } from '../review/useReviewSessionCoordinator'
import type { WorkspaceIntentSink } from './intents'
import type { ViewingCoordinator } from './useViewingCoordinator'

type ProjectAccess = NonNullable<ViewerState['project']>['access']

export interface WorkspaceReviewPresentation {
  coordinator: ReviewSessionCoordinator
  protocol?: 'legacy' | 'continuous'
  continuousCoordinator?: ContinuousReviewCoordinator
  capturedScope: ReviewScopeRequest | null
  feedbackCountByEntityId: ReadonlyMap<string, number>
  onReturnToMembers(entityIds: string[]): void
}

export type ImageReviewRoute = 'workbench' | 'outside_scope' | 'ordinary'

export function resolveImageReviewRoute({
  protocol = 'legacy',
  snapshot,
  file,
  capturedScope,
  projectAccess,
}: {
  protocol?: 'legacy' | 'continuous'
  snapshot: ReviewSessionSnapshot
  file: BrowserFile
  capturedScope: ReviewScopeRequest | null
  projectAccess: ProjectAccess
}): ImageReviewRoute {
  if (!isPreviewableImage(file) || projectAccess === 'read_only') {
    return 'ordinary'
  }
  if (protocol === 'continuous') return 'workbench'
  if (capturedScope === null) return 'ordinary'
  if (snapshot.reviewRoundId === null) {
    return snapshot.phase === 'idle' && snapshot.resume === null ? 'workbench' : 'ordinary'
  }
  return snapshot.members.some((member) => member.entityId === file.entityId)
    ? 'workbench'
    : 'outside_scope'
}

export interface WorkspaceImageReviewPreviewProps {
  file: BrowserFile
  files: BrowserFile[]
  magnifier: ViewerSettingsContextValue['magnifier']
  pointerClientPoint: MutableRefObject<Point | null>
  unavailableEntityIds: ReadonlySet<string>
  requestImage: ViewingCoordinator['requestPreviewImage']
  onNavigate(file: BrowserFile): void
  onClose(): void
  onDimensions(entityId: string, width: number, height: number): void
  projectAccess: ProjectAccess
  review: WorkspaceReviewPresentation
  emitIntent: WorkspaceIntentSink
  activeControllerRef: MutableRefObject<ImageReviewWorkbenchController | null>
  imageRenderer: ImageRendererPort
}

export default function WorkspaceImageReviewPreview(props: WorkspaceImageReviewPreviewProps) {
  const continuous = props.review.continuousCoordinator
  const protocol = props.review.protocol ?? (continuous === undefined ? 'legacy' : 'continuous')
  const route = resolveImageReviewRoute({
    protocol,
    snapshot: props.review.coordinator.snapshot,
    file: props.file,
    capturedScope: props.review.capturedScope,
    projectAccess: props.projectAccess,
  })

  if (protocol === 'continuous' && continuous === undefined)
    return <OrdinaryImagePreview {...props} />
  if (route === 'workbench' && (continuous !== undefined || props.review.capturedScope !== null)) {
    return (
      <ImageReviewOverlay {...props} scope={props.review.capturedScope} continuous={continuous} />
    )
  }
  if (route === 'outside_scope') return <OutsideScopeImagePreview {...props} />
  return <OrdinaryImagePreview {...props} />
}

function OrdinaryImagePreview(props: WorkspaceImageReviewPreviewProps) {
  return (
    <ImagePreview
      file={props.file}
      files={props.files}
      magnifier={props.magnifier}
      pointerClientPoint={props.pointerClientPoint}
      unavailableEntityIds={props.unavailableEntityIds}
      requestImage={props.requestImage}
      onNavigate={props.onNavigate}
      onClose={props.onClose}
      onDimensions={props.onDimensions}
      renderer={props.imageRenderer}
    />
  )
}

function ImageReviewOverlay({
  file,
  files,
  magnifier,
  pointerClientPoint,
  unavailableEntityIds,
  requestImage,
  onNavigate,
  onClose,
  onDimensions,
  review,
  emitIntent,
  activeControllerRef,
  scope,
  continuous,
  imageRenderer,
}: WorkspaceImageReviewPreviewProps & {
  scope: ReviewScopeRequest | null
  continuous: ContinuousReviewCoordinator | undefined
}) {
  const [abandonOpen, setAbandonOpen] = useState(false)
  const adapter = useMemo(
    () =>
      continuous === undefined
        ? legacyImageReviewWorkbenchAdapter(review.coordinator, definedScope(scope))
        : continuousImageReviewWorkbenchAdapter(continuous),
    [continuous, review.coordinator, scope],
  )
  const leave = useCallback(
    (intent: Parameters<ImageReviewWorkbenchController['requestLeave']>[0]) => {
      switch (intent.kind) {
        case 'navigate': {
          const currentIndex = files.findIndex((candidate) => candidate.entityId === file.entityId)
          const next = files[currentIndex + intent.offset]
          if (next !== undefined) onNavigate(next)
          return
        }
        case 'return_grid':
          onClose()
          return
        case 'close_project':
          emitIntent({ kind: 'close-project' })
          return
        case 'finish_review':
          review.coordinator.requestDiscard('context_replacement', () => {
            void review.coordinator.prepareCompletion()
          })
          return
        case 'abandon_review':
          review.coordinator.requestDiscard('context_replacement', () => setAbandonOpen(true))
      }
    },
    [emitIntent, file.entityId, files, onClose, onNavigate, review.coordinator],
  )
  const controller = useImageReviewWorkbench({
    adapter,
    entityId: file.entityId,
    onLeave: leave,
  })

  useLayoutEffect(() => {
    activeControllerRef.current = controller
    return () => {
      if (activeControllerRef.current === controller) activeControllerRef.current = null
    }
  }, [activeControllerRef, controller])

  return (
    <>
      <ImageReviewWorkspace
        file={file}
        files={files}
        magnifier={magnifier}
        pointerClientPoint={pointerClientPoint}
        unavailableEntityIds={unavailableEntityIds}
        requestImage={requestImage}
        onDimensions={onDimensions}
        controller={controller}
        renderer={imageRenderer}
      />
      {controller.leaveConfirmation !== null && (
        <ModalSheet
          title="放弃未保存的标注？"
          destructive
          onCancel={controller.cancelLeave}
          footer={
            <>
              <ViewerButton onClick={controller.cancelLeave}>继续编辑</ViewerButton>
              <ViewerButton
                tone="danger"
                onClick={() => void controller.discardUnsavedAndProceed()}
              >
                放弃未保存内容
              </ViewerButton>
            </>
          }
        >
          <p>当前标注或意见尚未保存，离开后无法恢复。</p>
        </ModalSheet>
      )}
      {abandonOpen && (
        <ReviewAbandonDialog review={review.coordinator} onClose={() => setAbandonOpen(false)} />
      )}
    </>
  )
}

function definedScope(scope: ReviewScopeRequest | null): ReviewScopeRequest {
  if (scope === null) throw new Error('Legacy review workbench requires a fixed scope')
  return scope
}

function OutsideScopeImagePreview(props: WorkspaceImageReviewPreviewProps) {
  const [abandonOpen, setAbandonOpen] = useState(false)
  const memberIds = props.review.coordinator.snapshot.members.flatMap((member) =>
    member.entityId === null ? [] : [member.entityId],
  )
  return (
    <>
      <OrdinaryImagePreview {...props} />
      <section
        className="review-local-notice review-outside-scope-notice"
        data-tone="warning"
        role="status"
      >
        <div>
          <strong>这张图片不在当前评审范围内</strong>
          <span>本轮素材范围已经固定；普通预览不会把这张图片加入本轮。</span>
        </div>
        <div className="review-outside-scope-notice__actions">
          <ViewerButton onClick={() => props.review.onReturnToMembers(memberIds)}>
            返回本轮素材
          </ViewerButton>
          <ViewerButton
            tone="primary"
            onClick={() =>
              props.review.coordinator.requestDiscard('context_replacement', () => {
                void props.review.coordinator.prepareCompletion()
              })
            }
          >
            完成当前评审
          </ViewerButton>
          <ViewerButton
            tone="quiet"
            onClick={() =>
              props.review.coordinator.requestDiscard('context_replacement', () =>
                setAbandonOpen(true),
              )
            }
          >
            放弃当前草稿后重新开始
          </ViewerButton>
        </div>
      </section>
      {abandonOpen && (
        <ReviewAbandonDialog
          review={props.review.coordinator}
          onClose={() => setAbandonOpen(false)}
        />
      )}
    </>
  )
}
