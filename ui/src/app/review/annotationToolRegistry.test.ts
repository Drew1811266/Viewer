import { describe, expect, it } from 'vitest'
import { ANNOTATION_TOOLS, toolForShortcut } from './annotationToolRegistry'

describe('annotation tool registry', () => {
  it('keeps the approved fixed menu order and shortcuts', () => {
    expect(ANNOTATION_TOOLS.map((tool) => tool.id)).toEqual([
      'point',
      'arrow',
      'brush',
      'rectangle',
      'ellipse',
    ])
    expect(ANNOTATION_TOOLS.map((tool) => tool.shortcut)).toEqual(['P', 'A', 'B', 'R', 'O'])
    expect(toolForShortcut('o')?.id).toBe('ellipse')
  })

  it('has unique stable ids, shortcuts, icons, and menu positions', () => {
    for (const key of ['id', 'shortcut', 'icon', 'position'] as const) {
      const values = ANNOTATION_TOOLS.map((tool) => tool[key])
      expect(new Set(values).size, key).toBe(values.length)
    }
    expect(ANNOTATION_TOOLS.map((tool) => tool.position)).toEqual([0, 1, 2, 3, 4])
  })
})
