import type { MagnifierContentProjection } from './magnifierGeometry'

export interface MagnifierOverlayFrame {
  projection: MagnifierContentProjection
  magnification: number
  pixelRatio: number
}

export type MagnifierOverlayPainter = (
  context: CanvasRenderingContext2D,
  frame: MagnifierOverlayFrame,
) => number
