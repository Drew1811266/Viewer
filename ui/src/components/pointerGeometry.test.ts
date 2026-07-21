import { describe, expect, it } from 'vitest'
import { pointDistance, verticalEdgeScrollDelta } from './pointerGeometry'

describe('pointerGeometry', () => {
  it('measures pointer distance in CSS pixels', () => {
    expect(pointDistance({ x: 10, y: 20 }, { x: 13, y: 24 })).toBe(5)
  })

  it('returns capped edge-scroll deltas and zero away from edges', () => {
    expect(verticalEdgeScrollDelta(100, 100, 500)).toBe(-18)
    expect(verticalEdgeScrollDelta(116, 100, 500)).toBe(-9)
    expect(verticalEdgeScrollDelta(300, 100, 500)).toBe(0)
    expect(verticalEdgeScrollDelta(484, 100, 500)).toBe(9)
    expect(verticalEdgeScrollDelta(500, 100, 500)).toBe(18)
  })
})
