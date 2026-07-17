import { act, renderHook } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { ViewerBridge } from '../api/viewer'
import type { IndexProgressEvent, SearchPage } from '../api/types'
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
    closeProject: vi.fn().mockResolvedValue(undefined),
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
    listenScan: vi.fn().mockResolvedValue(() => undefined),
    listenIndexProgress: vi.fn().mockResolvedValue(() => undefined),
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

    const readOnly = bridge('read_only')
    const readOnlyHook = renderHook(() => useViewerController(readOnly))
    await act(() => readOnlyHook.result.current.openProject('/fixture/project'))
    act(() => readOnlyHook.result.current.setSelectedEntityIds(['image-1']))
    await act(() => readOnlyHook.result.current.setReviewState('reject'))
    await act(() => readOnlyHook.result.current.toggleFavorite())
    expect(readOnly.setReviewState).not.toHaveBeenCalled()
    expect(readOnly.toggleFavorite).not.toHaveBeenCalled()
  })
})

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
