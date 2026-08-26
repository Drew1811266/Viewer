import { useId, useRef, useState } from 'react'
import type { ProjectAccess, ReviewScopeRequest } from '../../app/review/reviewModel'
import type { ReviewSessionCoordinator } from '../../app/review/useReviewSessionCoordinator'
import ModalSheet from '../ModalSheet'
import ViewerButton from '../ui/ViewerButton'
import ReviewCompletionDialog from './ReviewCompletionDialog'
import ReviewContextBar from './ReviewContextBar'
import ReviewInspector from './ReviewInspector'
import ReviewRecoveryNotice from './ReviewRecoveryNotice'
import ReviewStartDialog from './ReviewStartDialog'

interface ReviewWorkspaceLayerProps {
  review: ReviewSessionCoordinator
  selectedEntityIds: string[]
  projectAccess: ProjectAccess
  onReturnToMembers(entityIds: string[]): void
}

interface ReviewToolbarActionProps {
  review: ReviewSessionCoordinator
  scope: ReviewScopeRequest | null
  projectAccess: ProjectAccess
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
  onReturnToMembers,
}: ReviewWorkspaceLayerProps) {
  const [inspectorOpen, setInspectorOpen] = useState(false)
  const [abandonOpen, setAbandonOpen] = useState(false)
  const abandonCancelRef = useRef<HTMLButtonElement>(null)
  const active = review.snapshot.phase === 'active'
  const completed = review.snapshot.phase === 'completed_read_only'
  const members = review.snapshot.members.flatMap((member) =>
    member.entityId === null ? [] : [member.entityId],
  )

  return (
    <>
      <ReviewRecoveryNotice review={review} projectAccess={projectAccess} />
      {(active || completed) && (
        <ReviewContextBar
          snapshot={review.snapshot}
          inspectorOpen={inspectorOpen}
          onReturnToMembers={() => onReturnToMembers(members)}
          onToggleInspector={() => {
            if (inspectorOpen) {
              review.requestDiscard('inspector_close', () => setInspectorOpen(false))
            } else {
              if (!completed) review.beginCreate()
              setInspectorOpen(true)
            }
          }}
          onPrepareCompletion={() =>
            review.requestDiscard('context_replacement', () => void review.prepareCompletion())
          }
          onRequestAbandon={() => setAbandonOpen(true)}
        />
      )}
      {review.progress !== null && (
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
      )}
      {inspectorOpen && (active || completed) && (
        <ReviewInspector
          review={review}
          selectedEntityIds={selectedEntityIds}
          readOnly={completed}
          onClose={() => setInspectorOpen(false)}
        />
      )}
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
      {abandonOpen && (
        <ModalSheet
          title="放弃本轮评审？"
          destructive
          onCancel={() => setAbandonOpen(false)}
          initialFocusRef={abandonCancelRef}
          footer={
            <>
              <ViewerButton ref={abandonCancelRef} onClick={() => setAbandonOpen(false)}>
                保留本轮
              </ViewerButton>
              <ViewerButton
                tone="danger"
                onClick={() => {
                  void review.abandon()
                  setAbandonOpen(false)
                }}
              >
                确认放弃
              </ViewerButton>
            </>
          }
        >
          <p>放弃只删除未完成草稿，不会产生完成记录。删除成功前仍会保留本轮。</p>
        </ModalSheet>
      )}
      {review.discardConfirmation !== null && (
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
      )}
    </>
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
