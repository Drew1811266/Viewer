import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from 'react'
import type { ReviewAnchor } from '../../api/types'
import {
  type AnnotationEditorAction,
  type AnnotationEditorState,
  type AnnotationTool,
  annotationEditorReducer,
  hasUnsavedAnnotation,
  initialAnnotationEditorState,
  isValidAnnotationAnchor,
} from './annotationModel'
import type {
  ImageReviewPreparation,
  ImageReviewWorkbenchAdapter,
  ImageReviewWorkbenchFeedback,
  ImageReviewWorkbenchView,
} from './imageReviewWorkbenchAdapter'
import type { ImageReviewReadOnlyReason } from './reviewModel'

export type { ReviewAnchor } from '../../api/types'
export type { AnnotationEditorState, AnnotationTool }
export type SavedImageFeedback = ImageReviewWorkbenchFeedback

export type ReviewLeaveIntent =
  | { kind: 'navigate'; offset: -1 | 1 }
  | { kind: 'return_grid' }
  | { kind: 'close_project' }
  | { kind: 'finish_review' }
  | { kind: 'abandon_review' }

export interface UseImageReviewWorkbenchOptions {
  adapter: ImageReviewWorkbenchAdapter
  entityId: string
  onLeave?: (intent: ReviewLeaveIntent) => void | Promise<void>
}

export interface ImageReviewWorkbenchController {
  protocol: ImageReviewWorkbenchAdapter['protocol']
  tool: AnnotationTool
  editor: AnnotationEditorState
  dirty: boolean
  feedback: ReadonlyArray<SavedImageFeedback>
  selectedItemId: string | null
  redrawItemId: string | null
  railOpen: boolean
  readOnlyReason: ImageReviewReadOnlyReason
  restorableItemId: string | null
  statusMessage: string | null
  preparedImage: {
    entityId: string
    assetVersionId: string
    url: string
    width: number
    height: number
  } | null
  leaveConfirmation: ReviewLeaveIntent | null
  pendingMutationId?: string | null
  setTool(tool: AnnotationTool): void
  setTemporaryPan(active: boolean): void
  beginAnnotation(anchor: ReviewAnchor): void
  beginDrawing(anchor: ReviewAnchor, itemId?: string): boolean
  finishDrawing(anchor: ReviewAnchor | null): Promise<void>
  beginFeedbackTextEdit(itemId: string): void
  beginRedraw(itemId: string): void
  stageFeedbackAnchor(itemId: string, anchor: ReviewAnchor): boolean
  updateDraftAnchor(anchor: ReviewAnchor): void
  updateDraftText(text: string): void
  saveDraft(): Promise<void>
  cancelDraft(): void
  selectFeedback(itemId: string | null): void
  replaceFeedbackAnchor(itemId: string, anchor: ReviewAnchor): Promise<void>
  deleteFeedback(itemId: string): Promise<void>
  restoreDeletedFeedback(itemId: string): Promise<void>
  setRailOpen(open: boolean): void
  requestLeave(intent: ReviewLeaveIntent): Promise<'proceeded' | 'blocked'>
  cancelLeave(): void
  discardUnsavedAndProceed(): Promise<void>
}

export function useImageReviewWorkbench({
  adapter,
  entityId,
  onLeave,
}: UseImageReviewWorkbenchOptions): ImageReviewWorkbenchController {
  const [editor, reduce] = useReducer(
    annotationEditorReducer,
    undefined,
    initialAnnotationEditorState,
  )
  const [preparation, setPreparation] = useState<ImageReviewPreparation | null>(null)
  const [workbenchView, setWorkbenchView] = useState<ImageReviewWorkbenchView>(() =>
    adapter.view(entityId, null),
  )
  const [railOpen, setRailOpen] = useState(true)
  const [redrawItemId, setRedrawItemId] = useState<string | null>(null)
  const [leaveConfirmation, setLeaveConfirmation] = useState<ReviewLeaveIntent | null>(null)
  const [pendingMutationId, setPendingMutationId] = useState<string | null>(null)
  const adapterRef = useRef(adapter)
  const preparationRef = useRef<ImageReviewPreparation | null>(null)
  const editorRef = useRef(editor)
  const editorEntityRef = useRef<string | null>(null)
  const editorPreparationKeyRef = useRef<string | null>(null)
  const editorAssetVersionIdRef = useRef<string | null>(null)
  const editorItemRef = useRef<ImageReviewWorkbenchFeedback | null>(null)
  const statusMessageRef = useRef<{ entityId: string; message: string } | null>(null)
  const onLeaveRef = useRef(onLeave)
  const pendingLeaveRef = useRef<ReviewLeaveIntent | null>(null)
  const pendingMutationIdRef = useRef<string | null>(null)

  adapterRef.current = adapter
  editorRef.current = editor
  onLeaveRef.current = onLeave
  // Synchronous ownership also guards back-to-back pointer/leave/save commands
  // before React has committed the next render.
  const dispatch = useCallback((action: AnnotationEditorAction) => {
    editorRef.current = annotationEditorReducer(editorRef.current, action)
    reduce(action)
  }, [])

  const preparationKey = adapter.preparationKey(entityId)
  const viewKey = adapter.viewKey(entityId)
  useEffect(() => {
    let active = true
    preparationRef.current = null
    setPreparation(null)
    setWorkbenchView(adapterRef.current.view(entityId, null))
    void adapterRef.current.prepareEntity(entityId).then(
      (next) => {
        if (!active || next.key !== preparationKey) return
        preparationRef.current = next
        setPreparation(next)
        setWorkbenchView(adapterRef.current.view(entityId, next))
      },
      (cause) => {
        if (!active) return
        setWorkbenchView((current) => ({
          ...current,
          readOnlyReason: 'source_confirmation',
          statusMessage: reviewErrorMessage(cause),
        }))
      },
    )
    return () => {
      active = false
    }
  }, [entityId, preparationKey])

  useEffect(() => {
    setWorkbenchView(adapterRef.current.view(entityId, preparationRef.current))
  }, [entityId, preparation, viewKey])

  const saveDraft = useCallback(async () => {
    const draft = editorRef.current.phase
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
      sourceItemId: draft.sourceItemId,
      operation: draft.operation ?? 'text',
    }
    const currentPreparation = preparationRef.current
    if (
      editorEntityRef.current !== entityId ||
      editorPreparationKeyRef.current !== adapterRef.current.preparationKey(entityId) ||
      (adapterRef.current.protocol === 'continuous' &&
        (currentPreparation?.key !== editorPreparationKeyRef.current ||
          (currentPreparation?.prepared?.asset.id ?? null) !== editorAssetVersionIdRef.current))
    ) {
      dispatch({ type: 'request_save' })
      dispatch({
        type: 'save_failed',
        message: '这条未保存意见属于上一素材版本或评审会话，请返回原上下文或放弃后重画。',
      })
      return
    }
    statusMessageRef.current = null
    const existingIds = new Set(workbenchView.feedback.map((item) => item.itemId))
    const item = frozen.sourceItemId === null ? null : editorItemRef.current
    if (frozen.sourceItemId !== null && item === null) {
      dispatch({ type: 'request_save' })
      dispatch({ type: 'save_failed', message: '意见已变化，请重新选择。' })
      return
    }
    dispatch({ type: 'request_save' })
    const clientMutationId = pendingMutationIdRef.current ?? nextReviewClientMutationId(entityId)
    pendingMutationIdRef.current = clientMutationId
    setPendingMutationId(clientMutationId)
    try {
      const installed = await adapterRef.current.saveFeedback({
        entityId,
        clientMutationId,
        item,
        operation: frozen.operation,
        text: frozen.text,
        anchor: frozen.anchor,
        preparation: preparationRef.current ?? {
          key: adapterRef.current.preparationKey(entityId),
          prepared: null,
        },
      })
      setWorkbenchView(installed)
      if (installed.statusMessage !== null)
        statusMessageRef.current = { entityId, message: installed.statusMessage }
      const saved =
        frozen.sourceItemId === null
          ? installed.feedback.find((feedback) => !existingIds.has(feedback.itemId))
          : installed.feedback.find((feedback) => feedback.itemId === frozen.sourceItemId)
      if (saved === undefined) throw new Error('Saved feedback unavailable')
      dispatch({ type: 'save_succeeded', itemId: saved.itemId })
      clearEditorOwnership(
        editorEntityRef,
        editorPreparationKeyRef,
        editorAssetVersionIdRef,
        editorItemRef,
      )
      pendingMutationIdRef.current = null
      setPendingMutationId(null)
      setRedrawItemId(null)
    } catch (cause) {
      dispatch({ type: 'save_failed', message: reviewErrorMessage(cause) })
    }
  }, [dispatch, entityId, workbenchView.feedback])

  const beginDrawing = useCallback(
    (anchor: ReviewAnchor, itemId?: string) => {
      if (hasUnsavedAnnotation(editorRef.current) || workbenchView.readOnlyReason !== null)
        return false
      dispatch({ type: 'begin_drawing', anchor: cloneReviewAnchor(anchor), itemId })
      if (editorRef.current.phase.status === 'drawing') {
        captureEditorOwnership(
          adapterRef.current,
          entityId,
          preparationRef.current,
          editorEntityRef,
          editorPreparationKeyRef,
          editorAssetVersionIdRef,
        )
        editorItemRef.current =
          itemId === undefined
            ? null
            : cloneWorkbenchFeedback(
                workbenchView.feedback.find((feedback) => feedback.itemId === itemId) ?? null,
              )
        statusMessageRef.current = null
      }
      return editorRef.current.phase.status === 'drawing'
    },
    [dispatch, entityId, workbenchView.feedback, workbenchView.readOnlyReason],
  )

  const replaceFeedbackAnchor = useCallback(
    async (itemId: string, anchor: ReviewAnchor) => {
      const state = editorRef.current.phase
      if (state.status !== 'idle' && !(state.status === 'drawing' && state.sourceItemId === itemId))
        return
      const feedback = workbenchView.feedback.find((item) => item.itemId === itemId)
      if (feedback === undefined || workbenchView.readOnlyReason !== null) return
      dispatch({
        type: 'begin_edit',
        itemId,
        anchor: cloneReviewAnchor(anchor),
        text: feedback.text,
        operation: 'geometry',
      })
      captureEditorOwnership(
        adapterRef.current,
        entityId,
        preparationRef.current,
        editorEntityRef,
        editorPreparationKeyRef,
        editorAssetVersionIdRef,
      )
      editorItemRef.current = cloneWorkbenchFeedback(feedback)
      await saveDraft()
    },
    [dispatch, saveDraft, workbenchView.feedback, workbenchView.readOnlyReason],
  )

  const applySavedMutation = useCallback(
    async (operation: () => Promise<ImageReviewWorkbenchView>) => {
      if (hasUnsavedAnnotation(editorRef.current)) return
      statusMessageRef.current = null
      const next = await operation()
      if (next.statusMessage !== null)
        statusMessageRef.current = { entityId, message: next.statusMessage }
      setWorkbenchView(next)
    },
    [entityId],
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

  const preparedImage = useMemo(
    () => preparedImageForEntity(entityId, preparation),
    [entityId, preparation],
  )

  return {
    protocol: adapter.protocol,
    tool: editor.activeTool,
    editor,
    dirty: hasUnsavedAnnotation(editor),
    feedback: workbenchView.feedback,
    selectedItemId: editor.selectedItemId,
    redrawItemId,
    railOpen,
    readOnlyReason: workbenchView.readOnlyReason,
    restorableItemId: workbenchView.restorableItemId,
    statusMessage: workbenchStatusMessage(entityId, workbenchView, statusMessageRef.current),
    preparedImage,
    leaveConfirmation,
    pendingMutationId,
    setTool(tool) {
      if (hasUnsavedAnnotation(editorRef.current)) return
      setRedrawItemId(null)
      dispatch({ type: 'set_tool', tool })
    },
    setTemporaryPan(active) {
      dispatch({ type: active ? 'temporary_pan_start' : 'temporary_pan_end' })
    },
    beginAnnotation(anchor) {
      if (hasUnsavedAnnotation(editorRef.current) || workbenchView.readOnlyReason !== null) return
      dispatch({ type: 'begin_annotation', anchor })
      if (editorRef.current.phase.status === 'editing') {
        captureEditorOwnership(
          adapterRef.current,
          entityId,
          preparationRef.current,
          editorEntityRef,
          editorPreparationKeyRef,
          editorAssetVersionIdRef,
        )
        editorItemRef.current = null
        statusMessageRef.current = null
      }
    },
    beginDrawing,
    async finishDrawing(anchor) {
      const state = editorRef.current.phase
      if (state.status !== 'drawing') return
      if (anchor === null || !isValidAnnotationAnchor(anchor)) {
        dispatch({ type: 'cancel_draft' })
        clearEditorOwnership(
          editorEntityRef,
          editorPreparationKeyRef,
          editorAssetVersionIdRef,
          editorItemRef,
        )
      } else if (state.sourceItemId !== undefined) {
        await replaceFeedbackAnchor(state.sourceItemId, anchor)
      } else {
        dispatch({ type: 'update_draft_anchor', anchor })
        dispatch({ type: 'complete_drawing' })
      }
    },
    beginFeedbackTextEdit(itemId) {
      if (hasUnsavedAnnotation(editorRef.current) || workbenchView.readOnlyReason !== null) return
      const feedback = workbenchView.feedback.find((feedback) => feedback.itemId === itemId)
      if (feedback !== undefined)
        dispatch({
          type: 'begin_edit',
          itemId,
          anchor: feedback.anchor,
          text: feedback.text,
          operation: 'text',
        })
      if (feedback !== undefined) {
        captureEditorOwnership(
          adapterRef.current,
          entityId,
          preparationRef.current,
          editorEntityRef,
          editorPreparationKeyRef,
          editorAssetVersionIdRef,
        )
        editorItemRef.current = cloneWorkbenchFeedback(feedback)
        statusMessageRef.current = null
      }
    },
    beginRedraw(itemId) {
      if (hasUnsavedAnnotation(editorRef.current) || workbenchView.readOnlyReason !== null) return
      const feedback = workbenchView.feedback.find((feedback) => feedback.itemId === itemId)
      if (feedback === undefined || !feedback.anchor.kind.startsWith('image_')) return
      dispatch({ type: 'select_feedback', itemId })
      dispatch({ type: 'set_tool', tool: markupToolForAnchor(feedback.anchor) })
      setRedrawItemId(itemId)
    },
    stageFeedbackAnchor(itemId, anchor) {
      const state = editorRef.current.phase
      if (state.status === 'idle') {
        dispatch({ type: 'set_tool', tool: markupToolForAnchor(anchor) })
        return beginDrawing(anchor, itemId)
      }
      if (state.status !== 'drawing' || state.sourceItemId !== itemId) return false
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
      if (!adapterRef.current.discardPendingInput()) return
      dispatch({ type: 'cancel_draft' })
      if (editorRef.current.phase.status === 'idle') {
        clearEditorOwnership(
          editorEntityRef,
          editorPreparationKeyRef,
          editorAssetVersionIdRef,
          editorItemRef,
        )
        pendingMutationIdRef.current = null
        setPendingMutationId(null)
      }
      setRedrawItemId(null)
    },
    selectFeedback(itemId) {
      dispatch({ type: 'select_feedback', itemId })
    },
    replaceFeedbackAnchor,
    deleteFeedback(itemId) {
      const item = workbenchView.feedback.find((feedback) => feedback.itemId === itemId)
      return item === undefined
        ? Promise.resolve()
        : applySavedMutation(() =>
            adapterRef.current.deleteFeedback({
              entityId,
              item,
              preparation: preparationRef.current ?? {
                key: adapterRef.current.preparationKey(entityId),
                prepared: null,
              },
            }),
          )
    },
    restoreDeletedFeedback(itemId) {
      return applySavedMutation(() =>
        adapterRef.current.restoreDeletedFeedback({
          entityId,
          itemId,
          preparation: preparationRef.current,
        }),
      )
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
      if (editorRef.current.phase.status === 'saving') return
      if (!adapterRef.current.discardPendingInput()) return
      const pending = pendingLeaveRef.current
      pendingLeaveRef.current = null
      setLeaveConfirmation(null)
      dispatch({ type: 'cancel_draft' })
      clearEditorOwnership(
        editorEntityRef,
        editorPreparationKeyRef,
        editorAssetVersionIdRef,
        editorItemRef,
      )
      pendingMutationIdRef.current = null
      setPendingMutationId(null)
      setRedrawItemId(null)
      if (pending !== null) await onLeaveRef.current?.(pending)
    },
  }
}

let reviewClientMutationSequence = 0

function nextReviewClientMutationId(entityId: string): string {
  reviewClientMutationSequence =
    reviewClientMutationSequence >= Number.MAX_SAFE_INTEGER ? 1 : reviewClientMutationSequence + 1
  return `${entityId}:${reviewClientMutationSequence}`
}

function cloneReviewAnchor(anchor: ReviewAnchor): ReviewAnchor {
  switch (anchor.kind) {
    case 'asset':
      return { kind: 'asset' }
    case 'image_point':
      return { ...anchor }
    case 'image_arrow':
      return { kind: anchor.kind, tail: { ...anchor.tail }, head: { ...anchor.head } }
    case 'image_stroke':
      return { kind: anchor.kind, points: anchor.points.map((point) => ({ ...point })) }
    case 'image_rect':
    case 'image_ellipse':
    case 'video_point':
    case 'video_range':
      return { ...anchor }
    default:
      return assertNever(anchor)
  }
}

function markupToolForAnchor(anchor: ReviewAnchor): AnnotationTool {
  switch (anchor.kind) {
    case 'image_point':
      return 'point'
    case 'image_arrow':
      return 'arrow'
    case 'image_stroke':
      return 'brush'
    case 'image_rect':
      return 'rectangle'
    case 'image_ellipse':
      return 'ellipse'
    case 'asset':
    case 'video_point':
    case 'video_range':
      return 'browse'
    default:
      return assertNever(anchor)
  }
}

function cloneWorkbenchFeedback(
  feedback: ImageReviewWorkbenchFeedback | null,
): ImageReviewWorkbenchFeedback | null {
  return feedback === null
    ? null
    : {
        ...feedback,
        targetKey: feedback.targetKey === null ? null : { ...feedback.targetKey },
        anchor: cloneReviewAnchor(feedback.anchor),
      }
}

function captureEditorOwnership(
  adapter: ImageReviewWorkbenchAdapter,
  entityId: string,
  preparation: ImageReviewPreparation | null,
  entityRef: { current: string | null },
  preparationKeyRef: { current: string | null },
  assetVersionIdRef: { current: string | null },
) {
  entityRef.current = entityId
  preparationKeyRef.current = preparation?.key ?? adapter.preparationKey(entityId)
  assetVersionIdRef.current = preparation?.prepared?.asset.id ?? null
}

function clearEditorOwnership(
  entityRef: { current: string | null },
  preparationKeyRef: { current: string | null },
  assetVersionIdRef: { current: string | null },
  itemRef: { current: ImageReviewWorkbenchFeedback | null },
) {
  entityRef.current = null
  preparationKeyRef.current = null
  assetVersionIdRef.current = null
  itemRef.current = null
}

function preparedImageForEntity(entityId: string, preparation: ImageReviewPreparation | null) {
  const prepared = preparation?.prepared ?? null
  const preview = prepared?.preview ?? null
  return prepared?.asset.sourceEntityId === entityId &&
    preview?.assetVersionId === prepared.asset.id
    ? {
        entityId,
        assetVersionId: prepared.asset.id,
        url: preview.url,
        width: preview.width,
        height: preview.height,
      }
    : null
}

function workbenchStatusMessage(
  entityId: string,
  view: ImageReviewWorkbenchView,
  retained: { entityId: string; message: string } | null,
) {
  if (view.statusMessage !== null) return view.statusMessage
  if (retained?.entityId !== entityId) return null
  return view.readOnlyReason === null || retained.message === '已保存，待确认'
    ? retained.message
    : null
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
  if (
    typeof error === 'object' &&
    error !== null &&
    'message' in error &&
    typeof error.message === 'string' &&
    error.message.length > 0
  ) {
    return error.message
  }
  return '意见尚未保存，请重试。'
}

function assertNever(value: never): never {
  throw new Error(`Unsupported review anchor: ${JSON.stringify(value)}`)
}
