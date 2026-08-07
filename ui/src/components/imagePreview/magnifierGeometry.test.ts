import { describe, expect, it } from 'vitest'
import { lensDimensions, magnifierSourcePlacement } from './magnifierGeometry'

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
    const sourcePoint = { x: 1600, y: 1140 }
    const placement = magnifierSourcePlacement(sourcePoint, dimensions)

    expect(placement).toEqual({
      left: -1520,
      top: -1060,
      transformOriginX: 1600,
      transformOriginY: 1140,
    })
    expect(placement.left + sourcePoint.x).toBe(80)
    expect(placement.top + sourcePoint.y).toBe(80)
  })
})
