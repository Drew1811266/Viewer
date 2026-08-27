import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { ImageReviewWorkbenchController } from '../../app/review/useImageReviewWorkbench'
import type { ImagePreviewProjection } from '../imagePreview/ImagePreviewSurface'
import { createImagePreviewProjection } from '../imagePreview/imagePreviewProjection'
import AnnotationCanvas from './AnnotationCanvas'

const PROJECTION: ImagePreviewProjection = {
  sourceSize: { width: 640, height: 480 },
  stageRect: { left: 10, top: 20, width: 640, height: 480 },
  stageToNormalized: ({ x, y }) => ({ x: (x - 10) / 640, y: (y - 20) / 480 }),
  normalizedToStage: ({ x, y }) => ({ x: 10 + x * 640, y: 20 + y * 480 }),
}

function controller(
  overrides: Partial<ImageReviewWorkbenchController> = {},
): ImageReviewWorkbenchController {
  return {
    tool: 'browse',
    editor: {
      status: 'idle',
      tool: 'browse',
      temporarilyPanning: false,
      selectedFeedbackId: null,
    },
    dirty: false,
    feedback: [
      {
        feedbackId: 'feedback-1',
        ordinal: 1,
        text: '调整领口',
        createdAtMs: 1,
        anchor: { kind: 'image_rect', x: 0.1, y: 0.2, width: 0.3, height: 0.4 },
      },
    ],
    selectedFeedbackId: null,
    railOpen: true,
    readOnlyReason: null,
    restorableFeedbackId: null,
    leaveConfirmation: null,
    setTool: vi.fn(),
    setTemporaryPan: vi.fn(),
    beginAnnotation: vi.fn(),
    updateDraftAnchor: vi.fn(),
    updateDraftText: vi.fn(),
    saveDraft: vi.fn(async () => undefined),
    cancelDraft: vi.fn(),
    selectFeedback: vi.fn(),
    updateFeedbackText: vi.fn(async () => undefined),
    replaceFeedbackAnchor: vi.fn(async () => undefined),
    deleteFeedback: vi.fn(async () => undefined),
    restoreDeletedFeedback: vi.fn(async () => undefined),
    setRailOpen: vi.fn(),
    requestLeave: vi.fn(async () => 'proceeded' as const),
    cancelLeave: vi.fn(),
    discardUnsavedAndProceed: vi.fn(async () => undefined),
    ...overrides,
  }
}

describe('AnnotationCanvas', () => {
  beforeEach(() => vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null))
  afterEach(() => vi.restoreAllMocks())
  it('keeps the unsaved anchor visible while editing or retrying a failed save', () => {
    const context = {
      setTransform: vi.fn(),
      clearRect: vi.fn(),
      beginPath: vi.fn(),
      moveTo: vi.fn(),
      lineTo: vi.fn(),
      closePath: vi.fn(),
      stroke: vi.fn(),
      strokeRect: vi.fn(),
      save: vi.fn(),
      restore: vi.fn(),
      setLineDash: vi.fn(),
    }
    vi.mocked(HTMLCanvasElement.prototype.getContext).mockReturnValue(
      context as unknown as CanvasRenderingContext2D,
    )
    render(
      <AnnotationCanvas
        projection={PROJECTION}
        controller={controller({
          feedback: [],
          editor: {
            status: 'save_error',
            tool: 'rectangle',
            temporarilyPanning: false,
            selectedFeedbackId: null,
            sourceFeedbackId: null,
            draftAnchor: { kind: 'image_rect', x: 0.1, y: 0.2, width: 0.2, height: 0.3 },
            text: '保留未保存区域',
            message: '请重试',
          },
        })}
      />,
    )
    expect(context.setLineDash).toHaveBeenCalledWith([6, 4])
    expect(context.strokeRect).toHaveBeenCalledOnce()
    const rect = context.strokeRect.mock.calls[0] ?? []
    expect(rect[0]).toBeCloseTo(64)
    expect(rect[1]).toBeCloseTo(96)
    expect(rect[2]).toBeCloseTo(128)
    expect(rect[3]).toBeCloseTo(144)
  })
  it('selects a numbered marker and provides keyboard rectangle movement', () => {
    const review = controller({ selectedFeedbackId: 'feedback-1' })
    const parentKeyDown = vi.fn()
    const parentPointerDown = vi.fn()
    render(
      <div onKeyDown={parentKeyDown} onPointerDown={parentPointerDown}>
        <AnnotationCanvas projection={PROJECTION} controller={review} />
      </div>,
    )

    const marker = screen.getByRole('button', { name: '意见 1：调整领口' })
    fireEvent.click(marker)
    expect(review.selectFeedback).toHaveBeenCalledWith('feedback-1')
    fireEvent.pointerDown(marker, { clientX: 266, clientY: 116, pointerId: 7 })
    expect(parentPointerDown).not.toHaveBeenCalled()
    fireEvent.pointerUp(window, { clientX: 266, clientY: 116, pointerId: 7 })
    fireEvent.keyDown(marker, { key: 'ArrowRight' })
    expect(review.replaceFeedbackAnchor).toHaveBeenCalledWith('feedback-1', {
      kind: 'image_rect',
      x: 0.1 + 1 / 640,
      y: 0.2,
      width: 0.3,
      height: 0.4,
    })
    expect(screen.getAllByRole('button', { name: /调整意见 1/ })).toHaveLength(4)
    const handle = screen.getByRole('button', { name: '调整意见 1 右下角' })
    fireEvent.keyDown(handle, { key: 'ArrowRight', shiftKey: true })
    expect(review.replaceFeedbackAnchor).toHaveBeenLastCalledWith('feedback-1', {
      kind: 'image_rect',
      x: 0.1,
      y: 0.2,
      width: 0.3 + 10 / 640,
      height: 0.4,
    })
    expect(parentKeyDown).not.toHaveBeenCalled()
  })

  it('moves a selected rectangle by pointer and keeps the candidate until persistence resolves', async () => {
    let resolve!: () => void
    const pending = new Promise<void>((done) => {
      resolve = done
    })
    const review = controller({
      selectedFeedbackId: 'feedback-1',
      replaceFeedbackAnchor: vi.fn(() => pending),
    })
    render(<AnnotationCanvas projection={PROJECTION} controller={review} />)
    const marker = screen.getByRole('button', { name: '意见 1：调整领口' })
    fireEvent.pointerDown(marker, { clientX: 266, clientY: 116, pointerId: 3 })
    fireEvent.pointerMove(window, { clientX: 330, clientY: 164, pointerId: 3 })
    fireEvent.pointerUp(window, { clientX: 330, clientY: 164, pointerId: 3 })
    const replacement = vi.mocked(review.replaceFeedbackAnchor).mock.calls[0]?.[1]
    expect(replacement?.kind).toBe('image_rect')
    if (replacement?.kind !== 'image_rect') throw new Error('expected rectangle replacement')
    expect(replacement.x).toBeCloseTo(0.2)
    expect(replacement.y).toBeCloseTo(0.3)
    expect(screen.getByTestId('annotation-canvas')).toHaveAttribute('data-has-candidate', 'true')
    resolve()
    await pending
    await waitFor(() =>
      expect(screen.getByTestId('annotation-canvas')).not.toHaveAttribute('data-has-candidate'),
    )
  })

  it('retains a failed saved-geometry candidate beside the server geometry', async () => {
    const review = controller({
      selectedFeedbackId: 'feedback-1',
      replaceFeedbackAnchor: vi.fn().mockRejectedValue(new Error('write failed')),
    })
    render(<AnnotationCanvas projection={PROJECTION} controller={review} />)

    const marker = screen.getByRole('button', { name: '意见 1：调整领口' })
    fireEvent.pointerDown(marker, { clientX: 266, clientY: 116, pointerId: 3 })
    fireEvent.pointerMove(window, { clientX: 330, clientY: 164, pointerId: 3 })
    fireEvent.pointerUp(window, { clientX: 330, clientY: 164, pointerId: 3 })

    await waitFor(() => expect(review.replaceFeedbackAnchor).toHaveBeenCalledOnce())
    expect(screen.getByTestId('annotation-canvas')).toHaveAttribute('data-has-candidate', 'true')
    expect(marker).toBeVisible()
  })

  it('moves a selected rectangle when its visible marker starts outside the image', () => {
    const review = controller({ selectedFeedbackId: 'feedback-1' })
    const projection = createImagePreviewProjection(
      { left: 0, top: 0, width: 640, height: 480 },
      { mode: 'fit', zoom: 1, rotation: 0, offset: { x: 0, y: 0 } },
      { stage: { width: 640, height: 480 }, source: { width: 640, height: 480 }, fitInset: 1 },
    )
    render(<AnnotationCanvas projection={projection} controller={review} />)
    const marker = screen.getByRole('button', { name: '意见 1：调整领口' })
    expect(projection.stageToNormalized({ x: 256, y: -8 })).toBeNull()
    fireEvent.pointerDown(marker, { clientX: 256, clientY: -8, pointerId: 3 })
    fireEvent.pointerUp(window, { clientX: 320, clientY: 40, pointerId: 3 })
    const replacement = vi.mocked(review.replaceFeedbackAnchor).mock.calls[0]?.[1]
    expect(replacement?.kind).toBe('image_rect')
    if (replacement?.kind !== 'image_rect') throw new Error('expected rectangle replacement')
    expect(replacement.x).toBeCloseTo(0.2)
    expect(replacement.y).toBeCloseTo(0.3)
  })

  it('creates a normalized rectangle only after pointer release', () => {
    const review = controller({ tool: 'rectangle' })
    const parentPointerDown = vi.fn()
    render(
      <div onPointerDown={parentPointerDown}>
        <AnnotationCanvas projection={PROJECTION} controller={review} />
      </div>,
    )
    const canvas = screen.getByTestId('annotation-canvas')

    fireEvent.pointerDown(canvas, { clientX: 74, clientY: 68, pointerId: 1 })
    fireEvent.pointerMove(canvas, { clientX: 330, clientY: 260, pointerId: 1 })
    expect(review.beginAnnotation).not.toHaveBeenCalled()
    fireEvent.pointerUp(canvas, { clientX: 330, clientY: 260, pointerId: 1 })
    expect(review.beginAnnotation).toHaveBeenCalledWith({
      kind: 'image_rect',
      x: 0.1,
      y: 0.1,
      width: 0.4,
      height: 0.4,
    })
    expect(parentPointerDown).not.toHaveBeenCalled()
  })

  it('redraws a selected brush path without changing feedback identity or text', () => {
    const review = controller({
      tool: 'brush',
      selectedFeedbackId: 'feedback-2',
      feedback: [
        {
          feedbackId: 'feedback-2',
          ordinal: 2,
          text: '保留原文字',
          createdAtMs: 2,
          anchor: {
            kind: 'image_stroke',
            points: [
              { x: 0.1, y: 0.1 },
              { x: 0.2, y: 0.2 },
            ],
          },
        },
      ],
    })
    render(<AnnotationCanvas projection={PROJECTION} controller={review} />)
    const canvas = screen.getByTestId('annotation-canvas')
    fireEvent.pointerDown(canvas, { clientX: 74, clientY: 68, pointerId: 2 })
    fireEvent.pointerMove(canvas, { clientX: 202, clientY: 164, pointerId: 2 })
    fireEvent.pointerMove(canvas, { clientX: 330, clientY: 260, pointerId: 2 })
    fireEvent.pointerUp(canvas, { clientX: 458, clientY: 356, pointerId: 2 })

    expect(review.replaceFeedbackAnchor).toHaveBeenCalledWith(
      'feedback-2',
      expect.objectContaining({ kind: 'image_stroke' }),
    )
    expect(review.beginAnnotation).not.toHaveBeenCalled()
  })

  it('sizes the drawing buffer for device pixels and never marks whole-image feedback', () => {
    Object.defineProperty(window, 'devicePixelRatio', { value: 2, configurable: true })
    const review = controller({
      feedback: [
        {
          feedbackId: 'whole',
          ordinal: null,
          text: '整图意见',
          createdAtMs: 1,
          anchor: { kind: 'asset' },
        },
      ],
    })
    render(<AnnotationCanvas projection={PROJECTION} controller={review} />)
    expect(screen.getByTestId('annotation-canvas')).toHaveAttribute('width', '1280')
    expect(screen.queryByTestId('annotation-marker')).not.toBeInTheDocument()
    Object.defineProperty(window, 'devicePixelRatio', { value: 1, configurable: true })
  })
})
