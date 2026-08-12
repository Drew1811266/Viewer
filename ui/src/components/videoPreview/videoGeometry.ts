import type { VideoMedia, VideoSurfaceRect } from '../../api/types'

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

function finitePositive(value: number | null): value is number {
  return value !== null && Number.isFinite(value) && value > 0
}
