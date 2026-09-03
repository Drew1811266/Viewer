import { describe, expect, it } from 'vitest'
import type { ReviewAnchor } from '../../api/types'
import {
  annotationEditorReducer,
  hasUnsavedAnnotation,
  initialAnnotationEditorState,
  isValidAnnotationAnchor,
} from './annotationModel'

const RECT: ReviewAnchor = { kind: 'image_rect', x: 0.1, y: 0.2, width: 0.3, height: 0.4 }
const ELLIPSE: ReviewAnchor = {
  kind: 'image_ellipse',
  x: 0.2,
  y: 0.2,
  width: 0.3,
  height: 0.4,
}

describe('annotation editor model', () => {
  it('validates every extended image anchor without viewport-dependent thresholds', () => {
    expect(isValidAnnotationAnchor({ kind: 'image_point', x: 0, y: 1 })).toBe(true)
    expect(
      isValidAnnotationAnchor({
        kind: 'image_arrow',
        tail: { x: 0.1, y: 0.2 },
        head: { x: 0.8, y: 0.7 },
      }),
    ).toBe(true)
    expect(
      isValidAnnotationAnchor({
        kind: 'image_arrow',
        tail: { x: 0.1, y: 0.2 },
        head: { x: 0.1, y: 0.2 },
      }),
    ).toBe(false)
    expect(
      isValidAnnotationAnchor({
        kind: 'image_ellipse',
        x: 0.2,
        y: 0.3,
        width: 0.4,
        height: 0.5,
      }),
    ).toBe(true)
  })

  it('switches tools and treats Space as temporary pan without losing the selected tool', () => {
    let state = annotationEditorReducer(initialAnnotationEditorState(), {
      type: 'set_tool',
      tool: 'brush',
    })
    state = annotationEditorReducer(state, { type: 'temporary_pan_start' })
    expect(state).toEqual(
      expect.objectContaining({ activeTool: 'brush', temporarilyPanning: true }),
    )
    state = annotationEditorReducer(state, { type: 'temporary_pan_end' })
    expect(state).toEqual(
      expect.objectContaining({ activeTool: 'brush', temporarilyPanning: false }),
    )
  })

  it('cancels an empty pointer gesture and opens the editor for a valid mark', () => {
    const drawing = annotationEditorReducer(
      annotationEditorReducer(initialAnnotationEditorState(), {
        type: 'set_tool',
        tool: 'rectangle',
      }),
      {
        type: 'begin_drawing',
        anchor: { kind: 'image_rect', x: 0.2, y: 0.2, width: 0, height: 0 },
      },
    )

    expect(annotationEditorReducer(drawing, { type: 'complete_drawing' }).phase.status).toBe('idle')
    const validDrawing = annotationEditorReducer(drawing, {
      type: 'update_draft_anchor',
      anchor: RECT,
    })
    expect(annotationEditorReducer(validDrawing, { type: 'complete_drawing' }).phase).toEqual(
      expect.objectContaining({
        status: 'editing',
        draftAnchor: RECT,
        text: '',
        sourceItemId: null,
      }),
    )
  })

  it('keeps blank saves unsaved and preserves the draft across a failed save', () => {
    let state = annotationEditorReducer(initialAnnotationEditorState(), {
      type: 'begin_annotation',
      anchor: RECT,
    })
    expect(annotationEditorReducer(state, { type: 'request_save' }).phase.status).toBe('editing')

    state = annotationEditorReducer(state, { type: 'update_text', text: '  修正袖口  ' })
    state = annotationEditorReducer(state, { type: 'request_save' })
    expect(state.phase.status).toBe('saving')
    state = annotationEditorReducer(state, { type: 'save_failed', message: '写入失败' })
    expect(state.phase).toEqual(
      expect.objectContaining({
        status: 'save_error',
        draftAnchor: RECT,
        text: '  修正袖口  ',
        message: '写入失败',
      }),
    )
    expect(hasUnsavedAnnotation(state)).toBe(true)
  })

  it('preserves the active tool after save and uses two-stage Escape', () => {
    const saving = annotationEditorReducer(
      annotationEditorReducer(
        annotationEditorReducer(
          annotationEditorReducer(initialAnnotationEditorState(), {
            type: 'set_tool',
            tool: 'ellipse',
          }),
          { type: 'begin_drawing', anchor: ELLIPSE },
        ),
        { type: 'complete_drawing' },
      ),
      { type: 'update_text', text: '调整脸部轮廓' },
    )
    const requested = annotationEditorReducer(saving, { type: 'request_save' })
    const saved = annotationEditorReducer(requested, {
      type: 'save_succeeded',
      itemId: 'feedback-1',
    })
    expect(saved.activeTool).toBe('ellipse')
    expect(saved.phase.status).toBe('idle')
    expect(saved.selectedItemId).toBe('feedback-1')

    const escapedDraft = annotationEditorReducer(
      annotationEditorReducer(saved, {
        type: 'begin_annotation',
        anchor: RECT,
      }),
      { type: 'escape' },
    )
    expect(escapedDraft).toEqual(
      expect.objectContaining({
        activeTool: 'ellipse',
        selectedItemId: 'feedback-1',
        phase: { status: 'idle' },
      }),
    )
    expect(annotationEditorReducer(escapedDraft, { type: 'escape' })).toEqual(
      expect.objectContaining({ activeTool: 'browse', phase: { status: 'idle' } }),
    )
  })

  it('refuses to switch tools while a draft contains geometry or text', () => {
    const dirty = annotationEditorReducer(
      annotationEditorReducer(initialAnnotationEditorState(), {
        type: 'set_tool',
        tool: 'rectangle',
      }),
      { type: 'begin_annotation', anchor: RECT },
    )
    expect(annotationEditorReducer(dirty, { type: 'set_tool', tool: 'ellipse' })).toBe(dirty)
  })
})
