import { describe, expect, it } from 'vitest'
import {
  createMagnifierContentProjection,
  lensDimensions,
  magnifierShellPlacement,
  magnifierSourcePlacement,
} from './magnifierGeometry'

describe('magnifier geometry', () => {
  it.each([
    [0, { x: 140, y: 80 }],
    [90, { x: 120, y: 100 }],
    [180, { x: 100, y: 80 }],
    [270, { x: 120, y: 60 }],
  ] as const)('projects normalized geometry at %i°', (rotation, expected) => {
    const projection = createMagnifierContentProjection({
      sourceSize: { width: 100, height: 50 },
      sourcePoint: { x: 50, y: 25 },
      lensSize: { width: 240, height: 160 },
      contentScale: 2,
      rotation,
    })

    expect(projection.normalizedToLens({ x: 0.6, y: 0.5 })).toEqual(expected)
    expect(projection.normalizedToLens({ x: 0.5, y: 0.5 })).toEqual({ x: 120, y: 80 })
  })

  it.each([
    [{ width: 0, height: 50 }, { width: 240, height: 160 }, 2],
    [{ width: 100, height: -1 }, { width: 240, height: 160 }, 2],
    [{ width: 100, height: 50 }, { width: 0, height: 160 }, 2],
    [{ width: 100, height: 50 }, { width: 240, height: Number.NaN }, 2],
    [{ width: 100, height: 50 }, { width: 240, height: 160 }, 0],
  ])('rejects invalid source, lens, or scale input', (sourceSize, lensSize, contentScale) => {
    const projection = createMagnifierContentProjection({
      sourceSize,
      sourcePoint: { x: 50, y: 25 },
      lensSize,
      contentScale,
      rotation: 0,
    })

    expect(projection.normalizedToLens({ x: 0.5, y: 0.5 })).toBeNull()
  })

  it('rejects non-finite source and normalized points', () => {
    const invalidSource = createMagnifierContentProjection({
      sourceSize: { width: 100, height: 50 },
      sourcePoint: { x: Number.POSITIVE_INFINITY, y: 25 },
      lensSize: { width: 240, height: 160 },
      contentScale: 2,
      rotation: 0,
    })
    const projection = createMagnifierContentProjection({
      sourceSize: { width: 100, height: 50 },
      sourcePoint: { x: 50, y: 25 },
      lensSize: { width: 240, height: 160 },
      contentScale: 2,
      rotation: 0,
    })

    expect(invalidSource.normalizedToLens({ x: 0.5, y: 0.5 })).toBeNull()
    expect(projection.normalizedToLens({ x: Number.NaN, y: 0.5 })).toBeNull()
    expect(projection.normalizedToLens({ x: 0.5, y: Number.NEGATIVE_INFINITY })).toBeNull()
  })

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
