import {
  act,
  createEvent,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react'
import { afterEach, describe, expect, expectTypeOf, it, vi } from 'vitest'
import App from './App'
import type { ViewerBridge } from './api/viewer'
import { type AppShellState, useAppShellState } from './app/useAppShellState'
import {
  type OperationDialogsState,
  type UseOperationDialogsOptions,
  useOperationDialogs,
} from './app/useOperationDialogs'
import { type PreviewSessionState, usePreviewSession } from './app/usePreviewSession'
import {
  type RadialMenuSessionState,
  type UseRadialMenuSessionOptions,
  useRadialMenuSession,
} from './app/useRadialMenuSession'
import { type TextPanelPreferenceState, useTextPanelPreference } from './app/useTextPanelPreference'
import { defined } from './defined'

afterEach(() => {
  vi.unstubAllGlobals()
})

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((next, fail) => {
    resolve = next
    reject = fail
  })
  return { promise, resolve, reject }
}

function openRadialMenu(file: HTMLElement, pointerId = 90) {
  fireEvent.pointerDown(file, {
    pointerId,
    button: 2,
    clientX: 420,
    clientY: 260,
  })
}

function closeProjectFromMenu() {
  fireEvent.click(screen.getByRole('button', { name: '项目菜单' }))
  fireEvent.click(screen.getByRole('button', { name: '关闭项目' }))
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
      schemaVersion: 1,
      thumbnailDensity: 'standard',
    }),
    updateThumbnailDensity: vi.fn().mockImplementation(async (thumbnailDensity) => ({
      schemaVersion: 1,
      thumbnailDensity,
    })),
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
      types: { folders: 0, images: 0, otherFiles: 0 },
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

describe('App-local session coordinator contracts', () => {
  it('exposes the shell and preview state shapes keyed by backend session', () => {
    expect(useAppShellState).toBeTypeOf('function')
    expect(usePreviewSession).toBeTypeOf('function')
    expect(useTextPanelPreference).toBeTypeOf('function')
    expectTypeOf(useAppShellState).parameter(0).toEqualTypeOf<string>()
    expectTypeOf(usePreviewSession).parameter(0).toEqualTypeOf<string>()
    expectTypeOf(useTextPanelPreference).parameter(0).toEqualTypeOf<string>()
    expectTypeOf<ReturnType<typeof useAppShellState>>().toMatchTypeOf<AppShellState>()
    expectTypeOf<ReturnType<typeof usePreviewSession>>().toMatchTypeOf<PreviewSessionState>()
    expectTypeOf<
      ReturnType<typeof useTextPanelPreference>
    >().toMatchTypeOf<TextPanelPreferenceState>()
    expectTypeOf<keyof ReturnType<typeof useAppShellState>>().toEqualTypeOf<keyof AppShellState>()
    expectTypeOf<keyof ReturnType<typeof usePreviewSession>>().toEqualTypeOf<
      keyof PreviewSessionState
    >()
    expectTypeOf<keyof ReturnType<typeof useTextPanelPreference>>().toEqualTypeOf<
      keyof TextPanelPreferenceState
    >()
  })

  it('exposes dialog validity and radial context reset inputs', () => {
    expect(useOperationDialogs).toBeTypeOf('function')
    expect(useRadialMenuSession).toBeTypeOf('function')
    expectTypeOf(useOperationDialogs).parameter(0).toEqualTypeOf<UseOperationDialogsOptions>()
    expectTypeOf(useRadialMenuSession).parameter(0).toEqualTypeOf<UseRadialMenuSessionOptions>()
    expectTypeOf<ReturnType<typeof useOperationDialogs>>().toMatchTypeOf<OperationDialogsState>()
    expectTypeOf<ReturnType<typeof useRadialMenuSession>>().toMatchTypeOf<RadialMenuSessionState>()
  })
})

describe('Viewer empty state', () => {
  it('asks the user to import one project folder', () => {
    render(<App />)
    expect(screen.getByRole('heading', { name: 'Viewer' })).toBeVisible()
    expect(screen.getByText('拖入或选择一个项目文件夹')).toBeVisible()
    expect(screen.queryByRole('button', { name: '软件设置' })).not.toBeInTheDocument()
  })

  it('places one settings trigger immediately before the project menu after opening', async () => {
    const viewer = bridge()
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    await screen.findByRole('heading', { name: 'Catalog' })

    const settingsTrigger = screen.getByRole('button', { name: '软件设置' })
    const projectMenu = screen.getByRole('button', { name: '项目菜单' }).closest('.project-menu')
    expect(screen.getAllByRole('button', { name: '软件设置' })).toHaveLength(1)
    expect(settingsTrigger.nextElementSibling).toBe(projectMenu)
  })

  it('updates density optimistically and restores trigger focus when the dialog closes', async () => {
    const viewer = bridge()
    const save = deferred<Awaited<ReturnType<ViewerBridge['updateThumbnailDensity']>>>()
    vi.mocked(viewer.updateThumbnailDensity).mockReturnValue(save.promise)
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const trigger = await screen.findByRole('button', { name: '软件设置' })
    trigger.focus()
    fireEvent.click(trigger)

    const large = screen.getByRole('radio', { name: '大图' })
    fireEvent.click(large)
    expect(large).toBeChecked()
    fireEvent.click(screen.getByRole('button', { name: '关闭' }))

    expect(screen.queryByRole('dialog', { name: '软件设置' })).not.toBeInTheDocument()
    expect(trigger).toHaveFocus()
  })

  it('shows the latest settings save failure and rolls back the selected radio', async () => {
    const viewer = bridge()
    const save = deferred<Awaited<ReturnType<ViewerBridge['updateThumbnailDensity']>>>()
    vi.mocked(viewer.updateThumbnailDensity).mockReturnValue(save.promise)
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    fireEvent.click(await screen.findByRole('button', { name: '软件设置' }))
    fireEvent.click(screen.getByRole('radio', { name: '大图' }))
    await waitFor(() => expect(viewer.updateThumbnailDensity).toHaveBeenCalledWith('large'))

    save.reject({ userMessage: '设置未能保存' })

    expect(await screen.findByRole('alert')).toHaveTextContent('设置未能保存')
    expect(screen.getByRole('radio', { name: '标准' })).toBeChecked()
  })

  it('moves close-project into the compact project menu', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })
    expect(screen.queryByRole('button', { name: '关闭项目' })).not.toBeInTheDocument()
    openRadialMenu(file, 105)
    expect(screen.getByRole('menu', { name: '文件操作' })).toBeVisible()
    closeProjectFromMenu()
    await waitFor(() => expect(viewer.closeProject).toHaveBeenCalled())
    expect(screen.queryByRole('menu', { name: '文件操作' })).not.toBeInTheDocument()
  })

  it('keeps the mixed text shelf preference while navigating inside one project session', async () => {
    const viewer = bridge()
    vi.mocked(viewer.folderTree).mockResolvedValue([
      {
        entityId: 'folder-b',
        parentEntityId: null,
        relativePath: 'folder-b',
        name: 'folder-b',
        marker: { reviewState: null, favorite: false },
      },
    ])
    vi.mocked(viewer.queryFolder).mockImplementation(async (entityId) =>
      entityId === 'folder-b'
        ? mixedContentWorkspace({
            entityId: 'text-folder-b',
            relativePath: 'folder-b/folder-notes.txt',
            name: 'folder-notes.txt',
          })
        : mixedContentWorkspace(),
    )
    render(<App bridge={viewer} />)

    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const disclosure = await screen.findByRole('button', { name: /其它文件 · 1/ })
    expect(disclosure).toHaveAttribute('aria-expanded', 'false')
    fireEvent.click(disclosure)
    expect(disclosure).toHaveAttribute('aria-expanded', 'true')

    fireEvent.click(await screen.findByRole('treeitem', { name: 'folder-b' }))
    await waitFor(() => expect(viewer.queryFolder).toHaveBeenCalledWith('folder-b', false))
    expect(await screen.findByText('folder-b/folder-notes.txt')).toBeVisible()
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /其它文件 · 1/ })).toHaveAttribute(
        'aria-expanded',
        'true',
      ),
    )
  })

  it('exposes read-only status in the keyboard-operable project menu and keeps the banner', async () => {
    const viewer = bridge('read_only')
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    expect(await screen.findByRole('status', { name: '只读模式' })).toBeVisible()
    const projectMenu = screen.getByRole('button', { name: '项目菜单' })
    expect(projectMenu).toHaveAttribute('aria-expanded', 'false')

    fireEvent.keyDown(projectMenu, { key: 'Enter' })
    expect(projectMenu).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByText('访问权限：只读')).toBeVisible()
    fireEvent.keyDown(projectMenu, { key: ' ' })
    expect(projectMenu).toHaveAttribute('aria-expanded', 'false')
    expect(screen.getByText('访问权限：只读')).not.toBeVisible()
    expect(screen.getByRole('status', { name: '只读模式' })).toBeVisible()
  })

  it('resizes the sidebar through pointer and keyboard separator input', async () => {
    const viewer = bridge()
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    await screen.findByRole('heading', { name: 'Catalog' })
    const separator = screen.getByRole('separator', { name: '调整文件夹栏宽度' })
    expect(separator).toHaveAttribute('aria-valuemin', '200')
    expect(separator).toHaveAttribute('aria-valuemax', '420')
    expect(separator).toHaveAttribute('aria-valuenow', '260')
    fireEvent.pointerDown(separator, { pointerId: 101, button: 0, clientX: 260 })
    fireEvent.pointerMove(window, { pointerId: 101, clientX: 300 })
    fireEvent.pointerUp(window, { pointerId: 101, clientX: 300 })
    expect(screen.getByLabelText('文件夹栏')).toHaveStyle({ width: '300px' })
    expect(separator).toHaveAttribute('aria-valuenow', '300')
    fireEvent.keyDown(separator, { key: 'ArrowLeft' })
    expect(screen.getByLabelText('文件夹栏')).toHaveStyle({ width: '284px' })
    expect(separator).toHaveAttribute('aria-valuenow', '284')
    fireEvent.keyDown(separator, { key: 'ArrowRight' })
    expect(screen.getByLabelText('文件夹栏')).toHaveStyle({ width: '300px' })
    expect(separator).toHaveAttribute('aria-valuenow', '300')

    fireEvent.pointerDown(separator, { pointerId: 102, button: 0, clientX: 300 })
    fireEvent.pointerMove(window, { pointerId: 102, clientX: -1000 })
    fireEvent.pointerUp(window, { pointerId: 102, clientX: -1000 })
    fireEvent.keyDown(separator, { key: 'ArrowLeft' })
    expect(screen.getByLabelText('文件夹栏')).toHaveStyle({ width: '200px' })
    expect(separator).toHaveAttribute('aria-valuenow', '200')

    fireEvent.pointerDown(separator, { pointerId: 103, button: 0, clientX: 200 })
    fireEvent.pointerMove(window, { pointerId: 103, clientX: 1000 })
    fireEvent.pointerUp(window, { pointerId: 103, clientX: 1000 })
    fireEvent.keyDown(separator, { key: 'ArrowRight' })
    expect(screen.getByLabelText('文件夹栏')).toHaveStyle({ width: '420px' })
    expect(separator).toHaveAttribute('aria-valuenow', '420')
  })

  it('stops sidebar resize tracking when the pointer is cancelled', async () => {
    const viewer = bridge()
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    await screen.findByRole('heading', { name: 'Catalog' })
    const sidebar = screen.getByLabelText('文件夹栏')
    const separator = screen.getByRole('separator', { name: '调整文件夹栏宽度' })

    fireEvent.pointerDown(separator, { pointerId: 104, button: 0, clientX: 260 })
    fireEvent.pointerMove(window, { pointerId: 104, clientX: 280 })
    expect(sidebar).toHaveStyle({ width: '280px' })
    fireEvent.pointerCancel(window, { pointerId: 104, clientX: 280 })
    fireEvent.pointerMove(window, { pointerId: 104, clientX: 340 })
    const widthAfterCancelledMove = sidebar.style.width
    fireEvent.pointerUp(window, { pointerId: 104, clientX: 340 })

    expect(widthAfterCancelledMove).toBe('280px')
  })

  it('previews a filmstrip image in row order while keeping the category overview', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder)
      .mockResolvedValueOnce(categoryWorkspace())
      .mockResolvedValueOnce(compareContentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    const front = await screen.findByRole('button', { name: '预览 front.jpg' })
    front.focus()
    fireEvent.click(front)

    const preview = screen.getByRole('dialog', { name: '图片预览' })
    expect(preview).toHaveTextContent('front.jpg')
    expect(preview).toHaveTextContent('1 / 2')
    expect(screen.getByRole('region', { name: 'B01 图片' })).toBeInTheDocument()

    fireEvent.keyDown(preview, { key: 'ArrowRight' })
    expect(preview).toHaveTextContent('back.jpg')
    expect(preview).toHaveTextContent('2 / 2')

    fireEvent.click(screen.getByRole('button', { name: '关闭预览' }))
    expect(front).toHaveFocus()
    expect(viewer.queryFolder).toHaveBeenNthCalledWith(2, 'folder-b01', false)
  })

  it('applies global density and proportional request sizing to folder filmstrips', async () => {
    const viewer = bridge()
    const returned = compareContentWorkspace()
    vi.mocked(viewer.queryFolder)
      .mockResolvedValueOnce(categoryWorkspace())
      .mockResolvedValueOnce({
        ...returned,
        images: returned.images.map((file, index) => ({
          ...file,
          imageMetadata: index === 0 ? { width: 3, height: 2 } : { width: 2, height: 3 },
        })),
      })
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    const front = await screen.findByRole('button', { name: '预览 front.jpg' })
    await waitFor(() =>
      expect(viewer.requestImage).toHaveBeenCalledWith({
        entityId: 'image-1',
        representation: { kind: 'thumbnail', maxPixels: 198, scaleMilli: 1_000 },
      }),
    )
    expect(front.closest('[role="listitem"]')).toHaveStyle({
      width: '198px',
      height: '132px',
    })

    fireEvent.click(screen.getByRole('button', { name: '软件设置' }))
    fireEvent.click(screen.getByRole('radio', { name: '紧凑' }))

    await waitFor(() =>
      expect(front.closest('[role="listitem"]')).toHaveStyle({
        width: '144px',
        height: '96px',
      }),
    )
    expect(viewer.requestImage).toHaveBeenCalledWith({
      entityId: 'image-1',
      representation: { kind: 'thumbnail', maxPixels: 144, scaleMilli: 1_000 },
    })
  })

  it('applies global density to the normal content grid without replacing selection', async () => {
    const viewer = bridge()
    const baseContent = contentWorkspace()
    const content = {
      ...baseContent,
      images: [
        {
          ...defined(baseContent.images[0], 'Expected content image'),
          imageMetadata: { width: 3, height: 2 },
        },
      ],
    }
    vi.mocked(viewer.queryFolder).mockResolvedValue(content)
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))

    const file = await screen.findByRole('option', { name: 'front.jpg' })
    fireEvent.click(file)
    expect(
      defined(file.querySelector('.aspect-thumbnail'), 'Expected content aspect thumbnail'),
    ).toHaveStyle({
      width: '198px',
      height: '132px',
    })
    await waitFor(() =>
      expect(viewer.requestImage).toHaveBeenCalledWith({
        entityId: 'image-1',
        representation: { kind: 'thumbnail', maxPixels: 198, scaleMilli: 1_000 },
      }),
    )

    fireEvent.click(screen.getByRole('button', { name: '软件设置' }))
    fireEvent.click(screen.getByRole('radio', { name: '紧凑' }))

    await waitFor(() =>
      expect(
        defined(file.querySelector('.aspect-thumbnail'), 'Expected resized content thumbnail'),
      ).toHaveStyle({
        width: '144px',
        height: '96px',
      }),
    )
    expect(file).toHaveAttribute('aria-selected', 'true')
    expect(screen.getByRole('listbox', { name: '图片文件' })).toHaveAttribute(
      'aria-activedescendant',
      'file-image-1',
    )
    expect(viewer.requestImage).toHaveBeenCalledWith({
      entityId: 'image-1',
      representation: { kind: 'thumbnail', maxPixels: 144, scaleMilli: 1_000 },
    })
  })

  it('reloads visible filmstrip rows after an external category refresh', async () => {
    const viewer = bridge()
    let receiveProjectChanged: Parameters<ViewerBridge['listenProjectChanged']>[0] | undefined
    vi.mocked(viewer.listenProjectChanged).mockImplementation(async (handler) => {
      receiveProjectChanged = handler
      return () => undefined
    })
    const refreshedImages = {
      ...compareContentWorkspace(),
      images: [
        {
          ...defined(compareContentWorkspace().images[0], 'Expected comparison workspace image'),
          entityId: 'image-updated',
          name: 'updated.jpg',
          relativePath: 'id/updated.jpg',
        },
      ],
    }
    vi.mocked(viewer.queryFolder)
      .mockResolvedValueOnce(categoryWorkspace())
      .mockResolvedValueOnce(compareContentWorkspace())
      .mockResolvedValueOnce(categoryWorkspace())
      .mockResolvedValueOnce(refreshedImages)
    render(<App bridge={viewer} />)
    await waitFor(() => expect(receiveProjectChanged).toBeDefined())
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    await screen.findByRole('button', { name: '预览 front.jpg' })

    act(() => {
      receiveProjectChanged?.({
        sessionId: 'session-1',
        generation: 1,
        reason: 'external_change',
        added: 0,
        removed: 0,
        modified: 1,
        moved: 0,
        markerPathsMoved: 0,
        failed: 0,
      })
    })

    expect(await screen.findByRole('button', { name: '预览 updated.jpg' })).toBeVisible()
    expect(viewer.queryFolder).toHaveBeenNthCalledWith(4, 'folder-b01', false)
    expect(viewer.queryFolder).toHaveBeenCalledTimes(4)
  })

  it('invalidates an open filmstrip preview when its category projection refreshes', async () => {
    const viewer = bridge()
    let receiveProjectChanged: Parameters<ViewerBridge['listenProjectChanged']>[0] | undefined
    vi.mocked(viewer.listenProjectChanged).mockImplementation(async (handler) => {
      receiveProjectChanged = handler
      return () => undefined
    })
    const refreshedImages = {
      ...compareContentWorkspace(),
      images: [
        {
          ...defined(compareContentWorkspace().images[0], 'Expected comparison workspace image'),
          entityId: 'image-updated',
          name: 'updated.jpg',
          relativePath: 'id/updated.jpg',
        },
      ],
    }
    vi.mocked(viewer.queryFolder)
      .mockResolvedValueOnce(categoryWorkspace())
      .mockResolvedValueOnce(compareContentWorkspace())
      .mockResolvedValueOnce(categoryWorkspace())
      .mockResolvedValueOnce(refreshedImages)
    render(<App bridge={viewer} />)
    await waitFor(() => expect(receiveProjectChanged).toBeDefined())
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    fireEvent.click(await screen.findByRole('button', { name: '预览 front.jpg' }))
    expect(screen.getByRole('dialog', { name: '图片预览' })).toHaveTextContent('1 / 2')

    act(() => {
      receiveProjectChanged?.({
        sessionId: 'session-1',
        generation: 1,
        reason: 'external_change',
        added: 0,
        removed: 1,
        modified: 0,
        moved: 0,
        markerPathsMoved: 0,
        failed: 0,
      })
    })

    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: '图片预览' })).not.toBeInTheDocument(),
    )
    const updated = await screen.findByRole('button', { name: '预览 updated.jpg' })
    fireEvent.keyDown(window, { key: 'ArrowRight' })
    expect(screen.queryByText('back.jpg')).not.toBeInTheDocument()

    fireEvent.click(updated)
    expect(screen.getByRole('dialog', { name: '图片预览' })).toHaveTextContent('1 / 1')
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
    expect(screen.getByRole('heading', { name: 'Catalog' })).toBeVisible()
    expect(viewer.openPermissionSettings).not.toHaveBeenCalled()

    fireEvent.click(screen.getByRole('button', { name: '打开权限设置' }))
    expect(viewer.openPermissionSettings).toHaveBeenCalledOnce()
    expect(screen.getByRole('searchbox', { name: '搜索项目' })).toBeEnabled()
    fireEvent.click(screen.getByText('筛选与排序'))
    expect(screen.getByRole('combobox', { name: '排序方式' })).toBeEnabled()

    const front = await screen.findByRole('option', { name: 'front.jpg' })
    const back = screen.getByRole('option', { name: 'back.jpg' })
    fireEvent.doubleClick(front)
    expect(screen.getByRole('dialog', { name: '图片预览' })).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '关闭预览' }))
    fireEvent.click(front)
    fireEvent.click(back, { metaKey: true })

    openRadialMenu(back, 30)
    expect(screen.getByRole('menuitem', { name: '并排对比' })).toHaveAttribute(
      'aria-disabled',
      'false',
    )
    expect(screen.getByRole('menuitem', { name: '信息' })).toHaveAttribute('aria-disabled', 'false')
    expect(screen.getByRole('menuitem', { name: '标记' })).toHaveAttribute('aria-disabled', 'true')
    expect(screen.getByRole('menuitem', { name: '整理' })).toHaveAttribute('aria-disabled', 'true')
    expect(screen.getByRole('menuitem', { name: '移到废纸篓' })).toHaveAttribute(
      'aria-disabled',
      'true',
    )
    expect(screen.getByText('只读')).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '关闭文件操作' }))
    expect(screen.getByRole('button', { name: '整理 front.jpg' })).toBeDisabled()
    fireEvent.pointerDown(screen.getByRole('button', { name: '整理 front.jpg' }), {
      pointerId: 31,
      button: 0,
      clientX: 10,
      clientY: 10,
    })
    fireEvent.pointerMove(screen.getByRole('button', { name: '整理 front.jpg' }), {
      pointerId: 31,
      clientX: 20,
      clientY: 20,
    })
    fireEvent.pointerUp(screen.getByRole('button', { name: '整理 front.jpg' }), {
      pointerId: 31,
      clientX: 20,
      clientY: 20,
    })
    expect(viewer.preflightFileCommand).not.toHaveBeenCalled()
    fireEvent.dragStart(
      defined(front.querySelector('.file-export-surface'), 'Expected front image export surface'),
      {
        dataTransfer: viewerDragTransfer(),
      },
    )
    await waitFor(() =>
      expect(viewer.beginFinderDrag).toHaveBeenCalledWith({
        sessionId: 'session-1',
        generation: 1,
        entityIds: ['image-1', 'image-2'],
      }),
    )

    openRadialMenu(back, 32)
    fireEvent.click(screen.getByRole('menuitem', { name: '信息' }))
    expect(screen.getByRole('complementary', { name: '文件信息' })).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '关闭信息' }))

    openRadialMenu(back, 33)
    fireEvent.click(screen.getByRole('menuitem', { name: '并排对比' }))
    expect(screen.getByRole('region', { name: '图片对比' })).toBeVisible()
    expect(screen.getByRole('status', { name: '只读模式' })).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '关闭对比' }))
    fireEvent.click(screen.getByRole('button', { name: '其它文件 · 1' }))
    fireEvent.doubleClick(screen.getByRole('option', { name: 'notes.txt' }))
    expect(await screen.findByText('readonly product notes')).toBeVisible()
  })

  it('opens a generic unsupported-file preview without requesting text', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(genericOtherContentWorkspace())
    render(<App bridge={viewer} />)

    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    fireEvent.click(await screen.findByRole('button', { name: '其它文件 · 1' }))
    fireEvent.doubleClick(screen.getByRole('option', { name: 'license.other' }))

    expect(screen.getByRole('dialog', { name: 'license.other' })).toHaveTextContent('暂不支持预览')
    expect(viewer.previewText).not.toHaveBeenCalled()
  })

  it('opens unsupported images in the request-free image preview', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(unsupportedImageContentWorkspace())
    render(<App bridge={viewer} />)

    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    fireEvent.doubleClick(await screen.findByRole('option', { name: 'poster.webp' }))

    const dialog = screen.getByRole('dialog', { name: '图片预览' })
    expect(dialog).toBeVisible()
    expect(within(dialog).getByLabelText('poster.webp .WEBP 暂不支持预览')).toBeVisible()
    expect(screen.queryByRole('dialog', { name: 'poster.webp' })).not.toBeInTheDocument()
    expect(viewer.requestImage).not.toHaveBeenCalled()
    expect(viewer.previewText).not.toHaveBeenCalled()
  })

  it('reselects from a read-only project through normal close and forgets the old path', async () => {
    const viewer = bridge('read_only')
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    await screen.findByRole('heading', { name: 'Catalog' })

    fireEvent.click(screen.getByRole('button', { name: '重新选择目录' }))

    await waitFor(() => expect(viewer.closeProject).toHaveBeenCalledWith(undefined, 'project'))
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
    await screen.findByRole('heading', { name: 'Catalog' })

    act(() =>
      receiveCloseBlocked?.({
        sessionId: 'session-1',
        generation: 1,
        batchId: 'batch-1',
        target: 'project',
      }),
    )
    fireEvent.click(screen.getByRole('button', { name: '保持打开' }))
    expect(screen.queryByRole('dialog', { name: '文件操作尚未完成' })).not.toBeInTheDocument()
    expect(screen.getByRole('heading', { name: 'Catalog' })).toBeVisible()
    expect(viewer.closeProject).not.toHaveBeenCalled()

    act(() =>
      receiveCloseBlocked?.({
        sessionId: 'session-1',
        generation: 1,
        batchId: 'batch-1',
        target: 'project',
      }),
    )
    fireEvent.click(screen.getByRole('button', { name: '等待完成后关闭' }))
    await waitFor(() => expect(viewer.closeProject).toHaveBeenCalledWith('wait', 'project'))
    expect(await screen.findByRole('heading', { name: 'Viewer' })).toBeVisible()

    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    await screen.findByRole('heading', { name: 'Catalog' })
    act(() =>
      receiveCloseBlocked?.({
        sessionId: 'session-1',
        generation: 1,
        batchId: 'batch-2',
        target: 'application',
      }),
    )
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
    await screen.findByRole('heading', { name: 'Catalog' })

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
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    await waitFor(() => expect(receiveProjectClosed).toBeDefined())
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })
    openRadialMenu(file, 106)
    expect(screen.getByRole('menu', { name: '文件操作' })).toBeVisible()

    act(() => receiveProjectClosed?.())

    expect(await screen.findByRole('heading', { name: 'Viewer' })).toBeVisible()
    expect(screen.queryByRole('menu', { name: '文件操作' })).not.toBeInTheDocument()
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

    await waitFor(() => expect(viewer.openProject).toHaveBeenCalledWith('/fixture/dropped-project'))
    expect(await screen.findByRole('heading', { name: 'Catalog' })).toBeVisible()
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
      otherFiles: [],
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
    const file = screen.getByRole('option', { name: '1.jpg' })
    fireEvent.click(file)
    grid.scrollTop = 200
    fireEvent.scroll(grid)
    openRadialMenu(file, 107)
    fireEvent.keyDown(grid, { key: ' ' })
    await screen.findByRole('img', { name: '1.jpg' })
    expect(screen.queryByRole('menu', { name: '文件操作' })).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: '关闭预览' }))

    expect(screen.getByRole('listbox', { name: '图片文件' })).toBe(grid)
    expect(grid.scrollTop).toBe(200)
    expect(screen.getByRole('option', { name: '1.jpg' })).toHaveAttribute('aria-selected', 'true')
  })

  it('replaces persistent selection toolbars with the right-click radial menu', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })
    expect(screen.queryByLabelText('批量标记')).not.toBeInTheDocument()
    expect(screen.queryByLabelText('文件操作')).not.toBeInTheDocument()
    openRadialMenu(file)
    expect(screen.getByRole('menu', { name: '文件操作' })).toBeVisible()
    fireEvent.click(screen.getByRole('menuitem', { name: '信息' }))
    expect(screen.getByRole('complementary', { name: '文件信息' })).toBeVisible()
  })

  it('starts a fresh held gesture when a second right-click replaces click fallback', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const front = await screen.findByRole('option', { name: 'front.jpg' })
    const back = screen.getByRole('option', { name: 'back.jpg' })

    openRadialMenu(front, 201)
    fireEvent.pointerUp(window, { pointerId: 201, clientX: 423, clientY: 263 })
    openRadialMenu(back, 202)
    fireEvent.pointerMove(window, { pointerId: 202, clientX: 420, clientY: 170 })
    fireEvent.pointerUp(window, { pointerId: 202, clientX: 420, clientY: 170 })

    expect(screen.getByRole('dialog', { name: '图片预览' })).toHaveTextContent('back.jpg')
  })

  it('restores the original content target after a click fallback is replaced', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const grid = await screen.findByRole('listbox', { name: '图片文件' })
    const front = screen.getByRole('option', { name: 'front.jpg' })
    const back = screen.getByRole('option', { name: 'back.jpg' })

    openRadialMenu(front, 211)
    fireEvent.pointerUp(window, { pointerId: 211, clientX: 420, clientY: 260 })
    expect(screen.getByRole('menuitem', { name: '预览' })).toHaveFocus()

    openRadialMenu(back, 212)
    expect(screen.getByRole('menuitem', { name: '预览' })).toHaveFocus()
    fireEvent.keyDown(screen.getByRole('menu', { name: '文件操作' }), { key: 'Escape' })

    expect(screen.queryByRole('menu', { name: '文件操作' })).not.toBeInTheDocument()
    expect(grid).toHaveFocus()
  })

  it('keeps mixed-favorite copy truthful while routing the frozen selection to toggle', async () => {
    const viewer = bridge()
    const workspace = compareContentWorkspace()
    workspace.images[0] = {
      ...defined(workspace.images[0], 'Expected first workspace image'),
      marker: { reviewState: null, favorite: true },
    }
    vi.mocked(viewer.queryFolder).mockResolvedValue(workspace)
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const front = await screen.findByRole('option', { name: 'front.jpg' })
    const back = screen.getByRole('option', { name: 'back.jpg' })
    fireEvent.click(front)
    fireEvent.click(back, { metaKey: true })

    openRadialMenu(back, 203)
    fireEvent.click(screen.getByRole('menuitem', { name: '标记' }))
    const favorite = screen.getByRole('menuitemcheckbox', { name: '切换收藏' })
    expect(favorite).toHaveAttribute('aria-checked', 'mixed')
    fireEvent.click(favorite)

    await waitFor(() =>
      expect(viewer.toggleFavorite).toHaveBeenCalledWith({
        sessionId: 'session-1',
        generation: 1,
        entityIds: ['image-1', 'image-2'],
      }),
    )
  })

  it('clears an earlier invalid keyboard compare status through the radial compare owner', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const front = await screen.findByRole('option', { name: 'front.jpg' })
    const back = screen.getByRole('option', { name: 'back.jpg' })
    fireEvent.click(front)
    fireEvent.keyDown(window, { key: 'c' })
    expect(screen.getByText('请选择 2–20 张图片进行对比。')).toBeVisible()

    fireEvent.click(back, { metaKey: true })
    openRadialMenu(back, 204)
    fireEvent.click(screen.getByRole('menuitem', { name: '并排对比' }))

    expect(await screen.findByRole('region', { name: '图片对比' })).toBeVisible()
    expect(screen.queryByText('请选择 2–20 张 JPG 或 PNG 图片进行对比。')).not.toBeInTheDocument()
  })

  it('closes the radial snapshot as soon as a deferred project close starts', async () => {
    const viewer = bridge()
    const closing = deferred<'closed'>()
    vi.mocked(viewer.closeProject).mockImplementation(() => closing.promise)
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })
    openRadialMenu(file, 205)
    expect(screen.getByRole('menu', { name: '文件操作' })).toBeVisible()

    closeProjectFromMenu()

    expect(screen.queryByRole('menu', { name: '文件操作' })).not.toBeInTheDocument()
    expect(screen.getByRole('heading', { name: 'Catalog' })).toBeVisible()
    await act(async () => {
      closing.resolve('closed')
      await closing.promise
    })
  })

  it('invalidates a radial snapshot when the same session advances generation', async () => {
    const viewer = bridge()
    let receiveScan: Parameters<ViewerBridge['listenScan']>[0] | undefined
    vi.mocked(viewer.listenScan).mockImplementation(async (handler) => {
      receiveScan = handler
      return () => undefined
    })
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
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
    const file = await screen.findByRole('option', { name: 'front.jpg' })
    openRadialMenu(file, 206)
    expect(screen.getByRole('menu', { name: '文件操作' })).toBeVisible()

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
    await waitFor(() =>
      expect(screen.queryByRole('menu', { name: '文件操作' })).not.toBeInTheDocument(),
    )
  })

  it('routes keep, favorite, rename, copy, move, compare, and Trash through existing owners', async () => {
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
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspace())
    vi.mocked(viewer.preflightFileCommand).mockImplementation(async (request) => ({
      rows: request.items.map((item) => ({
        entityId: item.entityId,
        relativePath: `selected/${item.entityId}.jpg`,
        state: 'ready' as const,
      })),
      executable: true,
    }))
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const front = await screen.findByRole('option', { name: 'front.jpg' })
    openRadialMenu(front, 91)
    fireEvent.click(screen.getByRole('menuitem', { name: '标记' }))
    fireEvent.click(screen.getByRole('menuitemcheckbox', { name: '保留' }))
    await waitFor(() =>
      expect(viewer.setReviewState).toHaveBeenCalledWith({
        sessionId: 'session-1',
        generation: 1,
        entityIds: ['image-1'],
        reviewState: 'keep',
      }),
    )

    openRadialMenu(front, 92)
    fireEvent.click(screen.getByRole('menuitem', { name: '标记' }))
    fireEvent.click(screen.getByRole('menuitemcheckbox', { name: '收藏' }))
    await waitFor(() =>
      expect(viewer.toggleFavorite).toHaveBeenCalledWith({
        sessionId: 'session-1',
        generation: 1,
        entityIds: ['image-1'],
      }),
    )

    openRadialMenu(front, 93)
    fireEvent.click(screen.getByRole('menuitem', { name: '整理' }))
    fireEvent.click(screen.getByRole('menuitem', { name: '重命名' }))
    expect(screen.getByRole('dialog', { name: '重命名文件' })).toBeVisible()
    expect(screen.queryByRole('menu', { name: '文件操作' })).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: '取消' }))

    for (const [mode, actionLabel, dialogName] of [
      ['copy', '复制到', '选择复制目标'],
      ['move', '移动到', '选择移动目标'],
    ] as const) {
      openRadialMenu(front, mode === 'copy' ? 94 : 95)
      fireEvent.click(screen.getByRole('menuitem', { name: '整理' }))
      fireEvent.click(screen.getByRole('menuitem', { name: actionLabel }))
      const dialog = screen.getByRole('dialog', { name: dialogName })
      fireEvent.click(within(dialog).getByRole('radio', { name: /selected/ }))
      fireEvent.click(within(dialog).getByRole('button', { name: '检查冲突' }))
      await waitFor(() =>
        expect(viewer.preflightFileCommand).toHaveBeenCalledWith({
          sessionId: 'session-1',
          generation: 1,
          kind: mode,
          items: [
            {
              entityId: 'image-1',
              action: { kind: mode, destinationFolderId: 'folder-b' },
            },
          ],
        }),
      )
      fireEvent.click(within(dialog).getByRole('button', { name: '取消' }))
    }

    const back = screen.getByRole('option', { name: 'back.jpg' })
    fireEvent.click(back, { metaKey: true })
    openRadialMenu(back, 96)
    expect(screen.getByRole('menuitem', { name: '并排对比' })).toHaveAttribute(
      'aria-disabled',
      'false',
    )
    fireEvent.click(screen.getByRole('menuitem', { name: '并排对比' }))
    expect(await screen.findByRole('region', { name: '图片对比' })).toBeVisible()
    expect(screen.queryByRole('menu', { name: '文件操作' })).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: '关闭对比' }))

    openRadialMenu(front, 97)
    fireEvent.click(screen.getByRole('menuitem', { name: '移到废纸篓' }))
    expect(screen.getByRole('dialog', { name: '将文件移到废纸篓？' })).toBeVisible()
  })

  it('replaces only the right workspace with search and returns to folder context', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
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
    const file = screen.getByRole('option', { name: 'front.jpg' })

    fireEvent.keyDown(window, { key: 'f', metaKey: true })
    expect(search).toHaveFocus()
    openRadialMenu(file, 108)
    fireEvent.change(search, { target: { value: 'shoe' } })
    await waitFor(() => expect(viewer.searchProject).toHaveBeenCalledOnce())
    expect(screen.queryByRole('menu', { name: '文件操作' })).not.toBeInTheDocument()
    expect(await screen.findByRole('option', { name: /shoe.jpg/ })).toBeVisible()
    expect(screen.getByRole('tree', { name: '项目文件夹' })).toBeVisible()

    fireEvent.click(screen.getByRole('button', { name: '返回文件夹内容' }))
    expect(await screen.findByRole('option', { name: 'front.jpg' })).toBeVisible()
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

    const interactiveButton = screen.getByRole('button', { name: '整理 front.jpg' })
    interactiveButton.focus()
    fireEvent.keyDown(interactiveButton, { key: 'Enter' })
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

    closeProjectFromMenu()
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

  it('keeps writes blocked while busy without weakening single-selection preview', async () => {
    const viewer = bridge()
    const results = deferred<{ total: number; offset: number; items: [] }>()
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspace())
    vi.mocked(viewer.operationResults).mockImplementation(() => results.promise)
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
    const dialog = screen.getByRole('dialog', { name: '重命名文件' })
    fireEvent.change(within(dialog).getByRole('textbox', { name: '新文件名' }), {
      target: { value: 'hero.jpg' },
    })
    fireEvent.click(within(dialog).getByRole('button', { name: '重命名' }))
    await waitFor(() => expect(viewer.operationResults).toHaveBeenCalledOnce())

    openRadialMenu(file, 94)
    expect(screen.getByRole('menuitem', { name: '标记' })).toHaveAttribute('aria-disabled', 'true')
    expect(screen.getByRole('menuitem', { name: '整理' })).toHaveAttribute('aria-disabled', 'true')
    expect(screen.getByRole('menuitem', { name: '移到废纸篓' })).toHaveAttribute(
      'aria-disabled',
      'true',
    )
    expect(screen.getByRole('menuitem', { name: '预览' })).toHaveAttribute('aria-disabled', 'false')
    expect(screen.getByRole('menuitem', { name: '信息' })).toHaveAttribute('aria-disabled', 'false')
    fireEvent.click(screen.getByRole('menuitem', { name: '预览' }))
    expect(screen.getByRole('dialog', { name: '图片预览' })).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '关闭预览' }))

    const back = screen.getByRole('option', { name: 'back.jpg' })
    fireEvent.click(back, { metaKey: true })
    openRadialMenu(back, 95)
    const multiPreview = screen.getByRole('menuitem', { name: '预览' })
    expect(multiPreview).toHaveAttribute('aria-disabled', 'true')
    fireEvent.click(multiPreview)
    expect(screen.queryByRole('dialog', { name: '图片预览' })).not.toBeInTheDocument()

    await act(async () => {
      results.resolve({ total: 0, offset: 0, items: [] })
      await results.promise
    })
    await waitFor(() =>
      expect(screen.queryByRole('menu', { name: '文件操作' })).not.toBeInTheDocument(),
    )
  })

  it('routes an ordered frozen Option-copy pointer drop through the same commands', async () => {
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
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspace())
    vi.mocked(viewer.preflightFileCommand).mockResolvedValue({
      rows: [
        { entityId: 'image-1', relativePath: 'selected/front.jpg', state: 'ready' },
        { entityId: 'image-2', relativePath: 'selected/back.jpg', state: 'ready' },
      ],
      executable: true,
    })
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const front = await screen.findByRole('option', { name: 'front.jpg' })
    fireEvent.click(front)
    fireEvent.click(screen.getByRole('option', { name: 'back.jpg' }), { metaKey: true })
    const handle = screen.getByRole('button', { name: '整理 front.jpg' })
    const destination = await screen.findByRole('treeitem', { name: 'selected' })
    organizationPointerMove(handle, destination, { pointerId: 32, altKey: true })
    expect(screen.getByText('复制 2 项')).toBeVisible()
    expect(destination).toHaveAttribute('data-drop-mode', 'copy')
    fireEvent.pointerUp(handle, {
      pointerId: 32,
      altKey: false,
      clientX: 20,
      clientY: 20,
    })
    expect(screen.queryByText('复制 2 项')).not.toBeInTheDocument()

    await waitFor(() =>
      expect(viewer.preflightFileCommand).toHaveBeenCalledWith({
        sessionId: 'session-1',
        generation: 1,
        kind: 'copy',
        items: [
          {
            entityId: 'image-1',
            action: { kind: 'copy', destinationFolderId: 'folder-b' },
          },
          {
            entityId: 'image-2',
            action: { kind: 'copy', destinationFolderId: 'folder-b' },
          },
        ],
      }),
    )
    await waitFor(() =>
      expect(viewer.executeFileCommand).toHaveBeenCalledWith(
        expect.objectContaining({
          kind: 'copy',
          items: [
            {
              entityId: 'image-1',
              action: { kind: 'copy', destinationFolderId: 'folder-b' },
            },
            {
              entityId: 'image-2',
              action: { kind: 'copy', destinationFolderId: 'folder-b' },
            },
          ],
        }),
      ),
    )
    expect(viewer.beginFinderDrag).not.toHaveBeenCalled()
  })

  it('starts copy-only native export during the file-body dragstart', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })
    const exportSurface = defined(
      file.querySelector<HTMLElement>('.file-export-surface'),
      'Expected file export surface',
    )

    const event = createEvent.dragStart(exportSurface, { dataTransfer: viewerDragTransfer() })
    fireEvent(exportSurface, event)

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
    const handle = await screen.findByRole('button', { name: '整理 front.jpg' })
    const destination = await screen.findByRole('treeitem', { name: 'selected' })
    organizationPointerDrag(handle, destination, {
      pointerId: 33,
      altKey: false,
      releaseAltKey: true,
    })

    expect(viewer.beginFinderDrag).not.toHaveBeenCalled()
    await waitFor(() =>
      expect(viewer.preflightFileCommand).toHaveBeenCalledWith(
        expect.objectContaining({ kind: 'move' }),
      ),
    )
  })

  it('cancels an active pointer drag when the workspace identity changes', async () => {
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
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const handle = await screen.findByRole('button', { name: '整理 front.jpg' })
    const file = screen.getByRole('option', { name: 'front.jpg' })
    const destination = await screen.findByRole('treeitem', { name: 'selected' })
    organizationPointerMove(handle, destination, { pointerId: 34, altKey: false })
    expect(destination).toHaveAttribute('data-drop-mode', 'move')

    openRadialMenu(file, 109)
    fireEvent.click(screen.getByRole('treeitem', { name: 'id' }))
    await waitFor(() =>
      expect(screen.getByRole('treeitem', { name: 'id' })).toHaveAttribute('aria-selected', 'true'),
    )
    await waitFor(() =>
      expect(screen.queryByRole('menu', { name: '文件操作' })).not.toBeInTheDocument(),
    )
    fireEvent.pointerUp(handle, { pointerId: 34, clientX: 20, clientY: 20 })

    expect(viewer.preflightFileCommand).not.toHaveBeenCalled()
    expect(viewer.executeFileCommand).not.toHaveBeenCalled()
  })

  it('cancels an active pointer drag when the current workspace projection is replaced', async () => {
    const viewer = bridge()
    let receiveProjectChanged: Parameters<ViewerBridge['listenProjectChanged']>[0] | undefined
    vi.mocked(viewer.listenProjectChanged).mockImplementation(async (handler) => {
      receiveProjectChanged = handler
      return () => undefined
    })
    vi.mocked(viewer.folderTree).mockResolvedValue([
      {
        entityId: 'folder-b',
        parentEntityId: null,
        relativePath: 'selected',
        name: 'selected',
        marker: { reviewState: null, favorite: false },
      },
    ])
    const refreshedWorkspace = {
      ...contentWorkspace(),
      images: [
        {
          ...defined(contentWorkspace().images[0], 'Expected content workspace image'),
          modifiedNs: '2',
          marker: { reviewState: 'keep' as const, favorite: true },
        },
      ],
    }
    vi.mocked(viewer.queryFolder)
      .mockResolvedValueOnce(contentWorkspace())
      .mockResolvedValueOnce(refreshedWorkspace)
    render(<App bridge={viewer} />)
    await waitFor(() => expect(receiveProjectChanged).toBeDefined())
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const handle = await screen.findByRole('button', { name: '整理 front.jpg' })
    const destination = await screen.findByRole('treeitem', { name: 'selected' })
    organizationPointerMove(handle, destination, { pointerId: 38, altKey: false })
    expect(destination).toHaveAttribute('data-drop-mode', 'move')

    act(() => {
      receiveProjectChanged?.({
        sessionId: 'session-1',
        generation: 1,
        reason: 'external_change',
        added: 0,
        removed: 0,
        modified: 1,
        moved: 0,
        markerPathsMoved: 0,
        failed: 0,
      })
    })
    expect(await screen.findByText('保留 · 收藏')).toBeVisible()
    await waitFor(() => expect(destination).not.toHaveAttribute('data-drop-mode'))

    fireEvent.pointerUp(handle, { pointerId: 38, clientX: 20, clientY: 50 })

    expect(viewer.preflightFileCommand).not.toHaveBeenCalled()
    expect(viewer.executeFileCommand).not.toHaveBeenCalled()
  })

  it('closes the radial menu when external projection repair removes its file', async () => {
    const viewer = bridge()
    let receiveProjectChanged: Parameters<ViewerBridge['listenProjectChanged']>[0] | undefined
    vi.mocked(viewer.listenProjectChanged).mockImplementation(async (handler) => {
      receiveProjectChanged = handler
      return () => undefined
    })
    const initial = compareContentWorkspace()
    const surviving = {
      ...initial,
      images: [defined(initial.images[1], 'Expected surviving comparison image')],
    }
    vi.mocked(viewer.queryFolder).mockResolvedValueOnce(initial).mockResolvedValueOnce(surviving)
    render(<App bridge={viewer} />)
    await waitFor(() => expect(receiveProjectChanged).toBeDefined())
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const front = await screen.findByRole('option', { name: 'front.jpg' })
    openRadialMenu(front, 110)
    expect(screen.getByRole('menu', { name: '文件操作' })).toBeVisible()

    act(() => {
      receiveProjectChanged?.({
        sessionId: 'session-1',
        generation: 1,
        reason: 'external_change',
        added: 0,
        removed: 1,
        modified: 0,
        moved: 0,
        markerPathsMoved: 0,
        failed: 0,
      })
    })

    expect(await screen.findByText('部分正在查看的文件已在项目外发生变化。')).toBeVisible()
    await waitFor(() =>
      expect(screen.queryByRole('menu', { name: '文件操作' })).not.toBeInTheDocument(),
    )
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
    const handle = await screen.findByRole('button', { name: '整理 front.jpg' })
    organizationPointerMove(handle, target, { pointerId: 35, altKey: false })
    expect(target).toHaveAttribute('data-drop-invalid', 'true')
    fireEvent.pointerUp(handle, { pointerId: 35, clientX: 20, clientY: 20 })
    expect(viewer.preflightFileCommand).not.toHaveBeenCalled()

    organizationPointerMove(handle, target, { pointerId: 36, altKey: true })
    expect(target).toHaveAttribute('data-drop-mode', 'copy')
    fireEvent.pointerCancel(handle, { pointerId: 36 })
  })

  it('shows a safe retry message when native Finder drag cannot start', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(contentWorkspace())
    vi.mocked(viewer.beginFinderDrag).mockRejectedValue({ code: 'finder_drag_selection_stale' })
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const file = await screen.findByRole('option', { name: 'front.jpg' })
    fireEvent.dragStart(
      defined(file.querySelector('.file-export-surface'), 'Expected file export surface'),
      {
        dataTransfer: viewerDragTransfer(),
      },
    )

    expect(await screen.findByRole('alert')).toHaveTextContent('部分文件已发生变化，请刷新后重试。')
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
    const file = await screen.findByRole('option', { name: 'front.jpg' })
    fireEvent.dragStart(
      defined(file.querySelector('.file-export-surface'), 'Expected file export surface'),
      {
        dataTransfer: viewerDragTransfer(),
      },
    )

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
    const organizeHandle = screen.getByRole('button', { name: '整理 front.jpg' })
    fireEvent.click(front)
    fireEvent.click(back, { metaKey: true })

    fireEvent.keyDown(window, { key: 'c' })
    const compare = await screen.findByRole('region', { name: '图片对比' })
    expect(organizeHandle).toBeDisabled()
    expect(compare).toHaveFocus()
    fireEvent.click(screen.getByRole('button', { name: '关闭对比' }))

    expect(front).toHaveAttribute('aria-selected', 'true')
    expect(back).toHaveAttribute('aria-selected', 'true')
  })

  it('forwards compare pane cancellation signals through the production bridge boundary', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspace())
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const front = await screen.findByRole('option', { name: 'front.jpg' })
    const back = screen.getByRole('option', { name: 'back.jpg' })
    fireEvent.click(front)
    fireEvent.click(back, { metaKey: true })

    fireEvent.keyDown(window, { key: 'c' })
    await screen.findByRole('region', { name: '图片对比' })

    await waitFor(() => {
      expect(viewer.requestImage).toHaveBeenCalledWith(
        expect.objectContaining({
          representation: expect.objectContaining({ kind: 'fit_preview' }),
        }),
        expect.any(AbortSignal),
      )
    })
  })

  it('opens comparison with C after selecting 20 images', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspaceWithCount(20))
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const grid = await screen.findByRole('listbox', { name: '图片文件' })

    fireEvent.keyDown(grid, { key: 'a', metaKey: true })
    fireEvent.keyDown(window, { key: 'c' })

    expect(await screen.findByRole('region', { name: '图片对比' })).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '关闭对比' }))
    const mountedOptions = screen.getAllByRole('option')
    expect(mountedOptions.every((item) => item.ariaSelected === 'true')).toBe(true)
    openRadialMenu(defined(mountedOptions[0], 'Expected a mounted selected image'), 222)
    expect(screen.getByText('20 个文件')).toBeVisible()
  })

  it('uses one fit row for four portrait files in a wide comparison workspace', async () => {
    const resize = installCompareResizeObserver()
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(comparePortraitWorkspace(4))
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const grid = await screen.findByRole('listbox', { name: '图片文件' })

    fireEvent.keyDown(grid, { key: 'a', metaKey: true })
    fireEvent.keyDown(window, { key: 'c' })

    const compare = await screen.findByRole('region', { name: '图片对比' })
    act(() => resize.flush())
    expect(compare).toHaveAttribute('data-layout', 'fit-row')
  })

  it('keeps compare disabled for a 21-image selection', async () => {
    const viewer = bridge()
    vi.mocked(viewer.queryFolder).mockResolvedValue(compareContentWorkspaceWithCount(21))
    render(<App bridge={viewer} />)
    fireEvent.click(screen.getByRole('button', { name: '选择项目文件夹' }))
    const grid = await screen.findByRole('listbox', { name: '图片文件' })

    fireEvent.keyDown(grid, { key: 'a', metaKey: true })
    const image = screen.getByRole('option', { name: 'image-1.jpg' })
    openRadialMenu(image, 221)
    const compare = screen.getByRole('menuitem', { name: '并排对比' })
    expect(compare).toHaveAttribute('aria-disabled', 'true')
    expect(compare).toHaveAttribute('title', '最多同时对比 20 张图片')
    fireEvent.click(compare)

    expect(screen.queryByRole('region', { name: '图片对比' })).not.toBeInTheDocument()
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
    fireEvent.keyDown(window, { key: 'c' })

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
    fireEvent.keyDown(window, { key: 'c' })

    fireEvent.click(await screen.findByRole('button', { name: '移除 front.jpg' }))
    expect(await screen.findByRole('dialog', { name: '图片预览' })).toHaveTextContent('back.jpg')
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
    expect(screen.queryByRole('menu', { name: '文件操作' })).not.toBeInTheDocument()
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
    const handle = await screen.findByRole('button', { name: '整理 front.jpg' })
    const target = await screen.findByRole('treeitem', { name: 'selected' })
    organizationPointerDrag(handle, target, {
      pointerId: 37,
      altKey: false,
      releaseAltKey: false,
    })

    const dialog = await screen.findByRole('dialog', { name: '选择移动目标' })
    expect(within(dialog).getByRole('radio')).toBeChecked()
    expect(within(dialog).getByText('selected/front.jpg')).toBeVisible()
    expect(within(dialog).getByRole('combobox')).toHaveValue('')
    expect(viewer.executeFileCommand).not.toHaveBeenCalled()
  })
})

function organizationPointerMove(
  handle: HTMLElement,
  target: HTMLElement,
  { pointerId, altKey }: { pointerId: number; altKey: boolean },
) {
  let capturedPointerId: number | null = null
  Object.defineProperties(handle, {
    setPointerCapture: {
      configurable: true,
      value: vi.fn((nextPointerId: number) => {
        capturedPointerId = nextPointerId
      }),
    },
    hasPointerCapture: {
      configurable: true,
      value: vi.fn((nextPointerId: number) => capturedPointerId === nextPointerId),
    },
    releasePointerCapture: {
      configurable: true,
      value: vi.fn((nextPointerId: number) => {
        if (capturedPointerId === nextPointerId) capturedPointerId = null
      }),
    },
  })
  const surface = defined(
    target.closest<HTMLElement>('[data-organization-drop-surface]'),
    'Expected organization drop surface',
  )
  vi.spyOn(surface, 'getBoundingClientRect').mockReturnValue({
    left: 0,
    top: 0,
    right: 260,
    bottom: 100,
    width: 260,
    height: 100,
    x: 0,
    y: 0,
    toJSON: () => undefined,
  })
  Object.defineProperty(document, 'elementFromPoint', {
    configurable: true,
    value: vi.fn(() => target),
  })
  fireEvent.pointerDown(handle, {
    pointerId,
    button: 0,
    altKey,
    clientX: 10,
    clientY: 50,
  })
  fireEvent.pointerMove(handle, {
    pointerId,
    altKey: !altKey,
    clientX: 20,
    clientY: 50,
  })
}

function organizationPointerDrag(
  handle: HTMLElement,
  target: HTMLElement,
  {
    pointerId,
    altKey,
    releaseAltKey,
  }: { pointerId: number; altKey: boolean; releaseAltKey: boolean },
) {
  organizationPointerMove(handle, target, { pointerId, altKey })
  fireEvent.pointerUp(handle, {
    pointerId,
    altKey: releaseAltKey,
    clientX: 20,
    clientY: 20,
  })
}

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
    otherFiles: [],
  }
}

function mixedContentWorkspace({
  entityId = 'text-root',
  relativePath = 'root/root-notes.txt',
  name = 'root-notes.txt',
}: {
  entityId?: string
  relativePath?: string
  name?: string
} = {}) {
  return {
    ...contentWorkspace(),
    otherFiles: [
      {
        entityId,
        relativePath,
        name,
        kind: 'text' as const,
        size: 20,
        modifiedNs: '2',
        marker: { reviewState: null, favorite: false },
        imageMetadata: null,
        imageUrl: null,
      },
    ],
  }
}

function genericOtherContentWorkspace() {
  return {
    ...contentWorkspace(),
    otherFiles: [
      {
        entityId: 'other-license',
        relativePath: 'id/license.other',
        name: 'license.other',
        kind: 'other' as const,
        size: 20,
        modifiedNs: '3',
        marker: { reviewState: null, favorite: false },
        imageMetadata: null,
        imageUrl: null,
      },
    ],
  }
}

function unsupportedImageContentWorkspace() {
  const source = contentWorkspace().images[0]
  if (source === undefined) throw new Error('Expected supported image fixture')
  return {
    workspace: 'content' as const,
    images: [
      {
        ...source,
        name: 'poster.webp',
        relativePath: 'id/poster.webp',
        kind: 'unsupported_image' as const,
      },
    ],
    otherFiles: [],
  }
}

function categoryWorkspace() {
  return {
    workspace: 'category' as const,
    folders: [
      {
        entityId: 'folder-b01',
        relativePath: '角色/B01',
        name: 'B01',
        marker: { reviewState: null, favorite: false },
        imageCount: 2,
        otherFileCount: 0,
        reviewProgress: {
          total: 2,
          keep: 0,
          pending: 0,
          reject: 0,
          unmarked: 2,
          favorite: 0,
        },
        representativeImages: [],
      },
    ],
  }
}

function compareContentWorkspace() {
  const first = defined(contentWorkspace().images[0], 'Expected first content workspace image')
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
    otherFiles: [],
  }
}

function compareContentWorkspaceWithCount(count: number) {
  const source = defined(contentWorkspace().images[0], 'Expected source image')
  return {
    workspace: 'content' as const,
    images: Array.from({ length: count }, (_, index) => ({
      ...source,
      entityId: `image-${index + 1}`,
      relativePath: `id/image-${index + 1}.jpg`,
      name: `image-${index + 1}.jpg`,
      modifiedNs: String(index + 1),
    })),
    otherFiles: [],
  }
}

function comparePortraitWorkspace(count: number) {
  return {
    ...compareContentWorkspaceWithCount(count),
    images: compareContentWorkspaceWithCount(count).images.map((file) => ({
      ...file,
      imageMetadata: { width: 600, height: 800 },
    })),
  }
}

function installCompareResizeObserver() {
  const frames = new Map<number, FrameRequestCallback>()
  let nextFrameId = 1
  class Observer {
    private readonly callback: ResizeObserverCallback

    constructor(callback: ResizeObserverCallback) {
      this.callback = callback
    }

    observe(node: Element) {
      const bounds = node.classList.contains('compare-layout-region')
        ? { width: 1_700, height: 900 }
        : { width: 600, height: 800 }
      this.callback(
        [{ target: node, contentRect: bounds } as ResizeObserverEntry],
        this as unknown as ResizeObserver,
      )
    }

    disconnect() {}
  }
  vi.stubGlobal('ResizeObserver', Observer)
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
    const id = nextFrameId
    nextFrameId += 1
    frames.set(id, callback)
    return id
  })
  vi.stubGlobal('cancelAnimationFrame', (id: number) => {
    frames.delete(id)
  })
  return {
    flush() {
      while (frames.size > 0) {
        const callbacks = [...frames.values()]
        frames.clear()
        for (const callback of callbacks) callback(0)
      }
    },
  }
}

function readOnlyContentWorkspace() {
  return {
    ...compareContentWorkspace(),
    otherFiles: [
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
