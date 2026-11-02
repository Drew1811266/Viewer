import { describe, expect, it } from 'vitest'
import {
  ANNOTATION_TOOLS,
  LEGACY_ANNOTATION_TOOLS,
  toolForShortcut,
  toolsForProtocol,
} from './annotationToolRegistry'

describe('annotation tool registry', () => {
  it('keeps the approved fixed menu order and shortcuts', () => {
    expect(ANNOTATION_TOOLS.map((tool) => tool.id)).toEqual([
      'point',
      'arrow',
      'brush',
      'rectangle',
      'ellipse',
      'text',
      'line',
      'highlighter',
      'polygon',
      'ruler',
    ])
    expect(ANNOTATION_TOOLS.map((tool) => tool.shortcut)).toEqual([
      'P',
      'A',
      'B',
      'R',
      'O',
      'T',
      'L',
      'H',
      'G',
      'M',
    ])
    expect(toolForShortcut('o')?.id).toBe('ellipse')
    expect(toolForShortcut('t')?.id).toBe('text')
    expect(toolForShortcut('g')?.id).toBe('polygon')
    expect(toolForShortcut('m')?.id).toBe('ruler')
  })

  it('keeps the legacy protocol on the five original tools', () => {
    expect(toolsForProtocol('continuous')).toEqual(ANNOTATION_TOOLS)
    expect(toolsForProtocol('legacy')).toEqual(LEGACY_ANNOTATION_TOOLS)
    expect(LEGACY_ANNOTATION_TOOLS.map((tool) => tool.id)).toEqual([
      'point',
      'arrow',
      'brush',
      'rectangle',
      'ellipse',
    ])
    expect(toolForShortcut('t', LEGACY_ANNOTATION_TOOLS)).toBeUndefined()
  })

  it('has unique stable ids, shortcuts, icons, and menu positions', () => {
    for (const key of ['id', 'shortcut', 'icon', 'position'] as const) {
      const values = ANNOTATION_TOOLS.map((tool) => tool[key])
      expect(new Set(values).size, key).toBe(values.length)
    }
    expect(ANNOTATION_TOOLS.map((tool) => tool.position)).toEqual([0, 1, 2, 3, 4, 5, 6, 7, 8, 9])
  })
})
