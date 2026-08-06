import type { ThumbnailDensity } from '../api/types'

export type ThumbnailLevel = 1 | 2 | 3 | 4 | 5

export interface ThumbnailLevelOption {
  level: ThumbnailLevel
  density: ThumbnailDensity
  height: number
}

export const THUMBNAIL_LEVELS = [
  { level: 1, density: 'compact', height: 96 },
  { level: 2, density: 'standard', height: 132 },
  { level: 3, density: 'large', height: 168 },
  { level: 4, density: 'extra_large', height: 204 },
  { level: 5, density: 'maximum', height: 240 },
] as const satisfies readonly ThumbnailLevelOption[]

export const THUMBNAIL_HEIGHT = THUMBNAIL_LEVELS.reduce<Record<ThumbnailDensity, number>>(
  (heightByDensity, option) => {
    heightByDensity[option.density] = option.height
    return heightByDensity
  },
  {} as Record<ThumbnailDensity, number>,
)

function optionForLevel(level: number): ThumbnailLevelOption {
  const option = THUMBNAIL_LEVELS.find((candidate) => candidate.level === level)
  if (option === undefined) throw new RangeError(`Unsupported thumbnail level: ${level}`)
  return option
}

export function thumbnailDensityForLevel(level: number): ThumbnailDensity {
  return optionForLevel(level).density
}

export function thumbnailLevelForDensity(density: ThumbnailDensity): ThumbnailLevel {
  const option = THUMBNAIL_LEVELS.find((candidate) => candidate.density === density)
  if (option === undefined) throw new RangeError(`Unsupported thumbnail density: ${density}`)
  return option.level
}

export function thumbnailLevelValueText(level: ThumbnailLevel): string {
  const option = optionForLevel(level)
  return `档位 ${option.level}，${option.height} 像素`
}

export const MAX_THUMBNAIL_DEVICE_SCALE = 4
export const MAX_THUMBNAIL_PHYSICAL_EDGE = 4096

export function thumbnailRequestSize(width: number, height: number, devicePixelRatio: number) {
  const scale = Math.min(
    MAX_THUMBNAIL_DEVICE_SCALE,
    Math.max(1, Number.isFinite(devicePixelRatio) ? devicePixelRatio : 1),
  )
  return {
    maxPixels: Math.min(
      MAX_THUMBNAIL_PHYSICAL_EDGE,
      Math.max(1, Math.ceil(Math.max(width, height) * scale)),
    ),
    scaleMilli: Math.round(scale * 1_000),
  }
}
