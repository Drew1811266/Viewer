import { describe, expect, it } from 'vitest'
import { rectFromDrag, simplifyNormalizedStroke } from './annotationGeometry'

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
})
