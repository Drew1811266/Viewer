import { useId, useRef, useState } from 'react'
import type {
  ReviewArchivePlan,
  ReviewArchiveSelection,
  ReviewWorkspaceError,
} from '../../api/reviewWorkspaceTypes'
import type { ProjectAccess, ReviewScopeRequest } from '../../app/review/reviewModel'
import type { ContinuousReviewCoordinator } from '../../app/review/useContinuousReviewCoordinator'
import type { ReviewSessionCoordinator } from '../../app/review/useReviewSessionCoordinator'
import ModalSheet from '../ModalSheet'
import ViewerButton from '../ui/ViewerButton'
import ReviewArchiveDialog from './ReviewArchiveDialog'
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
  onReturnToMembers(entityIds: string[]): void
}

interface ArchiveDialogState {
  preview: ReviewArchivePlan
  selection: ReviewArchiveSelection
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
  onReturnToMembers,
}: ReviewWorkspaceLayerProps) {
  const [inspectorOpen, setInspectorOpen] = useState(false)
  const [abandonOpen, setAbandonOpen] = useState(false)
  const [archiveDialog, setArchiveDialog] = useState<ArchiveDialogState | null>(null)
  const [archiveBusy, setArchiveBusy] = useState(false)
  const [archiveError, setArchiveError] = useState<string | null>(null)
  const [archiveNotice, setArchiveNotice] = useState<string | null>(null)
  const active = review.snapshot.phase === 'active'
  const completed = review.snapshot.phase === 'completed_read_only'
  const members =
    continuousReview === undefined
      ? review.snapshot.members.flatMap((member) =>
          member.entityId === null ? [] : [member.entityId],
        )
      : (continuousReview.view?.current?.state.assets.flatMap((asset) =>
          asset.sourceEntityId === null ? [] : [asset.sourceEntityId],
        ) ?? [])
  const continuousFeedbackCount = continuousReview?.view?.current?.state.feedback.length
  const archiveUnavailable =
    continuousReview === undefined ||
    continuousReview.currentSnapshotId === null ||
    continuousReview.hasUncommittedInput ||
    archiveBusy

  async function openArchive() {
    if (continuousReview === undefined) return
    if (continuousReview.hasUncommittedInput) {
      setArchiveNotice('请先保存或取消正在编辑的意见。')
      return
    }
    const selection = unknownArchiveSelection(continuousReview)
    if (selection === null) {
      setArchiveNotice('当前没有可存档的意见。')
      return
    }
    setArchiveBusy(true)
    setArchiveError(null)
    try {
      const preview = await continuousReview.previewArchive(selection)
      setArchiveDialog({ preview, selection })
    } catch (cause) {
      setArchiveNotice(archiveErrorMessage(cause))
    } finally {
      setArchiveBusy(false)
    }
  }

  async function changeArchiveSelection(selection: ReviewArchiveSelection) {
    if (continuousReview === undefined || archiveDialog === null) return
    setArchiveBusy(true)
    setArchiveError(null)
    try {
      const preview = await continuousReview.previewArchive(selection)
      setArchiveDialog({ preview, selection })
    } catch (cause) {
      setArchiveError(archiveErrorMessage(cause))
    } finally {
      setArchiveBusy(false)
    }
  }

  async function confirmArchive() {
    if (continuousReview === undefined || archiveDialog === null) return
    if (continuousReview.hasUncommittedInput) {
      setArchiveError('请先保存或取消正在编辑的意见。')
      return
    }
    if (continuousReview.currentSnapshotId !== archiveDialog.preview.expectedSnapshotId) {
      setArchiveError('意见已变化，请重新查看存档范围')
      return
    }
    setArchiveBusy(true)
    setArchiveError(null)
    try {
      await continuousReview.commitArchive()
      setArchiveDialog(null)
      setArchiveNotice('意见已移入历史；如有需要可在历史中撤销。')
    } catch (cause) {
      setArchiveError(archiveErrorMessage(cause))
    } finally {
      setArchiveBusy(false)
    }
  }

  return (
    <>
      <ReviewRecoveryNotice review={review} projectAccess={projectAccess} />
      {!contextBarHidden && (continuousReview !== undefined || active || completed) && (
        <ReviewContextBar
          protocol={continuousReview === undefined ? 'legacy' : 'continuous'}
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
          onArchive={continuousReview === undefined ? undefined : () => void openArchive()}
          continuousFeedbackCount={continuousFeedbackCount}
          archiveDisabled={archiveUnavailable}
          archiveNotice={archiveNotice}
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
      {!contextBarHidden && inspectorOpen && (active || completed) && (
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
      {archiveDialog !== null && (
        <ReviewArchiveDialog
          preview={archiveDialog.preview}
          selection={archiveDialog.selection}
          busy={archiveBusy}
          error={archiveError}
          onSelectionChange={(selection) => void changeArchiveSelection(selection)}
          onConfirm={() => void confirmArchive()}
          onCancel={() => {
            if (!archiveBusy) {
              setArchiveDialog(null)
              setArchiveError(null)
            }
          }}
        />
      )}
      {abandonOpen && <ReviewAbandonDialog review={review} onClose={() => setAbandonOpen(false)} />}
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

function unknownArchiveSelection(
  coordinator: ContinuousReviewCoordinator,
): ReviewArchiveSelection | null {
  const current = coordinator.view?.current
  if (current === undefined || current === null) return null
  const targets = current.state.feedback.flatMap((feedback) =>
    feedback.targets.map((target) => ({
      feedbackId: feedback.id,
      textRevisionId: feedback.textRevisionId,
      targetId: target.id,
      targetRevisionId: target.revisionId,
    })),
  )
  if (targets.length === 0) return null
  return {
    expectedSnapshotId: current.reference.snapshotId,
    groups: [{ basis: { kind: 'unknown' }, targets }],
  }
}

function archiveErrorMessage(cause: unknown) {
  if (isReviewWorkspaceError(cause)) return cause.message
  return '无法预览或存档意见，请重试。'
}

function isReviewWorkspaceError(cause: unknown): cause is ReviewWorkspaceError {
  return typeof cause === 'object' && cause !== null && 'message' in cause
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
