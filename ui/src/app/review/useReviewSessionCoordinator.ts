import { useEffect, useRef, useState } from 'react'
import type {
  ReviewCompletionProposal,
  ReviewEditorState,
  ReviewProgress,
  ReviewScopeProposal,
  ReviewScopeRequest,
  ReviewSessionSnapshot,
} from '../../api/types'
import type { ReviewPort } from '../workspace/ports'
import {
  eligibleReviewTargetIds,
  emptyReviewEditor,
  hasUnsavedReviewText,
  idleReviewSnapshot,
} from './reviewModel'

export type ReviewDiscardReason =
  | 'inspector_close'
  | 'dialog_close'
  | 'project_close'
  | 'context_replacement'

export interface ReviewDiscardConfirmation {
  reason: ReviewDiscardReason
}

export interface ReviewSessionCoordinatorOptions {
  port: ReviewPort
  sessionId: string
  generation: number
  selectedEntityIds: string[]
  enabled?: boolean
}

export interface ReviewSessionCoordinator {
  snapshot: ReviewSessionSnapshot
  proposal: ReviewScopeProposal | null
  completion: ReviewCompletionProposal | null
  editor: ReviewEditorState
  progress: ReviewProgress | null
  error: string | null
  discardConfirmation: ReviewDiscardConfirmation | null
  previewStart(scope: ReviewScopeRequest): Promise<void>
  confirmStart(): Promise<void>
  dismissStart(): void
  resume(): Promise<void>
  beginCreate(): void
  setEditorText(text: string): void
  submitFeedback(): Promise<void>
  beginEdit(feedbackId: string): void
  saveEdit(): Promise<void>
  deleteFeedback(feedbackId: string): Promise<void>
  prepareCompletion(): Promise<void>
  confirmCompletion(): Promise<void>
  dismissCompletion(): void
  abandon(): Promise<void>
  cancelTask(): Promise<void>
  requestDiscard(reason: ReviewDiscardReason, continuation?: () => void): boolean
  confirmDiscard(): void
  cancelDiscard(): void
}

export function useReviewSessionCoordinator({
  port,
  sessionId,
  generation,
  selectedEntityIds,
  enabled = true,
}: ReviewSessionCoordinatorOptions): ReviewSessionCoordinator {
  const [snapshot, setSnapshot] = useState<ReviewSessionSnapshot>(idleReviewSnapshot)
  const [proposal, setProposal] = useState<ReviewScopeProposal | null>(null)
  const [completion, setCompletion] = useState<ReviewCompletionProposal | null>(null)
  const [editor, setEditor] = useState<ReviewEditorState>(emptyReviewEditor)
  const [progress, setProgress] = useState<ReviewProgress | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [discardConfirmation, setDiscardConfirmation] = useState<ReviewDiscardConfirmation | null>(
    null,
  )

  const portRef = useRef(port)
  const contextRef = useRef({ sessionId, generation })
  const selectedEntityIdsRef = useRef(selectedEntityIds)
  const snapshotRef = useRef(snapshot)
  const proposalRef = useRef(proposal)
  const completionRef = useRef(completion)
  const editorRef = useRef(editor)
  const epochRef = useRef(0)
  const queueRef = useRef<Promise<void>>(Promise.resolve())
  const previewRequestRef = useRef(0)
  const discardContinuationRef = useRef<(() => void) | null>(null)

  portRef.current = port
  contextRef.current = { sessionId, generation }
  selectedEntityIdsRef.current = selectedEntityIds

  function commitSnapshot(next: ReviewSessionSnapshot) {
    snapshotRef.current = next
    setSnapshot(next)
  }

  function commitProposal(next: ReviewScopeProposal | null) {
    proposalRef.current = next
    setProposal(next)
  }

  function commitCompletion(next: ReviewCompletionProposal | null) {
    completionRef.current = next
    setCompletion(next)
  }

  function commitEditor(next: ReviewEditorState) {
    editorRef.current = next
    setEditor(next)
  }

  useEffect(() => {
    const epoch = epochRef.current + 1
    epochRef.current = epoch
    queueRef.current = Promise.resolve()
    previewRequestRef.current += 1
    commitSnapshot(idleReviewSnapshot())
    commitProposal(null)
    commitCompletion(null)
    commitEditor(emptyReviewEditor())
    setProgress(null)
    setError(null)
    setDiscardConfirmation(null)
    discardContinuationRef.current = null

    if (!enabled) return

    const request = { sessionId, generation }
    let disposed = false
    let unlisten: (() => void) | null = null

    void port.reviewStatus(request).then(
      (next) => {
        if (!disposed && epochRef.current === epoch) commitSnapshot(next)
      },
      (cause: unknown) => {
        if (!disposed && epochRef.current === epoch) setError(reviewErrorMessage(cause))
      },
    )
    void port
      .listenReviewProgress((next) => {
        if (
          !disposed &&
          epochRef.current === epoch &&
          next.sessionId === sessionId &&
          next.generation === generation
        ) {
          setProgress(next)
        }
      })
      .then((stop) => {
        if (disposed || epochRef.current !== epoch) stop()
        else unlisten = stop
      })
      .catch((cause: unknown) => {
        if (!disposed && epochRef.current === epoch) setError(reviewErrorMessage(cause))
      })

    return () => {
      disposed = true
      unlisten?.()
    }
  }, [port, sessionId, generation, enabled])

  function enqueue(operation: (epoch: number) => Promise<void>): Promise<void> {
    const epoch = epochRef.current
    const queued = queueRef.current.then(async () => {
      if (epochRef.current === epoch) await operation(epoch)
    })
    queueRef.current = queued.catch(() => undefined)
    return queued
  }

  function currentGuard() {
    const current = snapshotRef.current
    if (current.reviewRoundId === null) return null
    return {
      ...contextRef.current,
      reviewRoundId: current.reviewRoundId,
      expectedRevision: current.revision,
    }
  }

  async function refreshAfterStale(epoch: number): Promise<void> {
    try {
      const next = await portRef.current.reviewStatus(contextRef.current)
      if (epochRef.current === epoch) commitSnapshot(next)
    } catch (cause) {
      if (epochRef.current === epoch) setError(reviewErrorMessage(cause))
    }
  }

  async function reportFailure(cause: unknown, epoch: number, editorSave = false): Promise<void> {
    if (epochRef.current !== epoch) return
    const message = reviewErrorMessage(cause)
    setError(message)
    if (isStaleReviewError(cause)) await refreshAfterStale(epoch)
    if (editorSave && epochRef.current === epoch) {
      commitEditor({ ...editorRef.current, saveState: 'error', error: message })
    }
  }

  function installMutation(next: ReviewSessionSnapshot) {
    commitSnapshot(next)
    commitCompletion(null)
    setProgress(null)
    setError(null)
  }

  function clearEditor() {
    commitEditor(emptyReviewEditor())
  }

  function resetCreateEditor() {
    commitEditor({
      ...emptyReviewEditor(),
      targetEntityIds: eligibleReviewTargetIds(
        selectedEntityIdsRef.current,
        snapshotRef.current.members,
      ),
    })
  }

  function requestDiscard(reason: ReviewDiscardReason, continuation?: () => void): boolean {
    if (hasUnsavedReviewText(editorRef.current)) {
      discardContinuationRef.current = continuation ?? null
      setDiscardConfirmation({ reason })
      return false
    }
    clearEditor()
    continuation?.()
    return true
  }

  function replaceEditor(next: ReviewEditorState) {
    requestDiscard('context_replacement', () => commitEditor(next))
  }

  return {
    snapshot,
    proposal,
    completion,
    editor,
    progress,
    error,
    discardConfirmation,
    async previewStart(scope) {
      const epoch = epochRef.current
      const requestId = previewRequestRef.current + 1
      previewRequestRef.current = requestId
      setError(null)
      try {
        const next = await portRef.current.reviewPreviewStart({ ...contextRef.current, scope })
        if (epochRef.current === epoch && previewRequestRef.current === requestId) {
          commitProposal(next)
        }
      } catch (cause) {
        if (epochRef.current === epoch && previewRequestRef.current === requestId) {
          setError(reviewErrorMessage(cause))
        }
      }
    },
    confirmStart() {
      const selectedProposal = proposalRef.current
      if (selectedProposal === null) return Promise.resolve()
      return enqueue(async (epoch) => {
        try {
          const next = await portRef.current.reviewStart({
            ...contextRef.current,
            proposalId: selectedProposal.proposalId,
          })
          if (epochRef.current === epoch) {
            installMutation(next)
            commitProposal(null)
          }
        } catch (cause) {
          await reportFailure(cause, epoch)
        }
      })
    },
    dismissStart() {
      previewRequestRef.current += 1
      commitProposal(null)
    },
    resume() {
      return enqueue(async (epoch) => {
        try {
          const next = await portRef.current.reviewResume(contextRef.current)
          if (epochRef.current === epoch) installMutation(next)
        } catch (cause) {
          await reportFailure(cause, epoch)
        }
      })
    },
    beginCreate() {
      replaceEditor({
        ...emptyReviewEditor(),
        targetEntityIds: eligibleReviewTargetIds(
          selectedEntityIdsRef.current,
          snapshotRef.current.members,
        ),
      })
    },
    setEditorText(text) {
      commitEditor({ ...editorRef.current, text, saveState: 'idle', error: null })
    },
    submitFeedback() {
      const work = editorRef.current
      if (work.mode !== 'create' || work.text.trim().length === 0) return Promise.resolve()
      const frozen = { ...work, targetEntityIds: [...work.targetEntityIds] }
      commitEditor({ ...work, saveState: 'saving', error: null })
      return enqueue(async (epoch) => {
        const guard = currentGuard()
        if (guard === null) {
          await reportFailure(new Error('Review round unavailable'), epoch, true)
          return
        }
        try {
          const next = await portRef.current.reviewAddFeedback({
            ...guard,
            text: frozen.text,
            targets: frozen.targetEntityIds.map((entityId) => ({
              entityId,
              anchor: { kind: 'asset' as const },
            })),
          })
          if (epochRef.current === epoch) {
            installMutation(next)
            resetCreateEditor()
          }
        } catch (cause) {
          await reportFailure(cause, epoch, true)
        }
      })
    },
    beginEdit(feedbackId) {
      const feedback = snapshotRef.current.feedback.find(
        (candidate) => candidate.feedbackId === feedbackId,
      )
      if (feedback === undefined) return
      replaceEditor({
        mode: 'edit',
        feedbackId,
        text: feedback.text,
        savedText: feedback.text,
        targetEntityIds: [...feedback.targetEntityIds],
        saveState: 'idle',
        error: null,
      })
    },
    saveEdit() {
      const work = editorRef.current
      if (work.mode !== 'edit' || work.feedbackId === null || work.text.trim().length === 0) {
        return Promise.resolve()
      }
      const frozen = { ...work, targetEntityIds: [...work.targetEntityIds] }
      commitEditor({ ...work, saveState: 'saving', error: null })
      return enqueue(async (epoch) => {
        const guard = currentGuard()
        if (guard === null) {
          await reportFailure(new Error('Review round unavailable'), epoch, true)
          return
        }
        try {
          const next = await portRef.current.reviewUpdateFeedbackText({
            ...guard,
            feedbackId: frozen.feedbackId as string,
            text: frozen.text,
          })
          if (epochRef.current === epoch) {
            installMutation(next)
            resetCreateEditor()
          }
        } catch (cause) {
          await reportFailure(cause, epoch, true)
        }
      })
    },
    deleteFeedback(feedbackId) {
      return enqueue(async (epoch) => {
        const guard = currentGuard()
        if (guard === null) return
        try {
          const next = await portRef.current.reviewDeleteFeedback({ ...guard, feedbackId })
          if (epochRef.current === epoch) installMutation(next)
        } catch (cause) {
          await reportFailure(cause, epoch)
        }
      })
    },
    prepareCompletion() {
      return enqueue(async (epoch) => {
        const guard = currentGuard()
        if (guard === null) return
        try {
          const next = await portRef.current.reviewCompletionSummary(guard)
          if (epochRef.current === epoch) {
            commitCompletion(next)
            setError(null)
          }
        } catch (cause) {
          await reportFailure(cause, epoch)
        }
      })
    },
    confirmCompletion() {
      const selectedCompletion = completionRef.current
      if (selectedCompletion === null) return Promise.resolve()
      return enqueue(async (epoch) => {
        const guard = currentGuard()
        if (guard === null) return
        try {
          const next = await portRef.current.reviewComplete({
            ...guard,
            proposalId: selectedCompletion.proposalId,
          })
          if (epochRef.current === epoch) installMutation(next)
        } catch (cause) {
          await reportFailure(cause, epoch)
        }
      })
    },
    dismissCompletion() {
      commitCompletion(null)
    },
    abandon() {
      return enqueue(async (epoch) => {
        const guard = currentGuard()
        if (guard === null) return
        try {
          const next = await portRef.current.reviewAbandon(guard)
          if (epochRef.current === epoch) {
            installMutation(next)
            clearEditor()
          }
        } catch (cause) {
          await reportFailure(cause, epoch)
        }
      })
    },
    async cancelTask() {
      try {
        await portRef.current.reviewCancelTask(contextRef.current)
      } catch (cause) {
        setError(reviewErrorMessage(cause))
      }
    },
    requestDiscard,
    confirmDiscard() {
      const continuation = discardContinuationRef.current
      discardContinuationRef.current = null
      setDiscardConfirmation(null)
      clearEditor()
      continuation?.()
    },
    cancelDiscard() {
      discardContinuationRef.current = null
      setDiscardConfirmation(null)
    },
  }
}

function isStaleReviewError(error: unknown): boolean {
  if (typeof error !== 'object' || error === null || !('code' in error)) return false
  return error.code === 'review_stale_revision' || error.code === 'review_stale_round'
}

function reviewErrorMessage(error: unknown): string {
  if (
    typeof error === 'object' &&
    error !== null &&
    'userMessage' in error &&
    typeof error.userMessage === 'string' &&
    error.userMessage.length > 0
  ) {
    return error.userMessage
  }
  return '评审操作未完成，请重试。'
}
