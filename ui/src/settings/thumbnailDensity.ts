import type { ThumbnailDensity } from '../api/types'

export const THUMBNAIL_HEIGHT: Record<ThumbnailDensity, number> = {
  compact: 96,
  standard: 132,
  large: 168,
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
