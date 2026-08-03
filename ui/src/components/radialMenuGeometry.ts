export interface Point {
  x: number
  y: number
}

export interface Viewport {
  width: number
  height: number
}

export const PRIMARY_INNER_RADIUS = 42
export const PRIMARY_OUTER_RADIUS = 108
export const SECONDARY_INNER_RADIUS = 112
export const SECONDARY_OUTER_RADIUS = 168
export const MOTION_THRESHOLD = 8
export const PRIMARY_SECTOR_DEGREES = 60
export const SECONDARY_SECTOR_DEGREES = 30

export function primaryCenterAngle(index: number): number {
  return -90 + index * PRIMARY_SECTOR_DEGREES
}

export function polarPoint(origin: Point, radius: number, degrees: number): Point {
  const radians = (degrees * Math.PI) / 180
  return {
    x: origin.x + radius * Math.cos(radians),
    y: origin.y + radius * Math.sin(radians),
  }
}

export function annularSectorPath(
  origin: Point,
  innerRadius: number,
  outerRadius: number,
  startDegrees: number,
  endDegrees: number,
): string {
  const outerStart = polarPoint(origin, outerRadius, startDegrees)
  const outerEnd = polarPoint(origin, outerRadius, endDegrees)
  const innerEnd = polarPoint(origin, innerRadius, endDegrees)
  const innerStart = polarPoint(origin, innerRadius, startDegrees)
  const largeArc = endDegrees - startDegrees > 180 ? 1 : 0
  return [
    `M ${format(outerStart.x)} ${format(outerStart.y)}`,
    `A ${outerRadius} ${outerRadius} 0 ${largeArc} 1 ${format(outerEnd.x)} ${format(outerEnd.y)}`,
    `L ${format(innerEnd.x)} ${format(innerEnd.y)}`,
    `A ${innerRadius} ${innerRadius} 0 ${largeArc} 0 ${format(innerStart.x)} ${format(innerStart.y)}`,
    'Z',
  ].join(' ')
}

export function primaryIndexAt(point: Point, origin: Point): number | null {
  const radius = distance(point, origin)
  if (radius < PRIMARY_INNER_RADIUS || radius > PRIMARY_OUTER_RADIUS) return null
  const degrees = normalizeDegrees(angle(point, origin) - -120)
  return Math.floor(degrees / PRIMARY_SECTOR_DEGREES) % 6
}

export function secondaryIndexAt(
  point: Point,
  origin: Point,
  anchorDegrees: number,
  itemCount: number,
): number | null {
  const radius = distance(point, origin)
  if (radius < SECONDARY_INNER_RADIUS || radius > SECONDARY_OUTER_RADIUS) return null
  const start = anchorDegrees - (itemCount * SECONDARY_SECTOR_DEGREES) / 2
  const relative = normalizeDegrees(angle(point, origin) - start)
  const span = itemCount * SECONDARY_SECTOR_DEGREES
  if (relative >= span) return null
  return Math.floor(relative / SECONDARY_SECTOR_DEGREES)
}

export function fitMenuOrigin(point: Point, viewport: Viewport): Point {
  const margin = SECONDARY_OUTER_RADIUS + 12
  return {
    x: clamp(point.x, margin, viewport.width - margin),
    y: clamp(point.y, margin, viewport.height - margin),
  }
}

function angle(point: Point, origin: Point): number {
  return (Math.atan2(point.y - origin.y, point.x - origin.x) * 180) / Math.PI
}

function distance(point: Point, origin: Point): number {
  return Math.hypot(point.x - origin.x, point.y - origin.y)
}

function normalizeDegrees(value: number): number {
  return ((value % 360) + 360) % 360
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.max(minimum, Math.min(maximum, value))
}

function format(value: number): string {
  return Number(value.toFixed(2)).toString()
}
