import { describe, expect, it } from 'vitest'
import {
  intersectingGridIndexes,
  marqueeDistance,
  normalizeMarquee,
  verticalAutoScrollDelta,
} from './marqueeSelection'

const geometry = {
  itemCount: 20,
  columns: 4,
  cellWidth: 100,
  cellHeight: 80,
  columnStride: 112,
  rowStride: 92,
}

describe('marqueeSelection', () => {
  it('normalizes forward and reverse pointer movement', () => {
    expect(normalizeMarquee({ x: 220, y: 180 }, { x: 10, y: 20 })).toEqual({
      left: 10, top: 20, right: 220, bottom: 180, width: 210, height: 160,
    })
  })

  it('measures the activation distance', () => {
    expect(marqueeDistance({ x: 1, y: 1 }, { x: 4, y: 5 })).toBe(5)
  })

  it('hits touching cells in stable item order and excludes gaps', () => {
    expect(intersectingGridIndexes(normalizeMarquee({ x: 100, y: 0 }, { x: 112, y: 80 }), geometry))
      .toEqual([0, 1])
    expect(intersectingGridIndexes(normalizeMarquee({ x: 105, y: 5 }, { x: 107, y: 70 }), geometry))
      .toEqual([])
  })

  it('hits virtual items outside the mounted viewport', () => {
    expect(intersectingGridIndexes(normalizeMarquee({ x: 0, y: 368 }, { x: 212, y: 448 }), geometry))
      .toEqual([16, 17])
  })

  it('clamps edge scrolling to the specified 18 pixel maximum', () => {
    expect(verticalAutoScrollDelta(100, 100, 300)).toBe(-18)
    expect(verticalAutoScrollDelta(116, 100, 300)).toBe(-9)
    expect(verticalAutoScrollDelta(284, 100, 300)).toBe(9)
    expect(verticalAutoScrollDelta(300, 100, 300)).toBe(18)
    expect(verticalAutoScrollDelta(200, 100, 300)).toBe(0)
  })
})
