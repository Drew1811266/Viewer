import { describe, expect, it } from 'vitest'
import type { BrowserFile } from '../../api/types'
import {
  filesForSelectAllScope,
  resolveAdaptiveContentMode,
  resolveSelectAllRequest,
} from './adaptiveOtherFilePanelModel'

const file = (entityId: string, kind: 'jpeg' | 'video' | 'text' | 'other'): BrowserFile => ({
  entityId,
  relativePath: entityId,
  name: entityId,
  kind,
  size: 1,
  modifiedNs: '1',
  marker: { reviewState: null, favorite: false },
  imageMetadata: null,
  imageUrl: null,
  videoMetadata: null,
  ...(kind === 'video'
    ? {
        videoMetadata: {
          durationUs: 1_000_000,
          displayWidth: 1920,
          displayHeight: 1080,
          rotationDegrees: 0,
          frameRateMillihertz: 30_000,
          videoCodec: 'h264',
          audioCodec: 'aac',
          probeStatus: 'ready' as const,
          failureKind: null,
          coverUrl: null,
        },
      }
    : {}),
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
    expect(resolveSelectAllRequest(3, 0, 0)).toEqual({
      kind: 'direct',
      scope: 'images',
    })
    expect(resolveSelectAllRequest(0, 2, 0)).toEqual({
      kind: 'direct',
      scope: 'videos',
    })
    expect(resolveSelectAllRequest(0, 0, 3)).toEqual({
      kind: 'direct',
      scope: 'otherFiles',
    })
    expect(resolveSelectAllRequest(3, 2, 1)).toEqual({
      kind: 'choice',
      scopes: ['images', 'videos', 'otherFiles'],
    })
    expect(resolveSelectAllRequest(0, 0, 0)).toEqual({ kind: 'none' })
  })

  it('returns exactly the requested files in stable display order', () => {
    const content = {
      images: [file('image-1', 'jpeg'), file('image-2', 'jpeg')],
      videos: [file('video-1', 'video')],
      otherFiles: [file('text-1', 'text'), file('other-2', 'other')],
    }
    expect(filesForSelectAllScope(content, 'images').map(({ entityId }) => entityId)).toEqual([
      'image-1',
      'image-2',
    ])
    expect(filesForSelectAllScope(content, 'videos').map(({ entityId }) => entityId)).toEqual([
      'video-1',
    ])
    expect(filesForSelectAllScope(content, 'otherFiles').map(({ entityId }) => entityId)).toEqual([
      'text-1',
      'other-2',
    ])
    expect(filesForSelectAllScope(content, 'all').map(({ entityId }) => entityId)).toEqual([
      'image-1',
      'image-2',
      'video-1',
      'text-1',
      'other-2',
    ])
  })
})
