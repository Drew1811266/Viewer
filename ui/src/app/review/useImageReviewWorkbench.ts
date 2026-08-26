import { useCallback, useEffect, useReducer, useRef, useState } from 'react'
import type { ReviewAnchor, ReviewScopeRequest, ReviewSessionSnapshot } from '../../api/types'
import {
  type AnnotationEditorState,
  type AnnotationTool,
  annotationEditorReducer,
  hasUnsavedAnnotation,
  initialAnnotationEditorState,
  isValidAnnotationAnchor,
} from './annotationModel'
import {
  type ImageReviewReadOnlyReason,
  imageFeedbackForEntity,
  imageReviewReadOnlyReason,
  type SavedImageFeedback,
} from './reviewModel'
import type { ReviewSessionCoordinator } from './useReviewSessionCoordinator'

export type { ReviewAnchor } from '../../api/types'
export type { AnnotationEditorState, AnnotationTool, SavedImageFeedback }

export type ReviewLeaveIntent =
  | { kind: 'navigate'; offset: -1 | 1 }
  | { kind: 'return_grid' }
  | { kind: 'close_project' }
  | { kind: 'finish_review' }
  | { kind: 'abandon_review' }

export interface UseImageReviewWorkbenchOptions {
  coordinator: ReviewSessionCoordinator
  entityId: string
  scope: ReviewScopeRequest
  onLeave?: (intent: ReviewLeaveIntent) => void | Promise<void>
}

export interface ImageReviewWorkbenchController {
  tool: AnnotationTool
  editor: AnnotationEditorState
  dirty: boolean
  feedback: ReadonlyArray<SavedImageFeedback>
  selectedFeedbackId: string | null
  railOpen: boolean
  readOnlyReason: ImageReviewReadOnlyReason
  restorableFeedbackId: string | null
  setTool(tool: AnnotationTool): void
  setTemporaryPan(active: boolean): void
  beginAnnotation(anchor: ReviewAnchor): void
  updateDraftAnchor(anchor: ReviewAnchor): void
  updateDraftText(text: string): void
  saveDraft(): Promise<void>
  cancelDraft(): void
  selectFeedback(feedbackId: string | null): void
  updateFeedbackText(feedbackId: string, text: string): Promise<void>
  replaceFeedbackAnchor(feedbackId: string, anchor: ReviewAnchor): Promise<void>
  deleteFeedback(feedbackId: string): Promise<void>
  restoreDeletedFeedback(feedbackId: string): Promise<void>
  setRailOpen(open: boolean): void
  requestLeave(intent: ReviewLeaveIntent): Promise<'proceeded' | 'blocked'>
  discardUnsavedAndProceed(): Promise<void>
}

export function useImageReviewWorkbench({
  coordinator,
  entityId,
  scope,
  onLeave,
}: UseImageReviewWorkbenchOptions): ImageReviewWorkbenchController {
  const [editor, dispatch] = useReducer(
    annotationEditorReducer,
    undefined,
    initialAnnotationEditorState,
  )
  const [snapshot, setSnapshot] = useState(coordinator.snapshot)
  const [railOpen, setRailOpen] = useState(true)
  const coordinatorRef = useRef(coordinator)
  const editorRef = useRef(editor)
  const onLeaveRef = useRef(onLeave)
  const capturedScopeRef = useRef(cloneReviewScope(scope))
  const previewRequestedRef = useRef(false)
  const previewPromiseRef = useRef<Promise<void> | null>(null)
  const pendingLeaveRef = useRef<ReviewLeaveIntent | null>(null)

  coordinatorRef.current = coordinator
  editorRef.current = editor
  onLeaveRef.current = onLeave

  useEffect(() => setSnapshot(coordinator.snapshot), [coordinator.snapshot])

  useEffect(() => {
    if (snapshot.phase !== 'idle' || previewRequestedRef.current) return
    previewRequestedRef.current = true
    previewPromiseRef.current = coordinatorRef.current.previewStart(capturedScopeRef.current)
  }, [snapshot.phase])

  const installSnapshot = useCallback((next: ReviewSessionSnapshot | null) => {
    if (next !== null) setSnapshot(next)
    return next
  }, [])

  const saveDraft = useCallback(async () => {
    const draft = editorRef.current
    if (
      (draft.status !== 'editing' && draft.status !== 'save_error') ||
      draft.text.trim().length === 0 ||
      !isValidAnnotationAnchor(draft.draftAnchor)
    ) {
      return
    }
    const frozen = {
      text: draft.text,
      anchor: cloneReviewAnchor(draft.draftAnchor),
      sourceFeedbackId: draft.sourceFeedbackId,
    }
    dispatch({ type: 'request_save' })
    try {
      let next: ReviewSessionSnapshot | null
      if (frozen.sourceFeedbackId !== null) {
        next = await coordinatorRef.current.updateAnchoredFeedbackText(
          frozen.sourceFeedbackId,
          frozen.text,
        )
      } else if (snapshot.phase === 'active') {
        next = await coordinatorRef.current.addAnchoredFeedback({
          entityId,
          text: frozen.text,
          anchor: frozen.anchor,
        })
      } else {
        await previewPromiseRef.current
        next = await coordinatorRef.current.startWithFeedback({
          entityId,
          text: frozen.text,
          anchor: frozen.anchor,
        })
      }
      const installed = installSnapshot(next)
      const saved =
        installed === null
          ? null
          : frozen.sourceFeedbackId === null
            ? imageFeedbackForEntity(installed, entityId).at(-1)
            : imageFeedbackForEntity(installed, entityId).find(
                (feedback) => feedback.feedbackId === frozen.sourceFeedbackId,
              )
      if (saved === null || saved === undefined) throw new Error('Saved feedback unavailable')
      dispatch({ type: 'save_succeeded', feedbackId: saved.feedbackId })
    } catch (cause) {
      dispatch({ type: 'save_failed', message: reviewErrorMessage(cause) })
    }
  }, [entityId, installSnapshot, snapshot.phase])

  const applySavedMutation = useCallback(
    async (operation: () => Promise<ReviewSessionSnapshot | null>) => {
      installSnapshot(await operation())
    },
    [installSnapshot],
  )

  const requestLeave = useCallback(async (intent: ReviewLeaveIntent) => {
    if (hasUnsavedAnnotation(editorRef.current)) {
      pendingLeaveRef.current = intent
      return 'blocked' as const
    }
    await onLeaveRef.current?.(intent)
    return 'proceeded' as const
  }, [])

  return {
    tool: editor.tool,
    editor,
    dirty: hasUnsavedAnnotation(editor),
    feedback: imageFeedbackForEntity(snapshot, entityId),
    selectedFeedbackId: editor.selectedFeedbackId,
    railOpen,
    readOnlyReason: imageReviewReadOnlyReason(snapshot, entityId),
    restorableFeedbackId: snapshot.restorableFeedbackId,
    setTool(tool) {
      dispatch({ type: 'set_tool', tool })
    },
    setTemporaryPan(active) {
      dispatch({ type: active ? 'temporary_pan_start' : 'temporary_pan_end' })
    },
    beginAnnotation(anchor) {
      if (imageReviewReadOnlyReason(snapshot, entityId) !== null) return
      dispatch({ type: 'begin_annotation', anchor })
    },
    updateDraftAnchor(anchor) {
      dispatch({ type: 'update_draft_anchor', anchor })
    },
    updateDraftText(text) {
      dispatch({ type: 'update_text', text })
    },
    saveDraft,
    cancelDraft() {
      dispatch({ type: 'cancel_draft' })
    },
    selectFeedback(feedbackId) {
      dispatch({ type: 'select_feedback', feedbackId })
    },
    updateFeedbackText(feedbackId, text) {
      return applySavedMutation(() =>
        coordinatorRef.current.updateAnchoredFeedbackText(feedbackId, text),
      )
    },
    replaceFeedbackAnchor(feedbackId, anchor) {
      return applySavedMutation(() =>
        coordinatorRef.current.replaceAnchoredFeedbackAnchor(feedbackId, entityId, anchor),
      )
    },
    deleteFeedback(feedbackId) {
      return applySavedMutation(() => coordinatorRef.current.deleteFeedback(feedbackId))
    },
    restoreDeletedFeedback(feedbackId) {
      return applySavedMutation(() => coordinatorRef.current.restoreDeletedFeedback(feedbackId))
    },
    setRailOpen,
    requestLeave,
    async discardUnsavedAndProceed() {
      const pending = pendingLeaveRef.current
      pendingLeaveRef.current = null
      dispatch({ type: 'cancel_draft' })
      if (pending !== null) await onLeaveRef.current?.(pending)
    },
  }
}

function cloneReviewAnchor(anchor: ReviewAnchor): ReviewAnchor {
  return anchor.kind === 'image_stroke'
    ? { kind: 'image_stroke', points: anchor.points.map((point) => ({ ...point })) }
    : { ...anchor }
}

function cloneReviewScope(scope: ReviewScopeRequest): ReviewScopeRequest {
  return scope.kind === 'selection'
    ? { kind: 'selection', entityIds: [...scope.entityIds] }
    : { ...scope }
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
  return '意见尚未保存，请重试。'
}
