import { describe, expect, it } from 'vitest'
import type { VideoMedia } from '../../api/types'
import { fitVideoMatteInsets, fitVideoRect, hasVideoGeometry } from './videoGeometry'

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
    expect(fitVideoRect(stage(100, 50, 800, 600), media())).toEqual({
      x: 100,
      y: 125,
      width: 800,
      height: 450,
    })
  })

  it.each([90, -90, 270])('swaps display axes for a %d-degree rotation', (rotationDegrees) => {
    expect(fitVideoRect(stage(100, 50, 800, 600), media({ rotationDegrees }))).toEqual({
      x: 331,
      y: 50,
      width: 338,
      height: 600,
    })
  })

  it('describes the exact opaque matte outside the fitted native surface', () => {
    expect(fitVideoMatteInsets(stage(100, 50, 800, 600), media())).toEqual({
      top: 75,
      right: 0,
      bottom: 75,
      left: 0,
    })
    expect(fitVideoMatteInsets(stage(100, 50, 800, 600), media({ rotationDegrees: 90 }))).toEqual({
      top: 0,
      right: 231,
      bottom: 0,
      left: 231,
    })
  })

  it('never publishes a negative matte width for fractional window geometry', () => {
    const insets = fitVideoMatteInsets(stage(0.4, 0.4, 800.2, 600.2), media())
    expect(Object.values(insets).every((value) => value >= 0)).toBe(true)
  })

  it('requires positive stage and media dimensions before fitting', () => {
    expect(hasVideoGeometry(stage(0, 0, 0, 600), media())).toBe(false)
    expect(hasVideoGeometry(stage(0, 0, 800, 600), media({ displayWidth: null }))).toBe(false)
    expect(hasVideoGeometry(stage(0, 0, 800, 600), media({ displayHeight: 0 }))).toBe(false)
    expect(hasVideoGeometry(stage(0, 0, 800, 600), media())).toBe(true)
  })
})
