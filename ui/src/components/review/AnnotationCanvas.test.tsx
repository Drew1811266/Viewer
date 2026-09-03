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
    protocol: 'legacy',
    tool: 'browse',
    editor: {
      activeTool: 'browse',
      temporarilyPanning: false,
      selectedItemId: null,
      phase: { status: 'idle' },
    },
    dirty: false,
    redrawItemId: null,
    beginDrawing: vi.fn(() => true),
    finishDrawing: vi.fn(async () => undefined),
    beginFeedbackTextEdit: vi.fn(),
    beginRedraw: vi.fn(),
    stageFeedbackAnchor: vi.fn(() => true),
    feedback: [
      {
        itemId: 'target-1',
        feedbackId: 'feedback-1',
        targetKey: null,
        assetVersionId: 'asset-1',
        ordinal: 1,
        text: '调整领口',
        createdAtMs: 1,
        anchor: { kind: 'image_rect', x: 0.1, y: 0.2, width: 0.3, height: 0.4 },
      },
    ],
    selectedItemId: null,
    railOpen: true,
    readOnlyReason: null,
    restorableItemId: null,
    statusMessage: null,
    preparedImage: null,
    leaveConfirmation: null,
    setTool: vi.fn(),
    setTemporaryPan: vi.fn(),
    beginAnnotation: vi.fn(),
    updateDraftAnchor: vi.fn(),
    updateDraftText: vi.fn(),
    saveDraft: vi.fn(async () => undefined),
    cancelDraft: vi.fn(),
    selectFeedback: vi.fn(),
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
            activeTool: 'rectangle',
            temporarilyPanning: false,
            selectedItemId: null,
            phase: {
              status: 'save_error',
              sourceItemId: null,
              draftAnchor: { kind: 'image_rect', x: 0.1, y: 0.2, width: 0.2, height: 0.3 },
              text: '保留未保存区域',
              message: '请重试',
            },
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
    const review = controller({ selectedItemId: 'target-1' })
    const parentKeyDown = vi.fn()
    const parentPointerDown = vi.fn()
    render(
      <div onKeyDown={parentKeyDown} onPointerDown={parentPointerDown}>
        <AnnotationCanvas projection={PROJECTION} controller={review} />
      </div>,
    )

    const marker = screen.getByRole('button', { name: '意见 1：调整领口' })
    fireEvent.click(marker)
    expect(review.selectFeedback).toHaveBeenCalledWith('target-1')
    fireEvent.pointerDown(marker, { clientX: 266, clientY: 116, pointerId: 7 })
    expect(parentPointerDown).not.toHaveBeenCalled()
    fireEvent.pointerUp(window, { clientX: 266, clientY: 116, pointerId: 7 })
    fireEvent.keyDown(marker, { key: 'ArrowRight' })
    expect(review.replaceFeedbackAnchor).toHaveBeenCalledWith('target-1', {
      kind: 'image_rect',
      x: 0.1 + 1 / 640,
      y: 0.2,
      width: 0.3,
      height: 0.4,
    })
    expect(screen.getAllByRole('button', { name: /调整意见 1/ })).toHaveLength(4)
    const handle = screen.getByRole('button', { name: '调整意见 1 右下角' })
    fireEvent.keyDown(handle, { key: 'ArrowRight', shiftKey: true })
    expect(review.replaceFeedbackAnchor).toHaveBeenLastCalledWith('target-1', {
      kind: 'image_rect',
      x: 0.1,
      y: 0.2,
      width: 0.3 + 10 / 640,
      height: 0.4,
    })
    expect(parentKeyDown).not.toHaveBeenCalled()
  })

  it('sends a selected rectangle pointer candidate to the controller before saving', async () => {
    let resolve!: () => void
    const pending = new Promise<void>((done) => {
      resolve = done
    })
    const review = controller({
      selectedItemId: 'target-1',
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
    expect(review.stageFeedbackAnchor).toHaveBeenCalledWith('target-1', replacement)
    resolve()
    await pending
    await waitFor(() =>
      expect(screen.getByTestId('annotation-canvas')).not.toHaveAttribute('data-has-candidate'),
    )
  })

  it('renders a controller-owned failed geometry candidate beside the server geometry', async () => {
    const review = controller({
      selectedItemId: 'target-1',
      editor: {
        activeTool: 'rectangle',
        temporarilyPanning: false,
        selectedItemId: 'target-1',
        phase: {
          status: 'save_error',
          sourceItemId: 'feedback-1',
          operation: 'geometry',
          draftAnchor: { kind: 'image_rect', x: 0.2, y: 0.3, width: 0.3, height: 0.4 },
          text: '调整领口',
          message: '请重试',
        },
      },
    })
    render(<AnnotationCanvas projection={PROJECTION} controller={review} />)

    const marker = screen.getByRole('button', { name: '意见 1：调整领口' })
    expect(screen.getByTestId('annotation-canvas')).toHaveAttribute('data-has-candidate', 'true')
    expect(marker).toBeVisible()
  })

  it('moves a selected rectangle when its visible marker starts outside the image', () => {
    const review = controller({ selectedItemId: 'target-1' })
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
    expect(review.finishDrawing).not.toHaveBeenCalled()
    fireEvent.pointerUp(canvas, { clientX: 330, clientY: 260, pointerId: 1 })
    expect(review.finishDrawing).toHaveBeenCalledWith({
      kind: 'image_rect',
      x: 0.1,
      y: 0.1,
      width: 0.4,
      height: 0.4,
    })
    expect(parentPointerDown).not.toHaveBeenCalled()
  })

  it('creates point and directional arrow anchors through one pointer lifecycle', () => {
    const pointReview = controller({ tool: 'point' })
    const pointView = render(<AnnotationCanvas projection={PROJECTION} controller={pointReview} />)
    let canvas = screen.getByTestId('annotation-canvas')
    fireEvent.pointerDown(canvas, { clientX: 74, clientY: 68, pointerId: 10 })
    fireEvent.pointerUp(window, { clientX: 74, clientY: 68, pointerId: 10 })
    expect(pointReview.finishDrawing).toHaveBeenCalledWith({ kind: 'image_point', x: 0.1, y: 0.1 })

    pointView.unmount()
    const arrowReview = controller({ tool: 'arrow' })
    render(<AnnotationCanvas projection={PROJECTION} controller={arrowReview} />)
    canvas = screen.getByTestId('annotation-canvas')
    fireEvent.pointerDown(canvas, { clientX: 74, clientY: 68, pointerId: 11 })
    fireEvent.pointerMove(window, { clientX: 330, clientY: 260, pointerId: 11 })
    fireEvent.pointerUp(window, { clientX: 330, clientY: 260, pointerId: 11 })
    expect(arrowReview.finishDrawing).toHaveBeenCalledWith({
      kind: 'image_arrow',
      tail: { x: 0.1, y: 0.1 },
      head: { x: 0.5, y: 0.5 },
    })
  })

  it('rejects a sub-6px arrow and keeps outside release and pointer cancel deterministic', () => {
    const review = controller({ tool: 'arrow' })
    render(<AnnotationCanvas projection={PROJECTION} controller={review} />)
    const canvas = screen.getByTestId('annotation-canvas')

    fireEvent.pointerDown(canvas, { clientX: 74, clientY: 68, pointerId: 12 })
    fireEvent.pointerUp(window, { clientX: 79, clientY: 68, pointerId: 12 })
    expect(review.finishDrawing).toHaveBeenLastCalledWith(null)

    fireEvent.pointerDown(canvas, { clientX: 74, clientY: 68, pointerId: 13 })
    fireEvent.pointerUp(window, { clientX: 714, clientY: 548, pointerId: 13 })
    expect(review.finishDrawing).toHaveBeenLastCalledWith({
      kind: 'image_arrow',
      tail: { x: 0.1, y: 0.1 },
      head: { x: 1, y: 1 },
    })

    fireEvent.pointerDown(canvas, { clientX: 74, clientY: 68, pointerId: 14 })
    fireEvent.pointerCancel(window, { clientX: 202, clientY: 164, pointerId: 14 })
    expect(review.cancelDraft).toHaveBeenCalledOnce()
  })

  it('creates free ellipses and constrains Shift to a visual circle on a non-square image', () => {
    const review = controller({ tool: 'ellipse' })
    render(<AnnotationCanvas projection={PROJECTION} controller={review} />)
    const canvas = screen.getByTestId('annotation-canvas')

    fireEvent.pointerDown(canvas, { clientX: 74, clientY: 68, pointerId: 15 })
    fireEvent.pointerUp(window, { clientX: 330, clientY: 260, pointerId: 15 })
    expect(review.finishDrawing).toHaveBeenLastCalledWith({
      kind: 'image_ellipse',
      x: 0.1,
      y: 0.1,
      width: 0.4,
      height: 0.4,
    })

    fireEvent.pointerDown(canvas, { clientX: 74, clientY: 68, pointerId: 16 })
    fireEvent.pointerUp(window, {
      clientX: 330,
      clientY: 260,
      pointerId: 16,
      shiftKey: true,
    })
    expect(review.finishDrawing).toHaveBeenLastCalledWith({
      kind: 'image_ellipse',
      x: 0.1,
      y: 0.1,
      width: 0.3,
      height: 0.4,
    })
  })

  it('moves a selected point and exposes both arrow endpoint controls', () => {
    const pointReview = controller({
      selectedItemId: 'point-item',
      feedback: [
        {
          itemId: 'point-item',
          feedbackId: 'point-feedback',
          targetKey: null,
          assetVersionId: 'asset-1',
          ordinal: 1,
          text: '修正这个点',
          createdAtMs: 1,
          anchor: { kind: 'image_point', x: 0.2, y: 0.2 },
        },
      ],
    })
    const pointView = render(<AnnotationCanvas projection={PROJECTION} controller={pointReview} />)
    const marker = screen.getByRole('button', { name: '意见 1：修正这个点' })
    fireEvent.pointerDown(marker, { clientX: 138, clientY: 116, pointerId: 20 })
    fireEvent.pointerMove(window, { clientX: 202, clientY: 164, pointerId: 20 })
    fireEvent.pointerUp(window, { clientX: 202, clientY: 164, pointerId: 20 })
    expect(pointReview.replaceFeedbackAnchor).toHaveBeenCalledWith('point-item', {
      kind: 'image_point',
      x: 0.3,
      y: 0.3,
    })

    pointView.unmount()
    const arrowReview = controller({
      selectedItemId: 'arrow-item',
      feedback: [
        {
          itemId: 'arrow-item',
          feedbackId: 'arrow-feedback',
          targetKey: null,
          assetVersionId: 'asset-1',
          ordinal: 2,
          text: '改变箭头方向',
          createdAtMs: 2,
          anchor: {
            kind: 'image_arrow',
            tail: { x: 0.2, y: 0.2 },
            head: { x: 0.5, y: 0.5 },
          },
        },
      ],
    })
    render(<AnnotationCanvas projection={PROJECTION} controller={arrowReview} />)
    const tail = screen.getByRole('button', { name: '调整意见 2 箭尾' })
    const head = screen.getByRole('button', { name: '调整意见 2 箭头' })
    fireEvent.pointerDown(tail, { clientX: 138, clientY: 116, pointerId: 21 })
    fireEvent.pointerMove(window, { clientX: 202, clientY: 164, pointerId: 21 })
    fireEvent.pointerUp(window, { clientX: 202, clientY: 164, pointerId: 21 })
    expect(arrowReview.replaceFeedbackAnchor).toHaveBeenLastCalledWith('arrow-item', {
      kind: 'image_arrow',
      tail: { x: 0.3, y: 0.3 },
      head: { x: 0.5, y: 0.5 },
    })
    expect(head).toBeVisible()
  })

  it('moves and resizes a selected ellipse through its interior and four handles', () => {
    const review = controller({
      selectedItemId: 'ellipse-item',
      feedback: [
        {
          itemId: 'ellipse-item',
          feedbackId: 'ellipse-feedback',
          targetKey: null,
          assetVersionId: 'asset-1',
          ordinal: 3,
          text: '调整脸部范围',
          createdAtMs: 3,
          anchor: { kind: 'image_ellipse', x: 0.1, y: 0.1, width: 0.4, height: 0.4 },
        },
      ],
    })
    render(<AnnotationCanvas projection={PROJECTION} controller={review} />)
    const moveTarget = screen.getByRole('button', { name: '移动意见 3 区域' })
    fireEvent.pointerDown(moveTarget, { clientX: 202, clientY: 164, pointerId: 22 })
    fireEvent.pointerMove(window, { clientX: 266, clientY: 212, pointerId: 22 })
    fireEvent.pointerUp(window, { clientX: 266, clientY: 212, pointerId: 22 })
    expect(review.replaceFeedbackAnchor).toHaveBeenLastCalledWith('ellipse-item', {
      kind: 'image_ellipse',
      x: 0.2,
      y: 0.2,
      width: 0.4,
      height: 0.4,
    })

    expect(screen.getAllByRole('button', { name: /调整意见 3/ })).toHaveLength(4)
    const handle = screen.getByRole('button', { name: '调整意见 3 右下角' })
    fireEvent.pointerDown(handle, { clientX: 330, clientY: 260, pointerId: 23 })
    fireEvent.pointerMove(window, { clientX: 394, clientY: 308, pointerId: 23 })
    fireEvent.pointerUp(window, { clientX: 394, clientY: 308, pointerId: 23 })
    expect(review.replaceFeedbackAnchor).toHaveBeenLastCalledWith('ellipse-item', {
      kind: 'image_ellipse',
      x: 0.1,
      y: 0.1,
      width: 0.5,
      height: 0.5,
    })
  })

  it('redraws a selected brush path without changing feedback identity or text', () => {
    const review = controller({
      tool: 'brush',
      selectedItemId: 'feedback-2',
      redrawItemId: 'feedback-2',
      feedback: [
        {
          itemId: 'feedback-2',
          feedbackId: 'feedback-2',
          targetKey: null,
          assetVersionId: 'asset-1',
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

    expect(review.beginDrawing).toHaveBeenCalledWith(
      expect.objectContaining({ kind: 'image_stroke' }),
      'feedback-2',
    )
    expect(review.finishDrawing).toHaveBeenCalledWith(
      expect.objectContaining({ kind: 'image_stroke' }),
    )
    expect(review.beginAnnotation).not.toHaveBeenCalled()
  })

  it('sizes the drawing buffer for device pixels and never marks whole-image feedback', () => {
    Object.defineProperty(window, 'devicePixelRatio', { value: 2, configurable: true })
    const review = controller({
      feedback: [
        {
          itemId: 'whole',
          feedbackId: 'whole',
          targetKey: null,
          assetVersionId: 'asset-1',
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
