import { act, createEvent, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { ViewerBridge } from './api/viewer'
import App from './App'

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
    requestImage: vi.fn().mockResolvedValue({
      cacheKey: 'test-image',
      url: 'viewer-image://localhost/session/test-image',
      width: 800,
      height: 600,
      backend: 'quick_look',
    }),
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
    previewRename: vi.fn().mockResolvedValue({ rows: [], executable: false }),
    preflightFileCommand: vi.fn().mockResolvedValue({ rows: [], executable: false }),
    executeFileCommand: vi.fn().mockResolvedValue({ batchId: 'batch-1' }),
    operationStatus: vi.fn().mockResolvedValue({
      sessionId: 'session-1',
      generation: 1,
      batchId: 'batch-1',
      target: 'project',
      lifecycle: 'queued',
      requested: 1,
      completed: 0,
      failed: 0,
      skipped: 0,
      cancelled: 0,
      activeEntityId: null,
    }),
    operationResults: vi.fn().mockResolvedValue({ total: 0, offset: 0, items: [] }),
    cancelOperation: vi.fn().mockResolvedValue(false),
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

describe('Viewer empty state', () => {
  it('asks the user to import one project folder', () => {
    render(<App />)
    expect(screen.getByRole('heading', { name: 'Viewer' })).toBeVisible()
    expect(screen.getByText('拖入或选择一个项目文件夹')).toBeVisible()
  })

  it('keeps read-only browsing and comparison available while disabling every write', async () => {
    const viewer = bridge('read_only')
    vi.mocked(viewer.queryFolder).mockResolvedValue(readOnlyContentWorkspace())
    vi.mocked(viewer.previewText).mockResolvedValue({
      entityId: 'text-1',
      format: 'plain_text',
      plainText: 'readonly product notes',
      markdownHtml: null,
      encoding: 'utf8',
      truncated: false,
    })
    render(<App bridge={viewer} />)

    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    expect(await screen.findByRole('status', { name: '只读模式' })).toBeVisible()
    expect(screen.getByText('Catalog')).toBeVisible()
    expect(viewer.openPermissionSettings).not.toHaveBeenCalled()

    fireEvent.click(screen.getByRole('button', { name: '打开权限设置' }))
    expect(viewer.openPermissionSettings).toHaveBeenCalledOnce()
    expect(screen.getByRole('searchbox', { name: '搜索项目' })).toBeEnabled()
    expect(screen.getByRole('combobox', { name: '排序方式' })).toBeEnabled()
    expect(screen.getByText('筛选')).toBeVisible()

    const front = await screen.findByRole('option', { name: 'front.jpg' })
    const back = screen.getByRole('option', { name: 'back.jpg' })
    fireEvent.doubleClick(front)
    expect(screen.getByRole('dialog', { name: '图片预览' })).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '关闭预览' }))
    fireEvent.click(front)
    fireEvent.click(back, { metaKey: true })

    expect(screen.getByRole('button', { name: '并排对比' })).toBeEnabled()
    expect(screen.getByRole('button', { name: '信息' })).toBeEnabled()
    expect(screen.getByRole('button', { name: '批量重命名' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '复制到…' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '移动到…' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '移到废纸篓' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '标记为保留' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '整理 front.jpg' })).toBeDisabled()
    fireEvent.dragStart(front, { dataTransfer: viewerDragTransfer() })
    await waitFor(() =>
      expect(viewer.beginFinderDrag).toHaveBeenCalledWith({
        sessionId: 'session-1',
        generation: 1,
        entityIds: ['image-1', 'image-2'],
      }),
    )

    fireEvent.click(screen.getByRole('button', { name: '信息' }))
    expect(screen.getByRole('complementary', { name: '文件信息' })).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '关闭信息' }))

    fireEvent.click(screen.getByRole('button', { name: '并排对比' }))
    expect(screen.getByRole('region', { name: '图片对比' })).toBeVisible()
    expect(screen.getByRole('status', { name: '只读模式' })).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '关闭对比' }))
    fireEvent.doubleClick(screen.getByRole('option', { name: 'notes.txt' }))
    expect(await screen.findByText('readonly product notes')).toBeVisible()
  })

  it('reselects from a read-only project through normal close and forgets the old path', async () => {
    const viewer = bridge('read_only')
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    await screen.findByText('Catalog')

    fireEvent.click(screen.getByRole('button', { name: '重新选择目录' }))

    await waitFor(() =>
      expect(viewer.closeProject).toHaveBeenCalledWith(undefined, 'project'),
    )
    expect(await screen.findByRole('heading', { name: 'Viewer' })).toBeVisible()
    expect(screen.getByRole('alert')).toHaveTextContent('已关闭只读项目，请选择已授权的目录。')
    expect(viewer.chooseProject).toHaveBeenCalledOnce()
  })

  it('offers stay, wait and cancel-pending choices when close is blocked', async () => {
    const viewer = bridge()
    let receiveCloseBlocked: Parameters<ViewerBridge['listenCloseBlocked']>[0] | undefined
    vi.mocked(viewer.listenCloseBlocked).mockImplementation(async (handler) => {
      receiveCloseBlocked = handler
      return () => undefined
    })
    render(<App bridge={viewer} />)
    await waitFor(() => expect(receiveCloseBlocked).toBeDefined())
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    await screen.findByText('Catalog')

    act(() => receiveCloseBlocked?.({
      sessionId: 'session-1',
      generation: 1,
      batchId: 'batch-1',
      target: 'project',
    }))
    fireEvent.click(screen.getByRole('button', { name: '保持打开' }))
    expect(screen.queryByRole('dialog', { name: '文件操作尚未完成' })).not.toBeInTheDocument()
    expect(screen.getByText('Catalog')).toBeVisible()
    expect(viewer.closeProject).not.toHaveBeenCalled()

    act(() => receiveCloseBlocked?.({
      sessionId: 'session-1',
      generation: 1,
      batchId: 'batch-1',
      target: 'project',
    }))
    fireEvent.click(screen.getByRole('button', { name: '等待完成后关闭' }))
    await waitFor(() => expect(viewer.closeProject).toHaveBeenCalledWith('wait', 'project'))
    expect(await screen.findByRole('heading', { name: 'Viewer' })).toBeVisible()

    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    await screen.findByText('Catalog')
    act(() => receiveCloseBlocked?.({
      sessionId: 'session-1',
      generation: 1,
      batchId: 'batch-2',
      target: 'application',
    }))
    fireEvent.click(screen.getByRole('button', { name: '取消待处理项目并关闭' }))
    await waitFor(() =>
      expect(viewer.closeProject).toHaveBeenLastCalledWith('cancel_pending', 'application'),
    )
    expect(await screen.findByRole('heading', { name: 'Viewer' })).toBeVisible()
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

  it('opens safe rename and Trash surfaces from keyboard without immediate deletion', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    vi.mocked(viewer.operationStatus).mockResolvedValue({
      sessionId: 'session-1',
      generation: 1,
      batchId: 'batch-1',
      lifecycle: 'completed',
      requested: 1,
      completed: 1,
      failed: 0,
      skipped: 0,
      cancelled: 0,
      activeEntityId: null,
    })
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })
    fireEvent.click(file)

    fireEvent.keyDown(window, { key: 'Enter' })
    const renameDialog = screen.getByRole('dialog', { name: '重命名文件' })
    expect(renameDialog).toBeVisible()
    fireEvent.change(within(renameDialog).getByRole('textbox', { name: '新文件名' }), {
      target: { value: 'hero.jpg' },
    })
    fireEvent.click(within(renameDialog).getByRole('button', { name: '重命名' }))
    await waitFor(() =>
      expect(viewer.executeFileCommand).toHaveBeenCalledWith(
        expect.objectContaining({
          kind: 'rename',
          items: [
            {
              entityId: 'image-1',
              action: { kind: 'rename', proposedName: 'hero', editExtension: false },
            },
          ],
        }),
      ),
    )
    await waitFor(() => expect(viewer.operationResults).toHaveBeenCalledOnce())

    fireEvent.keyDown(window, { key: 'Delete' })
    expect(screen.getByRole('dialog', { name: '将文件移到废纸篓？' })).toBeVisible()
    expect(viewer.executeFileCommand).toHaveBeenCalledTimes(1)
    fireEvent.click(screen.getByRole('button', { name: '移入废纸篓' }))
    await waitFor(() => expect(viewer.executeFileCommand).toHaveBeenCalledTimes(2))
  })

  it('routes Command-Z only outside editable and modal contexts', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })
    fireEvent.click(file)

    fireEvent.keyDown(window, { key: 'z', metaKey: true })
    await waitFor(() => expect(viewer.undoLastOperation).toHaveBeenCalledOnce())
    fireEvent.keyDown(window, { key: 'z', metaKey: true, shiftKey: true })
    expect(viewer.undoLastOperation).toHaveBeenCalledOnce()
    fireEvent.keyDown(window, { key: 'z', metaKey: true, altKey: true })
    fireEvent.keyDown(window, { key: 'z', metaKey: true, ctrlKey: true })
    expect(viewer.undoLastOperation).toHaveBeenCalledOnce()

    const renameButton = screen.getByRole('button', { name: '重命名' })
    renameButton.focus()
    fireEvent.keyDown(renameButton, { key: 'Enter' })
    expect(screen.queryByRole('dialog', { name: '重命名文件' })).not.toBeInTheDocument()

    const search = screen.getByRole('searchbox', { name: '搜索项目' })
    search.focus()
    fireEvent.keyDown(search, { key: 'z', metaKey: true })
    fireEvent.keyDown(search, { key: 'Delete' })
    expect(viewer.undoLastOperation).toHaveBeenCalledOnce()
    expect(screen.queryByRole('dialog', { name: '将文件移到废纸篓？' })).not.toBeInTheDocument()

    fireEvent.doubleClick(file)
    expect(screen.getByRole('dialog', { name: '图片预览' })).toBeVisible()
    fireEvent.keyDown(window, { key: 'Delete' })
    expect(screen.queryByRole('dialog', { name: '将文件移到废纸篓？' })).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: '关闭预览' }))

    fireEvent.keyDown(window, { key: 'Delete' })
    fireEvent.keyDown(window, { key: 'z', metaKey: true })
    expect(viewer.undoLastOperation).toHaveBeenCalledOnce()
  })

  it('invalidates an open operation dialog when project closing begins', async () => {
    const viewer = bridge()
    const closing = deferred<'closed'>()
    vi.mocked(viewer.closeProject).mockImplementation(() => closing.promise)
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    fireEvent.click(await screen.findByRole('option', { name: 'front.jpg' }))
    fireEvent.keyDown(window, { key: 'Enter' })
    expect(screen.getByRole('dialog', { name: '重命名文件' })).toBeVisible()

    fireEvent.click(screen.getByRole('button', { name: '关闭项目' }))
    expect(screen.queryByRole('dialog', { name: '重命名文件' })).not.toBeInTheDocument()
    expect(viewer.executeFileCommand).not.toHaveBeenCalled()

    await act(async () => {
      closing.resolve('closed')
      await closing.promise
    })
  })

  it('does not route Command-Z while a file operation is active', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    fireEvent.click(await screen.findByRole('option', { name: 'front.jpg' }))
    fireEvent.keyDown(window, { key: 'Enter' })
    const dialog = screen.getByRole('dialog', { name: '重命名文件' })
    fireEvent.change(within(dialog).getByRole('textbox', { name: '新文件名' }), {
      target: { value: 'hero.jpg' },
    })
    fireEvent.click(within(dialog).getByRole('button', { name: '重命名' }))
    await waitFor(() => expect(viewer.executeFileCommand).toHaveBeenCalledOnce())

    fireEvent.keyDown(window, { key: 'z', metaKey: true })
    expect(viewer.undoLastOperation).not.toHaveBeenCalled()
  })

  it('routes an internal file drop through the same preflight and execute commands', async () => {
    const viewer = bridge()
    vi.mocked(viewer.folderTree).mockResolvedValue([
      {
        entityId: 'folder-a',
        parentEntityId: null,
        relativePath: 'id',
        name: 'id',
        marker: { reviewState: null, favorite: false },
      },
      {
        entityId: 'folder-b',
        parentEntityId: null,
        relativePath: 'selected',
        name: 'selected',
        marker: { reviewState: null, favorite: false },
      },
    ])
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    vi.mocked(viewer.preflightFileCommand).mockResolvedValue({
      rows: [{ entityId: 'image-1', relativePath: 'selected/front.jpg', state: 'ready' }],
      executable: true,
    })
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const transfer = viewerDragTransfer()

    fireEvent.dragStart(await screen.findByRole('button', { name: '整理 front.jpg' }), {
      dataTransfer: transfer,
    })
    const destination = await screen.findByRole('treeitem', { name: 'selected' })
    fireEvent.dragOver(destination, { dataTransfer: transfer })
    fireEvent.drop(destination, { dataTransfer: transfer })

    await waitFor(() =>
      expect(viewer.preflightFileCommand).toHaveBeenCalledWith({
        sessionId: 'session-1',
        generation: 1,
        kind: 'move',
        items: [
          {
            entityId: 'image-1',
            action: { kind: 'move', destinationFolderId: 'folder-b' },
          },
        ],
      }),
    )
    await waitFor(() =>
      expect(viewer.executeFileCommand).toHaveBeenCalledWith(
        expect.objectContaining({
          kind: 'move',
          items: [
            {
              entityId: 'image-1',
              action: { kind: 'move', destinationFolderId: 'folder-b' },
            },
          ],
        }),
      ),
    )
  })

  it('starts copy-only native export during the file-body dragstart', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })

    const event = createEvent.dragStart(file, { dataTransfer: viewerDragTransfer() })
    fireEvent(file, event)

    expect(event.defaultPrevented).toBe(true)
    await waitFor(() =>
      expect(viewer.beginFinderDrag).toHaveBeenCalledWith({
        sessionId: 'session-1',
        generation: 1,
        entityIds: ['image-1'],
      }),
    )
  })

  it('uses the organization handle for a frozen default move without Finder export', async () => {
    const viewer = bridge()
    vi.mocked(viewer.folderTree).mockResolvedValue([
      {
        entityId: 'folder-b',
        parentEntityId: null,
        relativePath: 'selected',
        name: 'selected',
        marker: { reviewState: null, favorite: false },
      },
    ])
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const transfer = viewerDragTransfer()

    fireEvent.dragStart(await screen.findByRole('button', { name: '整理 front.jpg' }), {
      dataTransfer: transfer,
    })
    fireEvent.dragOver(await screen.findByRole('treeitem', { name: 'selected' }), {
      altKey: true,
      dataTransfer: transfer,
    })
    fireEvent.drop(screen.getByRole('treeitem', { name: 'selected' }), {
      altKey: true,
      dataTransfer: transfer,
    })

    expect(viewer.beginFinderDrag).not.toHaveBeenCalled()
    await waitFor(() =>
      expect(viewer.preflightFileCommand).toHaveBeenCalledWith(
        expect.objectContaining({ kind: 'move' }),
      ),
    )
  })

  it('keeps Option-copy frozen after it is released over the destination', async () => {
    const viewer = bridge()
    vi.mocked(viewer.folderTree).mockResolvedValue([
      {
        entityId: 'folder-b',
        parentEntityId: null,
        relativePath: 'selected',
        name: 'selected',
        marker: { reviewState: null, favorite: false },
      },
    ])
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const handle = await screen.findByRole('button', { name: '整理 front.jpg' })
    const transfer = viewerDragTransfer()
    const start = createEvent.dragStart(handle, { dataTransfer: transfer })
    Object.defineProperty(start, 'altKey', { value: true })
    fireEvent(handle, start)

    fireEvent.dragOver(await screen.findByRole('treeitem', { name: 'selected' }), {
      altKey: false,
      dataTransfer: transfer,
    })
    fireEvent.drop(screen.getByRole('treeitem', { name: 'selected' }), {
      altKey: false,
      dataTransfer: transfer,
    })

    await waitFor(() =>
      expect(viewer.preflightFileCommand).toHaveBeenCalledWith(
        expect.objectContaining({ kind: 'copy' }),
      ),
    )
    expect(viewer.beginFinderDrag).not.toHaveBeenCalled()
  })

  it('rejects a same-folder move target while allowing Option-copy', async () => {
    const viewer = bridge()
    vi.mocked(viewer.folderTree).mockResolvedValue([
      {
        entityId: 'folder-a',
        parentEntityId: null,
        relativePath: 'id',
        name: 'id',
        marker: { reviewState: null, favorite: false },
      },
    ])
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const target = await screen.findByRole('treeitem', { name: 'id' })
    const transfer = viewerDragTransfer()

    fireEvent.dragStart(await screen.findByRole('button', { name: '整理 front.jpg' }), {
      dataTransfer: transfer,
    })
    fireEvent.dragOver(target, { dataTransfer: transfer })
    expect(target).toHaveAttribute('data-drop-invalid', 'true')
    fireEvent.drop(target, { dataTransfer: transfer })
    expect(viewer.preflightFileCommand).not.toHaveBeenCalled()

    const handle = screen.getByRole('button', { name: '整理 front.jpg' })
    const copyStart = createEvent.dragStart(handle, { dataTransfer: transfer })
    Object.defineProperty(copyStart, 'altKey', { value: true })
    fireEvent(handle, copyStart)
    fireEvent.dragOver(target, { altKey: false, dataTransfer: transfer })
    expect(target).toHaveAttribute('data-drop-mode', 'copy')
  })

  it('shows a safe retry message when native Finder drag cannot start', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    vi.mocked(viewer.beginFinderDrag).mockRejectedValue({ code: 'finder_drag_selection_stale' })
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })
    fireEvent.dragStart(file, { dataTransfer: viewerDragTransfer() })

    expect(await screen.findByRole('alert')).toHaveTextContent(
      '部分文件已发生变化，请刷新后重试。',
    )
    expect(screen.queryByText(/Users\/private/)).not.toBeInTheDocument()
  })

  it('shows a generic retry message without exposing a native Finder error path', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    vi.mocked(viewer.beginFinderDrag).mockRejectedValue(
      new Error('/Users/private/project/front.jpg could not be dragged'),
    )
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    fireEvent.dragStart(await screen.findByRole('option', { name: 'front.jpg' }), {
      dataTransfer: viewerDragTransfer(),
    })

    expect(await screen.findByRole('alert')).toHaveTextContent('无法拖到 Finder，请重新拖动。')
    expect(screen.queryByText(/Users\/private/)).not.toBeInTheDocument()
  })

  it('opens compare with C and restores the mounted grid selection after closing', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const front = await screen.findByRole('option', { name: 'front.jpg' })
    const back = screen.getByRole('option', { name: 'back.jpg' })
    fireEvent.click(front)
    fireEvent.click(back, { metaKey: true })

    fireEvent.keyDown(window, { key: 'c' })
    const compare = await screen.findByRole('region', { name: '图片对比' })
    expect(compare).toHaveAttribute(
      'data-layout',
      'two_columns',
    )
    expect(compare).toHaveFocus()
    fireEvent.click(screen.getByRole('button', { name: '关闭对比' }))

    expect(front).toHaveAttribute('aria-selected', 'true')
    expect(back).toHaveAttribute('aria-selected', 'true')
  })

  it('applies an inline pane marker without replacing the underlying grid selection', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspace())
    vi.mocked(viewer.setReviewState).mockResolvedValue({ changes: [] })
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const front = await screen.findByRole('option', { name: 'front.jpg' })
    const back = screen.getByRole('option', { name: 'back.jpg' })
    fireEvent.click(front)
    fireEvent.click(back, { metaKey: true })
    fireEvent.click(screen.getByRole('button', { name: '并排对比' }))

    fireEvent.click(await screen.findByRole('button', { name: 'front.jpg 标记为保留' }))
    await waitFor(() =>
      expect(viewer.setReviewState).toHaveBeenCalledWith({
        sessionId: 'session-1',
        generation: 1,
        entityIds: ['image-1'],
        reviewState: 'keep',
      }),
    )
    fireEvent.click(screen.getByRole('button', { name: '关闭对比' }))
    expect(front).toHaveAttribute('aria-selected', 'true')
    expect(back).toHaveAttribute('aria-selected', 'true')
  })

  it('falls back to the surviving image preview when compare reaches one pane', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const front = await screen.findByRole('option', { name: 'front.jpg' })
    const back = screen.getByRole('option', { name: 'back.jpg' })
    fireEvent.click(front)
    fireEvent.click(back, { metaKey: true })
    fireEvent.click(screen.getByRole('button', { name: '并排对比' }))

    fireEvent.click(await screen.findByRole('button', { name: '移除 front.jpg' }))
    expect(await screen.findByRole('dialog', { name: '图片预览' })).toHaveTextContent(
      'back.jpg',
    )
    expect(screen.queryByRole('region', { name: '图片对比' })).not.toBeInTheDocument()
  })

  it('suppresses the C compare shortcut while an editable control owns the event', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const front = await screen.findByRole('option', { name: 'front.jpg' })
    const back = screen.getByRole('option', { name: 'back.jpg' })
    fireEvent.click(front)
    fireEvent.click(back, { metaKey: true })

    fireEvent.keyDown(screen.getByRole('searchbox', { name: '搜索项目' }), { key: 'c' })
    expect(screen.queryByRole('region', { name: '图片对比' })).not.toBeInTheDocument()
  })

  it('does not open a stale grid selection from search results and closes compare on search', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const front = await screen.findByRole('option', { name: 'front.jpg' })
    const back = screen.getByRole('option', { name: 'back.jpg' })
    fireEvent.click(front)
    fireEvent.click(back, { metaKey: true })
    fireEvent.keyDown(window, { key: 'c' })
    expect(await screen.findByRole('region', { name: '图片对比' })).toHaveFocus()

    const search = screen.getByRole('searchbox', { name: '搜索项目' })
    fireEvent.change(search, { target: { value: 'shoe' } })
    await waitFor(() =>
      expect(screen.queryByRole('region', { name: '图片对比' })).not.toBeInTheDocument(),
    )
    expect(screen.getByRole('button', { name: '并排对比' })).toBeDisabled()
    fireEvent.keyDown(window, { key: 'c' })
    expect(screen.queryByRole('region', { name: '图片对比' })).not.toBeInTheDocument()
  })

  it('opens the equivalent preselected conflict dialog when a dropped item is not ready', async () => {
    const viewer = bridge()
    vi.mocked(viewer.folderTree).mockResolvedValue([
      {
        entityId: 'folder-b',
        parentEntityId: null,
        relativePath: 'selected',
        name: 'selected',
        marker: { reviewState: null, favorite: false },
      },
    ])
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    vi.mocked(viewer.preflightFileCommand).mockResolvedValue({
      rows: [
        {
          entityId: 'image-1',
          relativePath: 'selected/front.jpg',
          state: 'conflict',
          code: 'destination_occupied',
        },
      ],
      executable: true,
    })
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const transfer = viewerDragTransfer()
    fireEvent.dragStart(await screen.findByRole('button', { name: '整理 front.jpg' }), {
      dataTransfer: transfer,
    })
    fireEvent.drop(await screen.findByRole('treeitem', { name: 'selected' }), {
      dataTransfer: transfer,
    })

    const dialog = await screen.findByRole('dialog', { name: '选择移动目标' })
    expect(within(dialog).getByRole('radio')).toBeChecked()
    expect(within(dialog).getByText('selected/front.jpg')).toBeVisible()
    expect(within(dialog).getByRole('combobox')).toHaveValue('')
    expect(viewer.executeFileCommand).not.toHaveBeenCalled()
  })
})

function viewerDragTransfer() {
  const data = new Map<string, string>()
  return {
    effectAllowed: 'uninitialized',
    dropEffect: 'none',
    get types() {
      return [...data.keys()]
    },
    setData(type: string, value: string) {
      data.set(type, value)
    },
    getData(type: string) {
      return data.get(type) ?? ''
    },
  }
}

function contentWorkspace() {
  return {
    workspace: 'content' as const,
    images: [
      {
        entityId: 'image-1',
        relativePath: 'id/front.jpg',
        name: 'front.jpg',
        kind: 'jpeg' as const,
        size: 100,
        modifiedNs: '1',
        marker: { reviewState: null, favorite: false },
        imageMetadata: null,
        imageUrl: null,
      },
    ],
    textFiles: [],
  }
}

function compareContentWorkspace() {
  const first = contentWorkspace().images[0]!
  return {
    workspace: 'content' as const,
    images: [
      first,
      {
        ...first,
        entityId: 'image-2',
        relativePath: 'id/back.jpg',
        name: 'back.jpg',
        modifiedNs: '2',
      },
    ],
    textFiles: [],
  }
}

function readOnlyContentWorkspace() {
  return {
    ...compareContentWorkspace(),
    textFiles: [
      {
        entityId: 'text-1',
        relativePath: 'id/notes.txt',
        name: 'notes.txt',
        kind: 'text' as const,
        size: 20,
        modifiedNs: '3',
        marker: { reviewState: null, favorite: false },
        imageMetadata: null,
        imageUrl: null,
      },
    ],
  }
}
