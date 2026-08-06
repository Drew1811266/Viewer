import { describe, expect, it } from 'vitest'
import type { ThumbnailDensity } from '../api/types'
import {
  THUMBNAIL_HEIGHT,
  THUMBNAIL_LEVELS,
  thumbnailDensityForLevel,
  thumbnailLevelForDensity,
  thumbnailLevelValueText,
  thumbnailRequestSize,
} from './thumbnailDensity'

const expectedLevels = [
  { level: 1, density: 'compact', height: 96 },
  { level: 2, density: 'standard', height: 132 },
  { level: 3, density: 'large', height: 168 },
  { level: 4, density: 'extra_large', height: 204 },
  { level: 5, density: 'maximum', height: 240 },
] as const

describe('thumbnail density levels', () => {
  it('maps every visible level to one typed density and exact height', () => {
    expect(THUMBNAIL_LEVELS).toEqual(expectedLevels)
    expect(THUMBNAIL_HEIGHT).toEqual({
      compact: 96,
      standard: 132,
      large: 168,
      extra_large: 204,
      maximum: 240,
    })
  })

  it.each(expectedLevels)(
    'round-trips visible level $level through $density',
    ({ level, density, height }) => {
      expect(thumbnailDensityForLevel(level)).toBe(density)
      expect(thumbnailLevelForDensity(density)).toBe(level)
      expect(thumbnailLevelValueText(level)).toBe(`档位 ${level}，${height} 像素`)
    },
  )

  it.each([0, 2.5, 6])('rejects non-stop level %s', (level) => {
    expect(() => thumbnailDensityForLevel(level)).toThrow(RangeError)
  })

  it('keeps the maximum DPR request below the existing safety cap', () => {
    expect(thumbnailRequestSize(240, 240, 4)).toEqual({
      maxPixels: 960,
      scaleMilli: 4_000,
    })
  })

  it('keeps the mapping compatible with the public density type', () => {
    const publicValues: ThumbnailDensity[] = expectedLevels.map(({ density }) => density)
    expect(publicValues).toEqual(['compact', 'standard', 'large', 'extra_large', 'maximum'])
  })
})
