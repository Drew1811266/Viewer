import { useId, useRef, useState } from 'react'
import type { ReviewArchiveSelection, ReviewHistorySelector } from '../../api/reviewWorkspaceTypes'
import type { ProjectAccess, ReviewScopeRequest } from '../../app/review/reviewModel'
import type { ContinuousReviewCoordinator } from '../../app/review/useContinuousReviewCoordinator'
import type { ReviewSessionCoordinator } from '../../app/review/useReviewSessionCoordinator'
import ModalSheet from '../ModalSheet'
import ViewerButton from '../ui/ViewerButton'
import ContinuousReviewWorkspaceLayer from './ContinuousReviewWorkspaceLayer'
import ReviewCompletionDialog from './ReviewCompletionDialog'
import ReviewContextBar from './ReviewContextBar'
import ReviewInspector from './ReviewInspector'
import ReviewRecoveryNotice from './ReviewRecoveryNotice'
import ReviewStartDialog from './ReviewStartDialog'

interface ReviewWorkspaceLayerProps {
  review: ReviewSessionCoordinator
  selectedEntityIds: string[]
  projectAccess: ProjectAccess
  contextBarHidden?: boolean
  /** Internal opt-in only. Task 21 selects the active protocol at the application boundary. */
  continuousReview?: ContinuousReviewCoordinator
  /** Optional verified or user-selected B basis. It is never inferred from an Agent claim. */
  archiveSelection?: ReviewArchiveSelection
  /** Internal typed selector only; the production entry remains inactive until Task 21. */
  historySelector?: ReviewHistorySelector
  onReturnToMembers(entityIds: string[]): void
}

interface ReviewToolbarActionProps {
  review: ReviewSessionCoordinator
  scope: ReviewScopeRequest | null
  projectAccess: ProjectAccess
}

interface ReviewAbandonDialogProps {
  review: ReviewSessionCoordinator
  onClose(): void
}

export function ReviewToolbarAction({ review, scope, projectAccess }: ReviewToolbarActionProps) {
  const explanationId = useId()
  const disabledReason = startDisabledReason(review, scope, projectAccess)
  return (
    <>
      <ViewerButton
        tone="primary"
        disabled={disabledReason !== null}
        aria-describedby={disabledReason === null ? undefined : explanationId}
        title={disabledReason ?? undefined}
        onClick={() => {
          if (scope !== null) void review.previewStart(scope)
        }}
      >
        开始评审
      </ViewerButton>
      {disabledReason !== null && (
        <span id={explanationId} className="visually-hidden">
          {disabledReason}
        </span>
      )}
    </>
  )
}

export default function ReviewWorkspaceLayer({
  review,
  selectedEntityIds,
  projectAccess,
  contextBarHidden = false,
  continuousReview,
  archiveSelection,
  historySelector,
  onReturnToMembers,
}: ReviewWorkspaceLayerProps) {
  if (continuousReview !== undefined) {
    return (
      <ContinuousReviewWorkspaceLayer
        review={review}
        coordinator={continuousReview}
        selectedEntityIds={selectedEntityIds}
        projectAccess={projectAccess}
        contextBarHidden={contextBarHidden}
        archiveSelection={archiveSelection}
        historySelector={historySelector}
        onReturnToMembers={onReturnToMembers}
      />
    )
  }
  return (
    <LegacyReviewWorkspaceLayer
      review={review}
      selectedEntityIds={selectedEntityIds}
      projectAccess={projectAccess}
      contextBarHidden={contextBarHidden}
      onReturnToMembers={onReturnToMembers}
    />
  )
}

function LegacyReviewWorkspaceLayer({
  review,
  selectedEntityIds,
  projectAccess,
  contextBarHidden = false,
  onReturnToMembers,
}: Omit<ReviewWorkspaceLayerProps, 'continuousReview' | 'archiveSelection' | 'historySelector'>) {
  const [inspectorOpen, setInspectorOpen] = useState(false)
  const [abandonOpen, setAbandonOpen] = useState(false)
  const active = review.snapshot.phase === 'active'
  const completed = review.snapshot.phase === 'completed_read_only'

  return (
    <>
      <ReviewRecoveryNotice review={review} projectAccess={projectAccess} />
      <ReviewWorkspaceContext
        review={review}
        contextBarHidden={contextBarHidden}
        inspectorOpen={inspectorOpen}
        completed={completed}
        onReturnToMembers={onReturnToMembers}
        onToggleInspector={() => {
          if (inspectorOpen) review.requestDiscard('inspector_close', () => setInspectorOpen(false))
          else {
            if (!completed) review.beginCreate()
            setInspectorOpen(true)
          }
        }}
        onAbandon={() => setAbandonOpen(true)}
      />
      <ReviewProgressNotice review={review} />
      <ReviewWorkspaceInspector
        review={review}
        selectedEntityIds={selectedEntityIds}
        contextBarHidden={contextBarHidden}
        inspectorOpen={inspectorOpen}
        active={active}
        completed={completed}
        onClose={() => setInspectorOpen(false)}
      />
      <ReviewWorkspaceDialogs
        review={review}
        abandonOpen={abandonOpen}
        onCloseAbandon={() => setAbandonOpen(false)}
      />
    </>
  )
}

function ReviewWorkspaceContext({
  review,
  contextBarHidden,
  inspectorOpen,
  completed,
  onReturnToMembers,
  onToggleInspector,
  onAbandon,
}: {
  review: ReviewSessionCoordinator
  contextBarHidden: boolean
  inspectorOpen: boolean
  completed: boolean
  onReturnToMembers(entityIds: string[]): void
  onToggleInspector(): void
  onAbandon(): void
}) {
  const active = review.snapshot.phase === 'active'
  if (contextBarHidden || (!active && !completed)) return null
  const members = review.snapshot.members.flatMap((member) =>
    member.entityId === null ? [] : [member.entityId],
  )
  return (
    <ReviewContextBar
      snapshot={review.snapshot}
      inspectorOpen={inspectorOpen}
      onReturnToMembers={() => onReturnToMembers(members)}
      onToggleInspector={onToggleInspector}
      onPrepareCompletion={() =>
        review.requestDiscard('context_replacement', () => void review.prepareCompletion())
      }
      onRequestAbandon={onAbandon}
    />
  )
}

function ReviewProgressNotice({ review }: { review: ReviewSessionCoordinator }) {
  if (review.progress === null) return null
  return (
    <div className="review-progress" role="status" aria-live="polite">
      <span>
        正在准备评审 {review.progress.completed}/{review.progress.total}
      </span>
      {review.progress.cancellable && (
        <ViewerButton tone="quiet" onClick={() => void review.cancelTask()}>
          取消
        </ViewerButton>
      )}
    </div>
  )
}

function ReviewWorkspaceInspector({
  review,
  selectedEntityIds,
  contextBarHidden,
  inspectorOpen,
  active,
  completed,
  onClose,
}: {
  review: ReviewSessionCoordinator
  selectedEntityIds: string[]
  contextBarHidden: boolean
  inspectorOpen: boolean
  active: boolean
  completed: boolean
  onClose(): void
}) {
  if (contextBarHidden || !inspectorOpen || (!active && !completed)) return null
  return (
    <ReviewInspector
      review={review}
      selectedEntityIds={selectedEntityIds}
      readOnly={completed}
      onClose={onClose}
    />
  )
}

function ReviewWorkspaceDialogs({
  review,
  abandonOpen,
  onCloseAbandon,
}: {
  review: ReviewSessionCoordinator
  abandonOpen: boolean
  onCloseAbandon(): void
}) {
  return (
    <>
      {review.proposal !== null && (
        <ReviewStartDialog
          proposal={review.proposal}
          busy={review.progress?.taskKind === 'start'}
          onConfirm={() => void review.confirmStart()}
          onCancel={review.dismissStart}
        />
      )}
      {review.completion !== null && (
        <ReviewCompletionDialog
          proposal={review.completion}
          busy={review.snapshot.phase === 'completing'}
          onConfirm={() => void review.confirmCompletion()}
          onCancel={review.dismissCompletion}
        />
      )}
      {abandonOpen && <ReviewAbandonDialog review={review} onClose={onCloseAbandon} />}
      {review.discardConfirmation !== null && <ReviewDiscardDialog review={review} />}
    </>
  )
}

function ReviewDiscardDialog({ review }: { review: ReviewSessionCoordinator }) {
  return (
    <ModalSheet
      title="放弃未保存的意见？"
      destructive
      onCancel={review.cancelDiscard}
      footer={
        <>
          <ViewerButton onClick={review.cancelDiscard}>继续编辑</ViewerButton>
          <ViewerButton tone="danger" onClick={review.confirmDiscard}>
            放弃未保存内容
          </ViewerButton>
        </>
      }
    >
      <p>当前自然语言意见尚未保存，离开后无法恢复。</p>
    </ModalSheet>
  )
}

export function ReviewAbandonDialog({ review, onClose }: ReviewAbandonDialogProps) {
  const abandonCancelRef = useRef<HTMLButtonElement>(null)
  return (
    <ModalSheet
      title="放弃本轮评审？"
      destructive
      onCancel={onClose}
      initialFocusRef={abandonCancelRef}
      footer={
        <>
          <ViewerButton ref={abandonCancelRef} onClick={onClose}>
            保留本轮
          </ViewerButton>
          <ViewerButton
            tone="danger"
            onClick={() => {
              void review.abandon()
              onClose()
            }}
          >
            确认放弃
          </ViewerButton>
        </>
      }
    >
      <p>放弃只删除未完成草稿，不会产生完成记录。删除成功前仍会保留本轮。</p>
    </ModalSheet>
  )
}

function startDisabledReason(
  review: ReviewSessionCoordinator,
  scope: ReviewScopeRequest | null,
  projectAccess: ProjectAccess,
): string | null {
  if (projectAccess === 'read_only') return '当前项目为只读，不能开始新的评审。'
  if (review.snapshot.resume !== null) return '请先继续未完成的评审。'
  if (scope === null) return '请先选择一个或多个搜索结果。'
  if (
    review.snapshot.phase === 'active' ||
    review.snapshot.phase === 'preparing' ||
    review.snapshot.phase === 'completing'
  ) {
    return '当前已有进行中的评审。'
  }
  if (review.snapshot.phase === 'write_unavailable') return '评审暂时不可写，请稍后重试。'
  if (review.snapshot.phase === 'recovery_required') return '评审记录需要恢复处理。'
  return null
}
