import type {
  ReviewAnchor,
  ReviewAnnotationColor,
  ReviewAnnotationStyle,
  ReviewAnnotationTextSize,
  ReviewAnnotationTool,
  ReviewAnnotationWidth,
} from '../../api/types'

export type {
  ReviewAnchor,
  ReviewAnnotationColor,
  ReviewAnnotationStyle,
} from '../../api/types'

const COLORS: Record<ReviewAnnotationColor, string> = {
  red: '#C62828',
  orange: '#EF6C00',
  yellow: '#F9A825',
  green: '#2E7D32',
  teal: '#00695C',
  blue: '#1565C0',
  purple: '#6A1B9A',
  black: '#212121',
}

const WIDTHS: Record<ReviewAnnotationWidth, number> = {
  thin: 1.5,
  medium: 2,
  thick: 4,
}

const TEXT_SIZES: Record<ReviewAnnotationTextSize, number> = {
  small: 12,
  medium: 14,
  large: 18,
}

export const ANNOTATION_COLORS = Object.keys(COLORS) as ReviewAnnotationColor[]
export const ANNOTATION_WIDTHS = Object.keys(WIDTHS) as ReviewAnnotationWidth[]
export const ANNOTATION_TEXT_SIZES = Object.keys(TEXT_SIZES) as ReviewAnnotationTextSize[]

export const HIGHLIGHTER_WIDTH_PX = 12
export const HIGHLIGHTER_OPACITY = 35
export const RULER_WIDTH_PX = 1.5

// Annotation visual system v2 (docs/design/0.3.0 frames 35-37).
export const ANNOTATION_HALO_COLOR = 'rgba(255, 255, 255, 0.9)'
export const ANNOTATION_HALO_PER_SIDE = 1.5
export const ANNOTATION_FILL_OPACITY = 0.1
export const ANNOTATION_CORNER_RADIUS = 4
export const ANNOTATION_BADGE_RADIUS = 10
export const ANNOTATION_BADGE_RING_WIDTH = 1.5
export const ANNOTATION_BADGE_FONT_PX = 10
export const ANNOTATION_SELECTED_RING_RADIUS = 14
export const ANNOTATION_SELECTED_RING_WIDTH = 2
export const ANNOTATION_SELECTION_OFFSET = 5
export const ANNOTATION_SELECTION_WIDTH = 1.5
export const ANNOTATION_HANDLE_SIZE = 8
export const ANNOTATION_HANDLE_HIT_SIZE = 24
export const TEXT_NOTE_RADIUS = 8
export const TEXT_NOTE_PADDING = 8
export const TEXT_NOTE_ACCENT_WIDTH = 3
export const TEXT_NOTE_MAX_WIDTH = 240

export function defaultAnnotationStyle(tool: ReviewAnnotationTool): ReviewAnnotationStyle {
  return {
    tool,
    color: 'red',
    width: tool === 'highlighter' || tool === 'brush' ? 'thick' : 'medium',
    opacityPercent: tool === 'highlighter' ? HIGHLIGHTER_OPACITY : 100,
    dashed: false,
    ...(tool === 'text' ? { textSize: 'medium' as const } : {}),
  }
}

export function withAnnotationTool(
  style: ReviewAnnotationStyle,
  tool: ReviewAnnotationTool,
): ReviewAnnotationStyle {
  return normalizeAnnotationStyle({
    ...style,
    tool,
    ...(tool === 'text' && style.textSize === undefined ? { textSize: 'medium' } : {}),
    ...(tool === 'highlighter'
      ? { opacityPercent: HIGHLIGHTER_OPACITY, dashed: false }
      : { opacityPercent: clampOpacity(style.opacityPercent) }),
    ...(supportsDashed(tool) ? {} : { dashed: false }),
  })
}

export function normalizeAnnotationStyle(style: ReviewAnnotationStyle): ReviewAnnotationStyle {
  const opacityPercent = supportsOpacity(style.tool)
    ? style.tool === 'highlighter'
      ? HIGHLIGHTER_OPACITY
      : clampOpacity(style.opacityPercent)
    : 100
  return {
    tool: style.tool,
    color: style.color,
    width: style.width,
    opacityPercent,
    dashed: supportsDashed(style.tool) ? style.dashed : false,
    ...(style.tool === 'text' ? { textSize: style.textSize ?? ('medium' as const) } : {}),
  }
}

export function styleForAnchor(
  anchor: ReviewAnchor,
  persisted?: ReviewAnnotationStyle | null,
): ReviewAnnotationStyle | null {
  const tool = annotationToolForAnchor(anchor, persisted)
  if (tool === null) return null
  if (persisted !== undefined && persisted !== null && persisted.tool === tool) {
    return normalizeAnnotationStyle(persisted)
  }
  return defaultAnnotationStyle(tool)
}

export function annotationToolForAnchor(
  anchor: ReviewAnchor,
  style?: ReviewAnnotationStyle | null,
): ReviewAnnotationTool | null {
  switch (anchor.kind) {
    case 'image_point':
      return 'point'
    case 'image_arrow':
      return 'arrow'
    case 'image_stroke':
      return style?.tool === 'highlighter' ? 'highlighter' : 'brush'
    case 'image_rect':
      return 'rectangle'
    case 'image_ellipse':
      return 'ellipse'
    case 'image_text':
      return 'text'
    case 'image_line':
      return style?.tool === 'ruler' ? 'ruler' : 'line'
    case 'image_polygon':
      return 'polygon'
    case 'asset':
    case 'video_point':
    case 'video_range':
      return null
  }
}

export function supportsDashed(tool: ReviewAnnotationTool): boolean {
  return tool === 'rectangle' || tool === 'ellipse' || tool === 'line' || tool === 'polygon'
}

export function supportsOpacity(tool: ReviewAnnotationTool): boolean {
  return tool !== 'text'
}

export function supportsColor(tool: ReviewAnnotationTool): boolean {
  return tool !== 'ruler'
}

export function supportsWidth(tool: ReviewAnnotationTool): boolean {
  return tool !== 'ruler' && tool !== 'text'
}

export function supportsTextSize(tool: ReviewAnnotationTool): boolean {
  return tool === 'text'
}

export function isExtendedAnnotationTool(tool: string): boolean {
  return (
    tool === 'text' ||
    tool === 'line' ||
    tool === 'highlighter' ||
    tool === 'polygon' ||
    tool === 'ruler'
  )
}

export function isExtendedAnnotationStyle(style?: ReviewAnnotationStyle | null): boolean {
  return style !== undefined && style !== null && isExtendedAnnotationTool(style.tool)
}

export function annotationColorHex(color: ReviewAnnotationColor): string {
  return COLORS[color]
}

export function annotationColorRgba(
  style: Pick<ReviewAnnotationStyle, 'color' | 'opacityPercent'>,
): string {
  return colorRgba(style.color, clampOpacity(style.opacityPercent) / 100)
}

export function annotationHaloWidth(style: ReviewAnnotationStyle): number {
  return annotationLineWidthCss(style) + ANNOTATION_HALO_PER_SIDE * 2
}

export function annotationArrowHead(width: number): { length: number; halfWidth: number } {
  const scale = Math.max(1, width / 2)
  return { length: 11 * scale, halfWidth: 5.2 * scale }
}

export function annotationFillRgba(style: Pick<ReviewAnnotationStyle, 'color'>): string {
  return colorRgba(style.color, ANNOTATION_FILL_OPACITY)
}

export function annotationRgb(style: Pick<ReviewAnnotationStyle, 'color'>): string {
  const hex = COLORS[style.color]
  return `rgb(${Number.parseInt(hex.slice(1, 3), 16)}, ${Number.parseInt(
    hex.slice(3, 5),
    16,
  )}, ${Number.parseInt(hex.slice(5, 7), 16)})`
}

export function annotationToneRgba(color: ReviewAnnotationColor, alpha: number): string {
  return colorRgba(color, alpha)
}

function colorRgba(color: ReviewAnnotationColor, alpha: number): string {
  const hex = COLORS[color]
  return `rgba(${Number.parseInt(hex.slice(1, 3), 16)}, ${Number.parseInt(
    hex.slice(3, 5),
    16,
  )}, ${Number.parseInt(hex.slice(5, 7), 16)}, ${alpha})`
}

export function annotationLineWidthCss(style: ReviewAnnotationStyle): number {
  if (style.tool === 'highlighter') return HIGHLIGHTER_WIDTH_PX
  if (style.tool === 'ruler') return RULER_WIDTH_PX
  return WIDTHS[style.width]
}

export function annotationTextSizeCss(style: ReviewAnnotationStyle): number {
  return TEXT_SIZES[style.textSize ?? 'medium']
}

export function annotationTextSizeLabel(style: ReviewAnnotationStyle): string {
  return String(TEXT_SIZES[style.textSize ?? 'medium'])
}

export function nextAnnotationTextSize(size: ReviewAnnotationTextSize): ReviewAnnotationTextSize {
  const index = ANNOTATION_TEXT_SIZES.indexOf(size)
  return ANNOTATION_TEXT_SIZES[
    (index + 1) % ANNOTATION_TEXT_SIZES.length
  ] as ReviewAnnotationTextSize
}

export function nextAnnotationOpacity(percent: number): number {
  const values = [100, 80, 60, 40, 20]
  return values.find((value) => value < percent) ?? values[0] ?? 100
}

export function annotationStyleKey(style?: ReviewAnnotationStyle | null): string {
  return style === undefined || style === null ? 'none' : JSON.stringify(style)
}

function clampOpacity(value: number): number {
  if (!Number.isFinite(value)) return 100
  return Math.max(20, Math.min(100, Math.round(value)))
}
