import { describe, expect, it } from 'vitest'
import type { BrowserFile } from '../../api/types'
import {
  filesForSelectAllScope,
  resolveAdaptiveContentMode,
  resolveSelectAllRequest,
} from './adaptiveTextPanelModel'

const file = (entityId: string, kind: 'jpeg' | 'text'): BrowserFile => ({
  entityId,
  relativePath: entityId,
  name: entityId,
  kind,
  size: 1,
  modifiedNs: '1',
  marker: { reviewState: null, favorite: false },
  imageMetadata: null,
  imageUrl: null,
})

describe('adaptiveTextPanelModel', () => {
  it.each([
    [2, 0, false, 'image_only'],
    [2, 3, false, 'mixed_collapsed'],
    [2, 3, true, 'mixed_expanded'],
    [0, 3, false, 'text_only'],
    [0, 3, true, 'text_only'],
    [0, 0, false, 'empty'],
  ] as const)(
    'resolves %i images, %i texts and preferred=%s as %s',
    (imageCount, textCount, preferredExpanded, expected) => {
      expect(
        resolveAdaptiveContentMode(imageCount, textCount, preferredExpanded),
      ).toBe(expected)
    },
  )

  it('resolves direct, choice, and empty select-all behavior', () => {
    expect(resolveSelectAllRequest(3, 0)).toEqual({
      kind: 'direct',
      scope: 'images',
    })
    expect(resolveSelectAllRequest(0, 3)).toEqual({
      kind: 'direct',
      scope: 'text',
    })
    expect(resolveSelectAllRequest(3, 2)).toEqual({ kind: 'choice' })
    expect(resolveSelectAllRequest(0, 0)).toEqual({ kind: 'none' })
  })

  it('returns exactly the requested files in stable display order', () => {
    const content = {
      images: [file('image-1', 'jpeg'), file('image-2', 'jpeg')],
      textFiles: [file('text-1', 'text'), file('text-2', 'text')],
    }
    expect(filesForSelectAllScope(content, 'images').map(({ entityId }) => entityId)).toEqual([
      'image-1',
      'image-2',
    ])
    expect(filesForSelectAllScope(content, 'text').map(({ entityId }) => entityId)).toEqual([
      'text-1',
      'text-2',
    ])
    expect(filesForSelectAllScope(content, 'all').map(({ entityId }) => entityId)).toEqual([
      'image-1',
      'image-2',
      'text-1',
      'text-2',
    ])
  })
})
