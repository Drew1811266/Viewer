import { describe, expect, it } from 'vitest'
import type { ImageRepresentation } from '../api/types'
import { createAcceptanceBridge } from './acceptanceBridge'
import { ACCEPTANCE_FILES } from './acceptanceFixtures'

describe('Viewer visual acceptance bridge', () => {
  it('implements the complete deterministic bridge and fails fast on unused business calls', async () => {
    const bridge = createAcceptanceBridge()

    expect(Object.keys(bridge).sort()).toEqual(
      [
        'beginFinderDrag',
        'cancelOperation',
        'cancelTask',
        'chooseProject',
        'closeProject',
        'executeFileCommand',
        'folderTree',
        'getViewerSettings',
        'listenCloseBlocked',
        'listenIndexProgress',
        'listenOperationProgress',
        'listenProjectChanged',
        'listenProjectClosed',
        'listenProjectDropEvents',
        'listenProjectDrops',
        'listenReviewProgress',
        'listenScan',
        'listenVideo',
        'openExternalLink',
        'openPermissionSettings',
        'openProject',
        'operationResults',
        'operationStatus',
        'preflightFileCommand',
        'previewRename',
        'previewText',
        'projectSnapshot',
        'queryFolder',
        'requestImage',
        'revealProjectInFileManager',
        'reviewAbandon',
        'reviewAddFeedback',
        'reviewCancelTask',
        'reviewComplete',
        'reviewCompletionSummary',
        'reviewDeleteFeedback',
        'reviewPreviewStart',
        'reviewResume',
        'reviewStart',
        'reviewStatus',
        'reviewUpdateFeedback',
        'searchProject',
        'searchTextSnippet',
        'selectionInfo',
        'setReviewState',
        'toggleFavorite',
        'undoLastOperation',
        'updateViewerSettings',
        'videoCacheClear',
        'videoCacheStats',
        'videoCancelOpen',
        'videoClose',
        'videoOpen',
        'videoPause',
        'videoPlay',
        'videoRequestCover',
        'videoRequestThumbnail',
        'videoSeek',
        'videoSetFullscreen',
        'videoSetMuted',
        'videoSetRate',
        'videoSetVolume',
        'videoStep',
      ].sort(),
    )
    await expect(bridge.getViewerSettings()).resolves.toEqual({
      schemaVersion: 4,
      thumbnailDensity: 'standard',
      magnifier: { shape: 'circle', magnification: 2, area: 'small' },
    })
    await expect(bridge.projectSnapshot()).resolves.toMatchObject({
      displayName: '测试图',
      access: 'read_write',
    })
    const unlisten = await bridge.listenScan(() => undefined)
    expect(unlisten).toBeTypeOf('function')
    expect(unlisten()).toBeUndefined()
    await expect(
      bridge.reviewStatus({ sessionId: '00000000-0000-4000-8000-000000000001', generation: 1 }),
    ).resolves.toMatchObject({ phase: 'idle', revision: 0 })
    const stopReviewProgress = await bridge.listenReviewProgress(() => undefined)
    expect(stopReviewProgress()).toBeUndefined()
    await expect(bridge.executeFileCommand({} as never)).rejects.toThrow(
      'Unexpected acceptance bridge call: executeFileCommand',
    )
    await expect(bridge.videoOpen({} as never)).rejects.toThrow(
      'Unexpected acceptance bridge call: videoOpen',
    )
    await expect(bridge.reviewStart({} as never)).rejects.toThrow(
      'Unexpected acceptance bridge call: reviewStart',
    )
  })

  it('uses exact method overrides without changing the remaining bridge contract', async () => {
    const overridden: ImageRepresentation = {
      cacheKey: 'override-image',
      url: '/override.jpg',
      width: 640,
      height: 480,
      backend: 'image_io',
    }
    const bridge = createAcceptanceBridge({
      requestImage: async () => overridden,
    })

    const file = ACCEPTANCE_FILES[0]
    expect(file).toBeDefined()
    await expect(
      bridge.requestImage({
        entityId: file?.entityId ?? 'missing',
        representation: { kind: 'original100_percent' },
      }),
    ).resolves.toEqual(overridden)
    await expect(bridge.getViewerSettings()).resolves.toMatchObject({
      thumbnailDensity: 'standard',
    })
  })
})
