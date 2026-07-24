import { describe, expect, it } from 'vitest'
import {
  annularSectorPath,
  fitMenuOrigin,
  MOTION_THRESHOLD,
  primaryCenterAngle,
  primaryIndexAt,
  secondaryIndexAt,
} from './radialMenuGeometry'

describe('radial menu geometry', () => {
  it('maps the confirmed six primary directions', () => {
    const origin = { x: 200, y: 200 }
    expect(primaryIndexAt({ x: 200, y: 110 }, origin)).toBe(0)
    expect(primaryIndexAt({ x: 280, y: 154 }, origin)).toBe(1)
    expect(primaryIndexAt({ x: 280, y: 246 }, origin)).toBe(2)
    expect(primaryIndexAt({ x: 200, y: 290 }, origin)).toBe(3)
    expect(primaryIndexAt({ x: 120, y: 246 }, origin)).toBe(4)
    expect(primaryIndexAt({ x: 120, y: 154 }, origin)).toBe(5)
    expect(primaryIndexAt(origin, origin)).toBeNull()
  })

  it('maps a five-item local fan around the selected direction', () => {
    const origin = { x: 200, y: 200 }
    expect(secondaryIndexAt({ x: 200, y: 40 }, origin, -30, 5)).toBe(0)
    expect(secondaryIndexAt({ x: 339, y: 120 }, origin, -30, 5)).toBe(2)
    expect(secondaryIndexAt({ x: 339, y: 280 }, origin, -30, 5)).toBe(4)
  })

  it('creates a closed annular sector path', () => {
    expect(annularSectorPath({ x: 0, y: 0 }, 40, 80, -30, 30)).toMatch(
      /^M .* A 80 80 .* L .* A 40 40 .* Z$/,
    )
  })

  it('fits the full expanded menu inside the viewport', () => {
    expect(fitMenuOrigin({ x: 10, y: 790 }, { width: 1280, height: 800 })).toEqual({
      x: 180,
      y: 620,
    })
  })

  it('fits the whole origin far enough inward to preserve preferred fan directions', () => {
    const viewport = { width: 1280, height: 800 }
    expect(fitMenuOrigin({ x: 4, y: 400 }, viewport)).toEqual({ x: 180, y: 400 })
    expect(fitMenuOrigin({ x: 640, y: 4 }, viewport)).toEqual({ x: 640, y: 180 })
    expect(fitMenuOrigin({ x: 640, y: 796 }, viewport)).toEqual({ x: 640, y: 620 })
    expect(fitMenuOrigin({ x: 1276, y: 400 }, viewport)).toEqual({ x: 1100, y: 400 })
    expect(primaryCenterAngle(1)).toBe(-30)
    expect(MOTION_THRESHOLD).toBe(12)
  })
})
