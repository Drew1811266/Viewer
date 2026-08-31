import { useId, useLayoutEffect, useRef, useState } from 'react'
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
  /** Optional verified or user-selected B basis. It is never inferred from an Agent claim. */
  archiveSelection?: ReviewArchiveSelection
  onReturnToMembers(entityIds: string[]): void
}

interface ArchiveDialogState {
  sessionKey: string | null
  preview: ReviewArchivePlan
  selection: ReviewArchiveSelection
}

type ContinuousArchivePreview = ReturnType<typeof useContinuousArchivePreview>

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
  onReturnToMembers,
}: ReviewWorkspaceLayerProps) {
  const [inspectorOpen, setInspectorOpen] = useState(false)
  const [abandonOpen, setAbandonOpen] = useState(false)
  const archive = useContinuousArchivePreview(continuousReview, archiveSelection)
  const active = review.snapshot.phase === 'active'
  const completed = review.snapshot.phase === 'completed_read_only'

  return (
    <>
      <ReviewRecoveryNotice review={review} projectAccess={projectAccess} />
      <ReviewWorkspaceContext
        review={review}
        continuousReview={continuousReview}
        archive={archive}
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
        archive={archive}
        abandonOpen={abandonOpen}
        onCloseAbandon={() => setAbandonOpen(false)}
      />
    </>
  )
}

function ReviewWorkspaceContext({
  review,
  continuousReview,
  archive,
  contextBarHidden,
  inspectorOpen,
  completed,
  onReturnToMembers,
  onToggleInspector,
  onAbandon,
}: {
  review: ReviewSessionCoordinator
  continuousReview: ContinuousReviewCoordinator | undefined
  archive: ContinuousArchivePreview
  contextBarHidden: boolean
  inspectorOpen: boolean
  completed: boolean
  onReturnToMembers(entityIds: string[]): void
  onToggleInspector(): void
  onAbandon(): void
}) {
  const active = review.snapshot.phase === 'active'
  if (contextBarHidden || (continuousReview === undefined && !active && !completed)) return null
  const members =
    continuousReview === undefined
      ? review.snapshot.members.flatMap((member) =>
          member.entityId === null ? [] : [member.entityId],
        )
      : (continuousReview.view?.current?.state.assets.flatMap((asset) =>
          asset.sourceEntityId === null ? [] : [asset.sourceEntityId],
        ) ?? [])
  const archiveUnavailable =
    continuousReview === undefined ||
    continuousReview.currentSnapshotId === null ||
    continuousReview.hasUncommittedInput ||
    archive.busy
  return (
    <ReviewContextBar
      protocol={continuousReview === undefined ? 'legacy' : 'continuous'}
      snapshot={review.snapshot}
      inspectorOpen={inspectorOpen}
      onReturnToMembers={() => onReturnToMembers(members)}
      onToggleInspector={onToggleInspector}
      onPrepareCompletion={() =>
        review.requestDiscard('context_replacement', () => void review.prepareCompletion())
      }
      onRequestAbandon={onAbandon}
      onArchive={continuousReview === undefined ? undefined : () => void archive.open()}
      continuousFeedbackCount={continuousReview?.view?.current?.state.feedback.length}
      archiveDisabled={archiveUnavailable}
      archiveNotice={archive.notice}
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
  archive,
  abandonOpen,
  onCloseAbandon,
}: {
  review: ReviewSessionCoordinator
  archive: ContinuousArchivePreview
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
      {archive.dialog !== null && (
        <ReviewArchiveDialog
          preview={archive.dialog.preview}
          selection={archive.dialog.selection}
          busy={archive.busy}
          error={archive.error}
          onSelectionChange={(selection) => void archive.changeSelection(selection)}
          onConfirm={() => void archive.confirm()}
          onCancel={archive.cancel}
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

function useContinuousArchivePreview(
  coordinator: ContinuousReviewCoordinator | undefined,
  suppliedSelection: ReviewArchiveSelection | undefined,
) {
  const [dialog, setDialog] = useState<ArchiveDialogState | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const sessionKey = coordinator?.workbenchSessionKey ?? null
  const sessionKeyRef = useRef(sessionKey)

  useLayoutEffect(() => {
    if (sessionKeyRef.current === sessionKey) return
    sessionKeyRef.current = sessionKey
    setDialog(null)
    setBusy(false)
    setError(null)
    setNotice(null)
  }, [sessionKey])

  function active(requestSessionKey: string | null) {
    return sessionKeyRef.current === requestSessionKey
  }

  async function preview(
    selection: ReviewArchiveSelection,
    requestSessionKey: string | null,
    reportError: (message: string) => void,
  ) {
    if (coordinator === undefined) return
    setBusy(true)
    setError(null)
    try {
      const plan = await coordinator.previewArchive(selection)
      if (active(requestSessionKey))
        setDialog({ sessionKey: requestSessionKey, preview: plan, selection })
    } catch (cause) {
      if (active(requestSessionKey)) reportError(archiveErrorMessage(cause))
    } finally {
      if (active(requestSessionKey)) setBusy(false)
    }
  }

  async function open() {
    if (coordinator === undefined) return
    if (coordinator.hasUncommittedInput) {
      setNotice('请先保存或取消正在编辑的意见。')
      return
    }
    const selection =
      suppliedSelection === undefined
        ? unknownArchiveSelection(coordinator)
        : structuredClone(suppliedSelection)
    if (!hasArchiveTargets(selection)) {
      setNotice('当前没有可存档的意见。')
      return
    }
    if (selection.expectedSnapshotId !== coordinator.currentSnapshotId) {
      setNotice('意见已变化，请重新查看存档范围')
      return
    }
    await preview(selection, sessionKey, setNotice)
  }

  async function changeSelection(selection: ReviewArchiveSelection) {
    if (dialog === null || dialog.sessionKey !== sessionKey) return
    if (!hasArchiveTargets(selection)) {
      setDialog({
        ...dialog,
        preview: emptyArchivePlan(dialog.preview),
        selection,
      })
      setError(null)
      return
    }
    await preview(selection, sessionKey, setError)
  }

  async function confirm() {
    if (coordinator === undefined || dialog === null || dialog.sessionKey !== sessionKey) return
    if (coordinator.hasUncommittedInput) {
      setError('请先保存或取消正在编辑的意见。')
      return
    }
    if (coordinator.currentSnapshotId !== dialog.preview.expectedSnapshotId) {
      setError('意见已变化，请重新查看存档范围')
      return
    }
    const requestSessionKey = sessionKey
    setBusy(true)
    setError(null)
    try {
      await coordinator.commitArchive()
      if (active(requestSessionKey)) {
        setDialog(null)
        setNotice('意见已移入历史；如有需要可在历史中撤销。')
      }
    } catch (cause) {
      if (active(requestSessionKey)) setError(archiveErrorMessage(cause))
    } finally {
      if (active(requestSessionKey)) setBusy(false)
    }
  }

  function cancel() {
    if (!busy) {
      setDialog(null)
      setError(null)
    }
  }

  return {
    dialog: dialog?.sessionKey === sessionKey ? dialog : null,
    busy,
    error,
    notice,
    open,
    changeSelection,
    confirm,
    cancel,
  }
}

function hasArchiveTargets(
  selection: ReviewArchiveSelection | null,
): selection is ReviewArchiveSelection {
  return selection?.groups.some((group) => group.targets.length > 0) ?? false
}

function emptyArchivePlan(preview: ReviewArchivePlan): ReviewArchivePlan {
  return {
    expectedSnapshotId: preview.expectedSnapshotId,
    groups: structuredClone(preview.groups),
    removed: [],
    retained: [],
    alreadyCovered: [],
  }
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
