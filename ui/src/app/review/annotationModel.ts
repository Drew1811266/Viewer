import type { ReviewAnchor } from '../../api/types'

export type AnnotationTool = 'browse' | 'brush' | 'rectangle'

interface AnnotationEditorInteraction {
  tool: AnnotationTool
  temporarilyPanning: boolean
  operation?: 'text' | 'geometry'
}

export type AnnotationEditorState =
  | (AnnotationEditorInteraction & {
      status: 'idle'
      selectedFeedbackId: string | null
    })
  | (AnnotationEditorInteraction & {
      status: 'drawing'
      tool: 'brush' | 'rectangle'
      draftAnchor: ReviewAnchor
      sourceFeedbackId?: string
      selectedFeedbackId: string | null
    })
  | (AnnotationEditorInteraction & {
      status: 'editing'
      draftAnchor: ReviewAnchor
      text: string
      sourceFeedbackId: string | null
      selectedFeedbackId: string | null
    })
  | (AnnotationEditorInteraction & {
      status: 'saving'
      draftAnchor: ReviewAnchor
      text: string
      sourceFeedbackId: string | null
      selectedFeedbackId: string | null
    })
  | (AnnotationEditorInteraction & {
      status: 'save_error'
      draftAnchor: ReviewAnchor
      text: string
      sourceFeedbackId: string | null
      selectedFeedbackId: string | null
      message: string
    })

export type AnnotationEditorAction =
  | { type: 'set_tool'; tool: AnnotationTool }
  | { type: 'temporary_pan_start' }
  | { type: 'temporary_pan_end' }
  | { type: 'begin_drawing'; anchor: ReviewAnchor; feedbackId?: string }
  | { type: 'update_draft_anchor'; anchor: ReviewAnchor }
  | { type: 'complete_drawing' }
  | { type: 'begin_annotation'; anchor: ReviewAnchor }
  | {
      type: 'begin_edit'
      feedbackId: string
      anchor: ReviewAnchor
      text: string
      operation?: 'text' | 'geometry'
    }
  | { type: 'update_text'; text: string }
  | { type: 'request_save' }
  | { type: 'save_failed'; message: string }
  | { type: 'save_succeeded'; feedbackId: string }
  | { type: 'select_feedback'; feedbackId: string | null }
  | { type: 'cancel_draft' }
  | { type: 'escape' }

export function initialAnnotationEditorState(): AnnotationEditorState {
  return {
    status: 'idle',
    tool: 'browse',
    temporarilyPanning: false,
    selectedFeedbackId: null,
  }
}

export function annotationEditorReducer(
  state: AnnotationEditorState,
  action: AnnotationEditorAction,
): AnnotationEditorState {
  switch (action.type) {
    case 'set_tool':
      return state.status === 'drawing' || state.status === 'saving'
        ? state
        : { ...state, tool: action.tool, temporarilyPanning: false }
    case 'temporary_pan_start':
      return { ...state, temporarilyPanning: true }
    case 'temporary_pan_end':
      return { ...state, temporarilyPanning: false }
    case 'begin_drawing':
      if (state.status !== 'idle') return state
      if (state.tool !== 'brush' && state.tool !== 'rectangle') return state
      return {
        status: 'drawing',
        tool: state.tool,
        temporarilyPanning: false,
        draftAnchor: action.anchor,
        sourceFeedbackId: action.feedbackId,
        selectedFeedbackId: selectedFeedbackId(state),
      }
    case 'update_draft_anchor':
      return state.status === 'drawing' ||
        state.status === 'editing' ||
        state.status === 'save_error'
        ? { ...state, draftAnchor: action.anchor }
        : state
    case 'complete_drawing':
      if (state.status !== 'drawing') return state
      return isValidAnnotationAnchor(state.draftAnchor)
        ? {
            status: 'editing',
            tool: state.tool,
            temporarilyPanning: false,
            draftAnchor: state.draftAnchor,
            text: '',
            sourceFeedbackId: null,
            selectedFeedbackId: state.selectedFeedbackId,
          }
        : idleState(state.selectedFeedbackId)
    case 'begin_annotation':
      if (state.status !== 'idle' || !isValidAnnotationAnchor(action.anchor)) return state
      return {
        status: 'editing',
        tool: state.tool,
        temporarilyPanning: false,
        draftAnchor: action.anchor,
        text: '',
        sourceFeedbackId: null,
        selectedFeedbackId: selectedFeedbackId(state),
      }
    case 'begin_edit':
      if (
        state.status !== 'idle' &&
        !(state.status === 'drawing' && state.sourceFeedbackId === action.feedbackId)
      )
        return state
      if (!isValidAnnotationAnchor(action.anchor)) return state
      return {
        status: 'editing',
        tool: state.tool,
        temporarilyPanning: false,
        draftAnchor: action.anchor,
        text: action.text,
        sourceFeedbackId: action.feedbackId,
        operation: action.operation,
        selectedFeedbackId: action.feedbackId,
      }
    case 'update_text':
      if (state.status === 'editing') return { ...state, text: action.text }
      if (state.status === 'save_error') {
        const { message: _message, ...editing } = state
        return { ...editing, status: 'editing', text: action.text }
      }
      return state
    case 'request_save':
      if (
        (state.status !== 'editing' && state.status !== 'save_error') ||
        state.text.trim().length === 0 ||
        !isValidAnnotationAnchor(state.draftAnchor)
      ) {
        return state
      }
      if (state.status === 'save_error') {
        const { message: _message, ...saving } = state
        return { ...saving, status: 'saving' }
      }
      return { ...state, status: 'saving' }
    case 'save_failed':
      return state.status === 'saving'
        ? { ...state, status: 'save_error', message: action.message }
        : state
    case 'save_succeeded':
      return state.status === 'saving' ? idleState(action.feedbackId) : state
    case 'select_feedback':
      return state.status === 'idle' ? { ...state, selectedFeedbackId: action.feedbackId } : state
    case 'cancel_draft':
      return state.status !== 'saving' && hasUnsavedAnnotation(state)
        ? idleState(selectedFeedbackId(state))
        : state
    case 'escape':
      if (state.status !== 'idle') return idleState(selectedFeedbackId(state))
      return { ...state, tool: 'browse', temporarilyPanning: false }
  }
}

export function hasUnsavedAnnotation(state: AnnotationEditorState): boolean {
  return (
    state.status === 'drawing' ||
    state.status === 'editing' ||
    state.status === 'saving' ||
    state.status === 'save_error'
  )
}

export function isValidAnnotationAnchor(anchor: ReviewAnchor): boolean {
  switch (anchor.kind) {
    case 'asset':
      return true
    case 'image_rect':
      return (
        finiteNormalized(anchor.x) &&
        finiteNormalized(anchor.y) &&
        Number.isFinite(anchor.width) &&
        Number.isFinite(anchor.height) &&
        anchor.width > 0 &&
        anchor.height > 0 &&
        anchor.x + anchor.width <= 1 &&
        anchor.y + anchor.height <= 1
      )
    case 'image_stroke': {
      if (anchor.points.length < 2 || anchor.points.length > 2_048) return false
      let minimumX = 1
      let minimumY = 1
      let maximumX = 0
      let maximumY = 0
      for (const point of anchor.points) {
        if (!finiteNormalized(point.x) || !finiteNormalized(point.y)) return false
        minimumX = Math.min(minimumX, point.x)
        minimumY = Math.min(minimumY, point.y)
        maximumX = Math.max(maximumX, point.x)
        maximumY = Math.max(maximumY, point.y)
      }
      return maximumX > minimumX && maximumY > minimumY
    }
    case 'video_point':
      return Number.isSafeInteger(anchor.positionUs) && anchor.positionUs >= 0
    case 'video_range':
      return (
        Number.isSafeInteger(anchor.startUs) &&
        Number.isSafeInteger(anchor.endUs) &&
        anchor.startUs >= 0 &&
        anchor.endUs > anchor.startUs
      )
  }
}

function idleState(selectedFeedbackId: string | null): AnnotationEditorState {
  return {
    status: 'idle',
    tool: 'browse',
    temporarilyPanning: false,
    selectedFeedbackId,
  }
}

function selectedFeedbackId(state: AnnotationEditorState): string | null {
  return state.selectedFeedbackId
}

function finiteNormalized(value: number): boolean {
  return Number.isFinite(value) && value >= 0 && value <= 1
}
