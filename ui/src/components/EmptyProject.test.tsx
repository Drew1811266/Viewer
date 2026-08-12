import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ProjectDropEvent, ViewerBridge } from '../api/viewer'
import EmptyProject from './EmptyProject'

function bridge(): ViewerBridge {
  return {
    chooseProject: vi.fn().mockResolvedValue('/fixture/project'),
    openProject: vi.fn().mockResolvedValue({
      projectId: 'project-1',
      sessionId: 'session-1',
      generation: 1,
      displayName: 'project',
      access: 'read_write',
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
    searchProject: vi.fn(),
    searchTextSnippet: vi.fn(),
    setReviewState: vi.fn(),
    toggleFavorite: vi.fn(),
    selectionInfo: vi.fn(),
    previewRename: vi.fn().mockResolvedValue({ rows: [], executable: false }),
    preflightFileCommand: vi.fn().mockResolvedValue({ rows: [], executable: false }),
    executeFileCommand: vi.fn().mockResolvedValue({ batchId: 'batch-1' }),
    operationStatus: vi.fn(),
    operationResults: vi.fn().mockResolvedValue({ total: 0, offset: 0, items: [] }),
    cancelOperation: vi.fn().mockResolvedValue(false),
    undoLastOperation: vi.fn().mockResolvedValue(null),
    beginFinderDrag: vi.fn().mockResolvedValue({ fileCount: 1 }),
    videoOpen: vi.fn(),
    videoClose: vi.fn(),
    videoPlay: vi.fn(),
    videoPause: vi.fn(),
    videoSeek: vi.fn(),
    videoStep: vi.fn(),
    videoSetVolume: vi.fn(),
    videoSetMuted: vi.fn(),
    videoSetRate: vi.fn(),
    videoSetSurfaceRect: vi.fn(),
    videoSetFullscreen: vi.fn(),
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
    listenProjectClosed: vi.fn().mockResolvedValue(() => undefined),
    listenProjectDrops: vi.fn().mockResolvedValue(() => undefined),
    listenProjectDropEvents: vi.fn().mockResolvedValue(() => undefined),
  }
}

describe('EmptyProject', () => {
  it('keeps the resting entry state to the approved three elements', () => {
    render(<EmptyProject bridge={bridge()} />)

    const entry = screen.getByTestId('project-drop-zone')
    expect(within(entry).getByRole('heading', { name: 'Viewer' })).toBeVisible()
    expect(within(entry).getByText('选择或拖入一个项目文件夹')).toBeVisible()
    expect(within(entry).getByRole('button', { name: '选择项目文件夹' })).toHaveClass(
      'empty-project-primary-action',
    )
    expect(within(entry).queryByText(/支持 JPG|不会记住|最近/)).not.toBeInTheDocument()
  })

  it('shows folder-drop feedback only while a directory is dragged over the window', () => {
    render(<EmptyProject bridge={bridge()} />)
    const entry = screen.getByTestId('project-drop-zone')

    fireEvent.dragEnter(entry, {
      dataTransfer: { items: [{ webkitGetAsEntry: () => ({ isDirectory: true }) }] },
    })
    expect(entry).toHaveAttribute('data-drop-state', 'valid')
    expect(screen.getByText('松开以打开项目')).toBeVisible()

    fireEvent.dragLeave(entry)
    expect(screen.queryByText('松开以打开项目')).not.toBeInTheDocument()
  })

  it('renders native Finder hover and invalid-drop feedback on the empty surface', async () => {
    const viewer = bridge()
    let receiveNativeDrop: ((event: ProjectDropEvent) => void) | undefined
    vi.mocked(viewer.listenProjectDropEvents).mockImplementation(async (handler) => {
      receiveNativeDrop = handler
      return () => undefined
    })
    const openProject = vi.fn().mockResolvedValue('invalid-root')
    render(<EmptyProject bridge={viewer} onOpenProject={openProject} />)
    await waitFor(() => expect(receiveNativeDrop).toBeDefined())
    const entry = screen.getByTestId('project-drop-zone')

    act(() => receiveNativeDrop?.({ type: 'enter', paths: ['/fixture/project'] }))
    expect(entry).toHaveAttribute('data-drop-state', 'valid')
    expect(screen.getByText('松开以打开项目')).toBeVisible()

    act(() => receiveNativeDrop?.({ type: 'leave' }))
    expect(entry).not.toHaveAttribute('data-drop-state')

    act(() => receiveNativeDrop?.({ type: 'drop', paths: ['/fixture/not-a-directory.jpg'] }))
    await waitFor(() => expect(openProject).toHaveBeenCalledWith('/fixture/not-a-directory.jpg'))
    expect(entry).toHaveAttribute('data-drop-state', 'invalid')
    expect(screen.getByRole('alert')).toHaveTextContent('请选择一个文件夹')
  })

  it('clears stale invalid-drop details when a valid native folder enters again', async () => {
    const viewer = bridge()
    let receiveNativeDrop: ((event: ProjectDropEvent) => void) | undefined
    vi.mocked(viewer.listenProjectDropEvents).mockImplementation(async (handler) => {
      receiveNativeDrop = handler
      return () => undefined
    })
    const openProject = vi.fn().mockResolvedValue('invalid-root')
    render(<EmptyProject bridge={viewer} onOpenProject={openProject} />)
    await waitFor(() => expect(receiveNativeDrop).toBeDefined())

    act(() => receiveNativeDrop?.({ type: 'drop', paths: ['/fixture/not-a-directory.jpg'] }))
    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('请选择一个文件夹'))

    act(() => receiveNativeDrop?.({ type: 'enter', paths: ['/fixture/project'] }))

    expect(screen.getByTestId('project-drop-zone')).toHaveAttribute('data-drop-state', 'valid')
    expect(screen.getByText('松开以打开项目')).toBeVisible()
    expect(screen.queryByText('无法打开此项目')).not.toBeInTheDocument()
    expect(screen.queryByText('请选择项目文件夹，不能导入单个文件。')).not.toBeInTheDocument()
  })

  it('replaces entry controls with an indeterminate opening state', async () => {
    const opening = new Promise<void>(() => undefined)
    render(<EmptyProject bridge={bridge()} onOpenProject={() => opening} />)

    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    expect(await screen.findByRole('heading', { name: 'project' })).toBeVisible()
    expect(screen.getByText('正在验证项目…')).toBeVisible()
    expect(screen.getByRole('progressbar', { name: '正在打开项目' })).toHaveClass(
      'project-opening-progress',
    )
    expect(screen.getByRole('progressbar', { name: '正在打开项目' })).not.toHaveAttribute(
      'aria-valuenow',
    )
    expect(screen.queryByRole('status', { name: '正在打开项目' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '选择项目文件夹' })).not.toBeInTheDocument()
  })

  it('opens the directory selected by the native chooser', async () => {
    const viewer = bridge()
    render(<EmptyProject bridge={viewer} />)

    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    await waitFor(() => expect(viewer.openProject).toHaveBeenCalledWith('/fixture/project'))
  })

  it('rejects a dropped file before opening a project', async () => {
    const viewer = bridge()
    render(<EmptyProject bridge={viewer} />)
    const item = {
      webkitGetAsEntry: () => ({ isDirectory: false }),
    }

    fireEvent.drop(screen.getByTestId('project-drop-zone'), {
      dataTransfer: { items: [item], files: [{ name: 'not-a-directory.jpg' }] },
    })

    expect(screen.getByTestId('project-drop-zone')).toHaveAttribute('data-drop-state', 'invalid')
    expect(await screen.findByRole('alert')).toHaveTextContent('请选择一个文件夹')
    expect(screen.getByRole('alert')).toHaveClass('project-drop-message')
    expect(viewer.openProject).not.toHaveBeenCalled()
  })

  it('replaces the idle entry with the concise project-open recovery state', async () => {
    const viewer = bridge()
    render(<EmptyProject bridge={viewer} errorMessage="项目审阅数据库不可用。" fatalError />)

    expect(screen.getByRole('heading', { name: '无法打开项目' })).toBeVisible()
    expect(screen.getByText('项目审阅数据库不可用。')).toBeVisible()
    expect(screen.getByRole('button', { name: '重新选择' })).toBeVisible()
    expect(screen.queryByRole('heading', { name: 'Viewer' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '选择项目文件夹' })).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: '重新选择' }))
    await waitFor(() => expect(viewer.chooseProject).toHaveBeenCalledOnce())
  })
})
