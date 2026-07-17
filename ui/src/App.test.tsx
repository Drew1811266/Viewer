import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ViewerBridge } from './api/viewer'
import App from './App'

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
    searchProject: vi.fn(),
    searchTextSnippet: vi.fn(),
    setReviewState: vi.fn(),
    toggleFavorite: vi.fn(),
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

describe('Viewer empty state', () => {
  it('asks the user to import one project folder', () => {
    render(<App />)
    expect(screen.getByRole('heading', { name: 'Viewer' })).toBeVisible()
    expect(screen.getByText('拖入或选择一个项目文件夹')).toBeVisible()
  })

  it('shows a persistent read-only banner for an active read-only project', async () => {
    render(<App bridge={bridge('read_only')} />)

    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    expect(await screen.findByText('只读项目')).toBeVisible()
    expect(screen.getByText('Catalog')).toBeVisible()
  })

  it('removes the scan listener when unmounted', async () => {
    const unlisten = vi.fn()
    const viewer = bridge()
    vi.mocked(viewer.listenScan).mockResolvedValue(unlisten)
    const rendered = render(<App bridge={viewer} />)
    await waitFor(() => expect(viewer.listenScan).toHaveBeenCalledOnce())

    rendered.unmount()

    await waitFor(() => expect(unlisten).toHaveBeenCalledOnce())
  })

  it('reconciles a newer scan generation through a fresh snapshot', async () => {
    const viewer = bridge()
    let receiveScan: Parameters<ViewerBridge['listenScan']>[0] | undefined
    vi.mocked(viewer.listenScan).mockImplementation(async (handler) => {
      receiveScan = handler
      return () => undefined
    })
    vi.mocked(viewer.projectSnapshot).mockResolvedValue({
      projectId: 'project-1',
      sessionId: 'session-1',
      generation: 2,
      displayName: 'Catalog',
      access: 'read_write',
    })
    render(<App bridge={viewer} />)
    await waitFor(() => expect(receiveScan).toBeDefined())
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    await screen.findByText('Catalog')

    act(() => {
      receiveScan?.({
        type: 'folders',
        sessionId: 'session-1',
        generation: 2,
        taskId: 'task-2',
        nodes: [],
      })
    })

    await waitFor(() => expect(viewer.projectSnapshot).toHaveBeenCalledOnce())
  })

  it('returns to the empty surface when the native window closes the session', async () => {
    const viewer = bridge()
    let receiveProjectClosed: (() => void) | undefined
    vi.mocked(viewer.listenProjectClosed).mockImplementation(async (handler) => {
      receiveProjectClosed = handler
      return () => undefined
    })
    render(<App bridge={viewer} />)
    await waitFor(() => expect(receiveProjectClosed).toBeDefined())
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    await screen.findByText('Catalog')

    act(() => receiveProjectClosed?.())

    expect(await screen.findByRole('heading', { name: 'Viewer' })).toBeVisible()
    expect(viewer.closeProject).not.toHaveBeenCalled()
  })

  it('opens exactly one project path delivered by the native drop bridge', async () => {
    const viewer = bridge()
    let receiveProjectDrop: ((paths: string[]) => void) | undefined
    vi.mocked(viewer.listenProjectDrops).mockImplementation(async (handler) => {
      receiveProjectDrop = handler
      return () => undefined
    })
    render(<App bridge={viewer} />)
    await waitFor(() => expect(receiveProjectDrop).toBeDefined())

    act(() => receiveProjectDrop?.(['/fixture/dropped-project']))

    await waitFor(() =>
      expect(viewer.openProject).toHaveBeenCalledWith('/fixture/dropped-project'),
    )
    expect(await screen.findByText('Catalog')).toBeVisible()
  })

  it('renders a safe fallback instead of raw thrown details', async () => {
    const viewer = bridge()
    vi.mocked(viewer.openProject).mockRejectedValue(
      new Error('/Users/private/project could not be opened'),
    )
    render(<App bridge={viewer} />)

    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    expect(await screen.findByRole('alert')).toHaveTextContent('操作未完成，请重试。')
    expect(screen.queryByText(/Users\/private/)).not.toBeInTheDocument()
  })

  it('keeps grid selection and scroll mounted across image preview', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue({
      workspace: 'content',
      images: Array.from({ length: 20 }, (_, index) => ({
        entityId: `image-${index}`,
        relativePath: `id-1/${index}.jpg`,
        name: `${index}.jpg`,
        kind: 'jpeg',
        size: 100,
        modifiedNs: String(index),
        marker: { reviewState: null, favorite: false },
        imageMetadata: null,
        imageUrl: null,
      })),
      textFiles: [],
    })
    vi.mocked(viewer.requestImage).mockImplementation(async ({ entityId }) => ({
      cacheKey: entityId,
      url: `viewer-image://localhost/session/${entityId}`,
      width: 800,
      height: 600,
      backend: 'quick_look',
    }))
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const grid = await screen.findByRole('listbox', { name: '图片文件' })
    fireEvent.click(screen.getByRole('option', { name: '1.jpg' }))
    grid.scrollTop = 200
    fireEvent.scroll(grid)
    fireEvent.keyDown(grid, { key: ' ' })
    await screen.findByRole('img', { name: '1.jpg' })

    fireEvent.click(screen.getByRole('button', { name: '关闭预览' }))

    expect(screen.getByRole('listbox', { name: '图片文件' })).toBe(grid)
    expect(grid.scrollTop).toBe(200)
    expect(screen.getByRole('option', { name: '1.jpg' })).toHaveAttribute(
      'aria-selected',
      'true',
    )
  })

  it('replaces only the right workspace with search and returns to folder context', async () => {
    const viewer = bridge()
    vi.mocked(viewer.searchProject).mockResolvedValue({
      revision: 1,
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
          entityId: 'image-1',
          relativePath: 'id-1/shoe.jpg',
          name: 'shoe.jpg',
          kind: 'jpeg',
          size: 10,
          modifiedNs: '1',
          marker: { reviewState: null, favorite: false },
          imageMetadata: null,
          matchedField: 'filename',
          score: 1,
          groupRelativePath: 'id-1',
          matchRanges: [{ start: 0, end: 4 }],
        },
      ],
    })
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const search = await screen.findByRole('searchbox', { name: '搜索项目' })

    fireEvent.keyDown(window, { key: 'f', metaKey: true })
    expect(search).toHaveFocus()
    fireEvent.change(search, { target: { value: 'shoe' } })
    await waitFor(() => expect(viewer.searchProject).toHaveBeenCalledOnce())
    expect(await screen.findByRole('option', { name: /shoe.jpg/ })).toBeVisible()
    expect(screen.getByRole('tree', { name: '项目文件夹' })).toBeVisible()

    fireEvent.click(screen.getByRole('button', { name: '返回文件夹内容' }))
    expect(await screen.findByText('此文件夹中没有支持的文件。')).toBeVisible()
  })
})
