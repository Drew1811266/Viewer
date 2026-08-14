import type { VideoMedia, VideoSurfaceRect } from '../../api/types'

export interface VideoMatteInsets {
  top: number
  right: number
  bottom: number
  left: number
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

export function fitVideoRect(stage: DOMRectReadOnly, media: VideoMedia): VideoSurfaceRect {
  if (!hasVideoGeometry(stage, media)) throw new RangeError('Video geometry is not ready')
  const width = media.displayWidth as number
  const height = media.displayHeight as number
  const rotated = Math.abs(media.rotationDegrees % 180) === 90
  const sourceWidth = rotated ? height : width
  const sourceHeight = rotated ? width : height
  const scale = Math.min(stage.width / sourceWidth, stage.height / sourceHeight)
  const fittedWidth = sourceWidth * scale
  const fittedHeight = sourceHeight * scale
  return {
    x: Math.round(stage.left + (stage.width - fittedWidth) / 2),
    y: Math.round(stage.top + (stage.height - fittedHeight) / 2),
    width: Math.round(fittedWidth),
    height: Math.round(fittedHeight),
  }
}

export function fitVideoMatteInsets(stage: DOMRectReadOnly, media: VideoMedia): VideoMatteInsets {
  const fitted = fitVideoRect(stage, media)
  const left = Math.max(0, fitted.x - stage.left)
  const top = Math.max(0, fitted.y - stage.top)
  return {
    top,
    right: Math.max(0, stage.width - left - fitted.width),
    bottom: Math.max(0, stage.height - top - fitted.height),
    left,
  }
}

function finitePositive(value: number | null): value is number {
  return value !== null && Number.isFinite(value) && value > 0
}
