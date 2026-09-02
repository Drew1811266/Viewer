import { describe, expect, it, vi } from 'vitest'
import type { ReviewAnchor, SavedImageFeedback } from '../../app/review/useImageReviewWorkbench'
import {
  annotationMarkerPoint,
  buildAnnotationScene,
  paintAnnotationScene,
} from './annotationScene'

const RECT: ReviewAnchor = {
  kind: 'image_rect',
  x: 0.1,
  y: 0.2,
  width: 0.3,
  height: 0.4,
}
const STROKE: ReviewAnchor = {
  kind: 'image_stroke',
  points: [
    { x: 0.2, y: 0.3 },
    { x: 0.4, y: 0.5 },
  ],
}

describe('annotation scene', () => {
  it('builds one ordered image-only scene with selection and transient appearance', () => {
    const rectFeedback = feedback('rect', 1, RECT)
    const strokeFeedback = feedback('stroke', 2, STROKE)
    const assetFeedback = feedback('asset', null, { kind: 'asset' })
    const transientAnchor: ReviewAnchor = {
      kind: 'image_rect',
      x: 0.5,
      y: 0.1,
      width: 0.2,
      height: 0.3,
    }

    expect(
      buildAnnotationScene({
        feedback: [rectFeedback, strokeFeedback, assetFeedback],
        selectedItemId: strokeFeedback.itemId,
        transientAnchor,
      }),
    ).toEqual([
      { itemId: 'rect', ordinal: 1, anchor: RECT, appearance: 'saved' },
      { itemId: 'stroke', ordinal: 2, anchor: STROKE, appearance: 'selected' },
      { itemId: null, ordinal: null, anchor: transientAnchor, appearance: 'transient' },
    ])
  })

  it('projects rectangle and stroke geometry without drawing ordinals when disabled', () => {
    const context = canvasContext()
    const scene = buildAnnotationScene({
      feedback: [feedback('rect', 1, RECT), feedback('stroke', 2, STROKE)],
      selectedItemId: null,
      transientAnchor: null,
    })

    expect(
      paintAnnotationScene(context.value, scene, PROJECTION, {
        color: '#b42318',
        lineWidth: 3,
        drawOrdinals: false,
        ordinalRadius: 14,
      }),
    ).toBe(2)
    const rectangle = context.strokeRect.mock.calls[0] ?? []
    expect(rectangle[0]).toBeCloseTo(20)
    expect(rectangle[1]).toBeCloseTo(20)
    expect(rectangle[2]).toBeCloseTo(60)
    expect(rectangle[3]).toBeCloseTo(40)
    expect(context.moveTo).toHaveBeenCalledWith(40, 30)
    expect(context.lineTo).toHaveBeenCalledWith(80, 50)
    expect(context.stroke).toHaveBeenCalledOnce()
    expect(context.arc).not.toHaveBeenCalled()
    expect(context.fillText).not.toHaveBeenCalled()
    expect(context.value.lineWidth).toBe(3)
    expect(context.value.strokeStyle).toBe('#b42318')
  })

  it('uses dashed transient geometry and restores canvas state', () => {
    const context = canvasContext()
    const scene = buildAnnotationScene({
      feedback: [],
      selectedItemId: null,
      transientAnchor: RECT,
    })

    expect(
      paintAnnotationScene(context.value, scene, PROJECTION, {
        color: '#b42318',
        lineWidth: 2,
        drawOrdinals: false,
        ordinalRadius: 14,
      }),
    ).toBe(1)
    expect(context.setLineDash).toHaveBeenCalledWith([6, 4])
    expect(context.save).toHaveBeenCalled()
    expect(context.restore).toHaveBeenCalled()
  })

  it('draws fixed-size ordinals at the same marker anchors used by the DOM layer', () => {
    const context = canvasContext()
    const scene = buildAnnotationScene({
      feedback: [feedback('rect', 7, RECT), feedback('stroke', 8, STROKE)],
      selectedItemId: null,
      transientAnchor: null,
    })

    paintAnnotationScene(context.value, scene, PROJECTION, {
      color: '#b42318',
      lineWidth: 4,
      drawOrdinals: true,
      ordinalRadius: 15,
    })

    expect(annotationMarkerPoint(RECT)).toEqual({ x: 0.4, y: 0.2 })
    expect(annotationMarkerPoint(STROKE)).toEqual({ x: 0.4, y: 0.5 })
    expect(context.arc).toHaveBeenNthCalledWith(1, 80, 20, 15, 0, Math.PI * 2)
    expect(context.arc).toHaveBeenNthCalledWith(2, 80, 50, 15, 0, Math.PI * 2)
    expect(context.fillText).toHaveBeenNthCalledWith(1, '7', 80, 20)
    expect(context.fillText).toHaveBeenNthCalledWith(2, '8', 80, 50)
  })

  it('counts only geometry whose projection can be painted', () => {
    const context = canvasContext()
    const projection = {
      normalizedToLocal: vi.fn(({ x, y }: { x: number; y: number }) =>
        x > 0.3 ? null : { x: x * 200, y: y * 100 },
      ),
    }

    expect(
      paintAnnotationScene(
        context.value,
        buildAnnotationScene({
          feedback: [feedback('rect', 1, RECT), feedback('stroke', 2, STROKE)],
          selectedItemId: null,
          transientAnchor: null,
        }),
        projection,
        { color: '#b42318', lineWidth: 2, drawOrdinals: true, ordinalRadius: 14 },
      ),
    ).toBe(1)
  })
})

const PROJECTION = {
  normalizedToLocal: ({ x, y }: { x: number; y: number }) => ({ x: x * 200, y: y * 100 }),
}

function feedback(
  itemId: string,
  ordinal: number | null,
  anchor: ReviewAnchor,
): SavedImageFeedback {
  return {
    itemId,
    feedbackId: `feedback-${itemId}`,
    targetKey: null,
    assetVersionId: 'asset-1',
    ordinal,
    text: itemId,
    createdAtMs: 1,
    anchor,
  }
}

function canvasContext() {
  const methods = {
    save: vi.fn(),
    restore: vi.fn(),
    setLineDash: vi.fn(),
    beginPath: vi.fn(),
    moveTo: vi.fn(),
    lineTo: vi.fn(),
    stroke: vi.fn(),
    strokeRect: vi.fn(),
    arc: vi.fn(),
    fill: vi.fn(),
    fillText: vi.fn(),
  }
  return {
    ...methods,
    value: {
      ...methods,
      lineCap: 'butt',
      lineJoin: 'miter',
      lineWidth: 1,
      strokeStyle: '#000',
      fillStyle: '#000',
      font: '',
      textAlign: 'start',
      textBaseline: 'alphabetic',
    } as unknown as CanvasRenderingContext2D,
  }
}
