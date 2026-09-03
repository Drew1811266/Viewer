import { describe, expect, it } from 'vitest'
import type { AnnotationHitTestItem } from './annotationHitTest'
import { hitTestAnnotation } from './annotationHitTest'

const arrow = (itemId: string, selected = false): AnnotationHitTestItem => ({
  itemId,
  ordinal: 1,
  selected,
  anchor: {
    kind: 'image_arrow',
    tail: { x: 0.2, y: 0.5 },
    head: { x: 0.8, y: 0.5 },
  },
})

describe('annotation hit testing', () => {
  it('keeps one fixed eight CSS pixel endpoint tolerance at every zoom', () => {
    for (const zoom of [0.5, 1, 4]) {
      const imageSizeCss = { width: 1_000 * zoom, height: 800 * zoom }
      const sevenPixelsBelowHead = { x: 0.8, y: 0.5 + 7 / imageSizeCss.height }
      const ninePixelsBelowHead = { x: 0.8, y: 0.5 + 9 / imageSizeCss.height }

      expect(
        hitTestAnnotation({ point: sevenPixelsBelowHead, imageSizeCss, items: [arrow('arrow')] }),
      ).toMatchObject({ itemId: 'arrow', part: 'handle', handle: 'head' })
      expect(
        hitTestAnnotation({ point: ninePixelsBelowHead, imageSizeCss, items: [arrow('arrow')] }),
      ).toBeNull()
    }
  })

  it('orders overlaps by handle, selected item, ordinal, distance, then later item', () => {
    const imageSizeCss = { width: 1_000, height: 1_000 }
    const point = { x: 0.5, y: 0.5 }
    const selectedRect: AnnotationHitTestItem = {
      itemId: 'selected',
      ordinal: 2,
      selected: true,
      anchor: { kind: 'image_rect', x: 0.4, y: 0.4, width: 0.2, height: 0.2 },
    }
    const handleArrow = arrow('handle')
    handleArrow.anchor = {
      kind: 'image_arrow',
      tail: { x: 0.1, y: 0.1 },
      head: point,
    }
    expect(
      hitTestAnnotation({ point, imageSizeCss, items: [selectedRect, handleArrow] }),
    ).toMatchObject({ itemId: 'handle', part: 'handle' })

    const ordinal: AnnotationHitTestItem = {
      ...arrow('ordinal'),
      ordinalPoint: point,
    }
    expect(
      hitTestAnnotation({ point, imageSizeCss, items: [ordinal, selectedRect] }),
    ).toMatchObject({ itemId: 'selected' })

    const unselectedRect = { ...selectedRect, itemId: 'rect', selected: false }
    expect(
      hitTestAnnotation({ point, imageSizeCss, items: [unselectedRect, ordinal] }),
    ).toMatchObject({ itemId: 'ordinal', part: 'ordinal' })

    const farther = arrow('farther')
    const nearer = arrow('nearer')
    farther.anchor = {
      kind: 'image_arrow',
      tail: { x: 0.2, y: 0.495 },
      head: { x: 0.8, y: 0.495 },
    }
    nearer.anchor = {
      kind: 'image_arrow',
      tail: { x: 0.2, y: 0.499 },
      head: { x: 0.8, y: 0.499 },
    }
    expect(hitTestAnnotation({ point, imageSizeCss, items: [farther, nearer] })).toMatchObject({
      itemId: 'nearer',
    })

    const earlier = arrow('earlier')
    const later = arrow('later')
    expect(hitTestAnnotation({ point, imageSizeCss, items: [earlier, later] })).toMatchObject({
      itemId: 'later',
    })
  })
})
