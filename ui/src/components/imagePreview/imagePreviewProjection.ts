import {
  type ImageViewportGeometry,
  type ImageViewportState,
  type Point,
  type Size,
  type StageRect,
  sourcePointAtStagePoint,
  sourcePointToStagePoint,
  stagePointToSourcePoint,
} from './imageGeometry'
import { isPositiveSize } from './usePreviewStageSize'

export interface ImagePreviewProjection {
  sourceSize: Size
  /** Origin is in client pixels; dimensions and overlay offsets are logical CSS pixels. */
  stageRect: StageRect
  /** Client coordinates include browser/CSS zoom. Existing handles may start outside the image. */
  stageToNormalized(point: Point, options?: { allowOutsideImage: boolean }): Point | null
  /** Subtract stageRect's origin to obtain a local CSS overlay position. */
  normalizedToStage(point: Point): Point | null
}

export function createImagePreviewProjection(
  bounds: StageRect | null,
  state: ImageViewportState,
  geometry: ImageViewportGeometry,
  sourceSize: Size = geometry.source,
): ImagePreviewProjection {
  const ready =
    bounds !== null &&
    isPositiveSize(bounds) &&
    isPositiveSize(geometry.stage) &&
    isPositiveSize(geometry.source)
  const origin = { left: bounds?.left ?? 0, top: bounds?.top ?? 0 }
  return {
    sourceSize,
    stageRect: { ...origin, ...geometry.stage },
    stageToNormalized(point, options) {
      if (!ready || bounds === null || !Number.isFinite(point.x) || !Number.isFinite(point.y))
        return null
      const project = options?.allowOutsideImage ? stagePointToSourcePoint : sourcePointAtStagePoint
      const source = project(
        {
          x: ((point.x - bounds.left) * geometry.stage.width) / bounds.width,
          y: ((point.y - bounds.top) * geometry.stage.height) / bounds.height,
        },
        state,
        geometry,
      )
      return source === null
        ? null
        : { x: source.x / geometry.source.width, y: source.y / geometry.source.height }
    },
    normalizedToStage(point) {
      if (
        !ready ||
        !Number.isFinite(point.x) ||
        !Number.isFinite(point.y) ||
        point.x < 0 ||
        point.x > 1 ||
        point.y < 0 ||
        point.y > 1
      )
        return null
      const local = sourcePointToStagePoint(
        {
          x: point.x * geometry.source.width,
          y: point.y * geometry.source.height,
        },
        state,
        geometry,
      )
      return { x: origin.left + local.x, y: origin.top + local.y }
    },
  }
}
