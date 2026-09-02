import { describe, expect, it } from 'vitest'
import {
  clampOffset,
  displayScale,
  type ImageViewportGeometry,
  type ImageViewportState,
  normalizedToStage,
  panBounds,
  remapSourcePoint,
  sourcePointAtStagePoint,
  sourcePointToStagePoint,
  stagePointToSourcePoint,
  stageToNormalized,
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
  const source = { width: 6720, height: 4480 }
  const stage = { left: 120, top: 80, width: 960, height: 640 }

  it('maps the rendered image center to canonical normalized coordinates', () => {
    expect(stageToNormalized({ x: 600, y: 400 }, source, stage, 0, 1)).toEqual({
      x: 0.5,
      y: 0.5,
    })
  })

  it.each([0, 1, 2, 3] as const)(
    'round-trips canonical coordinates through rotation %i',
    (quarterTurns) => {
      for (const zoom of [0.5, 1, 2, 4]) {
        const normalized = { x: 0.23, y: 0.71 }
        const projected = normalizedToStage(normalized, source, stage, quarterTurns, zoom)
        expect(projected).not.toBeNull()
        expect(
          stageToNormalized(
            projected as { x: number; y: number },
            source,
            stage,
            quarterTurns,
            zoom,
          ),
        ).toEqual(
          expect.objectContaining({ x: expect.closeTo(0.23, 10), y: expect.closeTo(0.71, 10) }),
        )
      }
    },
  )

  it('returns null when source or stage geometry is unavailable', () => {
    expect(stageToNormalized({ x: 0, y: 0 }, { width: 0, height: 1 }, stage, 0, 1)).toBeNull()
    expect(normalizedToStage({ x: 0.5, y: 0.5 }, source, { ...stage, width: 0 }, 0, 1)).toBeNull()
  })

  it('fills the fit inset even when the current representation is smaller than the stage', () => {
    const proxyGeometry: ImageViewportGeometry = {
      stage: { width: 1920, height: 1000 },
      source: { width: 560, height: 373 },
      fitInset: 0.9,
    }

    const scale = displayScale(state(), proxyGeometry)

    expect(scale).toBeCloseTo(900 / 373)
    expect(proxyGeometry.source.height * scale).toBeCloseTo(900)
    expect(proxyGeometry.source.width * scale).toBeGreaterThan(1300)
  })

  it('gives every image with the same aspect ratio the same fitted display size', () => {
    const stage = { width: 960, height: 600 }
    const lowResolution = { stage, source: { width: 1_500, height: 1_000 }, fitInset: 0.9 }
    const highResolution = { stage, source: { width: 6_300, height: 4_200 }, fitInset: 0.9 }

    const lowScale = displayScale(state(), lowResolution)
    const highScale = displayScale(state(), highResolution)

    expect(lowResolution.source.width * lowScale).toBeCloseTo(810)
    expect(lowResolution.source.height * lowScale).toBeCloseTo(540)
    expect(highResolution.source.width * highScale).toBeCloseTo(810)
    expect(highResolution.source.height * highScale).toBeCloseTo(540)
  })

  it('treats fitted display as the only 100% baseline', () => {
    expect(displayScale(state(), GEOMETRY)).toBeCloseTo(0.45)
    expect(displayScale(state({ mode: 'free', zoom: 1.5 }), GEOMETRY)).toBeCloseTo(0.675)
    expect(displayScale(state({ mode: 'free', zoom: 2 }), GEOMETRY)).toBeCloseTo(0.9)
    expect(displayScale(state({ rotation: 90 }), GEOMETRY)).toBeCloseTo(0.36)
    expect(panBounds(state(), GEOMETRY)).toEqual({ x: 0, y: 0 })
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
    const rotated = state({ mode: 'free', zoom: 1, rotation: 90 })

    expect(sourcePointToStagePoint({ x: 0, y: 0 }, rotated, geometry)).toEqual({
      x: 340,
      y: 20,
    })
    expect(stagePointToSourcePoint({ x: 340, y: 20 }, rotated, geometry)).toEqual({
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
    expect(panBounds(state({ mode: 'free', zoom: 2 }), empty)).toEqual({ x: 0, y: 0 })
    expect(stagePointToSourcePoint({ x: 0, y: 0 }, state(), empty)).toBeNull()
    expect(sourcePointAtStagePoint({ x: 0, y: 0 }, state(), empty)).toBeNull()
  })
})
