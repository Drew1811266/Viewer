import type { MagnifierArea, MagnifierMagnification, MagnifierShape } from '../../api/types'
import type { Point, Size } from './imageGeometry'

const LENS_DIMENSIONS: Record<MagnifierArea, Record<MagnifierShape, Size>> = {
  small: {
    circle: { width: 160, height: 160 },
    rounded_rectangle: { width: 180, height: 120 },
  },
  medium: {
    circle: { width: 220, height: 220 },
    rounded_rectangle: { width: 240, height: 160 },
  },
  large: {
    circle: { width: 300, height: 300 },
    rounded_rectangle: { width: 330, height: 220 },
  },
}

export interface MagnifierSourcePlacement {
  left: number
  top: number
  transformOriginX: number
  transformOriginY: number
}

export function lensDimensions(shape: MagnifierShape, area: MagnifierArea): Size {
  return LENS_DIMENSIONS[area][shape]
}

export function magnifierSourcePlacement(
  sourcePoint: Point,
  dimensions: Size,
  _magnification: MagnifierMagnification,
): MagnifierSourcePlacement {
  return {
    left: dimensions.width / 2 - sourcePoint.x,
    top: dimensions.height / 2 - sourcePoint.y,
    transformOriginX: sourcePoint.x,
    transformOriginY: sourcePoint.y,
  }
}
