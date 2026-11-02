import { describe, expect, it } from 'vitest'
import { defaultAnnotationStyle, styleForAnchor } from './annotationStyle'

describe('annotation style defaults', () => {
  it('matches the upgraded tool appearance when a historical target has no style', () => {
    expect(defaultAnnotationStyle('brush')).toMatchObject({
      color: 'red',
      width: 'thick',
      opacityPercent: 100,
      dashed: false,
    })
    expect(defaultAnnotationStyle('rectangle')).toMatchObject({
      color: 'red',
      width: 'medium',
    })
    expect(defaultAnnotationStyle('highlighter')).toMatchObject({
      width: 'thick',
      opacityPercent: 35,
    })
    expect(
      styleForAnchor({
        kind: 'image_stroke',
        points: [
          { x: 0.1, y: 0.2 },
          { x: 0.4, y: 0.5 },
        ],
      }),
    ).toMatchObject({ tool: 'brush', width: 'thick' })
    expect(
      styleForAnchor({ kind: 'image_rect', x: 0.1, y: 0.2, width: 0.3, height: 0.2 }),
    ).toMatchObject({ tool: 'rectangle', width: 'medium' })
    expect(
      styleForAnchor({
        kind: 'image_line',
        tail: { x: 0.1, y: 0.2 },
        head: { x: 0.8, y: 0.7 },
      }),
    ).toMatchObject({ tool: 'line', width: 'medium' })
  })
})
