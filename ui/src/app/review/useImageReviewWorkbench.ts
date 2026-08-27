import { useCallback, useEffect, useReducer, useRef, useState } from 'react'
import type { ReviewAnchor, ReviewScopeRequest, ReviewSessionSnapshot } from '../../api/types'
import {
  type AnnotationEditorAction,
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
  redrawFeedbackId: string | null
  railOpen: boolean
  readOnlyReason: ImageReviewReadOnlyReason
  restorableFeedbackId: string | null
  leaveConfirmation: ReviewLeaveIntent | null
  setTool(tool: AnnotationTool): void
  setTemporaryPan(active: boolean): void
  beginAnnotation(anchor: ReviewAnchor): void
  beginDrawing(anchor: ReviewAnchor, feedbackId?: string): boolean
  finishDrawing(anchor: ReviewAnchor | null): Promise<void>
  beginFeedbackTextEdit(feedbackId: string): void
  beginRedraw(feedbackId: string): void
  stageFeedbackAnchor(feedbackId: string, anchor: ReviewAnchor): boolean
  updateDraftAnchor(anchor: ReviewAnchor): void
  updateDraftText(text: string): void
  saveDraft(): Promise<void>
  cancelDraft(): void
  selectFeedback(feedbackId: string | null): void
  replaceFeedbackAnchor(feedbackId: string, anchor: ReviewAnchor): Promise<void>
  deleteFeedback(feedbackId: string): Promise<void>
  restoreDeletedFeedback(feedbackId: string): Promise<void>
  setRailOpen(open: boolean): void
  requestLeave(intent: ReviewLeaveIntent): Promise<'proceeded' | 'blocked'>
  cancelLeave(): void
  discardUnsavedAndProceed(): Promise<void>
}

export function useImageReviewWorkbench({
  coordinator,
  entityId,
  scope,
  onLeave,
}: UseImageReviewWorkbenchOptions): ImageReviewWorkbenchController {
  const [editor, reduce] = useReducer(
    annotationEditorReducer,
    undefined,
    initialAnnotationEditorState,
  )
  const [snapshot, setSnapshot] = useState(coordinator.snapshot)
  const [railOpen, setRailOpen] = useState(true)
  const [redrawFeedbackId, setRedrawFeedbackId] = useState<string | null>(null)
  const [leaveConfirmation, setLeaveConfirmation] = useState<ReviewLeaveIntent | null>(null)
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
  // Synchronous ownership also guards back-to-back pointer/leave/save commands
  // before React has committed the next render.
  const dispatch = useCallback((action: AnnotationEditorAction) => {
    editorRef.current = annotationEditorReducer(editorRef.current, action)
    reduce(action)
  }, [])

  useEffect(() => setSnapshot(coordinator.snapshot), [coordinator.snapshot])

  useEffect(() => {
    if (snapshot.phase !== 'idle' || previewRequestedRef.current) return
    previewRequestedRef.current = true
    previewPromiseRef.current = coordinatorRef.current.captureStart(capturedScopeRef.current)
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
      operation: draft.operation,
    }
    const existingIds = new Set(
      imageFeedbackForEntity(snapshot, entityId).map((item) => item.feedbackId),
    )
    dispatch({ type: 'request_save' })
    try {
      let next: ReviewSessionSnapshot | null
      if (frozen.sourceFeedbackId !== null) {
        next =
          frozen.operation === 'geometry'
            ? await coordinatorRef.current.replaceAnchoredFeedbackAnchor(
                frozen.sourceFeedbackId,
                entityId,
                frozen.anchor,
              )
            : await coordinatorRef.current.updateAnchoredFeedbackText(
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
            ? imageFeedbackForEntity(installed, entityId).find(
                (item) => !existingIds.has(item.feedbackId),
              )
            : imageFeedbackForEntity(installed, entityId).find(
                (feedback) => feedback.feedbackId === frozen.sourceFeedbackId,
              )
      if (saved === null || saved === undefined) throw new Error('Saved feedback unavailable')
      dispatch({ type: 'save_succeeded', feedbackId: saved.feedbackId })
      setRedrawFeedbackId(null)
    } catch (cause) {
      dispatch({ type: 'save_failed', message: reviewErrorMessage(cause) })
    }
  }, [dispatch, entityId, installSnapshot, snapshot])

  const beginDrawing = useCallback(
    (anchor: ReviewAnchor, feedbackId?: string) => {
      if (
        hasUnsavedAnnotation(editorRef.current) ||
        imageReviewReadOnlyReason(snapshot, entityId) !== null
      )
        return false
      dispatch({ type: 'begin_drawing', anchor: cloneReviewAnchor(anchor), feedbackId })
      return editorRef.current.status === 'drawing'
    },
    [dispatch, entityId, snapshot],
  )

  const replaceFeedbackAnchor = useCallback(
    async (feedbackId: string, anchor: ReviewAnchor) => {
      const state = editorRef.current
      if (
        state.status !== 'idle' &&
        !(state.status === 'drawing' && state.sourceFeedbackId === feedbackId)
      )
        return
      const feedback = imageFeedbackForEntity(snapshot, entityId).find(
        (item) => item.feedbackId === feedbackId,
      )
      if (feedback === undefined || imageReviewReadOnlyReason(snapshot, entityId) !== null) return
      dispatch({
        type: 'begin_edit',
        feedbackId,
        anchor: cloneReviewAnchor(anchor),
        text: feedback.text,
        operation: 'geometry',
      })
      await saveDraft()
    },
    [dispatch, entityId, saveDraft, snapshot],
  )

  const applySavedMutation = useCallback(
    async (operation: () => Promise<ReviewSessionSnapshot | null>) => {
      if (hasUnsavedAnnotation(editorRef.current)) return
      installSnapshot(await operation())
    },
    [installSnapshot],
  )

  const requestLeave = useCallback(async (intent: ReviewLeaveIntent) => {
    if (hasUnsavedAnnotation(editorRef.current)) {
      pendingLeaveRef.current = intent
      setLeaveConfirmation(intent)
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
    redrawFeedbackId,
    railOpen,
    readOnlyReason: imageReviewReadOnlyReason(snapshot, entityId),
    restorableFeedbackId: snapshot.restorableFeedbackId,
    leaveConfirmation,
    setTool(tool) {
      if (hasUnsavedAnnotation(editorRef.current)) return
      setRedrawFeedbackId(null)
      dispatch({ type: 'set_tool', tool })
    },
    setTemporaryPan(active) {
      dispatch({ type: active ? 'temporary_pan_start' : 'temporary_pan_end' })
    },
    beginAnnotation(anchor) {
      if (imageReviewReadOnlyReason(snapshot, entityId) !== null) return
      dispatch({ type: 'begin_annotation', anchor })
    },
    beginDrawing,
    async finishDrawing(anchor) {
      const state = editorRef.current
      if (state.status !== 'drawing') return
      if (anchor === null || !isValidAnnotationAnchor(anchor)) {
        dispatch({ type: 'cancel_draft' })
      } else if (state.sourceFeedbackId !== undefined) {
        await replaceFeedbackAnchor(state.sourceFeedbackId, anchor)
      } else {
        dispatch({ type: 'update_draft_anchor', anchor })
        dispatch({ type: 'complete_drawing' })
      }
    },
    beginFeedbackTextEdit(feedbackId) {
      if (
        hasUnsavedAnnotation(editorRef.current) ||
        imageReviewReadOnlyReason(snapshot, entityId) !== null
      )
        return
      const feedback = imageFeedbackForEntity(snapshot, entityId).find(
        (item) => item.feedbackId === feedbackId,
      )
      if (feedback !== undefined)
        dispatch({
          type: 'begin_edit',
          feedbackId,
          anchor: feedback.anchor,
          text: feedback.text,
          operation: 'text',
        })
    },
    beginRedraw(feedbackId) {
      if (
        hasUnsavedAnnotation(editorRef.current) ||
        imageReviewReadOnlyReason(snapshot, entityId) !== null
      )
        return
      if (
        !imageFeedbackForEntity(snapshot, entityId).some(
          (item) => item.feedbackId === feedbackId && item.anchor.kind === 'image_stroke',
        )
      )
        return
      dispatch({ type: 'select_feedback', feedbackId })
      dispatch({ type: 'set_tool', tool: 'brush' })
      setRedrawFeedbackId(feedbackId)
    },
    stageFeedbackAnchor(feedbackId, anchor) {
      const state = editorRef.current
      if (state.status === 'idle') {
        dispatch({ type: 'set_tool', tool: anchor.kind === 'image_stroke' ? 'brush' : 'rectangle' })
        return beginDrawing(anchor, feedbackId)
      }
      if (state.status !== 'drawing' || state.sourceFeedbackId !== feedbackId) return false
      dispatch({ type: 'update_draft_anchor', anchor })
      return true
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
      setRedrawFeedbackId(null)
    },
    selectFeedback(feedbackId) {
      dispatch({ type: 'select_feedback', feedbackId })
    },
    replaceFeedbackAnchor,
    deleteFeedback(feedbackId) {
      return applySavedMutation(() => coordinatorRef.current.deleteFeedback(feedbackId))
    },
    restoreDeletedFeedback(feedbackId) {
      return applySavedMutation(() => coordinatorRef.current.restoreDeletedFeedback(feedbackId))
    },
    setRailOpen,
    requestLeave,
    cancelLeave() {
      pendingLeaveRef.current = null
      setLeaveConfirmation(null)
    },
    async discardUnsavedAndProceed() {
      // A committed write cannot be discarded mid-flight. Keep the confirmation
      // and ownership until it settles, then allow explicit discard/retry.
      if (editorRef.current.status === 'saving') return
      const pending = pendingLeaveRef.current
      pendingLeaveRef.current = null
      setLeaveConfirmation(null)
      dispatch({ type: 'cancel_draft' })
      setRedrawFeedbackId(null)
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
