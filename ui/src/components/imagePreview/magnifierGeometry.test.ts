import { describe, expect, it } from 'vitest'
import {
  lensDimensions,
  magnifierShellPlacement,
  magnifierSourcePlacement,
} from './magnifierGeometry'

describe('magnifier geometry', () => {
  it('uses the six approved fixed shape and area dimensions', () => {
    expect(lensDimensions('circle', 'small')).toEqual({ width: 200, height: 200 })
    expect(lensDimensions('circle', 'medium')).toEqual({ width: 280, height: 280 })
    expect(lensDimensions('circle', 'large')).toEqual({ width: 380, height: 380 })
    expect(lensDimensions('rounded_rectangle', 'small')).toEqual({ width: 230, height: 150 })
    expect(lensDimensions('rounded_rectangle', 'medium')).toEqual({ width: 300, height: 200 })
    expect(lensDimensions('rounded_rectangle', 'large')).toEqual({ width: 420, height: 280 })
  })

  it('places the sampled original pixel at the lens center used as the CSS transform origin', () => {
    const dimensions = { width: 200, height: 200 }
    const sourcePoint = { x: 1600, y: 1200 }
    const placement = magnifierSourcePlacement(sourcePoint, dimensions)

    expect(placement).toEqual({
      left: -1500,
      top: -1100,
      transformOriginX: 1600,
      transformOriginY: 1200,
    })
    expect(placement.left + sourcePoint.x).toBe(100)
    expect(placement.top + sourcePoint.y).toBe(100)
  })

  it('places the lens to the lower-right when both positive axes fit', () => {
    expect(
      magnifierShellPlacement(
        { x: 200, y: 150 },
        { width: 640, height: 480 },
        { width: 200, height: 200 },
      ),
    ).toEqual({
      center: { x: 318, y: 268 },
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
        { width: 200, height: 200 },
      ),
    ).toEqual({
      center: { x: 482, y: 322 },
      origin: { x: 200, y: 200 },
      horizontal: 'left',
      vertical: 'above',
    })
  })

  it('flips each constrained axis independently', () => {
    const stage = { width: 640, height: 480 }
    const lens = { width: 200, height: 200 }

    expect(magnifierShellPlacement({ x: 600, y: 150 }, stage, lens)).toEqual({
      center: { x: 482, y: 268 },
      origin: { x: 200, y: 0 },
      horizontal: 'left',
      vertical: 'below',
    })
    expect(magnifierShellPlacement({ x: 200, y: 440 }, stage, lens)).toEqual({
      center: { x: 318, y: 322 },
      origin: { x: 0, y: 200 },
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
