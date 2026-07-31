import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { BrowserFile, ImageRepresentation, ImageRepresentationRequest } from '../api/types'
import { defined } from '../defined'
import CompareWorkspace from './CompareWorkspace'

const files = ['a', 'b', 'c', 'd'].map((id) => image(id, `${id}.jpg`))
let activeResizeHarness: CompareResizeHarness | null = null

afterEach(() => {
  activeResizeHarness = null
  vi.unstubAllGlobals()
})

describe('CompareWorkspace', () => {
  it('groups comparison controls and exposes the active image pane', () => {
    const picturedFiles = files.slice(0, 2).map((file, index) => ({
      ...file,
      name: `图片 ${index + 1}.jpg`,
    }))

    renderWorkspace({ files: picturedFiles })

    const workspace = screen.getByRole('region', { name: '图片对比' })
    const toolbar = within(workspace).getByRole('toolbar', { name: '对比工具' })
    const leading = toolbar.querySelector('.compare-toolbar-leading')
    const transforms = toolbar.querySelector('.compare-toolbar-transform')
    const actions = toolbar.querySelector('.compare-toolbar-actions')
    expect(transforms).toHaveClass('preview-segmented-controls')
    expect(leading).toHaveTextContent('2 张图片')
    expect(leading).toHaveTextContent('图片 1.jpg')
    expect(
      within(transforms as HTMLElement).getByRole('button', { name: '适应窗口' }),
    ).toBeVisible()
    expect(within(transforms as HTMLElement).getByRole('button', { name: '100%' })).toBeVisible()
    expect(
      within(transforms as HTMLElement).getByRole('button', { name: '顺时针旋转当前图片' }),
    ).toBeVisible()
    expect(
      within(transforms as HTMLElement).queryByRole('button', { name: /切换为.*变换/ }),
    ).not.toBeInTheDocument()
    expect(
      within(actions as HTMLElement).getByRole('button', { name: '切换为独立变换' }),
    ).toHaveTextContent('同步')
    const complete = within(actions as HTMLElement).getByRole('button', { name: '完成对比' })
    expect(complete).toHaveClass('preview-complete-action')
    expect(complete).toHaveTextContent('完成')
    const panes = within(workspace).getAllByRole('group', { name: /图片/ })
    expect(panes[0]).toHaveAttribute('data-active', 'true')
    fireEvent.focus(defined(panes[1], 'Expected second comparison pane'))
    expect(leading).toHaveTextContent('图片 2.jpg')
  })

  it('rejects invalid cardinality and non-image candidates with safe feedback', () => {
    const one = renderWorkspace({ files: files.slice(0, 1) })
    expect(screen.getByRole('alert')).toHaveTextContent('请选择 2–20 张图片进行对比')
    one.unmount()

    renderWorkspace({
      files: [
        defined(files[0], 'Expected first comparison fixture'),
        { ...defined(files[1], 'Expected second comparison fixture'), kind: 'text' },
      ],
    })
    expect(screen.getByRole('alert')).toHaveTextContent('请选择 2–20 张图片进行对比')
  })

  it('keeps unsupported panes request-free and disables active transforms', async () => {
    const unsupported = {
      ...image('raw', 'raw.cr2'),
      kind: 'unsupported_image' as const,
    }
    const supported = image('supported', 'supported.jpg')
    const requestImage = vi.fn((_file: BrowserFile) => {
      return new Promise<ImageRepresentation>(() => undefined)
    })

    renderWorkspace({ files: [unsupported, supported], requestImage })

    expect(screen.getByLabelText('raw.cr2 .CR2 暂不支持预览')).toBeVisible()
    await waitFor(() => expect(requestImage).toHaveBeenCalled())
    expect(requestImage.mock.calls.map(([file]) => file.entityId)).toEqual(['supported'])
    expect(screen.getByRole('button', { name: '适应窗口' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '100%' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '缩小当前对比' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '放大当前对比' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '顺时针旋转当前图片' })).toBeDisabled()
  })

  it('uses one row for four portraits in a wide workspace', () => {
    const resize = installCompareResizeObserver()
    renderWorkspace({ files: portraitFiles(4) })
    act(() => resize.workspace(1_700, 900))
    const workspace = screen.getByRole('region', { name: '图片对比' })
    expect(workspace).toHaveAttribute('data-layout', 'fit-row')
    const fit = screen.getByRole('list', { name: '全部图片对比' })
    expect(within(fit).getAllByRole('listitem')).toHaveLength(4)
    expect(
      within(fit)
        .getAllByRole('listitem')
        .map((item) => [item.getAttribute('aria-posinset'), item.getAttribute('aria-setsize')]),
    ).toEqual([
      ['1', '4'],
      ['2', '4'],
      ['3', '4'],
      ['4', '4'],
    ])
  })

  it('uses a grid for four landscapes', () => {
    const resize = installCompareResizeObserver()
    renderWorkspace({ files: landscapeFiles(4) })
    act(() => resize.workspace(1_700, 900))
    expect(screen.getByRole('region', { name: '图片对比' })).toHaveAttribute(
      'data-layout',
      'fit-grid',
    )
  })

  it('scrolls a retained active pane into view when a fit row becomes a scrolling strip', () => {
    const resize = installCompareResizeObserver()
    renderWorkspace({ files: portraitFiles(4) })
    act(() => resize.workspace(1_700, 900))
    fireEvent.focus(pane('portrait-3'))

    act(() => resize.workspace(426, 900))

    const viewport = screen.getByRole('list', { name: '滚动图片对比' })
    const activeItem = defined(
      pane('portrait-3').closest<HTMLElement>('[role="listitem"]'),
      'Expected active pane layout item',
    )
    const activeLeft = Number.parseFloat(activeItem.style.left)
    const activeRight = activeLeft + Number.parseFloat(activeItem.style.width)
    expect(activeLeft).toBeLessThan(viewport.scrollLeft + 426)
    expect(activeRight).toBeGreaterThan(viewport.scrollLeft)
  })

  it('virtualizes a scrolling portrait set', () => {
    const resize = installCompareResizeObserver()
    const requestImage = vi.fn(() => new Promise<ImageRepresentation>(() => undefined))
    renderWorkspace({ files: portraitFiles(20), requestImage })
    act(() => resize.workspace(1_700, 900))
    expect(screen.getByRole('list', { name: '滚动图片对比' })).toHaveAttribute(
      'data-axis',
      'horizontal',
    )
    act(() => resize.stages(600, 800))
    const mountedItems = screen.getAllByRole('listitem')
    expect(mountedItems.length).toBeGreaterThan(0)
    expect(mountedItems.length).toBeLessThan(20)
    expect(requestImage).toHaveBeenCalledTimes(mountedItems.length)
    expect(
      mountedItems.map((item) => [
        item.getAttribute('aria-posinset'),
        item.getAttribute('aria-setsize'),
      ]),
    ).toEqual([
      ['1', '20'],
      ['2', '20'],
      ['3', '20'],
      ['4', '20'],
      ['5', '20'],
      ['6', '20'],
    ])
  })

  it('unmounts every old non-active pane and aborts each pending request at scroll end', async () => {
    const resize = installCompareResizeObserver()
    const proxySignals = new Map<string, AbortSignal | undefined>()
    const requestImage = vi.fn(
      (
        file: BrowserFile,
        request: ImageRepresentationRequest,
        signal?: AbortSignal,
      ): Promise<ImageRepresentation> => {
        if (request.kind === 'fit_preview') proxySignals.set(file.entityId, signal)
        return new Promise((_resolve, reject) => {
          signal?.addEventListener(
            'abort',
            () => reject(new DOMException('request cancelled', 'AbortError')),
            { once: true },
          )
        })
      },
    )
    renderWorkspace({ files: portraitFiles(20), requestImage })
    act(() => resize.workspace(1_700, 900))
    act(() => resize.stages(600, 800))
    const initiallyMountedIds = screen
      .getAllByRole('listitem')
      .map((item) => defined(item.dataset.compareEntityId, 'Expected mounted compare entity'))
    expect(initiallyMountedIds).toEqual([
      'portrait-0',
      'portrait-1',
      'portrait-2',
      'portrait-3',
      'portrait-4',
      'portrait-5',
    ])
    await waitFor(() =>
      expect(initiallyMountedIds.every((entityId) => proxySignals.has(entityId))).toBe(true),
    )
    const initialSignals = new Map(
      initiallyMountedIds.map((entityId) => [
        entityId,
        defined(proxySignals.get(entityId), `Expected signal for ${entityId}`),
      ]),
    )

    const viewport = screen.getByRole('list', { name: '滚动图片对比' })
    viewport.scrollLeft = 10_000
    fireEvent.scroll(viewport)

    const oldNonActiveIds = initiallyMountedIds.filter((entityId) => entityId !== 'portrait-0')
    await waitFor(() => {
      for (const entityId of oldNonActiveIds) {
        expect(
          screen.queryByRole('group', { name: `对比 ${entityId}.jpg` }),
        ).not.toBeInTheDocument()
        expect(initialSignals.get(entityId)?.aborted).toBe(true)
      }
    })
    expect(screen.getByRole('group', { name: '对比 portrait-0.jpg' })).toBeInTheDocument()
    expect(initialSignals.get('portrait-0')?.aborted).toBe(false)
  })

  it('synchronizes by default, retains independent transforms and keeps rotation pane-local', () => {
    renderWorkspace()
    fireEvent.click(screen.getByRole('button', { name: '放大当前对比' }))
    expect(pane('a')).toHaveAttribute('data-scale', '1.25')
    expect(pane('b')).toHaveAttribute('data-scale', '1.25')

    fireEvent.click(screen.getByRole('button', { name: '切换为独立变换' }))
    fireEvent.focus(pane('b'))
    fireEvent.click(screen.getByRole('button', { name: '放大当前对比' }))
    expect(pane('a')).toHaveAttribute('data-scale', '1.25')
    expect(pane('b')).toHaveAttribute('data-scale', '1.5625')
    fireEvent.click(screen.getByRole('button', { name: '顺时针旋转当前图片' }))
    expect(pane('a')).toHaveAttribute('data-rotation', '0')
    expect(pane('b')).toHaveAttribute('data-rotation', '90')
  })

  it('preserves surviving pane transforms on removal and exits at one or zero panes', () => {
    const changed = vi.fn()
    const rendered = renderWorkspace({ files, onEntityIdsChange: changed })
    fireEvent.click(screen.getByRole('button', { name: '切换为独立变换' }))
    fireEvent.focus(pane('b'))
    fireEvent.click(screen.getByRole('button', { name: '放大当前对比' }))
    fireEvent.click(screen.getByRole('button', { name: '移除 d.jpg' }))
    act(() => activeResizeHarness?.flush())
    expect(changed).toHaveBeenLastCalledWith(['a', 'b', 'c'])
    expect(pane('b')).toHaveAttribute('data-scale', '1.25')
    rendered.unmount()

    renderWorkspace({ onEntityIdsChange: changed })
    fireEvent.click(screen.getByRole('button', { name: '移除 a.jpg' }))
    expect(changed).toHaveBeenLastCalledWith(['b'])
    fireEvent.click(screen.getByRole('button', { name: '完成对比' }))
    expect(changed).toHaveBeenLastCalledWith([])
  })

  it('moves activity to the nearest following source-order neighbor when removing the active pane', () => {
    const changed = vi.fn()
    renderWorkspace({ files, onEntityIdsChange: changed })
    fireEvent.focus(pane('c'))

    fireEvent.click(screen.getByRole('button', { name: '移除 c.jpg' }))

    expect(changed).toHaveBeenLastCalledWith(['a', 'b', 'd'])
    expect(pane('d')).toHaveClass('is-active')
    expect(pane('b')).not.toHaveClass('is-active')
  })

  it('supports focused keyboard exit and never disables compare in read-only mode', () => {
    const changed = vi.fn()
    renderWorkspace({ readOnly: true, onEntityIdsChange: changed })
    const workspace = screen.getByRole('region', { name: '图片对比' })
    workspace.focus()
    fireEvent.keyDown(workspace, { key: 'Escape' })
    expect(changed).toHaveBeenCalledWith([])
    expect(screen.getByRole('button', { name: 'a.jpg 标记为保留' })).toBeDisabled()
  })

  it('requests 100% only for the active pane and reuses resident fit proxies', async () => {
    const requestImage = vi.fn(
      (_file: BrowserFile, _request: ImageRepresentationRequest) =>
        new Promise<ImageRepresentation>(() => undefined),
    )
    renderWorkspace({ requestImage })
    fireEvent.focus(pane('b'))
    fireEvent.click(screen.getByRole('button', { name: '100%' }))
    await waitFor(() =>
      expect(requestImage).toHaveBeenCalledWith(
        files[1],
        {
          kind: 'original100_percent',
        },
        expect.any(AbortSignal),
      ),
    )
    expect(
      requestImage.mock.calls.filter(([, request]) => request.kind === 'original100_percent'),
    ).toHaveLength(1)

    fireEvent.click(screen.getByRole('button', { name: '适应窗口' }))
    expect(
      requestImage.mock.calls.filter(([, request]) => request.kind === 'fit_preview'),
    ).toHaveLength(2)
  })

  it('recomputes actual pixels when 100% is requested before dimensions arrive', async () => {
    const requestImage = vi.fn((_file: BrowserFile, request: ImageRepresentationRequest) =>
      request.kind === 'original100_percent'
        ? Promise.resolve({
            cacheKey: 'original-a',
            url: 'viewer-image://localhost/original-a',
            width: 4_000,
            height: 3_000,
            backend: 'image_io' as const,
          })
        : new Promise<ImageRepresentation>(() => undefined),
    )
    renderWorkspace({ requestImage })
    fireEvent.click(screen.getByRole('button', { name: '100%' }))

    await waitFor(() => expect(pane('a')).toHaveAttribute('data-scale', '6.25'))
  })

  it('returns the transform to fit when an original exceeds the budget', async () => {
    const status = vi.fn()
    const requestImage = vi.fn((_file: BrowserFile, request: ImageRepresentationRequest) =>
      request.kind === 'original100_percent'
        ? Promise.reject({ code: 'image_budget_exceeded' })
        : Promise.resolve({
            cacheKey: 'proxy',
            url: 'viewer-image://localhost/proxy',
            width: 800,
            height: 600,
            backend: 'image_io' as const,
          }),
    )
    renderWorkspace({ requestImage, onStatus: status })
    await screen.findByRole('img', { name: 'a.jpg' })
    fireEvent.click(screen.getByRole('button', { name: '100%' }))

    await waitFor(() => expect(pane('a')).toHaveAttribute('data-scale', '1'))
    expect(status).toHaveBeenCalledWith('原图超出安全预览限制，已继续使用适窗代理。')
  })

  it('serializes rapid original switches and keeps every pane proxy resident', async () => {
    const firstOriginal = deferred<ImageRepresentation>()
    const secondOriginal = deferred<ImageRepresentation>()
    let originalCalls = 0
    const requestImage = vi.fn((file: BrowserFile, request: ImageRepresentationRequest) => {
      if (request.kind === 'original100_percent') {
        originalCalls += 1
        return originalCalls === 1 ? firstOriginal.promise : secondOriginal.promise
      }
      return Promise.resolve({
        cacheKey: `proxy-${file.entityId}`,
        url: `viewer-image://localhost/proxy-${file.entityId}`,
        width: 800,
        height: 600,
        backend: 'image_io' as const,
      })
    })
    renderWorkspace({ requestImage })
    await screen.findByRole('img', { name: 'a.jpg' })
    await screen.findByRole('img', { name: 'b.jpg' })

    fireEvent.click(screen.getByRole('button', { name: '100%' }))
    await waitFor(() => expect(originalCalls).toBe(1))
    fireEvent.focus(pane('b'))
    fireEvent.click(screen.getByRole('button', { name: '100%' }))
    expect(originalCalls).toBe(1)
    expect(screen.getByRole('img', { name: 'a.jpg' })).toHaveAttribute(
      'src',
      'viewer-image://localhost/proxy-a',
    )

    await act(async () => firstOriginal.resolve(imageRepresentation('original-a')))
    await waitFor(() => expect(originalCalls).toBe(2))
    await act(async () => secondOriginal.resolve(imageRepresentation('original-b')))
    expect(await screen.findByRole('img', { name: 'b.jpg' })).toHaveAttribute(
      'src',
      'viewer-image://localhost/original-b',
    )
  })

  it('lets a cancelled running original settle before starting the next serialized job', async () => {
    const originalEntityIds: string[] = []
    const originalSignals = new Map<string, AbortSignal | undefined>()
    const requestImage = vi.fn(
      (
        file: BrowserFile,
        request: ImageRepresentationRequest,
        signal?: AbortSignal,
      ): Promise<ImageRepresentation> => {
        if (request.kind !== 'original100_percent') {
          return Promise.resolve(imageRepresentation(`proxy-${file.entityId}`))
        }
        originalEntityIds.push(file.entityId)
        originalSignals.set(file.entityId, signal)
        return new Promise((_resolve, reject) => {
          signal?.addEventListener(
            'abort',
            () => reject(new DOMException('request cancelled', 'AbortError')),
            { once: true },
          )
        })
      },
    )
    renderWorkspace({ requestImage })
    await screen.findByRole('img', { name: 'a.jpg' })
    await screen.findByRole('img', { name: 'b.jpg' })

    fireEvent.click(screen.getByRole('button', { name: '100%' }))
    await waitFor(() => expect(originalEntityIds).toEqual(['a']))
    fireEvent.focus(pane('b'))
    fireEvent.click(screen.getByRole('button', { name: '100%' }))

    await waitFor(() => expect(originalSignals.get('a')?.aborted).toBe(true))
    await waitFor(() => expect(originalEntityIds).toEqual(['a', 'b']))
    expect(originalSignals.get('b')).toBeInstanceOf(AbortSignal)
  })

  it('never starts an original job after its queued caller is aborted', async () => {
    const running = deferred<ImageRepresentation>()
    const originalEntityIds: string[] = []
    const requestImage = vi.fn((file: BrowserFile, request: ImageRepresentationRequest) => {
      if (request.kind !== 'original100_percent') {
        return Promise.resolve(imageRepresentation(`proxy-${file.entityId}`))
      }
      originalEntityIds.push(file.entityId)
      return running.promise
    })
    renderWorkspace({ requestImage })
    await screen.findByRole('img', { name: 'a.jpg' })
    await screen.findByRole('img', { name: 'b.jpg' })

    fireEvent.click(screen.getByRole('button', { name: '100%' }))
    await waitFor(() => expect(originalEntityIds).toEqual(['a']))
    fireEvent.focus(pane('b'))
    fireEvent.click(screen.getByRole('button', { name: '100%' }))
    await waitFor(() => expect(pane('b')).toHaveAttribute('data-scale', '6.25'))
    fireEvent.click(screen.getByRole('button', { name: '适应窗口' }))
    await act(async () => running.resolve(imageRepresentation('original-a')))

    expect(originalEntityIds).toEqual(['a'])
  })

  it('drops a stale queued original before it reaches the native image lane', async () => {
    const running = deferred<ImageRepresentation>()
    const latest = deferred<ImageRepresentation>()
    const originalEntityIds: string[] = []
    const requestImage = vi.fn((file: BrowserFile, request: ImageRepresentationRequest) => {
      if (request.kind === 'original100_percent') {
        originalEntityIds.push(file.entityId)
        return originalEntityIds.length === 1 ? running.promise : latest.promise
      }
      return Promise.resolve({
        cacheKey: `proxy-${file.entityId}`,
        url: `viewer-image://localhost/proxy-${file.entityId}`,
        width: 800,
        height: 600,
        backend: 'image_io' as const,
      })
    })
    renderWorkspace({ requestImage })
    await screen.findByRole('img', { name: 'a.jpg' })
    fireEvent.click(screen.getByRole('button', { name: '100%' }))
    await waitFor(() => expect(originalEntityIds).toEqual(['a']))
    fireEvent.focus(pane('b'))
    fireEvent.click(screen.getByRole('button', { name: '100%' }))
    fireEvent.focus(pane('a'))
    fireEvent.click(screen.getByRole('button', { name: '100%' }))
    expect(originalEntityIds).toEqual(['a'])

    await act(async () => running.resolve(imageRepresentation('stale-a')))
    await waitFor(() => expect(originalEntityIds).toEqual(['a', 'a']))
    await act(async () => latest.resolve(imageRepresentation('latest-a')))
    expect(await screen.findByRole('img', { name: 'a.jpg' })).toHaveAttribute(
      'src',
      'viewer-image://localhost/latest-a',
    )
  })

  it('repairs externally removed panes without resetting surviving transforms', async () => {
    const changed = vi.fn()
    const rendered = renderWorkspace({ files: files.slice(0, 3), onEntityIdsChange: changed })
    fireEvent.click(screen.getByRole('button', { name: '切换为独立变换' }))
    fireEvent.focus(pane('b'))
    fireEvent.click(screen.getByRole('button', { name: '放大当前对比' }))

    rendered.rerender(workspace({ files: files.slice(0, 2), onEntityIdsChange: changed }))
    act(() => activeResizeHarness?.flush())
    await waitFor(() =>
      expect(screen.getByRole('region', { name: '图片对比' })).toHaveAttribute(
        'data-layout',
        'fit-row',
      ),
    )
    expect(pane('b')).toHaveAttribute('data-scale', '1.25')

    rendered.rerender(workspace({ files: files.slice(0, 1), onEntityIdsChange: changed }))
    await waitFor(() => expect(changed).toHaveBeenLastCalledWith(['a']))
  })

  it('keeps a failed pane and its neighbors in their planned rectangles', async () => {
    const resize = installCompareResizeObserver()
    const requestImage = vi.fn((file: BrowserFile) =>
      file.entityId === 'landscape-1'
        ? Promise.reject(new Error('corrupt image'))
        : Promise.resolve(imageRepresentation(`proxy-${file.entityId}`)),
    )
    renderWorkspace({ files: landscapeFiles(4), requestImage })
    act(() => resize.workspace(1_700, 900))

    const failedItem = defined(
      pane('landscape-1').parentElement,
      'Expected planned item for landscape-1',
    )
    const neighboringItem = defined(
      pane('landscape-2').parentElement,
      'Expected planned item for landscape-2',
    )
    const failedRectangle = failedItem.getAttribute('style')
    const neighboringRectangle = neighboringItem.getAttribute('style')
    act(() => resize.stages(800, 400))

    expect(await screen.findByRole('alert')).toHaveTextContent('无法预览该图片')
    expect(failedItem).toHaveAttribute('style', failedRectangle)
    expect(neighboringItem).toHaveAttribute('style', neighboringRectangle)
  })

  it('recovers missing natural dimensions for one controlled layout correction', async () => {
    const resize = installCompareResizeObserver()
    const missingMetadata = landscapeFiles(4).map((file) => ({
      ...file,
      imageMetadata: null,
    }))
    const requestImage = vi.fn((file: BrowserFile) =>
      Promise.resolve({
        ...imageRepresentation(`proxy-${file.entityId}`),
        width: 1_200,
        height: 800,
      }),
    )
    renderWorkspace({ files: missingMetadata, requestImage })
    act(() => resize.workspace(1_700, 900))
    expect(screen.getByRole('region', { name: '图片对比' })).toHaveAttribute(
      'data-layout',
      'fit-row',
    )

    act(() => resize.stages(800, 400))
    await waitFor(() => expect(requestImage).toHaveBeenCalledTimes(4))
    await act(async () => Promise.resolve())
    act(() => resize.flush())
    await waitFor(() =>
      expect(screen.getByRole('region', { name: '图片对比' })).toHaveAttribute(
        'data-layout',
        'fit-grid',
      ),
    )
    const correctedRectangles = screen
      .getAllByRole('listitem')
      .map((item) => item.getAttribute('style'))

    act(() => resize.stages(800, 400))
    await waitFor(() => expect(requestImage).toHaveBeenCalledTimes(8))
    await act(async () => Promise.resolve())
    expect(resize.pendingFrames()).toBe(0)
    expect(screen.getAllByRole('listitem').map((item) => item.getAttribute('style'))).toEqual(
      correctedRectangles,
    )
  })
})

function pane(id: string) {
  return screen.getByRole('group', { name: `对比 ${id}.jpg` })
}

function renderWorkspace(overrides: Partial<React.ComponentProps<typeof CompareWorkspace>> = {}) {
  const installDefaultSize = activeResizeHarness === null
  const resize = activeResizeHarness ?? installCompareResizeObserver()
  const rendered = render(workspace(overrides))
  if (installDefaultSize) {
    act(() => resize.workspace(1_700, 900))
    act(() => resize.stages(640, 480))
  }
  return rendered
}

function workspace(overrides: Partial<React.ComponentProps<typeof CompareWorkspace>> = {}) {
  return (
    <CompareWorkspace
      files={files.slice(0, 2)}
      readOnly={false}
      requestImage={vi.fn(() => new Promise<ImageRepresentation>(() => undefined))}
      onEntityIdsChange={vi.fn()}
      onSetReview={vi.fn()}
      onToggleFavorite={vi.fn()}
      onStatus={vi.fn()}
      {...overrides}
    />
  )
}

function image(entityId: string, name: string): BrowserFile {
  return {
    entityId,
    relativePath: `id/${name}`,
    name,
    kind: 'jpeg',
    size: 100,
    modifiedNs: '1',
    marker: { reviewState: null, favorite: false },
    imageMetadata: { width: 4_000, height: 3_000 },
    imageUrl: null,
  }
}

function imageRepresentation(token: string): ImageRepresentation {
  return {
    cacheKey: token,
    url: `viewer-image://localhost/${token}`,
    width: 4_000,
    height: 3_000,
    backend: 'image_io',
  }
}

function portraitFiles(count: number): BrowserFile[] {
  return Array.from({ length: count }, (_, index) => ({
    ...image(`portrait-${index}`, `portrait-${index}.jpg`),
    imageMetadata: { width: 600, height: 800 },
  }))
}

function landscapeFiles(count: number): BrowserFile[] {
  return Array.from({ length: count }, (_, index) => ({
    ...image(`landscape-${index}`, `landscape-${index}.jpg`),
    imageMetadata: { width: 1_200, height: 800 },
  }))
}

interface CompareResizeHarness {
  workspace: (width: number, height: number) => void
  stages: (width: number, height: number) => void
  flush: () => void
  pendingFrames: () => number
}

function installCompareResizeObserver(): CompareResizeHarness {
  const callbacks = new Map<Element, ResizeObserverCallback>()
  const frames = new Map<number, FrameRequestCallback>()
  let nextFrameId = 1
  class Observer {
    private readonly callback: ResizeObserverCallback
    private readonly observed = new Set<Element>()

    constructor(callback: ResizeObserverCallback) {
      this.callback = callback
    }

    observe(node: Element) {
      this.observed.add(node)
      callbacks.set(node, this.callback)
    }

    unobserve(node: Element) {
      this.observed.delete(node)
      callbacks.delete(node)
    }

    disconnect() {
      for (const node of this.observed) callbacks.delete(node)
      this.observed.clear()
    }
  }
  vi.stubGlobal('ResizeObserver', Observer)
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
    const frameId = nextFrameId
    nextFrameId += 1
    frames.set(frameId, callback)
    return frameId
  })
  vi.stubGlobal('cancelAnimationFrame', (frameId: number) => {
    frames.delete(frameId)
  })

  const trigger = (node: Element, width: number, height: number) => {
    callbacks.get(node)?.(
      [{ target: node, contentRect: { width, height } } as ResizeObserverEntry],
      {} as ResizeObserver,
    )
  }
  const flush = () => {
    while (frames.size > 0) {
      const pending = [...frames.values()]
      frames.clear()
      for (const callback of pending) callback(0)
    }
  }
  const harness = {
    workspace(width: number, height: number) {
      const node = document.querySelector('.compare-layout-region')
      if (node !== null) trigger(node, width, height)
      flush()
    },
    stages(width: number, height: number) {
      for (const node of document.querySelectorAll('.compare-pane-stage')) {
        trigger(node, width, height)
      }
    },
    flush,
    pendingFrames: () => frames.size,
  }
  activeResizeHarness = harness
  return harness
}

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((next) => {
    resolve = next
  })
  return { promise, resolve }
}
