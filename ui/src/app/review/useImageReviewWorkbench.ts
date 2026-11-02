import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from 'react'
import type { ReviewAnchor, ReviewAnnotationStyle } from '../../api/types'
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
  annotationToolForAnchor,
  defaultAnnotationStyle,
  normalizeAnnotationStyle,
  styleForAnchor,
} from './annotationStyle'
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

interface AnnotationValue {
  text: string
  anchor: ReviewAnchor
  style: ReviewAnnotationStyle | null
}

type AnnotationHistoryEntry =
  | { kind: 'add'; itemId: string; after: AnnotationValue }
  | { kind: 'update'; itemId: string; before: AnnotationValue; after: AnnotationValue }

interface AnnotationHistory {
  undo: AnnotationHistoryEntry[]
  redo: AnnotationHistoryEntry[]
}

const EMPTY_ANNOTATION_HISTORY: AnnotationHistory = { undo: [], redo: [] }

export interface UseImageReviewWorkbenchOptions {
  adapter: ImageReviewWorkbenchAdapter
  entityId: string
  onLeave?: (intent: ReviewLeaveIntent) => void | Promise<void>
}

export interface ImageReviewWorkbenchController {
  protocol: ImageReviewWorkbenchAdapter['protocol']
  tool: AnnotationTool
  annotationStyle: ReviewAnnotationStyle
  styleBusy: boolean
  canUndo: boolean
  canRedo: boolean
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
  setAnnotationStyle(style: ReviewAnnotationStyle): void
  undo(): void
  redo(): void
  setTemporaryPan(active: boolean): void
  beginAnnotation(anchor: ReviewAnchor, style?: ReviewAnnotationStyle): void
  beginDrawing(anchor: ReviewAnchor, itemId?: string, style?: ReviewAnnotationStyle): boolean
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
  const [styleBusy, setStyleBusy] = useState(false)
  const [leaveConfirmation, setLeaveConfirmation] = useState<ReviewLeaveIntent | null>(null)
  const [pendingMutationId, setPendingMutationId] = useState<string | null>(null)
  const [history, setHistory] = useState<AnnotationHistory>(EMPTY_ANNOTATION_HISTORY)
  const [historyBusy, setHistoryBusy] = useState(false)
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
  const historyRef = useRef(history)
  const historyBusyRef = useRef(false)

  adapterRef.current = adapter
  editorRef.current = editor
  onLeaveRef.current = onLeave
  // Synchronous ownership also guards back-to-back pointer/leave/save commands
  // before React has committed the next render.
  const dispatch = useCallback((action: AnnotationEditorAction) => {
    editorRef.current = annotationEditorReducer(editorRef.current, action)
    reduce(action)
  }, [])

  const setHistoryState = useCallback((next: AnnotationHistory) => {
    historyRef.current = next
    setHistory(next)
  }, [])

  const recordHistory = useCallback(
    (entry: AnnotationHistoryEntry) => {
      const current = historyRef.current
      setHistoryState({ undo: [...current.undo, entry], redo: [] })
    },
    [setHistoryState],
  )

  const clearRedo = useCallback(() => {
    if (historyRef.current.redo.length === 0) return
    setHistoryState({ ...historyRef.current, redo: [] })
  }, [setHistoryState])

  const preparationKey = adapter.preparationKey(entityId)
  const viewKey = adapter.viewKey(entityId)
  const preparedAssetVersionId = preparation?.prepared?.asset.id ?? null
  useEffect(() => {
    setHistoryState(EMPTY_ANNOTATION_HISTORY)
  }, [setHistoryState, entityId, preparationKey, preparedAssetVersionId])

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
      style: cloneAnnotationStyle(
        draft.style ??
          defaultAnnotationStyle(annotationToolForAnchor(draft.draftAnchor) ?? 'brush'),
      ),
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
    const beforeValue = item === null ? null : annotationValue(item)
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
        style: frozen.style,
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
      if (adapterRef.current.protocol === 'continuous') {
        if (beforeValue === null) {
          recordHistory({ kind: 'add', itemId: saved.itemId, after: annotationValue(saved) })
        } else {
          const afterValue = {
            text: frozen.text,
            anchor: frozen.anchor,
            style: frozen.style,
          }
          if (!sameAnnotationValue(beforeValue, afterValue)) {
            recordHistory({
              kind: 'update',
              itemId: saved.itemId,
              before: beforeValue,
              after: afterValue,
            })
          }
        }
      }
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
  }, [dispatch, entityId, recordHistory, workbenchView.feedback])

  const beginDrawing = useCallback(
    (anchor: ReviewAnchor, itemId?: string, style?: ReviewAnnotationStyle) => {
      if (hasUnsavedAnnotation(editorRef.current) || workbenchView.readOnlyReason !== null)
        return false
      dispatch({
        type: 'begin_drawing',
        anchor: cloneReviewAnchor(anchor),
        itemId,
        ...(style === undefined ? {} : { style: cloneAnnotationStyle(style) }),
      })
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
        style: styleForAnchor(feedback.anchor, feedback.style) ?? undefined,
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

  const replaceFeedbackStyle = useCallback(
    async (itemId: string, style: ReviewAnnotationStyle) => {
      if (adapterRef.current.protocol !== 'continuous') return
      if (hasUnsavedAnnotation(editorRef.current) || workbenchView.readOnlyReason !== null) return
      if (styleBusy) return
      const feedback = workbenchView.feedback.find((item) => item.itemId === itemId)
      if (
        feedback === undefined ||
        !feedback.anchor.kind.startsWith('image_') ||
        (feedback.style != null && sameStyle(feedback.style, style))
      )
        return
      const beforeValue = annotationValue(feedback)
      setStyleBusy(true)
      try {
        const installed = await adapterRef.current.saveFeedback({
          entityId,
          item: feedback,
          operation: 'geometry',
          text: feedback.text,
          anchor: cloneReviewAnchor(feedback.anchor),
          style: cloneAnnotationStyle(style),
          preparation: preparationRef.current ?? {
            key: adapterRef.current.preparationKey(entityId),
            prepared: null,
          },
        })
        setWorkbenchView(installed)
        if (installed.statusMessage !== null)
          statusMessageRef.current = { entityId, message: installed.statusMessage }
        const updated = installed.feedback.find((item) => item.itemId === itemId)
        if (updated !== undefined) {
          recordHistory({
            kind: 'update',
            itemId,
            before: beforeValue,
            after: annotationValue(updated),
          })
        }
      } catch (cause) {
        statusMessageRef.current = { entityId, message: reviewErrorMessage(cause) }
      } finally {
        setStyleBusy(false)
      }
    },
    [entityId, recordHistory, styleBusy, workbenchView.feedback, workbenchView.readOnlyReason],
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
  const selectedFeedback = workbenchView.feedback.find(
    (item) => item.itemId === editor.selectedItemId,
  )
  const fallbackStyle = defaultAnnotationStyle(
    editor.activeTool === 'browse' ? 'rectangle' : editor.activeTool,
  )
  const annotationStyle =
    editor.phase.status !== 'idle'
      ? (editor.phase.style ?? editor.style ?? fallbackStyle)
      : selectedFeedback === undefined
        ? (editor.style ?? fallbackStyle)
        : ((styleBusy ? editor.style : null) ??
          styleForAnchor(selectedFeedback.anchor, selectedFeedback.style) ??
          editor.style ??
          fallbackStyle)
  const setAnnotationStyle = useCallback(
    (style: ReviewAnnotationStyle) => {
      if (editorRef.current.phase.status === 'saving') return
      const next = normalizeAnnotationStyle(style)
      dispatch({ type: 'set_style', style: next })
      const selectedItemId = editorRef.current.selectedItemId
      const selected = workbenchView.feedback.find((item) => item.itemId === selectedItemId)
      if (
        editorRef.current.phase.status === 'idle' &&
        selectedItemId !== null &&
        selected?.anchor.kind.startsWith('image_') &&
        adapterRef.current.protocol === 'continuous'
      ) {
        void replaceFeedbackStyle(selectedItemId, next)
      }
    },
    [dispatch, replaceFeedbackStyle],
  )

  const applyHistoryValue = useCallback(
    async (itemId: string, value: AnnotationValue) => {
      const feedback = workbenchView.feedback.find((item) => item.itemId === itemId)
      if (feedback === undefined) return null
      const installed = await adapterRef.current.saveFeedback({
        entityId,
        item: feedback,
        operation: 'geometry',
        text: value.text,
        anchor: cloneReviewAnchor(value.anchor),
        style: value.style === null ? null : cloneAnnotationStyle(value.style),
        preparation: preparationRef.current ?? {
          key: adapterRef.current.preparationKey(entityId),
          prepared: null,
        },
      })
      setWorkbenchView(installed)
      if (installed.statusMessage !== null)
        statusMessageRef.current = { entityId, message: installed.statusMessage }
      return installed
    },
    [entityId, workbenchView.feedback],
  )

  const runHistory = useCallback(
    async (direction: 'undo' | 'redo') => {
      if (historyBusyRef.current || adapterRef.current.protocol !== 'continuous') return
      if (workbenchView.readOnlyReason !== null || hasUnsavedAnnotation(editorRef.current)) return
      if (editorRef.current.phase.status !== 'idle') return
      const entry =
        direction === 'undo' ? historyRef.current.undo.at(-1) : historyRef.current.redo.at(-1)
      if (entry === undefined) return
      historyBusyRef.current = true
      setHistoryBusy(true)
      statusMessageRef.current = null
      const preparation = preparationRef.current ?? {
        key: adapterRef.current.preparationKey(entityId),
        prepared: null,
      }
      const dropEntry = () => {
        const current = historyRef.current
        setHistoryState(
          direction === 'undo'
            ? { undo: withoutHistoryEntry(current.undo, entry), redo: current.redo }
            : { undo: current.undo, redo: withoutHistoryEntry(current.redo, entry) },
        )
      }
      try {
        if (direction === 'undo') {
          if (entry.kind === 'add') {
            const feedback = workbenchView.feedback.find((item) => item.itemId === entry.itemId)
            if (feedback === undefined) {
              dropEntry()
              return
            }
            const installed = await adapterRef.current.deleteFeedback({
              entityId,
              item: feedback,
              preparation,
            })
            setWorkbenchView(installed)
            if (installed.statusMessage !== null)
              statusMessageRef.current = { entityId, message: installed.statusMessage }
            if (editorRef.current.selectedItemId === entry.itemId)
              dispatch({ type: 'select_feedback', itemId: null })
          } else {
            const installed = await applyHistoryValue(entry.itemId, entry.before)
            if (installed === null) {
              dropEntry()
              return
            }
          }
          const current = historyRef.current
          setHistoryState({
            undo: withoutHistoryEntry(current.undo, entry),
            redo: [...current.redo, entry],
          })
          return
        }
        if (entry.kind === 'update') {
          const installed = await applyHistoryValue(entry.itemId, entry.after)
          if (installed === null) {
            dropEntry()
            return
          }
          const current = historyRef.current
          setHistoryState({
            undo: [...current.undo, entry],
            redo: withoutHistoryEntry(current.redo, entry),
          })
          return
        }
        const existingIds = new Set(workbenchView.feedback.map((item) => item.itemId))
        const installed = await adapterRef.current.saveFeedback({
          entityId,
          item: null,
          operation: 'text',
          text: entry.after.text,
          anchor: cloneReviewAnchor(entry.after.anchor),
          style: entry.after.style === null ? null : cloneAnnotationStyle(entry.after.style),
          preparation,
        })
        setWorkbenchView(installed)
        if (installed.statusMessage !== null)
          statusMessageRef.current = { entityId, message: installed.statusMessage }
        const added = installed.feedback.find((item) => !existingIds.has(item.itemId))
        if (added === undefined) {
          dropEntry()
          return
        }
        const remap = (candidate: AnnotationHistoryEntry): AnnotationHistoryEntry =>
          candidate.itemId === entry.itemId ? { ...candidate, itemId: added.itemId } : candidate
        const current = historyRef.current
        setHistoryState({
          undo: [...current.undo.map(remap), { ...entry, itemId: added.itemId }],
          redo: withoutHistoryEntry(current.redo, entry).map(remap),
        })
      } catch (cause) {
        statusMessageRef.current = { entityId, message: reviewErrorMessage(cause) }
      } finally {
        historyBusyRef.current = false
        setHistoryBusy(false)
      }
    },
    [
      applyHistoryValue,
      dispatch,
      entityId,
      setHistoryState,
      workbenchView.feedback,
      workbenchView.readOnlyReason,
    ],
  )

  const historyAvailable =
    adapter.protocol === 'continuous' &&
    editor.phase.status === 'idle' &&
    workbenchView.readOnlyReason === null &&
    !styleBusy &&
    !historyBusy

  return {
    protocol: adapter.protocol,
    tool: editor.activeTool,
    annotationStyle,
    styleBusy,
    canUndo: historyAvailable && history.undo.length > 0,
    canRedo: historyAvailable && history.redo.length > 0,
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
    setAnnotationStyle,
    undo() {
      void runHistory('undo')
    },
    redo() {
      void runHistory('redo')
    },
    setTemporaryPan(active) {
      dispatch({ type: active ? 'temporary_pan_start' : 'temporary_pan_end' })
    },
    beginAnnotation(anchor, style) {
      if (hasUnsavedAnnotation(editorRef.current) || workbenchView.readOnlyReason !== null) return
      dispatch({
        type: 'begin_annotation',
        anchor,
        ...(style === undefined ? {} : { style: cloneAnnotationStyle(style) }),
      })
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
          style: styleForAnchor(feedback.anchor, feedback.style) ?? undefined,
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
      const tool = annotationToolForAnchor(feedback.anchor, feedback.style)
      if (tool === null) return
      dispatch({ type: 'set_tool', tool })
      setRedrawItemId(itemId)
    },
    stageFeedbackAnchor(itemId, anchor) {
      const state = editorRef.current.phase
      if (state.status === 'idle') {
        const feedback = workbenchView.feedback.find((item) => item.itemId === itemId)
        const tool =
          feedback === undefined ? null : annotationToolForAnchor(feedback.anchor, feedback.style)
        if (tool !== null) dispatch({ type: 'set_tool', tool })
        return beginDrawing(anchor, itemId, feedback?.style ?? undefined)
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
          ).then(() => clearRedo())
    },
    restoreDeletedFeedback(itemId) {
      return applySavedMutation(() =>
        adapterRef.current.restoreDeletedFeedback({
          entityId,
          itemId,
          preparation: preparationRef.current,
        }),
      ).then(() => clearRedo())
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

function annotationValue(feedback: ImageReviewWorkbenchFeedback): AnnotationValue {
  return {
    text: feedback.text,
    anchor: cloneReviewAnchor(feedback.anchor),
    style: feedback.style == null ? null : cloneAnnotationStyle(feedback.style),
  }
}

function sameAnnotationValue(left: AnnotationValue, right: AnnotationValue): boolean {
  return (
    left.text === right.text &&
    JSON.stringify(left.anchor) === JSON.stringify(right.anchor) &&
    (left.style === null || right.style === null
      ? left.style === right.style
      : sameStyle(left.style, right.style))
  )
}

function withoutHistoryEntry(
  entries: ReadonlyArray<AnnotationHistoryEntry>,
  entry: AnnotationHistoryEntry,
): AnnotationHistoryEntry[] {
  return entries.at(-1) === entry
    ? entries.slice(0, -1)
    : entries.filter((candidate) => candidate !== entry)
}

function cloneReviewAnchor(anchor: ReviewAnchor): ReviewAnchor {
  switch (anchor.kind) {
    case 'asset':
      return { kind: 'asset' }
    case 'image_point':
    case 'image_text':
      return { ...anchor }
    case 'image_arrow':
    case 'image_line':
      return { kind: anchor.kind, tail: { ...anchor.tail }, head: { ...anchor.head } }
    case 'image_stroke':
    case 'image_polygon':
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

function cloneAnnotationStyle(style: ReviewAnnotationStyle): ReviewAnnotationStyle {
  return { ...style }
}

function sameStyle(left: ReviewAnnotationStyle, right: ReviewAnnotationStyle): boolean {
  return (
    left.tool === right.tool &&
    left.color === right.color &&
    left.width === right.width &&
    left.opacityPercent === right.opacityPercent &&
    left.dashed === right.dashed &&
    left.textSize === right.textSize
  )
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
        style:
          feedback.style === null || feedback.style === undefined
            ? null
            : cloneAnnotationStyle(feedback.style),
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
