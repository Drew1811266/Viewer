import { describe, expect, it } from 'vitest'
import {
  clampOffset,
  displayScale,
  type ImageViewportGeometry,
  type ImageViewportState,
  panBounds,
  remapSourcePoint,
  sourcePointAtStagePoint,
  sourcePointToStagePoint,
  stagePointToSourcePoint,
  zoomAtAnchor,
} from './imageGeometry'

const GEOMETRY: ImageViewportGeometry = {
  stage: { width: 500, height: 400 },
  source: { width: 1000, height: 800 },
  fitInset: 0.9,
}

function state(overrides: Partial<ImageViewportState> = {}): ImageViewportState {
  return {
    mode: 'fit',
    zoom: 1,
    rotation: 0,
    offset: { x: 0, y: 0 },
    ...overrides,
  }
}

describe('image viewport geometry', () => {
  it('derives fit, original, free, and rotated pan bounds from literal dimensions', () => {
    expect(displayScale(state(), GEOMETRY)).toBeCloseTo(0.45)
    expect(displayScale(state({ rotation: 90 }), GEOMETRY)).toBeCloseTo(0.36)
    expect(panBounds(state(), GEOMETRY)).toEqual({ x: 0, y: 0 })
    expect(panBounds(state({ mode: 'original' }), GEOMETRY)).toEqual({ x: 250, y: 200 })
    expect(panBounds(state({ mode: 'free', zoom: 2 }), GEOMETRY)).toEqual({ x: 200, y: 160 })
    expect(panBounds(state({ mode: 'free', zoom: 2, rotation: 90 }), GEOMETRY)).toEqual({
      x: 38,
      y: 160,
    })
    expect(clampOffset({ x: 999, y: -999 }, { x: 110, y: 250 })).toEqual({
      x: 110,
      y: -250,
    })
  })

  it('maps known points through clockwise rotation and inverse mapping', () => {
    const geometry: ImageViewportGeometry = {
      stage: { width: 500, height: 400 },
      source: { width: 100, height: 50 },
      fitInset: 0.9,
    }
    const rotated = state({ mode: 'original', rotation: 90 })

    expect(sourcePointToStagePoint({ x: 0, y: 0 }, rotated, geometry)).toEqual({
      x: 275,
      y: 150,
    })
    expect(stagePointToSourcePoint({ x: 275, y: 150 }, rotated, geometry)).toEqual({
      x: 0,
      y: 0,
    })
  })

  it.each([0, 90, 180, 270] as const)(
    'round-trips actual pixels and rejects gray-stage pixels at %d degrees',
    (rotation) => {
      const viewport = state({ mode: 'free', zoom: 1.5, rotation, offset: { x: 12, y: -18 } })
      const source = { x: 720, y: 310 }
      const stage = sourcePointToStagePoint(source, viewport, GEOMETRY)

      expect(stagePointToSourcePoint(stage, viewport, GEOMETRY)).toEqual(
        expect.objectContaining({ x: expect.closeTo(720, 6), y: expect.closeTo(310, 6) }),
      )
      expect(sourcePointAtStagePoint(stage, viewport, GEOMETRY)).not.toBeNull()
      expect(sourcePointAtStagePoint({ x: -100, y: -100 }, viewport, GEOMETRY)).toBeNull()
    },
  )

  it('preserves a source point under an off-center zoom anchor until a boundary clamps it', () => {
    const anchor = { x: 400, y: 300 }
    const sourceBefore = stagePointToSourcePoint(anchor, state(), GEOMETRY)

    const zoomed = zoomAtAnchor(state(), GEOMETRY, 2, anchor)

    expect(zoomed).toEqual({
      mode: 'free',
      zoom: 2,
      rotation: 0,
      offset: { x: -150, y: -100 },
    })
    expect(stagePointToSourcePoint(anchor, zoomed, GEOMETRY)).toEqual(
      expect.objectContaining({
        x: expect.closeTo(sourceBefore?.x ?? 0, 6),
        y: expect.closeTo(sourceBefore?.y ?? 0, 6),
      }),
    )
    expect(zoomAtAnchor(state({ mode: 'free', zoom: 7 }), GEOMETRY, 10, anchor).zoom).toBe(8)
    expect(zoomAtAnchor(state({ mode: 'free', zoom: 0.2 }), GEOMETRY, 0.01, anchor).zoom).toBe(0.1)
  })

  it('remaps a fit-representation pixel to the corresponding original pixel', () => {
    expect(
      remapSourcePoint(
        { x: 600, y: 400 },
        { width: 1200, height: 800 },
        { width: 6000, height: 4000 },
      ),
    ).toEqual({ x: 3000, y: 2000 })
  })

  it('returns safe values for zero-sized geometry', () => {
    const empty = {
      stage: { width: 0, height: 0 },
      source: { width: 0, height: 0 },
      fitInset: 0.9,
    }
    expect(displayScale(state(), empty)).toBe(0)
    expect(panBounds(state({ mode: 'original' }), empty)).toEqual({ x: 0, y: 0 })
    expect(stagePointToSourcePoint({ x: 0, y: 0 }, state(), empty)).toBeNull()
    expect(sourcePointAtStagePoint({ x: 0, y: 0 }, state(), empty)).toBeNull()
  })
})
