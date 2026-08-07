import { describe, expect, it } from 'vitest'
import {
  lensDimensions,
  magnifierShellPlacement,
  magnifierSourcePlacement,
} from './magnifierGeometry'

describe('magnifier geometry', () => {
  it('uses the six approved fixed shape and area dimensions', () => {
    expect(lensDimensions('circle', 'small')).toEqual({ width: 160, height: 160 })
    expect(lensDimensions('circle', 'medium')).toEqual({ width: 220, height: 220 })
    expect(lensDimensions('circle', 'large')).toEqual({ width: 300, height: 300 })
    expect(lensDimensions('rounded_rectangle', 'small')).toEqual({ width: 180, height: 120 })
    expect(lensDimensions('rounded_rectangle', 'medium')).toEqual({ width: 240, height: 160 })
    expect(lensDimensions('rounded_rectangle', 'large')).toEqual({ width: 330, height: 220 })
  })

  it('places the sampled original pixel at the lens center used as the CSS transform origin', () => {
    const dimensions = { width: 160, height: 160 }
    const sourcePoint = { x: 1600, y: 1200 }
    const placement = magnifierSourcePlacement(sourcePoint, dimensions)

    expect(placement).toEqual({
      left: -1520,
      top: -1120,
      transformOriginX: 1600,
      transformOriginY: 1200,
    })
    expect(placement.left + sourcePoint.x).toBe(80)
    expect(placement.top + sourcePoint.y).toBe(80)
  })

  it('places the lens to the lower-right when both positive axes fit', () => {
    expect(
      magnifierShellPlacement(
        { x: 200, y: 150 },
        { width: 640, height: 480 },
        { width: 160, height: 160 },
      ),
    ).toEqual({
      center: { x: 298, y: 248 },
      origin: { x: 0, y: 0 },
      horizontal: 'right',
      vertical: 'below',
    })
  })

  it('flips both axes near the lower-right stage edge', () => {
    expect(
      magnifierShellPlacement(
        { x: 600, y: 440 },
        { width: 640, height: 480 },
        { width: 160, height: 160 },
      ),
    ).toEqual({
      center: { x: 502, y: 342 },
      origin: { x: 160, y: 160 },
      horizontal: 'left',
      vertical: 'above',
    })
  })

  it('flips each constrained axis independently', () => {
    const stage = { width: 640, height: 480 }
    const lens = { width: 160, height: 160 }

    expect(magnifierShellPlacement({ x: 600, y: 150 }, stage, lens)).toEqual({
      center: { x: 502, y: 248 },
      origin: { x: 160, y: 0 },
      horizontal: 'left',
      vertical: 'below',
    })
    expect(magnifierShellPlacement({ x: 200, y: 440 }, stage, lens)).toEqual({
      center: { x: 298, y: 342 },
      origin: { x: 0, y: 160 },
      horizontal: 'right',
      vertical: 'above',
    })
  })

  it('clamps the center inside a stage when neither side has the preferred gap', () => {
    const placement = magnifierShellPlacement(
      { x: 100, y: 90 },
      { width: 200, height: 180 },
      { width: 160, height: 160 },
    )

    expect(placement.center.x).toBeGreaterThanOrEqual(80)
    expect(placement.center.x).toBeLessThanOrEqual(120)
    expect(placement.center.y).toBeGreaterThanOrEqual(80)
    expect(placement.center.y).toBeLessThanOrEqual(100)
  })
})
