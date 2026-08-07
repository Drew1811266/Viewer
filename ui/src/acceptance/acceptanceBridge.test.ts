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
        'listenScan',
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
        'searchProject',
        'searchTextSnippet',
        'selectionInfo',
        'setReviewState',
        'toggleFavorite',
        'undoLastOperation',
        'updateViewerSettings',
      ].sort(),
    )
    await expect(bridge.getViewerSettings()).resolves.toEqual({
      schemaVersion: 2,
      thumbnailDensity: 'standard',
      magnifier: { shape: 'circle', magnification: 4, area: 'small' },
    })
    await expect(bridge.projectSnapshot()).resolves.toMatchObject({
      displayName: '测试图',
      access: 'read_write',
    })
    const unlisten = await bridge.listenScan(() => undefined)
    expect(unlisten).toBeTypeOf('function')
    expect(unlisten()).toBeUndefined()
    await expect(bridge.executeFileCommand({} as never)).rejects.toThrow(
      'Unexpected acceptance bridge call: executeFileCommand',
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
