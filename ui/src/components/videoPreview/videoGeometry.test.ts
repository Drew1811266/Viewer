import { describe, expect, it } from 'vitest'
import type { VideoMedia } from '../../api/types'
import { fitVideoSurfaceLayout, hasVideoGeometry } from './videoGeometry'

function stage(left: number, top: number, width: number, height: number): DOMRectReadOnly {
  return {
    x: left,
    y: top,
    left,
    top,
    right: left + width,
    bottom: top + height,
    width,
    height,
    toJSON: () => undefined,
  }
}

function media(overrides: Partial<VideoMedia> = {}): VideoMedia {
  return {
    durationUs: 10_000_000,
    displayWidth: 1_920,
    displayHeight: 1_080,
    rotationDegrees: 0,
    ...overrides,
  }
}

describe('native video surface geometry', () => {
  it('centers a landscape video inside stage coordinates', () => {
    expect(fitVideoSurfaceLayout(stage(100, 50, 800, 600), media())).toEqual({
      surfaceRect: { x: 100, y: 125, width: 800, height: 450 },
      aperture: { left: 0, top: 75, width: 800, height: 450 },
    })
  })

  it.each([90, -90, 270])('swaps display axes for a %d-degree rotation', (rotationDegrees) => {
    expect(fitVideoSurfaceLayout(stage(100, 50, 800, 600), media({ rotationDegrees }))).toEqual({
      surfaceRect: { x: 331, y: 50, width: 338, height: 600 },
      aperture: { left: 231, top: 0, width: 338, height: 600 },
    })
  })

  it('derives the local aperture from the exact integer native rect for fractional geometry', () => {
    expect(fitVideoSurfaceLayout(stage(0.4, 0.4, 800.2, 600.2), media())).toEqual({
      surfaceRect: { x: 0, y: 75, width: 800, height: 450 },
      aperture: { left: -0.4, top: 74.6, width: 800, height: 450 },
    })
  })

  it('requires positive stage and media dimensions before fitting', () => {
    expect(hasVideoGeometry(stage(0, 0, 0, 600), media())).toBe(false)
    expect(hasVideoGeometry(stage(0, 0, 800, 600), media({ displayWidth: null }))).toBe(false)
    expect(hasVideoGeometry(stage(0, 0, 800, 600), media({ displayHeight: 0 }))).toBe(false)
    expect(hasVideoGeometry(stage(0, 0, 800, 600), media())).toBe(true)
  })
})
