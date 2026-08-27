import { describe, expect, it } from 'vitest'
import { type ImageViewportState, sourcePointToStagePoint } from './imageGeometry'
import { createImagePreviewProjection } from './imagePreviewProjection'

describe('image preview overlay projection', () => {
  for (const browserZoom of [1, 2]) {
    for (const rotation of [0, 90, 180, 270] as const) {
      it(`maps client input and logical overlay positions at zoom ${browserZoom}, rotation ${rotation}`, () => {
        const state: ImageViewportState = {
          mode: 'free',
          zoom: 1.25,
          rotation,
          offset: { x: 12, y: -8 },
        }
        const geometry = {
          stage: { width: 512, height: 220 },
          source: { width: 640, height: 480 },
          fitInset: 0.9,
        }
        const bounds = { left: 24, top: 282, width: 512 * browserZoom, height: 220 * browserZoom }
        const projection = createImagePreviewProjection(bounds, state, geometry)
        const normalized = { x: 0.43, y: 0.29 }
        const logical = sourcePointToStagePoint(
          { x: normalized.x * 640, y: normalized.y * 480 },
          state,
          geometry,
        )
        const input = projection.stageToNormalized({
          x: bounds.left + logical.x * browserZoom,
          y: bounds.top + logical.y * browserZoom,
        })
        expect(input?.x).toBeCloseTo(normalized.x)
        expect(input?.y).toBeCloseTo(normalized.y)
        const overlay = projection.normalizedToStage(normalized)
        expect((overlay?.x ?? 0) - projection.stageRect.left).toBeCloseTo(logical.x)
        expect((overlay?.y ?? 0) - projection.stageRect.top).toBeCloseTo(logical.y)
        expect(projection.stageRect).toMatchObject(geometry.stage)
        const outside = sourcePointToStagePoint({ x: -32, y: 120 }, state, geometry)
        const outsideClient = {
          x: bounds.left + outside.x * browserZoom,
          y: bounds.top + outside.y * browserZoom,
        }
        expect(projection.stageToNormalized(outsideClient)).toBeNull()
        const handleInput = projection.stageToNormalized(outsideClient, { allowOutsideImage: true })
        expect(handleInput?.x).toBeCloseTo(-0.05)
        expect(handleInput?.y).toBeCloseTo(0.25)
        expect(projection.stageToNormalized({ x: Number.NaN, y: 0 })).toBeNull()
        expect(
          projection.stageToNormalized(
            { x: 0, y: Number.POSITIVE_INFINITY },
            { allowOutsideImage: true },
          ),
        ).toBeNull()
      })
    }
  }
  it('fails closed before stage measurement and outside normalized image bounds', () => {
    const projection = createImagePreviewProjection(
      null,
      { mode: 'fit', zoom: 1, rotation: 0, offset: { x: 0, y: 0 } },
      { stage: { width: 0, height: 0 }, source: { width: 0, height: 0 }, fitInset: 0.9 },
    )
    expect(projection.stageToNormalized({ x: 5, y: 5 })).toBeNull()
    expect(projection.normalizedToStage({ x: 1.1, y: 0 })).toBeNull()
  })
})
