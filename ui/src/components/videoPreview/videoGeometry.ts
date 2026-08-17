import type { VideoMedia, VideoSurfaceRect } from '../../api/types'

export interface VideoAperture {
  left: number
  top: number
  width: number
  height: number
}

export interface VideoSurfaceLayout {
  surfaceRect: VideoSurfaceRect
  aperture: VideoAperture
}

export function hasVideoGeometry(stage: DOMRectReadOnly, media: VideoMedia): boolean {
  return (
    finitePositive(stage.width) &&
    finitePositive(stage.height) &&
    Number.isFinite(stage.left) &&
    Number.isFinite(stage.top) &&
    finitePositive(media.displayWidth) &&
    finitePositive(media.displayHeight)
  )
}

export function fitVideoSurfaceLayout(
  stage: DOMRectReadOnly,
  media: VideoMedia,
): VideoSurfaceLayout {
  if (!hasVideoGeometry(stage, media)) throw new RangeError('Video geometry is not ready')
  const width = media.displayWidth as number
  const height = media.displayHeight as number
  const rotated = Math.abs(media.rotationDegrees % 180) === 90
  const sourceWidth = rotated ? height : width
  const sourceHeight = rotated ? width : height
  const scale = Math.min(stage.width / sourceWidth, stage.height / sourceHeight)
  const fittedWidth = sourceWidth * scale
  const fittedHeight = sourceHeight * scale
  const surfaceRect = {
    x: Math.round(stage.left + (stage.width - fittedWidth) / 2),
    y: Math.round(stage.top + (stage.height - fittedHeight) / 2),
    width: Math.round(fittedWidth),
    height: Math.round(fittedHeight),
  }
  return {
    surfaceRect,
    aperture: {
      left: normalizeZero(surfaceRect.x - stage.left),
      top: normalizeZero(surfaceRect.y - stage.top),
      width: surfaceRect.width,
      height: surfaceRect.height,
    },
  }
}

function finitePositive(value: number | null): value is number {
  return value !== null && Number.isFinite(value) && value > 0
}

function normalizeZero(value: number): number {
  return Object.is(value, -0) ? 0 : value
}
