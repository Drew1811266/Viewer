import { describe, expect, it } from 'vitest'
import {
  arrowFromDrag,
  ellipseFromDrag,
  moveAnchor,
  pointAnchor,
  rectFromDrag,
  resizeBoxAnchor,
  simplifyNormalizedStroke,
} from './annotationGeometry'

describe('annotation geometry', () => {
  it('simplifies a stroke deterministically and preserves endpoints', () => {
    const input = Array.from({ length: 5_000 }, (_, index) => ({
      x: index / 4_999,
      y: 0.5 + Math.sin(index / 40) * 0.01,
    }))

    const first = simplifyNormalizedStroke(input, {
      sourceWidth: 6_720,
      sourceHeight: 4_480,
      maxPoints: 2_048,
    })
    const second = simplifyNormalizedStroke(input, {
      sourceWidth: 6_720,
      sourceHeight: 4_480,
      maxPoints: 2_048,
    })

    expect(first).toEqual(second)
    expect(first?.[0]).toEqual(input[0])
    expect(first?.at(-1)).toEqual(input.at(-1))
    expect(first?.length).toBeLessThanOrEqual(2_048)
  })

  it('normalizes a dragged rectangle in every direction and clamps to the image', () => {
    expect(rectFromDrag({ x: 0.8, y: 0.7 }, { x: -0.1, y: 1.1 })).toEqual({
      x: 0,
      y: 0.7,
      width: 0.8,
      height: 0.3,
    })
  })

  it('rejects degenerate strokes and rectangles instead of approximating them', () => {
    expect(
      simplifyNormalizedStroke(
        [
          { x: 0.5, y: 0.5 },
          { x: 0.5, y: 0.5 },
        ],
        { sourceWidth: 100, sourceHeight: 100, maxPoints: 2_048 },
      ),
    ).toBeNull()
    expect(rectFromDrag({ x: 0.5, y: 0.5 }, { x: 0.5, y: 0.7 })).toBeNull()
  })

  it('creates exact point and directional arrow anchors', () => {
    expect(pointAnchor({ x: 0, y: 1 })).toEqual({ kind: 'image_point', x: 0, y: 1 })
    expect(pointAnchor({ x: -0.01, y: 0.5 })).toBeNull()
    expect(arrowFromDrag({ x: 0.1, y: 0.2 }, { x: 0.7, y: 0.8 })).toEqual({
      kind: 'image_arrow',
      tail: { x: 0.1, y: 0.2 },
      head: { x: 0.7, y: 0.8 },
    })
    expect(arrowFromDrag({ x: 0.4, y: 0.4 }, { x: 0.4, y: 0.4 })).toBeNull()
  })

  it('constrains ellipses to visual circles in oriented source pixels', () => {
    const ellipse = ellipseFromDrag(
      { x: 0.1, y: 0.2 },
      { x: 0.5, y: 0.7 },
      { constrainCircle: true, sourceWidth: 1_200, sourceHeight: 800 },
    )

    expect(ellipse).toMatchObject({ kind: 'image_ellipse', x: 0.1, y: 0.2 })
    expect((ellipse?.width ?? 0) * 1_200).toBeCloseTo((ellipse?.height ?? 0) * 800)
  })

  it('moves whole shapes without changing size and resizes only the active box corner', () => {
    expect(
      moveAnchor(
        { kind: 'image_ellipse', x: 0.7, y: 0.7, width: 0.2, height: 0.2 },
        { x: 0.4, y: -0.9 },
      ),
    ).toEqual({ kind: 'image_ellipse', x: 0.8, y: 0, width: 0.2, height: 0.2 })

    expect(
      resizeBoxAnchor(
        { kind: 'image_rect', x: 0.2, y: 0.2, width: 0.4, height: 0.4 },
        'south_east',
        { x: 0.8, y: 0.9 },
      ),
    ).toEqual({ kind: 'image_rect', x: 0.2, y: 0.2, width: 0.6, height: 0.7 })
  })
})
