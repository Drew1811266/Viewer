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

export const MAGNIFIER_POINTER_GAP = 18

export interface MagnifierShellPlacement {
  center: Point
  origin: Point
  horizontal: 'left' | 'right'
  vertical: 'above' | 'below'
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

export function magnifierShellPlacement(
  pointer: Point,
  stage: Size,
  lens: Size,
): MagnifierShellPlacement {
  const horizontal = placeAxis(pointer.x, stage.width, lens.width, 'right', 'left')
  const vertical = placeAxis(pointer.y, stage.height, lens.height, 'below', 'above')
  return {
    center: { x: horizontal.center, y: vertical.center },
    origin: { x: horizontal.origin, y: vertical.origin },
    horizontal: horizontal.side,
    vertical: vertical.side,
  }
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

function placeAxis<Positive extends 'right' | 'below', Negative extends 'left' | 'above'>(
  pointer: number,
  stageLength: number,
  lensLength: number,
  positiveName: Positive,
  negativeName: Negative,
): { center: number; side: Positive | Negative; origin: number } {
  const positiveStart = pointer + MAGNIFIER_POINTER_GAP
  const negativeStart = pointer - MAGNIFIER_POINTER_GAP - lensLength
  const positiveFits = positiveStart + lensLength <= stageLength
  const negativeFits = negativeStart >= 0
  const usePositive = positiveFits || (!negativeFits && stageLength - pointer >= pointer)
  const start = clamp(
    usePositive ? positiveStart : negativeStart,
    0,
    Math.max(0, stageLength - lensLength),
  )
  return {
    center: start + lensLength / 2,
    side: usePositive ? positiveName : negativeName,
    origin: usePositive ? 0 : lensLength,
  }
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, value))
}
