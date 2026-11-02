import type { ReviewAnchor } from '../../api/types'

/**
 * Single user-facing label for every review anchor kind.
 *
 * The switch is exhaustive on purpose: adding a new anchor kind must fail the
 * type check instead of silently rendering an empty or wrong label in the
 * feedback rail, archive dialog or history panel.
 */
export function reviewAnchorLabel(kind: ReviewAnchor['kind'] | undefined): string {
  switch (kind) {
    case undefined:
    case 'asset':
      return '整图意见'
    case 'image_point':
      return '点位'
    case 'image_arrow':
      return '箭头'
    case 'image_line':
      return '直线'
    case 'image_stroke':
      return '画笔轨迹'
    case 'image_rect':
      return '矩形区域'
    case 'image_ellipse':
      return '椭圆区域'
    case 'image_text':
      return '文字标注'
    case 'image_polygon':
      return '多边形区域'
    case 'video_point':
      return '视频点位'
    case 'video_range':
      return '视频片段'
    default:
      return assertNever(kind)
  }
}

function assertNever(value: never): never {
  throw new Error(`Unsupported review anchor kind: ${String(value)}`)
}
