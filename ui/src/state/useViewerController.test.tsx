import { act, renderHook } from '@testing-library/react'
import type { Dispatch, MutableRefObject } from 'react'
import { afterEach, beforeEach, describe, expect, expectTypeOf, it, vi } from 'vitest'
import type {
  CloseBlockedEvent,
  CloseChoice,
  CloseRequestOutcome,
  CloseTarget,
  FileCommandPreflight,
  IndexProgressEvent,
  OperationProgressEvent,
  OperationStarted,
  ProjectChangedEvent,
  ProjectSnapshot,
  ReviewState,
  ScanEvent,
  SearchFilters,
  SearchLayout,
  SearchPage,
  SearchSort,
} from '../api/types'
import type { ViewerBridge } from '../api/viewer'
import { defined } from '../defined'
import type { ControllerCore, RefreshProjection } from './controllers/types'
import { useLifecycleSubscriptions } from './controllers/useLifecycleSubscriptions'
import type {
  OperationController,
  useOperationController,
} from './controllers/useOperationController'
import {
  type ProjectSessionController,
  useProjectSessionController,
} from './controllers/useProjectSessionController'
import type { SearchController, useSearchController } from './controllers/useSearchController'
import type {
  SelectionMarkerController,
  SelectionMarkerControllerCore,
  useSelectionMarkerController,
} from './controllers/useSelectionMarkerController'
import { useViewerController, type ViewerController } from './useViewerController'
import type { SearchFilterChip } from './viewerReducer'
import { emptySearchFilters, initialViewerState } from './viewerReducer'
import type { ViewerAction, ViewerState } from './viewerState'

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((next, fail) => {
    resolve = next
    reject = fail
  })
  return { promise, resolve, reject }
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
    getViewerSettings: vi.fn().mockResolvedValue({
      schemaVersion: 4,
      thumbnailDensity: 'standard',
      magnifier: { shape: 'circle', magnification: 2, area: 'small' },
    }),
    updateViewerSettings: vi.fn().mockImplementation(async (settings) => ({
      schemaVersion: 4,
      ...settings,
    })),
    folderTree: vi.fn().mockResolvedValue([]),
    queryFolder: vi.fn().mockResolvedValue({ workspace: 'empty' }),
    requestImage: vi.fn(),
    previewText: vi.fn(),
    openExternalLink: vi.fn(),
    revealProjectInFileManager: vi.fn(),
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
      types: { folders: 0, images: 0, videos: 0, otherFiles: 0 },
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
    reviewStatus: vi.fn(),
    reviewPreviewStart: vi.fn(),
    reviewStart: vi.fn(),
    reviewResume: vi.fn(),
    reviewAddFeedback: vi.fn(),
    reviewUpdateFeedback: vi.fn(),
    reviewDeleteFeedback: vi.fn(),
    reviewCompletionSummary: vi.fn(),
    reviewComplete: vi.fn(),
    reviewAbandon: vi.fn(),
    reviewCancelTask: vi.fn().mockResolvedValue(false),
    videoOpen: vi.fn(),
    videoCancelOpen: vi.fn().mockResolvedValue(true),
    videoClose: vi.fn(),
    videoPlay: vi.fn(),
    videoPause: vi.fn(),
    videoSeek: vi.fn(),
    videoStep: vi.fn(),
    videoSetVolume: vi.fn(),
    videoSetMuted: vi.fn(),
    videoSetRate: vi.fn(),
    videoSetFullscreen: vi.fn(),
    videoRequestCover: vi.fn(),
    videoRequestThumbnail: vi.fn(),
    videoCacheStats: vi.fn(),
    videoCacheClear: vi.fn(),
    openPermissionSettings: vi.fn().mockResolvedValue(undefined),
    listenScan: vi.fn().mockResolvedValue(() => undefined),
    listenIndexProgress: vi.fn().mockResolvedValue(() => undefined),
    listenOperationProgress: vi.fn().mockResolvedValue(() => undefined),
    listenProjectChanged: vi.fn().mockResolvedValue(() => undefined),
    listenCloseBlocked: vi.fn().mockResolvedValue(() => undefined),
    listenVideo: vi.fn().mockResolvedValue(() => undefined),
    listenReviewProgress: vi.fn().mockResolvedValue(() => undefined),
    listenProjectClosed: vi.fn().mockResolvedValue(() => undefined),
    listenProjectDrops: vi.fn().mockResolvedValue(() => undefined),
    listenProjectDropEvents: vi.fn().mockResolvedValue(() => undefined),
  }
}

beforeEach(() => vi.useFakeTimers())
afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

type ExpectedControllerCore = {
  bridge: ViewerBridge
  state: ViewerState
  stateRef: MutableRefObject<ViewerState>
  sessionEpochRef: MutableRefObject<number>
  dispatch: Dispatch<ViewerAction>
}

type ExpectedRefreshProjection = (
  project: ProjectSnapshot,
  selectedFolderId: string | null,
  selectedFolderPath: string,
  showingAggregate: boolean,
  repairMissingFolder?: boolean,
) => Promise<void>

type ExpectedProjectSessionController = {
  sessionEpoch: number
  refreshProjection: ExpectedRefreshProjection
  openProject(path: string): Promise<'opened' | 'invalid-root' | 'failed'>
  closeProject(choice?: CloseChoice, target?: CloseTarget): Promise<CloseRequestOutcome | undefined>
  reselectProject(): Promise<CloseRequestOutcome | undefined>
  selectFolder(entityId: string | null): Promise<void>
  showAllDescendants(): Promise<void>
  cancelTask(taskId: string): Promise<void>
}

type ExpectedSearchController = {
  setSearchText(text: string): void
  setSearchScope(folderId: string | null): void
  setSearchFilters(filters: SearchFilters): void
  setSearchSort(sort: SearchSort): void
  setSearchLayout(layout: SearchLayout): void
  removeSearchFilter(chip: SearchFilterChip): void
  clearSearchFilters(): void
  setVisibleSearchHits(entityIds: string[]): void
  setSearchPage(offset: number): void
  returnToFolderContext(): void
}

type ExpectedSelectionMarkerController = {
  setSelectedEntityIds(entityIds: string[]): void
  setReviewState(reviewState: ReviewState | null, entityIdsOverride?: string[]): Promise<void>
  toggleFavorite(entityIdsOverride?: string[]): Promise<void>
  setPreviewEntityId(entityId: string | null): void
  setCompareEntityIds(entityIds: string[]): void
  consumeContextRepair(): void
}

type ExpectedOperationController = {
  previewRename: ViewerController['previewRename']
  preflightFileCommand: ViewerController['preflightFileCommand']
  executeFileCommand: ViewerController['executeFileCommand']
  cancelOperation: ViewerController['cancelOperation']
  loadOperationResults: ViewerController['loadOperationResults']
  undoLastOperation: ViewerController['undoLastOperation']
  receiveOperationProgress(progress: OperationProgressEvent): void
}

type ExpectedLifecycleHandlers = {
  receiveOperationProgress(progress: OperationProgressEvent): void
  refreshProjection: RefreshProjection
}

interface ExpectedSelectionMarkerControllerCore extends ControllerCore {
  refreshProjection: RefreshProjection
  operationRequestPendingRef: MutableRefObject<boolean>
  undoRequestPendingRef: MutableRefObject<boolean>
  activeBatchRef: MutableRefObject<string | null>
}

describe('useViewerController M2 coordination', () => {
  it('defines the shared project session controller contract', () => {
    expectTypeOf<ControllerCore>().toEqualTypeOf<ExpectedControllerCore>()
    expectTypeOf<RefreshProjection>().toEqualTypeOf<ExpectedRefreshProjection>()
    expectTypeOf<ProjectSessionController>().toEqualTypeOf<ExpectedProjectSessionController>()
  })

  it('defines the search and selection marker controller contracts', () => {
    expectTypeOf<SearchController>().toEqualTypeOf<ExpectedSearchController>()
    expectTypeOf<SelectionMarkerController>().toEqualTypeOf<ExpectedSelectionMarkerController>()
    expectTypeOf<SelectionMarkerControllerCore>().toEqualTypeOf<ExpectedSelectionMarkerControllerCore>()
    expectTypeOf<typeof useSearchController>().toEqualTypeOf<
      (core: ControllerCore, sessionEpoch: number) => SearchController
    >()
    expectTypeOf<typeof useSelectionMarkerController>().toEqualTypeOf<
      (core: SelectionMarkerControllerCore, sessionEpoch: number) => SelectionMarkerController
    >()
  })

  it('defines the operation and lifecycle controller contracts', () => {
    expectTypeOf<OperationController>().toEqualTypeOf<ExpectedOperationController>()
    expectTypeOf<typeof useOperationController>().toEqualTypeOf<
      (
        core: ControllerCore,
        sessionEpoch: number,
        refreshProjection: RefreshProjection,
      ) => OperationController
    >()
    expectTypeOf<typeof useLifecycleSubscriptions>().toEqualTypeOf<
      (core: ControllerCore, handlers: ExpectedLifecycleHandlers) => void
    >()
  })

  it('registers and cleans up each lifecycle listener once through the lifecycle hook', async () => {
    const viewer = bridge()
    const cleanupOrder: string[] = []
    vi.mocked(viewer.listenOperationProgress).mockResolvedValue(() => {
      cleanupOrder.push('operation-progress')
    })
    vi.mocked(viewer.listenProjectChanged).mockResolvedValue(() => {
      cleanupOrder.push('project-change')
    })
    vi.mocked(viewer.listenCloseBlocked).mockResolvedValue(() => {
      cleanupOrder.push('close-blocked')
    })
    const core: ControllerCore = {
      bridge: viewer,
      state: initialViewerState,
      stateRef: { current: initialViewerState },
      sessionEpochRef: { current: 0 },
      dispatch: vi.fn(),
    }
    const { unmount } = renderHook(() =>
      useLifecycleSubscriptions(core, {
        receiveOperationProgress: vi.fn(),
        refreshProjection: vi.fn().mockResolvedValue(undefined),
      }),
    )
    await act(async () => {
      await Promise.resolve()
      await Promise.resolve()
    })

    expect(viewer.listenOperationProgress).toHaveBeenCalledOnce()
    expect(viewer.listenProjectChanged).toHaveBeenCalledOnce()
    expect(viewer.listenCloseBlocked).toHaveBeenCalledOnce()

    unmount()
    expect(cleanupOrder).toEqual(['operation-progress', 'project-change', 'close-blocked'])
  })

  it('keeps lifecycle registration single-owned when composed by the Viewer facade', async () => {
    const viewer = bridge()
    const { result } = renderHook(() => useViewerController(viewer))
    await act(async () => {
      await Promise.resolve()
      await Promise.resolve()
    })

    expect(viewer.listenOperationProgress).toHaveBeenCalledOnce()
    expect(viewer.listenProjectChanged).toHaveBeenCalledOnce()
    expect(viewer.listenCloseBlocked).toHaveBeenCalledOnce()
    expect(Object.keys(result.current)).toEqual([
      'state',
      'openProject',
      'closeProject',
      'reselectProject',
      'selectFolder',
      'showAllDescendants',
      'cancelTask',
      'setSearchText',
      'setSearchScope',
      'setSearchFilters',
      'setSearchSort',
      'setSearchLayout',
      'removeSearchFilter',
      'clearSearchFilters',
      'setVisibleSearchHits',
      'setSearchPage',
      'returnToFolderContext',
      'setSelectedEntityIds',
      'setReviewState',
      'toggleFavorite',
      'previewRename',
      'preflightFileCommand',
      'executeFileCommand',
      'cancelOperation',
      'loadOperationResults',
      'undoLastOperation',
      'beginFinderDrag',
      'setPreviewEntityId',
      'setCompareEntityIds',
      'consumeContextRepair',
      'clearCloseBlocked',
      'openPermissionSettings',
    ])
  })

  it('forwards Finder drag with the active project identity and ignores unavailable input', async () => {
    const viewer = bridge()
    const { result } = renderHook(() => useViewerController(viewer))

    await act(() => result.current.beginFinderDrag(['image-1']))
    expect(viewer.beginFinderDrag).not.toHaveBeenCalled()

    await act(() => result.current.openProject('/fixture/project'))
    await act(() => result.current.beginFinderDrag([]))
    expect(viewer.beginFinderDrag).not.toHaveBeenCalled()

    await act(() => result.current.beginFinderDrag(['image-1', 'video-1']))
    expect(viewer.beginFinderDrag).toHaveBeenCalledWith({
      sessionId: 'session-1',
      generation: 1,
      entityIds: ['image-1', 'video-1'],
    })
  })

  it('keeps the close-blocked subscription outside the project session controller', async () => {
    const viewer = bridge()
    renderHook(() =>
      useProjectSessionController({
        bridge: viewer,
        state: initialViewerState,
        stateRef: { current: initialViewerState },
        sessionEpochRef: { current: 0 },
        dispatch: vi.fn(),
      }),
    )
    await act(async () => {
      await Promise.resolve()
    })

    expect(viewer.listenCloseBlocked).not.toHaveBeenCalled()
  })

  it('requests the first projection immediately after the project opens', async () => {
    const viewer = bridge()
    const workspace = deferred<Awaited<ReturnType<ViewerBridge['queryFolder']>>>()
    vi.mocked(viewer.queryFolder).mockImplementation(() => workspace.promise)
    const { result } = renderHook(() => useViewerController(viewer))
    let opening!: Promise<'opened' | 'invalid-root' | 'failed'>

    act(() => {
      opening = result.current.openProject('/fixture/project')
    })
    await act(async () => {
      await Promise.resolve()
      await Promise.resolve()
    })

    expect(result.current.state.project?.displayName).toBe('Catalog')
    expect(result.current.state.workspace).toBeNull()
    expect(viewer.queryFolder).toHaveBeenCalledOnce()
    expect(result.current.state.projectionTransition).toEqual({
      selectedFolderId: null,
      selectedFolderPath: '',
      showingAggregate: false,
    })

    await act(async () => {
      workspace.resolve({ workspace: 'empty' })
      await opening
    })
    expect(result.current.state.projectionTransition).toBeNull()
    expect(result.current.state.workspace).toEqual({ workspace: 'empty' })
  })

  it('restores an already-open native session after the webview reloads', async () => {
    const viewer = bridge()
    vi.mocked(viewer.projectSnapshot).mockResolvedValue({
      projectId: 'project-1',
      sessionId: 'session-1',
      generation: 4,
      displayName: 'Recovered Catalog',
      access: 'read_write',
    })
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace(['restored.jpg']))

    const { result } = renderHook(() => useViewerController(viewer))

    await act(async () => {
      await Promise.resolve()
      await Promise.resolve()
      await Promise.resolve()
    })

    expect(viewer.projectSnapshot).toHaveBeenCalledOnce()
    expect(viewer.openProject).not.toHaveBeenCalled()
    expect(result.current.state.status).toBe('active')
    expect(result.current.state.project).toMatchObject({
      sessionId: 'session-1',
      generation: 4,
      displayName: 'Recovered Catalog',
    })
    expect(result.current.state.workspace).toEqual(contentWorkspace(['restored.jpg']))
  })

  it('recovers the native session when choosing a project races webview restoration', async () => {
    const viewer = bridge()
    const snapshot = deferred<ProjectSnapshot | null>()
    vi.mocked(viewer.projectSnapshot).mockImplementation(() => snapshot.promise)
    vi.mocked(viewer.openProject).mockRejectedValue({
      code: 'project_already_open',
      category: 'conflict',
      userMessage: '请先关闭当前项目。',
      retryable: false,
      taskId: null,
      itemId: null,
    })
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace(['restored.jpg']))
    const { result } = renderHook(() => useViewerController(viewer))
    let opening!: ReturnType<typeof result.current.openProject>

    act(() => {
      opening = result.current.openProject('/fixture/project')
    })
    await act(async () => {
      await Promise.resolve()
      await Promise.resolve()
    })
    expect(viewer.openProject).toHaveBeenCalledOnce()

    await act(async () => {
      snapshot.resolve({
        projectId: 'project-1',
        sessionId: 'session-1',
        generation: 4,
        displayName: 'Recovered Catalog',
        access: 'read_write',
      })
      expect(await opening).toBe('opened')
    })

    expect(result.current.state.status).toBe('active')
    expect(result.current.state.project).toMatchObject({ sessionId: 'session-1', generation: 4 })
    expect(result.current.state.workspace).toEqual(contentWorkspace(['restored.jpg']))
  })

  it('preserves the already-open error when the native session cannot be recovered', async () => {
    const viewer = bridge()
    vi.mocked(viewer.openProject).mockRejectedValue({
      code: 'project_already_open',
      category: 'conflict',
      userMessage: '请先关闭当前项目。',
      retryable: false,
      taskId: null,
      itemId: null,
    })
    vi.mocked(viewer.projectSnapshot).mockRejectedValue(new Error('snapshot unavailable'))
    const { result } = renderHook(() => useViewerController(viewer))

    await act(async () => {
      expect(await result.current.openProject('/fixture/project')).toBe('failed')
    })

    expect(result.current.state.status).toBe('error')
    expect(result.current.state.project).toBeNull()
    expect(result.current.state.errorMessage).toBe('请先关闭当前项目。')
  })

  it('keeps the newest folder when older projection success and failure settle late', async () => {
    const viewer = bridge()
    const folders = ['folder-a', 'folder-b', 'folder-c'].map((entityId) => ({
      entityId,
      parentEntityId: null,
      relativePath: entityId,
      name: entityId,
      marker: { reviewState: null, favorite: false },
    }))
    vi.mocked(viewer.folderTree).mockResolvedValue(folders)
    const { result } = renderHook(() => useViewerController(viewer))
    await act(() => result.current.openProject('/fixture/project'))

    const folderA = deferred<Awaited<ReturnType<ViewerBridge['queryFolder']>>>()
    const folderB = deferred<Awaited<ReturnType<ViewerBridge['queryFolder']>>>()
    const folderC = deferred<Awaited<ReturnType<ViewerBridge['queryFolder']>>>()
    vi.mocked(viewer.queryFolder).mockImplementation((entityId) => {
      if (entityId === 'folder-a') return folderA.promise
      if (entityId === 'folder-b') return folderB.promise
      if (entityId === 'folder-c') return folderC.promise
      return Promise.resolve({ workspace: 'empty' })
    })

    let requestA!: ReturnType<typeof result.current.selectFolder>
    let requestB!: ReturnType<typeof result.current.selectFolder>
    let requestC!: ReturnType<typeof result.current.selectFolder>
    act(() => {
      requestA = result.current.selectFolder('folder-a')
      requestB = result.current.selectFolder('folder-b')
      requestC = result.current.selectFolder('folder-c')
    })

    expect(result.current.state.projectionTransition).toEqual({
      selectedFolderId: 'folder-c',
      selectedFolderPath: 'folder-c',
      showingAggregate: false,
    })

    await act(async () => {
      folderC.resolve(contentWorkspace(['c']))
      await requestC
    })
    await act(async () => {
      folderA.resolve(contentWorkspace(['a']))
      await requestA
    })
    await act(async () => {
      folderB.reject(new Error('stale folder failed'))
      await requestB
    })

    expect(result.current.state.selectedFolderId).toBe('folder-c')
    expect(result.current.state.workspace).toEqual(contentWorkspace(['c']))
    expect(result.current.state.projectionTransition).toBeNull()
    expect(result.current.state.errorMessage).toBeNull()
  })

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
        {
          ...defined(page(1, 'body', 'body').hits[0], 'Expected body search hit'),
          entityId: 'body-hit',
        },
        {
          ...defined(page(1, 'name', 'filename').hits[0], 'Expected filename search hit'),
          entityId: 'name-hit',
        },
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

  it('accepts current index progress, refreshes completed metadata, and maps Cmd-F to search focus intent', async () => {
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

    await act(async () => {
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
        complete: true,
      })
      await Promise.resolve()
      await Promise.resolve()
    })
    expect(viewer.queryFolder).toHaveBeenCalledTimes(2)
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
    expect(writableHook.result.current.state.selectedEntityIds).toEqual(['image-1', 'image-2'])
    await act(() => writableHook.result.current.setReviewState('reject', ['image-2', 'image-2']))
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
    expect(writableHook.result.current.state.selectedEntityIds).toEqual(['image-1', 'image-2'])

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

  it('keeps the operation in finishing state until results and projection cleanup settle', async () => {
    const viewer = bridge()
    const results = deferred<{ total: number; offset: number; items: [] }>()
    let receiveOperation: ((event: OperationProgressEvent) => void) | undefined
    vi.mocked(viewer.listenOperationProgress).mockImplementation(async (handler) => {
      receiveOperation = handler
      return () => undefined
    })
    vi.mocked(viewer.operationResults).mockImplementation(() => results.promise)
    const { result } = renderHook(() => useViewerController(viewer))
    await act(() => result.current.openProject('/fixture/project'))
    await act(async () => {
      await Promise.resolve()
      await Promise.resolve()
    })
    await act(() =>
      result.current.executeFileCommand('rename', [
        {
          entityId: 'image-1',
          action: { kind: 'rename', proposedName: 'hero.jpg', editExtension: true },
        },
      ]),
    )

    act(() => receiveOperation?.(operationProgress('session-1', 1, 'batch-1')))
    expect(result.current.state.operation.active?.lifecycle).toBe('completed')
    expect(result.current.state.operation.finishing).toBe(true)
    await act(() =>
      result.current.executeFileCommand('trash', [
        { entityId: 'image-2', action: { kind: 'trash' } },
      ]),
    )
    expect(viewer.executeFileCommand).toHaveBeenCalledOnce()

    await act(async () => {
      results.resolve({ total: 0, offset: 0, items: [] })
      await results.promise
      await Promise.resolve()
      await Promise.resolve()
    })
    expect(result.current.state.operation.finishing).toBe(false)
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
    await act(() =>
      result.current.executeFileCommand('trash', [
        { entityId: 'image-1', action: { kind: 'trash' } },
      ]),
    )

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

  it('refreshes grid and search after an all-failed file batch may have mutated disk', async () => {
    const viewer = bridge()
    let receiveOperation: ((event: OperationProgressEvent) => void) | undefined
    vi.mocked(viewer.listenOperationProgress).mockImplementation(async (handler) => {
      receiveOperation = handler
      return () => undefined
    })
    vi.mocked(viewer.queryFolder)
      .mockResolvedValueOnce(contentWorkspace(['destination-before-replace']))
      .mockResolvedValueOnce(contentWorkspace(['source-after-failed-replace']))
    vi.mocked(viewer.searchProject)
      .mockResolvedValueOnce(page(1, 'destination-before-replace', 'filename'))
      .mockResolvedValueOnce(page(2, 'source-after-failed-replace', 'filename'))
    vi.mocked(viewer.operationResults).mockResolvedValue({
      total: 1,
      offset: 0,
      items: [
        {
          entityId: 'source',
          relativePath: 'source.jpg',
          status: 'failed',
          code: 'verification_failed',
        },
      ],
    })
    const { result } = renderHook(() => useViewerController(viewer))
    await act(() => result.current.openProject('/fixture/project'))
    act(() =>
      result.current.setSearchFilters({
        ...emptySearchFilters,
        kinds: ['jpeg'],
      }),
    )
    await act(() => vi.advanceTimersByTimeAsync(0))
    await act(async () => {
      await Promise.resolve()
      await Promise.resolve()
    })
    expect(result.current.state.search.page?.hits[0]?.name).toBe('destination-before-replace')
    await act(() =>
      result.current.executeFileCommand('copy', [
        {
          entityId: 'source',
          action: { kind: 'copy', destinationFolderId: 'folder-2' },
        },
      ]),
    )

    await act(async () => {
      receiveOperation?.({
        ...operationProgress('session-1', 1, 'batch-1'),
        completed: 0,
        failed: 1,
      })
      await Promise.resolve()
      await Promise.resolve()
      await Promise.resolve()
    })
    await act(() => vi.advanceTimersByTimeAsync(0))
    await act(async () => {
      await Promise.resolve()
      await Promise.resolve()
    })

    expect(viewer.folderTree).toHaveBeenCalledTimes(2)
    expect(viewer.queryFolder).toHaveBeenCalledTimes(2)
    expect(result.current.state.workspace).toEqual(
      contentWorkspace(['source-after-failed-replace']),
    )
    expect(viewer.searchProject).toHaveBeenCalledTimes(2)
    expect(result.current.state.search.page?.hits[0]?.name).toBe('source-after-failed-replace')
  })

  it('drops a terminal refresh that settles after closing and reopening the same backend identity', async () => {
    const viewer = bridge()
    const oldTerminalFolders = deferred<Awaited<ReturnType<ViewerBridge['folderTree']>>>()
    let receiveOperation: ((event: OperationProgressEvent) => void) | undefined
    vi.mocked(viewer.listenOperationProgress).mockImplementation(async (handler) => {
      receiveOperation = handler
      return () => undefined
    })
    vi.mocked(viewer.folderTree)
      .mockResolvedValueOnce([])
      .mockImplementationOnce(() => oldTerminalFolders.promise)
      .mockResolvedValueOnce([])
    const { result } = renderHook(() => useViewerController(viewer))
    await act(() => result.current.openProject('/fixture/project'))
    await act(async () => {
      await Promise.resolve()
      await Promise.resolve()
    })
    await act(() =>
      result.current.executeFileCommand('trash', [
        { entityId: 'image-1', action: { kind: 'trash' } },
      ]),
    )

    act(() => receiveOperation?.(operationProgress('session-1', 1, 'batch-1')))
    expect(viewer.folderTree).toHaveBeenCalledTimes(2)
    await act(() => result.current.closeProject())
    await act(() => result.current.openProject('/fixture/project'))
    vi.mocked(viewer.searchProject).mockClear()
    act(() =>
      result.current.setSearchFilters({
        ...emptySearchFilters,
        kinds: ['jpeg'],
      }),
    )
    await act(() => vi.advanceTimersByTimeAsync(0))
    expect(viewer.searchProject).toHaveBeenCalledOnce()

    await act(async () => {
      oldTerminalFolders.resolve([])
      await Promise.resolve()
      await Promise.resolve()
      await Promise.resolve()
    })
    await act(() => vi.advanceTimersByTimeAsync(0))
    await act(async () => {
      await Promise.resolve()
      await Promise.resolve()
    })

    expect(viewer.searchProject).toHaveBeenCalledOnce()
  })

  it('releases a terminal batch after the same project advances generation', async () => {
    const viewer = bridge()
    const oldTerminalFolders = deferred<Awaited<ReturnType<ViewerBridge['folderTree']>>>()
    let receiveOperation: ((event: OperationProgressEvent) => void) | undefined
    let receiveScan: ((event: ScanEvent) => void) | undefined
    vi.mocked(viewer.listenOperationProgress).mockImplementation(async (handler) => {
      receiveOperation = handler
      return () => undefined
    })
    vi.mocked(viewer.listenScan).mockImplementation(async (handler) => {
      receiveScan = handler
      return () => undefined
    })
    vi.mocked(viewer.folderTree)
      .mockResolvedValueOnce([])
      .mockImplementationOnce(() => oldTerminalFolders.promise)
      .mockResolvedValue([])
    vi.mocked(viewer.projectSnapshot).mockResolvedValue({
      projectId: 'project-1',
      sessionId: 'session-1',
      generation: 2,
      displayName: 'Catalog',
      access: 'read_write',
    })
    vi.mocked(viewer.executeFileCommand)
      .mockResolvedValueOnce({ batchId: 'batch-1' })
      .mockResolvedValueOnce({ batchId: 'batch-2' })
    const { result } = renderHook(() => useViewerController(viewer))
    await act(() => result.current.openProject('/fixture/project'))
    await act(async () => {
      await Promise.resolve()
      await Promise.resolve()
    })
    await act(() =>
      result.current.executeFileCommand('trash', [
        { entityId: 'image-1', action: { kind: 'trash' } },
      ]),
    )

    act(() => receiveOperation?.(operationProgress('session-1', 1, 'batch-1')))
    await act(async () => {
      receiveScan?.({
        type: 'finished',
        sessionId: 'session-1',
        generation: 2,
        taskId: 'scan-2',
        totals: { folders: 0, files: 0, failed: 0 },
      })
      await Promise.resolve()
      await Promise.resolve()
      await Promise.resolve()
    })
    expect(result.current.state.project?.generation).toBe(2)
    expect(result.current.state.operation.finishing).toBe(true)

    await act(async () => {
      oldTerminalFolders.resolve([])
      await Promise.resolve()
      await Promise.resolve()
      await Promise.resolve()
    })
    await act(() =>
      result.current.executeFileCommand('trash', [
        { entityId: 'image-2', action: { kind: 'trash' } },
      ]),
    )

    expect(result.current.state.operation.finishing).toBe(false)
    expect(viewer.executeFileCommand).toHaveBeenCalledTimes(2)
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
    await act(() =>
      result.current.executeFileCommand('trash', [
        { entityId: 'image-1', action: { kind: 'trash' } },
      ]),
    )
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
    act(() => result.current.consumeContextRepair())
    expect(result.current.state.contextRepair?.suggestedEntityId).toBeNull()
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

  it('admits only one undo and blocks a new file command until undo settles', async () => {
    const viewer = bridge()
    const undo = deferred<Awaited<ReturnType<ViewerBridge['undoLastOperation']>>>()
    vi.mocked(viewer.undoLastOperation).mockImplementation(() => undo.promise)
    const { result } = renderHook(() => useViewerController(viewer))
    await act(() => result.current.openProject('/fixture/project'))

    let first!: ReturnType<typeof result.current.undoLastOperation>
    act(() => {
      first = result.current.undoLastOperation()
      void result.current.undoLastOperation()
      void result.current.executeFileCommand('trash', [
        { entityId: 'image-1', action: { kind: 'trash' } },
      ])
    })
    expect(viewer.undoLastOperation).toHaveBeenCalledOnce()
    expect(viewer.executeFileCommand).not.toHaveBeenCalled()
    expect(result.current.state.operation.pending).toBe(true)

    await act(async () => {
      undo.resolve(null)
      await first
    })
    expect(result.current.state.operation.pending).toBe(false)
  })

  it('never mutates a hidden folder selection while search results show another entity', async () => {
    const viewer = bridge()
    const { result } = renderHook(() => useViewerController(viewer))
    await act(() => result.current.openProject('/fixture/project'))
    act(() => {
      result.current.setSelectedEntityIds(['hidden-a'])
      result.current.setSearchText('needle')
      result.current.setSelectedEntityIds(['hidden-a'])
      result.current.setVisibleSearchHits(['visible-b'])
    })
    await act(() => result.current.setReviewState('keep'))
    await act(() => result.current.toggleFavorite())

    expect(viewer.setReviewState).not.toHaveBeenCalled()
    expect(viewer.toggleFavorite).not.toHaveBeenCalled()
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
    await act(() =>
      result.current.previewRename(['image-1'], {
        find: '',
        replacement: '',
        prefix: 'x-',
        suffix: '',
        sequence: null,
      }),
    )
    await act(() =>
      result.current.preflightFileCommand('trash', [
        { entityId: 'image-1', action: { kind: 'trash' } },
      ]),
    )
    await act(() =>
      result.current.executeFileCommand('trash', [
        { entityId: 'image-1', action: { kind: 'trash' } },
      ]),
    )
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
  ] satisfies [string, FileCommandPreflight][])(
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
    vi.mocked(viewer.closeProject).mockResolvedValueOnce('stayed').mockResolvedValueOnce('closed')
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

  it('clears the frontend session when cache cleanup fails after the backend already closed', async () => {
    const viewer = bridge()
    vi.mocked(viewer.closeProject).mockRejectedValueOnce({
      code: 'project_closed_cache_cleanup_failed',
      category: 'environment',
      userMessage: '项目已关闭，但临时缓存未能清除；退出 Viewer 后将重试。',
      retryable: true,
      taskId: null,
      itemId: null,
    })
    const { result } = renderHook(() => useViewerController(viewer))
    await act(() => result.current.openProject('/fixture/project'))
    act(() => result.current.setSelectedEntityIds(['image-1']))

    await act(() => result.current.closeProject())

    expect(result.current.state.project).toBeNull()
    expect(result.current.state.status).toBe('empty')
    expect(result.current.state.selectedEntityIds).toEqual([])
    expect(result.current.state.errorMessage).toBe(
      '项目已关闭，但临时缓存未能清除；退出 Viewer 后将重试。',
    )
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
    videos: [],
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
      videoMetadata: null,
    })),
    otherFiles: [],
  }
}

function page(revision: number, name: string, matchedField: 'filename' | 'body'): SearchPage {
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
