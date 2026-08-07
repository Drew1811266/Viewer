export type PreviewMode = 'fit' | 'original' | 'free'
export type PreviewRotation = 0 | 90 | 180 | 270

export interface Point {
  x: number
  y: number
}

export interface Size {
  width: number
  height: number
}

export interface ImageViewportState {
  mode: PreviewMode
  zoom: number
  rotation: PreviewRotation
  offset: Point
}

export interface ImageViewportGeometry {
  stage: Size
  source: Size
  fitInset: number
}

export const MIN_PREVIEW_ZOOM = 0.1
export const MAX_PREVIEW_ZOOM = 8

export function displayScale(state: ImageViewportState, geometry: ImageViewportGeometry): number {
  if (!hasArea(geometry.stage) || !hasArea(geometry.source)) return 0
  if (state.mode === 'original') return 1
  const quarterTurn = state.rotation === 90 || state.rotation === 270
  const sourceWidth = quarterTurn ? geometry.source.height : geometry.source.width
  const sourceHeight = quarterTurn ? geometry.source.width : geometry.source.height
  const fitScale = Math.min(
    1,
    (geometry.stage.width * geometry.fitInset) / sourceWidth,
    (geometry.stage.height * geometry.fitInset) / sourceHeight,
  )
  return fitScale * (state.mode === 'free' ? state.zoom : 1)
}

export function panBounds(state: ImageViewportState, geometry: ImageViewportGeometry): Point {
  const scale = displayScale(state, geometry)
  if (scale === 0) return { x: 0, y: 0 }
  const quarterTurn = state.rotation === 90 || state.rotation === 270
  const width = (quarterTurn ? geometry.source.height : geometry.source.width) * scale
  const height = (quarterTurn ? geometry.source.width : geometry.source.height) * scale
  return {
    x: Math.max(0, (width - geometry.stage.width) / 2),
    y: Math.max(0, (height - geometry.stage.height) / 2),
  }
}

export function clampOffset(offset: Point, bounds: Point): Point {
  return {
    x: clamp(offset.x, -bounds.x, bounds.x),
    y: clamp(offset.y, -bounds.y, bounds.y),
  }
}

export function sourcePointToStagePoint(
  sourcePoint: Point,
  state: ImageViewportState,
  geometry: ImageViewportGeometry,
): Point {
  const scale = displayScale(state, geometry)
  const centered = {
    x: (sourcePoint.x - geometry.source.width / 2) * scale,
    y: (sourcePoint.y - geometry.source.height / 2) * scale,
  }
  const rotated = rotateClockwise(centered, state.rotation)
  return {
    x: geometry.stage.width / 2 + state.offset.x + rotated.x,
    y: geometry.stage.height / 2 + state.offset.y + rotated.y,
  }
}

export function stagePointToSourcePoint(
  stagePoint: Point,
  state: ImageViewportState,
  geometry: ImageViewportGeometry,
): Point | null {
  const scale = displayScale(state, geometry)
  if (scale === 0) return null
  const translated = {
    x: stagePoint.x - geometry.stage.width / 2 - state.offset.x,
    y: stagePoint.y - geometry.stage.height / 2 - state.offset.y,
  }
  const unrotated = rotateCounterClockwise(translated, state.rotation)
  return {
    x: unrotated.x / scale + geometry.source.width / 2,
    y: unrotated.y / scale + geometry.source.height / 2,
  }
}

export function sourcePointAtStagePoint(
  stagePoint: Point,
  state: ImageViewportState,
  geometry: ImageViewportGeometry,
): Point | null {
  const sourcePoint = stagePointToSourcePoint(stagePoint, state, geometry)
  if (
    sourcePoint === null ||
    sourcePoint.x < 0 ||
    sourcePoint.y < 0 ||
    sourcePoint.x > geometry.source.width ||
    sourcePoint.y > geometry.source.height
  ) {
    return null
  }
  return sourcePoint
}

export function remapSourcePoint(sourcePoint: Point, from: Size, to: Size): Point {
  if (!hasArea(from) || !hasArea(to)) return { x: 0, y: 0 }
  return {
    x: (sourcePoint.x / from.width) * to.width,
    y: (sourcePoint.y / from.height) * to.height,
  }
}

export function zoomAtAnchor(
  state: ImageViewportState,
  geometry: ImageViewportGeometry,
  factor: number,
  anchor: Point,
): ImageViewportState {
  const currentZoom = state.mode === 'free' ? state.zoom : 1
  const next: ImageViewportState = {
    ...state,
    mode: 'free',
    zoom: clamp(currentZoom * factor, MIN_PREVIEW_ZOOM, MAX_PREVIEW_ZOOM),
    offset: { x: 0, y: 0 },
  }
  const sourcePoint = stagePointToSourcePoint(anchor, state, geometry)
  if (sourcePoint === null) return next
  const unanchoredPoint = sourcePointToStagePoint(sourcePoint, next, geometry)
  const desiredOffset = {
    x: anchor.x - unanchoredPoint.x,
    y: anchor.y - unanchoredPoint.y,
  }
  return {
    ...next,
    offset: clampOffset(desiredOffset, panBounds(next, geometry)),
  }
}

function rotateClockwise(point: Point, rotation: PreviewRotation): Point {
  if (rotation === 90) return { x: -point.y, y: point.x }
  if (rotation === 180) return { x: -point.x, y: -point.y }
  if (rotation === 270) return { x: point.y, y: -point.x }
  return point
}

function rotateCounterClockwise(point: Point, rotation: PreviewRotation): Point {
  if (rotation === 90) return { x: point.y, y: -point.x }
  if (rotation === 180) return { x: -point.x, y: -point.y }
  if (rotation === 270) return { x: -point.y, y: point.x }
  return point
}

function hasArea(size: Size): boolean {
  return (
    Number.isFinite(size.width) && Number.isFinite(size.height) && size.width > 0 && size.height > 0
  )
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.max(minimum, Math.min(maximum, value))
}
