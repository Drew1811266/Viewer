import { describe, expect, it } from 'vitest'
import type { ReviewAnchor } from '../../api/types'
import { reviewAnchorLabel } from './reviewAnchorLabel'

describe('reviewAnchorLabel', () => {
  it('labels every image anchor kind without falling back to the whole image', () => {
    const labels = new Map<ReviewAnchor['kind'], string>([
      ['asset', '整图意见'],
      ['image_point', '点位'],
      ['image_arrow', '箭头'],
      ['image_line', '直线'],
      ['image_stroke', '画笔轨迹'],
      ['image_rect', '矩形区域'],
      ['image_ellipse', '椭圆区域'],
      ['image_text', '文字标注'],
      ['image_polygon', '多边形区域'],
      ['video_point', '视频点位'],
      ['video_range', '视频片段'],
    ])
    for (const [kind, label] of labels) expect(reviewAnchorLabel(kind)).toBe(label)
  })

  it('treats a missing target anchor as a whole-image opinion', () => {
    expect(reviewAnchorLabel(undefined)).toBe('整图意见')
  })

  it('never labels an image region as a whole-image opinion', () => {
    const imageKinds: ReviewAnchor['kind'][] = [
      'image_point',
      'image_arrow',
      'image_line',
      'image_stroke',
      'image_rect',
      'image_ellipse',
      'image_text',
      'image_polygon',
    ]
    for (const kind of imageKinds) expect(reviewAnchorLabel(kind)).not.toBe('整图意见')
  })
})
