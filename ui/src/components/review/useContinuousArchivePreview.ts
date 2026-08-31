import { useLayoutEffect, useRef, useState } from 'react'
import type {
  ReviewArchivePlan,
  ReviewArchiveSelection,
  ReviewWorkspaceError,
} from '../../api/reviewWorkspaceTypes'
import type { ContinuousReviewCoordinator } from '../../app/review/useContinuousReviewCoordinator'
import { availableArchiveGroups } from './ReviewArchiveDialog'

interface ArchiveDialogState {
  sessionKey: string | null
  preview: ReviewArchivePlan
  selection: ReviewArchiveSelection
}

export function useContinuousArchivePreview(
  coordinator: ContinuousReviewCoordinator,
  suppliedSelection: ReviewArchiveSelection | undefined,
) {
  const [dialog, setDialog] = useState<ArchiveDialogState | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const sessionKey = coordinator.workbenchSessionKey
  const sessionKeyRef = useRef(sessionKey)

  useLayoutEffect(() => {
    if (sessionKeyRef.current === sessionKey) return
    sessionKeyRef.current = sessionKey
    setDialog(null)
    setBusy(false)
    setError(null)
    setNotice(null)
  }, [sessionKey])

  function active(requestSessionKey: string) {
    return sessionKeyRef.current === requestSessionKey
  }

  async function preview(
    selection: ReviewArchiveSelection,
    requestSessionKey: string,
    reportError: (message: string) => void,
  ) {
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
        preview: emptyArchivePlan(dialog.preview, dialog.selection),
        selection,
      })
      setError(null)
      return
    }
    await preview(selection, sessionKey, setError)
  }

  async function confirm() {
    if (dialog === null || dialog.sessionKey !== sessionKey) return
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

function emptyArchivePlan(
  preview: ReviewArchivePlan,
  priorSelection: ReviewArchiveSelection,
): ReviewArchivePlan {
  return {
    expectedSnapshotId: preview.expectedSnapshotId,
    groups: structuredClone(availableArchiveGroups(preview, priorSelection)),
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
