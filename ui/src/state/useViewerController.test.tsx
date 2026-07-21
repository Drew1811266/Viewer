import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { ViewerBridge } from '../api/viewer'
import type {
  CloseBlockedEvent,
  FileCommandPreflight,
  IndexProgressEvent,
  OperationProgressEvent,
  OperationStarted,
  ProjectChangedEvent,
  SearchPage,
} from '../api/types'
import { emptySearchFilters } from './viewerReducer'
import { useViewerController } from './useViewerController'

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((next) => {
    resolve = next
  })
  return { promise, resolve }
}

function bridge(access: 'read_write' | 'read_only' = 'read_write'): ViewerBridge {
  return {
    chooseProject: vi.fn().mockResolvedValue('/fixture/project'),
    openProject: vi.fn().mockResolvedValue({
      projectId: 'project-1',
      sessionId: 'session-1',
      generation: 1,
      displayName: 'Catalog',
      access,
    }),
    closeProject: vi.fn().mockResolvedValue('closed'),
    projectSnapshot: vi.fn().mockResolvedValue(null),
    folderTree: vi.fn().mockResolvedValue([]),
    queryFolder: vi.fn().mockResolvedValue({ workspace: 'empty' }),
    requestImage: vi.fn(),
    previewText: vi.fn(),
    openExternalLink: vi.fn(),
    cancelTask: vi.fn().mockResolvedValue(false),
    searchProject: vi.fn().mockResolvedValue(page(1, 'default', 'filename')),
    searchTextSnippet: vi.fn().mockResolvedValue({
      revision: 1,
      entityId: 'body-hit',
      snippet: 'visible snippet',
    }),
    setReviewState: vi.fn().mockResolvedValue({ changes: [] }),
    toggleFavorite: vi.fn().mockResolvedValue({ changes: [] }),
    selectionInfo: vi.fn().mockResolvedValue({
      relativePaths: [],
      totalSize: 0,
      types: { folders: 0, images: 0, textFiles: 0 },
      commonReview: { state: 'none_selected' },
      commonFavorite: { state: 'none_selected' },
    }),
    previewRename: vi.fn().mockResolvedValue({ rows: [], executable: false }),
    preflightFileCommand: vi.fn().mockResolvedValue({ rows: [], executable: false }),
    executeFileCommand: vi.fn().mockResolvedValue({ batchId: 'batch-1' }),
    operationStatus: vi.fn().mockResolvedValue({
      sessionId: 'session-1',
      generation: 1,
      batchId: 'batch-1',
      lifecycle: 'queued',
      requested: 1,
      completed: 0,
      failed: 0,
      skipped: 0,
      cancelled: 0,
      activeEntityId: null,
    }),
    operationResults: vi.fn().mockResolvedValue({ total: 0, offset: 0, items: [] }),
    cancelOperation: vi.fn().mockResolvedValue(true),
    undoLastOperation: vi.fn().mockResolvedValue(null),
    beginFinderDrag: vi.fn().mockResolvedValue({ fileCount: 1 }),
    openPermissionSettings: vi.fn().mockResolvedValue(undefined),
    listenScan: vi.fn().mockResolvedValue(() => undefined),
    listenIndexProgress: vi.fn().mockResolvedValue(() => undefined),
    listenOperationProgress: vi.fn().mockResolvedValue(() => undefined),
    listenProjectChanged: vi.fn().mockResolvedValue(() => undefined),
    listenCloseBlocked: vi.fn().mockResolvedValue(() => undefined),
    listenProjectClosed: vi.fn().mockResolvedValue(() => undefined),
    listenProjectDrops: vi.fn().mockResolvedValue(() => undefined),
  }
}

beforeEach(() => vi.useFakeTimers())
afterEach(() => vi.useRealTimers())

describe('useViewerController M2 coordination', () => {
  it('debounces text by 120 ms and ignores a late older response', async () => {
    const viewer = bridge()
    const first = deferred<SearchPage>()
    const second = deferred<SearchPage>()
    vi.mocked(viewer.searchProject)
      .mockImplementationOnce(() => first.promise)
      .mockImplementationOnce(() => second.promise)
    const { result } = renderHook(() => useViewerController(viewer))
    await act(() => result.current.openProject('/fixture/project'))

    act(() => result.current.setSearchText('old'))
    await act(() => vi.advanceTimersByTimeAsync(119))
    expect(viewer.searchProject).not.toHaveBeenCalled()
    await act(() => vi.advanceTimersByTimeAsync(1))
    expect(viewer.searchProject).toHaveBeenCalledTimes(1)

    act(() => result.current.setSearchText('new'))
    await act(() => vi.advanceTimersByTimeAsync(120))
    expect(viewer.searchProject).toHaveBeenCalledTimes(2)
    await act(async () => {
      second.resolve(page(2, 'new', 'filename'))
      await Promise.resolve()
    })
    expect(result.current.state.search.page?.hits[0]?.name).toBe('new')
    await act(async () => {
      first.resolve(page(1, 'old', 'filename'))
      await Promise.resolve()
    })
    expect(result.current.state.search.page?.hits[0]?.name).toBe('new')
  })

  it('runs filter changes immediately and only requests snippets for visible body hits', async () => {
    const viewer = bridge()
    vi.mocked(viewer.searchProject).mockResolvedValue({
      ...page(1, 'body', 'body'),
      hits: [
        { ...page(1, 'body', 'body').hits[0]!, entityId: 'body-hit' },
        { ...page(1, 'name', 'filename').hits[0]!, entityId: 'name-hit' },
      ],
    })
    const { result } = renderHook(() => useViewerController(viewer))
    await act(() => result.current.openProject('/fixture/project'))

    act(() =>
      result.current.setSearchFilters({
        ...emptySearchFilters,
        favoriteOnly: true,
      }),
    )
    await act(() => vi.advanceTimersByTimeAsync(0))
    expect(viewer.searchProject).toHaveBeenCalledOnce()
    act(() => result.current.setVisibleSearchHits(['body-hit', 'name-hit']))
    await act(async () => {
      await Promise.resolve()
      await Promise.resolve()
    })
    expect(viewer.searchTextSnippet).toHaveBeenCalledOnce()
    expect(viewer.searchTextSnippet).toHaveBeenCalledWith(
      expect.objectContaining({ entityId: 'body-hit' }),
    )
  })

  it('accepts current index progress and maps Cmd-F to search focus intent', async () => {
    const viewer = bridge()
    let receiveProgress: ((progress: IndexProgressEvent) => void) | undefined
    vi.mocked(viewer.listenIndexProgress).mockImplementation(async (handler) => {
      receiveProgress = handler
      return () => undefined
    })
    const { result } = renderHook(() => useViewerController(viewer))
    await act(() => result.current.openProject('/fixture/project'))
    await act(async () => {
      await Promise.resolve()
      await Promise.resolve()
    })
    expect(receiveProgress).toBeDefined()

    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'f', metaKey: true }))
      receiveProgress?.({
        sessionId: 'session-1',
        generation: 1,
        imagesTotal: 2,
        imagesReady: 1,
        imagesFailed: 0,
        textTotal: 1,
        textReady: 1,
        textSkipped: 0,
        textFailed: 0,
        complete: false,
      })
    })
    expect(result.current.state.search.focusRequest).toBe(1)
    expect(result.current.state.indexProgress?.imagesReady).toBe(1)
  })

  it('preserves selection after marker responses and suppresses writes for read-only projects', async () => {
    const writable = bridge()
    vi.mocked(writable.setReviewState).mockResolvedValue({
      changes: [
        {
          entityId: 'image-1',
          relativePath: 'id/image.jpg',
          kind: 'jpeg',
          marker: { reviewState: 'keep', favorite: false },
        },
      ],
    })
    const writableHook = renderHook(() => useViewerController(writable))
    await act(() => writableHook.result.current.openProject('/fixture/project'))
    act(() => writableHook.result.current.setSelectedEntityIds(['image-1', 'image-2']))
    await act(() => writableHook.result.current.setReviewState('keep'))
    expect(writableHook.result.current.state.selectedEntityIds).toEqual([
      'image-1',
      'image-2',
    ])
    await act(() =>
      writableHook.result.current.setReviewState('reject', ['image-2', 'image-2']),
    )
    expect(writable.setReviewState).toHaveBeenLastCalledWith({
      sessionId: 'session-1',
      generation: 1,
      entityIds: ['image-2'],
      reviewState: 'reject',
    })
    await act(() => writableHook.result.current.toggleFavorite(['image-1']))
    expect(writable.toggleFavorite).toHaveBeenLastCalledWith({
      sessionId: 'session-1',
      generation: 1,
      entityIds: ['image-1'],
    })
    expect(writableHook.result.current.state.selectedEntityIds).toEqual([
      'image-1',
      'image-2',
    ])

    const readOnly = bridge('read_only')
    const readOnlyHook = renderHook(() => useViewerController(readOnly))
    await act(() => readOnlyHook.result.current.openProject('/fixture/project'))
    act(() => readOnlyHook.result.current.setSelectedEntityIds(['image-1']))
    await act(() => readOnlyHook.result.current.setReviewState('reject'))
    await act(() => readOnlyHook.result.current.toggleFavorite())
    expect(readOnly.setReviewState).not.toHaveBeenCalled()
    expect(readOnly.toggleFavorite).not.toHaveBeenCalled()
  })

  it('coordinates one current operation and refreshes projection once after completed results', async () => {
    const viewer = bridge()
    let receiveOperation: ((event: OperationProgressEvent) => void) | undefined
    let receiveProjectChanged: ((event: ProjectChangedEvent) => void) | undefined
    vi.mocked(viewer.listenOperationProgress).mockImplementation(async (handler) => {
      receiveOperation = handler
      return () => undefined
    })
    vi.mocked(viewer.listenProjectChanged).mockImplementation(async (handler) => {
      receiveProjectChanged = handler
      return () => undefined
    })
    vi.mocked(viewer.operationResults).mockResolvedValue({
      total: 1,
      offset: 0,
      items: [
        {
          entityId: 'image-1',
          relativePath: 'id/image.jpg',
          status: 'completed',
          code: 'renamed',
        },
      ],
    })
    const { result } = renderHook(() => useViewerController(viewer))
    await act(() => result.current.openProject('/fixture/project'))
    await act(async () => {
      await Promise.resolve()
      await Promise.resolve()
    })
    expect(receiveOperation).toBeDefined()
    await act(() =>
      result.current.executeFileCommand('rename', [
        {
          entityId: 'image-1',
          action: { kind: 'rename', proposedName: 'hero.jpg', editExtension: true },
        },
      ]),
    )
    expect(viewer.executeFileCommand).toHaveBeenCalledWith(
      expect.objectContaining({
        sessionId: 'session-1',
        generation: 1,
        kind: 'rename',
      }),
    )
    act(() => receiveOperation?.(operationProgress('old-session', 1, 'batch-1')))
    expect(result.current.state.operation.active?.lifecycle).toBe('queued')
    await act(async () => {
      receiveOperation?.(operationProgress('session-1', 1, 'batch-1'))
      await Promise.resolve()
      await Promise.resolve()
      await Promise.resolve()
    })
    expect(viewer.operationResults).toHaveBeenCalledOnce()
    expect(result.current.state.operation.results?.items[0]?.code).toBe('renamed')
    expect(viewer.folderTree).toHaveBeenCalledTimes(2)
    expect(viewer.queryFolder).toHaveBeenCalledTimes(2)
    await act(() => result.current.loadOperationResults(200))
    expect(viewer.operationResults).toHaveBeenLastCalledWith({
      sessionId: 'session-1',
      generation: 1,
      batchId: 'batch-1',
      offset: 200,
      limit: 200,
    })
    await act(async () => {
      receiveProjectChanged?.({
        ...projectChanged('session-1', 1),
        reason: 'expected_viewer_change',
      })
      await Promise.resolve()
    })
    expect(viewer.folderTree).toHaveBeenCalledTimes(2)
    expect(viewer.queryFolder).toHaveBeenCalledTimes(2)
  })

  it('admits only one command request while startup is pending and cancels the current batch', async () => {
    const viewer = bridge()
    const start = deferred<OperationStarted>()
    vi.mocked(viewer.executeFileCommand).mockImplementation(() => start.promise)
    const { result } = renderHook(() => useViewerController(viewer))
    await act(() => result.current.openProject('/fixture/project'))

    let first!: Promise<OperationStarted | null>
    act(() => {
      first = result.current.executeFileCommand('trash', [
        { entityId: 'image-1', action: { kind: 'trash' } },
      ])
      void result.current.executeFileCommand('trash', [
        { entityId: 'image-2', action: { kind: 'trash' } },
      ])
    })
    await act(() => result.current.undoLastOperation())
    expect(viewer.executeFileCommand).toHaveBeenCalledOnce()
    expect(viewer.undoLastOperation).not.toHaveBeenCalled()
    await act(async () => {
      start.resolve({ batchId: 'batch-1' })
      await first
    })
    await act(() => result.current.undoLastOperation())
    expect(viewer.undoLastOperation).not.toHaveBeenCalled()
    await act(() => result.current.cancelOperation())
    expect(viewer.cancelOperation).toHaveBeenCalledWith({
      sessionId: 'session-1',
      generation: 1,
      batchId: 'batch-1',
    })
  })

  it('refreshes the file projection even when completed result details cannot be loaded', async () => {
    const viewer = bridge()
    let receiveOperation: ((event: OperationProgressEvent) => void) | undefined
    vi.mocked(viewer.listenOperationProgress).mockImplementation(async (handler) => {
      receiveOperation = handler
      return () => undefined
    })
    vi.mocked(viewer.operationResults).mockRejectedValue(new Error('result unavailable'))
    const { result } = renderHook(() => useViewerController(viewer))
    await act(() => result.current.openProject('/fixture/project'))
    await act(async () => {
      await Promise.resolve()
      await Promise.resolve()
    })
    await act(() => result.current.executeFileCommand('trash', [
      { entityId: 'image-1', action: { kind: 'trash' } },
    ]))

    await act(async () => {
      receiveOperation?.(operationProgress('session-1', 1, 'batch-1'))
      await Promise.resolve()
      await Promise.resolve()
      await Promise.resolve()
    })

    expect(viewer.operationResults).toHaveBeenCalledOnce()
    expect(viewer.folderTree).toHaveBeenCalledTimes(2)
    expect(viewer.queryFolder).toHaveBeenCalledTimes(2)
  })

  it('keeps the newest operation result page when navigation responses arrive out of order', async () => {
    const viewer = bridge()
    let receiveOperation: ((event: OperationProgressEvent) => void) | undefined
    vi.mocked(viewer.listenOperationProgress).mockImplementation(async (handler) => {
      receiveOperation = handler
      return () => undefined
    })
    vi.mocked(viewer.operationResults).mockResolvedValueOnce({ total: 500, offset: 0, items: [] })
    const { result } = renderHook(() => useViewerController(viewer))
    await act(() => result.current.openProject('/fixture/project'))
    await act(async () => {
      await Promise.resolve()
      await Promise.resolve()
    })
    await act(() => result.current.executeFileCommand('trash', [
      { entityId: 'image-1', action: { kind: 'trash' } },
    ]))
    await act(async () => {
      receiveOperation?.(operationProgress('session-1', 1, 'batch-1'))
      await Promise.resolve()
      await Promise.resolve()
      await Promise.resolve()
    })

    const older = deferred<{ total: number; offset: number; items: [] }>()
    const newer = deferred<{ total: number; offset: number; items: [] }>()
    vi.mocked(viewer.operationResults)
      .mockImplementationOnce(() => older.promise)
      .mockImplementationOnce(() => newer.promise)
    let olderRequest!: Promise<unknown>
    let newerRequest!: Promise<unknown>
    act(() => {
      olderRequest = result.current.loadOperationResults(200)
      newerRequest = result.current.loadOperationResults(400)
    })
    await act(async () => {
      newer.resolve({ total: 500, offset: 400, items: [] })
      await newerRequest
    })
    await act(async () => {
      older.resolve({ total: 500, offset: 200, items: [] })
      await olderRequest
    })
    expect(result.current.state.operation.results?.offset).toBe(400)
  })

  it('repairs current context after project changes and records close coordination', async () => {
    const viewer = bridge()
    let receiveProjectChanged: ((event: ProjectChangedEvent) => void) | undefined
    let receiveCloseBlocked: ((event: CloseBlockedEvent) => void) | undefined
    vi.mocked(viewer.listenProjectChanged).mockImplementation(async (handler) => {
      receiveProjectChanged = handler
      return () => undefined
    })
    vi.mocked(viewer.listenCloseBlocked).mockImplementation(async (handler) => {
      receiveCloseBlocked = handler
      return () => undefined
    })
    vi.mocked(viewer.queryFolder)
      .mockResolvedValueOnce(contentWorkspace(['a', 'b', 'c']))
      .mockResolvedValueOnce(contentWorkspace(['a', 'c']))
    const { result } = renderHook(() => useViewerController(viewer))
    await act(() => result.current.openProject('/fixture/project'))
    await act(async () => {
      await Promise.resolve()
      await Promise.resolve()
    })
    act(() => {
      result.current.setSelectedEntityIds(['b', 'c'])
      result.current.setPreviewEntityId('b')
      result.current.setCompareEntityIds(['a', 'b', 'c'])
    })
    await act(async () => {
      receiveProjectChanged?.(projectChanged('session-1', 1))
      await Promise.resolve()
      await Promise.resolve()
      await Promise.resolve()
    })
    expect(result.current.state.selectedEntityIds).toEqual(['c'])
    expect(result.current.state.compareEntityIds).toEqual(['a', 'c'])
    expect(result.current.state.contextRepair?.suggestedEntityId).toBe('c')
    act(() => {
      receiveCloseBlocked?.({
        sessionId: 'session-1',
        generation: 1,
        batchId: 'batch-9',
        target: 'project',
      })
    })
    expect(result.current.state.closeBlocked?.batchId).toBe('batch-9')
  })

  it('suppresses organization commands for read-only projects', async () => {
    const viewer = bridge('read_only')
    const { result } = renderHook(() => useViewerController(viewer))
    await act(() => result.current.openProject('/fixture/project'))
    await act(() =>
      result.current.executeFileCommand('trash', [
        { entityId: 'image-1', action: { kind: 'trash' } },
      ]),
    )
    await act(() => result.current.undoLastOperation())
    expect(viewer.executeFileCommand).not.toHaveBeenCalled()
    expect(viewer.undoLastOperation).not.toHaveBeenCalled()
  })

  it('rejects organization previews and mutations once project closing begins', async () => {
    const viewer = bridge()
    const closing = deferred<'closed'>()
    vi.mocked(viewer.closeProject).mockImplementation(() => closing.promise)
    const { result } = renderHook(() => useViewerController(viewer))
    await act(() => result.current.openProject('/fixture/project'))

    let close!: ReturnType<typeof result.current.closeProject>
    act(() => {
      close = result.current.closeProject()
    })
    expect(result.current.state.status).toBe('closing')
    await act(() => result.current.previewRename(['image-1'], {
      find: '',
      replacement: '',
      prefix: 'x-',
      suffix: '',
      sequence: null,
    }))
    await act(() => result.current.preflightFileCommand('trash', [
      { entityId: 'image-1', action: { kind: 'trash' } },
    ]))
    await act(() => result.current.executeFileCommand('trash', [
      { entityId: 'image-1', action: { kind: 'trash' } },
    ]))
    await act(() => result.current.undoLastOperation())

    expect(viewer.previewRename).not.toHaveBeenCalled()
    expect(viewer.preflightFileCommand).not.toHaveBeenCalled()
    expect(viewer.executeFileCommand).not.toHaveBeenCalled()
    expect(viewer.undoLastOperation).not.toHaveBeenCalled()
    await act(async () => {
      closing.resolve('closed')
      await close
    })
  })

  it.each([
    [
      'all-ready',
      {
        rows: [{ entityId: 'image-1', relativePath: 'image-1.png', state: 'ready' }],
        executable: true,
      },
    ],
    [
      'conflict',
      {
        rows: [
          {
            entityId: 'image-1',
            relativePath: 'image-1.png',
            state: 'conflict',
            code: 'destination_occupied',
          },
        ],
        executable: true,
      },
    ],
  ] satisfies [string, FileCommandPreflight][]) (
    'drops a late %s preflight after closing and reopening the same backend identity',
    async (_label, response) => {
      const viewer = bridge()
      const pending = deferred<FileCommandPreflight>()
      vi.mocked(viewer.preflightFileCommand).mockImplementationOnce(() => pending.promise)
      const { result } = renderHook(() => useViewerController(viewer))
      await act(() => result.current.openProject('/fixture/project'))

      let request!: ReturnType<typeof result.current.preflightFileCommand>
      act(() => {
        request = result.current.preflightFileCommand('move', [
          {
            entityId: 'image-1',
            action: { kind: 'move', destinationFolderId: 'folder-2' },
          },
        ])
      })
      expect(viewer.preflightFileCommand).toHaveBeenCalledOnce()
      await act(() => result.current.closeProject())
      await act(() => result.current.openProject('/fixture/project'))

      let resolved: FileCommandPreflight | null | undefined
      await act(async () => {
        pending.resolve(response)
        resolved = await request
      })

      expect(result.current.state.project).toMatchObject({
        sessionId: 'session-1',
        generation: 1,
      })
      expect(resolved).toBeNull()
    },
  )

  it('retains the complete session when close stays and clears it only after a chosen close', async () => {
    const viewer = bridge()
    vi.mocked(viewer.closeProject)
      .mockResolvedValueOnce('stayed')
      .mockResolvedValueOnce('closed')
    const { result } = renderHook(() => useViewerController(viewer))
    await act(() => result.current.openProject('/fixture/project'))
    act(() => result.current.setSelectedEntityIds(['image-1']))

    await act(() => result.current.closeProject())

    expect(result.current.state.status).toBe('active')
    expect(result.current.state.project?.sessionId).toBe('session-1')
    expect(result.current.state.selectedEntityIds).toEqual(['image-1'])

    await act(() => result.current.closeProject('wait'))

    expect(viewer.closeProject).toHaveBeenNthCalledWith(1, undefined, 'project')
    expect(viewer.closeProject).toHaveBeenNthCalledWith(2, 'wait', 'project')
    expect(result.current.state.project).toBeNull()
    expect(result.current.state.status).toBe('empty')
  })
})

function operationProgress(
  sessionId: string,
  generation: number,
  batchId: string,
): OperationProgressEvent {
  return {
    sessionId,
    generation,
    batchId,
    lifecycle: 'completed',
    requested: 1,
    completed: 1,
    failed: 0,
    skipped: 0,
    cancelled: 0,
    activeEntityId: null,
  }
}

function projectChanged(sessionId: string, generation: number): ProjectChangedEvent {
  return {
    sessionId,
    generation,
    reason: 'external_change',
    added: 0,
    removed: 1,
    modified: 0,
    moved: 0,
    markerPathsMoved: 0,
    failed: 0,
  }
}

function contentWorkspace(ids: string[]) {
  return {
    workspace: 'content' as const,
    images: ids.map((entityId) => ({
      entityId,
      relativePath: `${entityId}.jpg`,
      name: `${entityId}.jpg`,
      kind: 'jpeg' as const,
      size: 1,
      modifiedNs: '1',
      marker: { reviewState: null, favorite: false },
      imageMetadata: null,
      imageUrl: null,
    })),
    textFiles: [],
  }
}

function page(
  revision: number,
  name: string,
  matchedField: 'filename' | 'body',
): SearchPage {
  return {
    revision,
    total: 1,
    progress: {
      imagesTotal: 1,
      imagesReady: 1,
      imagesFailed: 0,
      textTotal: 0,
      textReady: 0,
      textSkipped: 0,
      textFailed: 0,
      complete: true,
    },
    hits: [
      {
        entityId: `${name}-hit`,
        relativePath: `id/${name}.jpg`,
        name,
        kind: 'jpeg',
        size: 10,
        modifiedNs: '1',
        marker: { reviewState: null, favorite: false },
        imageMetadata: null,
        matchedField,
        score: 1,
        groupRelativePath: 'id',
        matchRanges: [],
      },
    ],
  }
}
