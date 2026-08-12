import { describe, expect, it } from 'vitest'
import type { BrowserFile, VideoFile } from '../api/types'
import { validatePreviewSelection, videoPreviewNeighbors } from './previewPolicy'

const file = (kind: BrowserFile['kind']) => ({ kind })

describe('validatePreviewSelection', () => {
  it('allows one indexed file of any kind', () => {
    expect(validatePreviewSelection([file('jpeg')])).toEqual({
      ok: true,
      mode: 'single',
    })
    expect(validatePreviewSelection([file('other')])).toEqual({
      ok: true,
      mode: 'single',
    })
    expect(validatePreviewSelection([file('video')])).toEqual({
      ok: true,
      mode: 'single',
    })
  })

  it('allows exactly two previewable text files', () => {
    expect(validatePreviewSelection([file('text'), file('markdown')])).toEqual({
      ok: true,
      mode: 'split_text',
    })
  })

  it('rejects two files unless both are previewable text', () => {
    expect(validatePreviewSelection([file('text'), file('other')])).toEqual({
      ok: false,
      reason: '仅支持单文件预览，或同时预览 2 个文本文件',
    })
  })

  it('gives previewable text overflow its specific reason', () => {
    expect(validatePreviewSelection([file('text'), file('text'), file('text')])).toEqual({
      ok: false,
      reason: '文本预览最多支持 2 个可预览文件',
    })
  })

  it('uses the general reason for larger mixed selections', () => {
    expect(validatePreviewSelection([file('jpeg'), file('text'), file('other')])).toEqual({
      ok: false,
      reason: '仅支持单文件预览，或同时预览 2 个文本文件',
    })
  })
})

describe('videoPreviewNeighbors', () => {
  const videos = [video('a'), video('b'), video('c')]

  it('keeps workspace video order outside search', () => {
    expect(videoPreviewNeighbors(videos, null).map((candidate) => candidate.entityId)).toEqual([
      'video-a',
      'video-b',
      'video-c',
    ])
  })

  it('uses only video hits in search-result order', () => {
    expect(
      videoPreviewNeighbors(videos, [
        { entityId: 'video-c', kind: 'video' },
        { entityId: 'text-1', kind: 'text' },
        { entityId: 'missing-video', kind: 'video' },
        { entityId: 'video-a', kind: 'video' },
      ]).map((candidate) => candidate.entityId),
    ).toEqual(['video-c', 'video-a'])
  })
})

function video(suffix: string): VideoFile {
  return {
    entityId: `video-${suffix}`,
    relativePath: `id/${suffix}.mp4`,
    name: `${suffix}.mp4`,
    kind: 'video',
    size: 1,
    modifiedNs: '1',
    marker: { reviewState: null, favorite: false },
    imageMetadata: null,
    imageUrl: null,
    videoMetadata: {
      durationUs: 1,
      displayWidth: 1,
      displayHeight: 1,
      rotationDegrees: 0,
      frameRateMillihertz: null,
      videoCodec: null,
      audioCodec: null,
      probeStatus: 'ready',
      failureKind: null,
      coverUrl: null,
    },
  }
}
