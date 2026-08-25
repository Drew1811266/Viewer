import { describe, expect, it } from 'vitest'
import type { BrowserFile, VideoFile } from '../../api/types'
import type { PreviewSession } from '../usePreviewSession'
import {
  accumulatePreviewRepair,
  activeTextPreviewFiles,
  resolveActivePreviewFile,
  resolveActivePreviewFiles,
  unavailablePreviewEntityIds,
  viewingVideoNeighbors,
} from './viewingModel'

function file(entityId: string, kind: BrowserFile['kind'] = 'jpeg'): BrowserFile {
  return {
    entityId,
    relativePath: `${entityId}.file`,
    name: `${entityId}.file`,
    kind,
    size: 1,
    modifiedNs: '1',
    marker: { reviewState: null, favorite: false },
    imageMetadata: kind === 'jpeg' ? { width: 1, height: 1 } : null,
    imageUrl: null,
    videoMetadata: null,
  }
}

function video(entityId: string): VideoFile {
  return {
    ...file(entityId, 'video'),
    kind: 'video',
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

describe('viewingModel', () => {
  it('restricts video neighbors to videos in current search-hit order', () => {
    const videoA = video('video-a')
    const videoB = video('video-b')
    const workspace = {
      workspace: 'content' as const,
      images: [file('image-1')],
      videos: [videoA, videoB],
      otherFiles: [file('text-1', 'text')],
    }

    expect(
      viewingVideoNeighbors(workspace, true, [
        { entityId: 'video-b', kind: 'video' },
        { entityId: 'image-1', kind: 'jpeg' },
        { entityId: 'missing', kind: 'video' },
        { entityId: 'video-a', kind: 'video' },
      ]).map((candidate) => candidate.entityId),
    ).toEqual(['video-b', 'video-a'])
    expect(
      viewingVideoNeighbors(workspace, false, []).map((candidate) => candidate.entityId),
    ).toEqual(['video-a', 'video-b'])
  })

  it('retains exactly two ordered files for split text preview', () => {
    const left = file('left', 'markdown')
    const right = file('right', 'text')
    const session: PreviewSession = {
      file: left,
      files: [left, right],
      folderOverviewIdentity: null,
    }

    expect(activeTextPreviewFiles(session)).toEqual([left, right])
  })

  it('keeps session order and falls back to its active file while removals become unavailable', () => {
    const first = file('first')
    const removed = file('removed')
    const session: PreviewSession = {
      file: removed,
      files: [first, removed],
      folderOverviewIdentity: 'overview-1',
    }
    const workspace = {
      workspace: 'content' as const,
      images: [first],
      videos: [],
      otherFiles: [],
    }
    const files = resolveActivePreviewFiles(session, workspace, [])
    const repair = accumulatePreviewRepair(
      { unavailableEntityIds: new Set(), message: null },
      session,
      {
        removedEntityIds: ['removed'],
        suggestedEntityId: 'first',
        message: '文件已发生变化。',
      },
    )

    expect(files).toEqual([first, removed])
    expect(resolveActivePreviewFile(session, files)).toBe(removed)
    expect([...unavailablePreviewEntityIds(repair, null)]).toEqual(['removed'])
  })
})
