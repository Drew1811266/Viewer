import { describe, expect, it } from 'vitest'
import type { BrowserFile } from '../../api/types'
import {
  filesForSelectAllScope,
  resolveAdaptiveContentMode,
  resolveSelectAllRequest,
} from './adaptiveOtherFilePanelModel'

const file = (entityId: string, kind: 'jpeg' | 'text' | 'other'): BrowserFile => ({
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

describe('adaptiveOtherFilePanelModel', () => {
  it.each([
    [2, 0, false, 'image_only'],
    [2, 3, false, 'mixed_collapsed'],
    [2, 3, true, 'mixed_expanded'],
    [0, 3, false, 'other_only'],
    [0, 3, true, 'other_only'],
    [0, 0, false, 'empty'],
  ] as const)(
    'resolves %i images, %i other files and preferred=%s as %s',
    (imageCount, otherCount, preferredExpanded, expected) => {
      expect(resolveAdaptiveContentMode(imageCount, otherCount, preferredExpanded)).toBe(expected)
    },
  )

  it('resolves direct, choice, and empty select-all behavior', () => {
    expect(resolveSelectAllRequest(3, 0)).toEqual({
      kind: 'direct',
      scope: 'images',
    })
    expect(resolveSelectAllRequest(0, 3)).toEqual({
      kind: 'direct',
      scope: 'other',
    })
    expect(resolveSelectAllRequest(3, 2)).toEqual({ kind: 'choice' })
    expect(resolveSelectAllRequest(0, 0)).toEqual({ kind: 'none' })
  })

  it('returns exactly the requested files in stable display order', () => {
    const content = {
      images: [file('image-1', 'jpeg'), file('image-2', 'jpeg')],
      otherFiles: [file('text-1', 'text'), file('other-2', 'other')],
    }
    expect(filesForSelectAllScope(content, 'images').map(({ entityId }) => entityId)).toEqual([
      'image-1',
      'image-2',
    ])
    expect(filesForSelectAllScope(content, 'other').map(({ entityId }) => entityId)).toEqual([
      'text-1',
      'other-2',
    ])
    expect(filesForSelectAllScope(content, 'all').map(({ entityId }) => entityId)).toEqual([
      'image-1',
      'image-2',
      'text-1',
      'other-2',
    ])
  })
})
