import type { ReviewAnchor } from '../../api/types'
import type { AnnotationTool, MarkupTool } from './annotationToolRegistry'

export type { AnnotationTool } from './annotationToolRegistry'

type AnnotationOperation = 'text' | 'geometry'

export type AnnotationEditorPhase =
  | { status: 'idle' }
  | { status: 'drawing'; draftAnchor: ReviewAnchor; sourceItemId?: string }
  | {
      status: 'editing'
      draftAnchor: ReviewAnchor
      text: string
      sourceItemId: string | null
      operation?: AnnotationOperation
    }
  | {
      status: 'saving'
      draftAnchor: ReviewAnchor
      text: string
      sourceItemId: string | null
      operation?: AnnotationOperation
    }
  | {
      status: 'save_error'
      draftAnchor: ReviewAnchor
      text: string
      sourceItemId: string | null
      operation?: AnnotationOperation
      message: string
    }

export interface AnnotationInteractionState {
  activeTool: AnnotationTool
  temporarilyPanning: boolean
  selectedItemId: string | null
  phase: AnnotationEditorPhase
}

export type AnnotationEditorState = AnnotationInteractionState

export type AnnotationEditorAction =
  | { type: 'set_tool'; tool: AnnotationTool }
  | { type: 'temporary_pan_start' }
  | { type: 'temporary_pan_end' }
  | { type: 'begin_drawing'; anchor: ReviewAnchor; itemId?: string }
  | { type: 'update_draft_anchor'; anchor: ReviewAnchor }
  | { type: 'complete_drawing' }
  | { type: 'begin_annotation'; anchor: ReviewAnchor }
  | {
      type: 'begin_edit'
      itemId: string
      anchor: ReviewAnchor
      text: string
      operation?: AnnotationOperation
    }
  | { type: 'update_text'; text: string }
  | { type: 'request_save' }
  | { type: 'save_failed'; message: string }
  | { type: 'save_succeeded'; itemId: string }
  | { type: 'select_feedback'; itemId: string | null }
  | { type: 'cancel_draft' }
  | { type: 'escape' }

export function initialAnnotationEditorState(): AnnotationInteractionState {
  return {
    activeTool: 'browse',
    temporarilyPanning: false,
    selectedItemId: null,
    phase: idlePhase(),
  }
}

export function annotationEditorReducer(
  state: AnnotationInteractionState,
  action: AnnotationEditorAction,
): AnnotationInteractionState {
  switch (action.type) {
    case 'set_tool':
      return state.phase.status === 'idle'
        ? { ...state, activeTool: action.tool, temporarilyPanning: false }
        : state
    case 'temporary_pan_start':
      return { ...state, temporarilyPanning: true }
    case 'temporary_pan_end':
      return { ...state, temporarilyPanning: false }
    case 'begin_drawing':
      if (
        state.phase.status !== 'idle' ||
        state.activeTool === 'browse' ||
        !toolOwnsAnchor(state.activeTool, action.anchor)
      ) {
        return state
      }
      return {
        ...state,
        temporarilyPanning: false,
        phase: {
          status: 'drawing',
          draftAnchor: action.anchor,
          sourceItemId: action.itemId,
        },
      }
    case 'update_draft_anchor':
      return state.phase.status === 'drawing' ||
        state.phase.status === 'editing' ||
        state.phase.status === 'save_error'
        ? { ...state, phase: { ...state.phase, draftAnchor: action.anchor } }
        : state
    case 'complete_drawing':
      if (state.phase.status !== 'drawing') return state
      return {
        ...state,
        phase: isValidAnnotationAnchor(state.phase.draftAnchor)
          ? {
              status: 'editing',
              draftAnchor: state.phase.draftAnchor,
              text: '',
              sourceItemId: null,
            }
          : idlePhase(),
      }
    case 'begin_annotation':
      if (state.phase.status !== 'idle' || !isValidAnnotationAnchor(action.anchor)) return state
      return {
        ...state,
        temporarilyPanning: false,
        phase: {
          status: 'editing',
          draftAnchor: action.anchor,
          text: '',
          sourceItemId: null,
        },
      }
    case 'begin_edit':
      if (
        state.phase.status !== 'idle' &&
        !(state.phase.status === 'drawing' && state.phase.sourceItemId === action.itemId)
      ) {
        return state
      }
      if (!isValidAnnotationAnchor(action.anchor)) return state
      return {
        ...state,
        temporarilyPanning: false,
        selectedItemId: action.itemId,
        phase: {
          status: 'editing',
          draftAnchor: action.anchor,
          text: action.text,
          sourceItemId: action.itemId,
          operation: action.operation,
        },
      }
    case 'update_text':
      if (state.phase.status === 'editing') {
        return { ...state, phase: { ...state.phase, text: action.text } }
      }
      if (state.phase.status === 'save_error') {
        const { message: _message, ...editing } = state.phase
        return { ...state, phase: { ...editing, status: 'editing', text: action.text } }
      }
      return state
    case 'request_save':
      if (
        (state.phase.status !== 'editing' && state.phase.status !== 'save_error') ||
        state.phase.text.trim().length === 0 ||
        !isValidAnnotationAnchor(state.phase.draftAnchor)
      ) {
        return state
      }
      if (state.phase.status === 'save_error') {
        const { message: _message, ...saving } = state.phase
        return { ...state, phase: { ...saving, status: 'saving' } }
      }
      return { ...state, phase: { ...state.phase, status: 'saving' } }
    case 'save_failed':
      return state.phase.status === 'saving'
        ? { ...state, phase: { ...state.phase, status: 'save_error', message: action.message } }
        : state
    case 'save_succeeded':
      return state.phase.status === 'saving'
        ? { ...state, selectedItemId: action.itemId, phase: idlePhase() }
        : state
    case 'select_feedback':
      return state.phase.status === 'idle' ? { ...state, selectedItemId: action.itemId } : state
    case 'cancel_draft':
      return state.phase.status !== 'saving' && hasUnsavedAnnotation(state)
        ? { ...state, phase: idlePhase() }
        : state
    case 'escape':
      if (state.phase.status !== 'idle') return { ...state, phase: idlePhase() }
      return { ...state, activeTool: 'browse', temporarilyPanning: false }
  }
}

export function hasUnsavedAnnotation(state: AnnotationInteractionState): boolean {
  return state.phase.status !== 'idle'
}

export function isValidAnnotationAnchor(anchor: ReviewAnchor): boolean {
  switch (anchor.kind) {
    case 'asset':
      return true
    case 'image_point':
      return finiteNormalized(anchor.x) && finiteNormalized(anchor.y)
    case 'image_arrow':
      return (
        finiteNormalized(anchor.tail.x) &&
        finiteNormalized(anchor.tail.y) &&
        finiteNormalized(anchor.head.x) &&
        finiteNormalized(anchor.head.y) &&
        (anchor.tail.x !== anchor.head.x || anchor.tail.y !== anchor.head.y)
      )
    case 'image_rect':
    case 'image_ellipse':
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
    default:
      return assertNever(anchor)
  }
}

function idlePhase(): Extract<AnnotationEditorPhase, { status: 'idle' }> {
  return { status: 'idle' }
}

function toolOwnsAnchor(tool: MarkupTool, anchor: ReviewAnchor): boolean {
  switch (tool) {
    case 'point':
      return anchor.kind === 'image_point'
    case 'arrow':
      return anchor.kind === 'image_arrow'
    case 'brush':
      return anchor.kind === 'image_stroke'
    case 'rectangle':
      return anchor.kind === 'image_rect'
    case 'ellipse':
      return anchor.kind === 'image_ellipse'
  }
}

function finiteNormalized(value: number): boolean {
  return Number.isFinite(value) && value >= 0 && value <= 1
}

function assertNever(value: never): never {
  throw new Error(`Unsupported annotation anchor: ${JSON.stringify(value)}`)
}
