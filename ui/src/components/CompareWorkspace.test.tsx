import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { StrictMode } from 'react'
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
    const leading = toolbar.querySelector('.viewer-toolbar__leading')
    const transforms = toolbar.querySelector('.viewer-toolbar__center')
    const actions = toolbar.querySelector('.viewer-toolbar__actions')
    expect(toolbar).toHaveClass('viewer-toolbar')
    expect(
      within(transforms as HTMLElement).getByRole('toolbar', { name: '对比显示控制' }),
    ).toHaveClass('viewer-segmented-control')
    expect(leading).toHaveTextContent('2 张图片')
    expect(leading).toHaveTextContent('图片 1.jpg')
    expect(
      within(transforms as HTMLElement).getByRole('button', { name: '适应窗口' }),
    ).toBeVisible()
    expect(within(transforms as HTMLElement).getByRole('button', { name: '100%' })).toBeVisible()
    expect(
      within(transforms as HTMLElement).getByRole('button', { name: '顺时针旋转当前图片' }),
    ).toHaveClass('viewer-icon-button')
    for (const name of ['缩小当前对比', '放大当前对比', '顺时针旋转当前图片']) {
      expect(
        within(transforms as HTMLElement)
          .getByRole('button', { name })
          .querySelector('.viewer-icon'),
      ).toHaveAttribute('aria-hidden', 'true')
    }
    expect(
      within(transforms as HTMLElement).queryByRole('button', { name: /切换为.*变换/ }),
    ).not.toBeInTheDocument()
    expect(
      within(actions as HTMLElement).getByRole('button', { name: '切换为独立变换' }),
    ).toHaveTextContent('同步')
    const complete = within(actions as HTMLElement).getByRole('button', { name: '完成对比' })
    expect(complete).toHaveClass('viewer-button', 'preview-complete-action')
    expect(complete).toHaveTextContent('完成')
    const panes = within(workspace).getAllByRole('group', { name: /图片/ })
    expect(panes[0]).toHaveAttribute('data-active', 'true')
    fireEvent.focus(defined(panes[1], 'Expected second comparison pane'))
    expect(leading).toHaveTextContent('图片 2.jpg')
  })

  it('rejects invalid cardinality and non-image candidates with safe feedback', () => {
    const one = renderWorkspace({ files: files.slice(0, 1) })
    expect(screen.getByRole('alert')).toHaveTextContent('请选择 2–8 张图片进行对比')
    one.unmount()

    renderWorkspace({
      files: [
        defined(files[0], 'Expected first comparison fixture'),
        { ...defined(files[1], 'Expected second comparison fixture'), kind: 'text' },
      ],
    })
    expect(screen.getByRole('alert')).toHaveTextContent('请选择 2–8 张图片进行对比')
  })

  it('keeps unsupported panes request-free and disables active transforms', async () => {
    const unsupported = {
      ...image('raw', 'raw.cr2'),
      kind: 'unsupported_image' as const,
    }
    const supported = image('supported', 'supported.jpg')
    const requestImage = vi.fn((_file: BrowserFile, _request: ImageRepresentationRequest) => {
      return new Promise<ImageRepresentation>(() => undefined)
    })

    renderWorkspace({ files: [unsupported, supported], requestImage })

    expect(screen.getByLabelText('raw.cr2 .CR2 暂不支持预览')).toBeVisible()
    await waitFor(() => expect(requestImage).toHaveBeenCalledTimes(2))
    expect(
      requestImage.mock.calls.map(([requestedFile, request]) => [
        requestedFile.entityId,
        request.kind,
      ]),
    ).toEqual([
      ['supported', 'original100_percent'],
      ['supported', 'fit_preview'],
    ])
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
    const requestImage = vi.fn(
      (_file: BrowserFile, _request: ImageRepresentationRequest) =>
        new Promise<ImageRepresentation>(() => undefined),
    )
    renderWorkspace({ files: portraitFiles(8), requestImage })
    act(() => resize.workspace(1_700, 900))
    expect(screen.getByRole('list', { name: '滚动图片对比' })).toHaveAttribute(
      'data-axis',
      'horizontal',
    )
    act(() => resize.stages(600, 800))
    const mountedItems = screen.getAllByRole('listitem')
    expect(mountedItems.length).toBeGreaterThan(0)
    expect(mountedItems.length).toBeLessThan(8)
    expect(
      requestImage.mock.calls.filter(([, request]) => request.kind === 'fit_preview'),
    ).toHaveLength(mountedItems.length)
    expect(
      requestImage.mock.calls.filter(([, request]) => request.kind === 'original100_percent'),
    ).toHaveLength(2)
    expect(
      mountedItems.map((item) => [
        item.getAttribute('aria-posinset'),
        item.getAttribute('aria-setsize'),
      ]),
    ).toEqual([
      ['1', '8'],
      ['2', '8'],
      ['3', '8'],
      ['4', '8'],
      ['5', '8'],
      ['6', '8'],
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
    renderWorkspace({ files: portraitFiles(8), requestImage })
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

    await waitFor(() => {
      expect(screen.queryByRole('group', { name: '对比 portrait-1.jpg' })).not.toBeInTheDocument()
      expect(initialSignals.get('portrait-1')?.aborted).toBe(true)
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

  it('loads every mounted original through a two-wide FIFO queue without starvation', async () => {
    const queuedFiles = portraitFiles(4)
    const originals = new Map(
      queuedFiles.map((file) => [file.entityId, deferred<ImageRepresentation>()]),
    )
    const started: string[] = []
    const requestImage = vi.fn((file: BrowserFile, request: ImageRepresentationRequest) => {
      if (request.kind === 'fit_preview') {
        return Promise.resolve(imageRepresentation(`proxy-${file.entityId}`))
      }
      started.push(file.entityId)
      return defined(originals.get(file.entityId), `Expected original for ${file.entityId}`).promise
    })
    renderWorkspace({ files: queuedFiles, requestImage })

    await waitFor(() => expect(started).toHaveLength(2))
    await act(async () =>
      originals.get('portrait-0')?.resolve(imageRepresentation('original-portrait-0')),
    )
    await waitFor(() =>
      expect([...new Set(started)]).toEqual(['portrait-0', 'portrait-1', 'portrait-2']),
    )
    await act(async () =>
      originals.get('portrait-1')?.resolve(imageRepresentation('original-portrait-1')),
    )
    await act(async () =>
      originals.get('portrait-2')?.resolve(imageRepresentation('original-portrait-2')),
    )
    await waitFor(() =>
      expect([...new Set(started)]).toEqual([
        'portrait-0',
        'portrait-1',
        'portrait-2',
        'portrait-3',
      ]),
    )
    await act(async () =>
      originals.get('portrait-3')?.resolve(imageRepresentation('original-portrait-3')),
    )

    for (const file of queuedFiles) {
      expect(await screen.findByRole('img', { name: file.name })).toHaveAttribute(
        'src',
        `viewer-image://localhost/original-${file.entityId}`,
      )
    }
  })

  it('keeps the original scheduler available after StrictMode effect preflight', async () => {
    const resize = installCompareResizeObserver()
    const requestImage = vi.fn((file: BrowserFile, request: ImageRepresentationRequest) =>
      Promise.resolve(
        imageRepresentation(
          `${request.kind === 'original100_percent' ? 'original' : 'proxy'}-${file.entityId}`,
        ),
      ),
    )
    render(<StrictMode>{workspace({ requestImage })}</StrictMode>)
    act(() => resize.workspace(1_700, 900))
    act(() => resize.stages(640, 480))

    expect(await screen.findByRole('img', { name: 'a.jpg' })).toHaveAttribute(
      'src',
      'viewer-image://localhost/original-a',
    )
    expect(await screen.findByRole('img', { name: 'b.jpg' })).toHaveAttribute(
      'src',
      'viewer-image://localhost/original-b',
    )
  })

  it('keeps Fit and 100% as geometry controls without requesting another source', async () => {
    const requestImage = vi.fn((file: BrowserFile, request: ImageRepresentationRequest) =>
      request.kind === 'original100_percent'
        ? Promise.resolve(imageRepresentation(`original-${file.entityId}`))
        : Promise.resolve(imageRepresentation(`proxy-${file.entityId}`)),
    )
    renderWorkspace({ requestImage })
    expect(await screen.findByRole('img', { name: 'a.jpg' })).toHaveAttribute(
      'src',
      'viewer-image://localhost/original-a',
    )
    expect(await screen.findByRole('img', { name: 'b.jpg' })).toHaveAttribute(
      'src',
      'viewer-image://localhost/original-b',
    )
    const originalCount = requestImage.mock.calls.filter(
      ([, request]) => request.kind === 'original100_percent',
    ).length
    const proxyCount = requestImage.mock.calls.filter(
      ([, request]) => request.kind === 'fit_preview',
    ).length

    fireEvent.focus(pane('b'))
    fireEvent.click(screen.getByRole('button', { name: '100%' }))
    fireEvent.click(screen.getByRole('button', { name: '适应窗口' }))
    fireEvent.click(screen.getByRole('button', { name: '100%' }))

    expect(
      requestImage.mock.calls.filter(([, request]) => request.kind === 'original100_percent'),
    ).toHaveLength(originalCount)
    expect(
      requestImage.mock.calls.filter(([, request]) => request.kind === 'fit_preview'),
    ).toHaveLength(proxyCount)
  })

  it('recomputes actual pixels when 100% is requested before dimensions arrive', async () => {
    const original = deferred<ImageRepresentation>()
    const withoutMetadata = files.slice(0, 2).map((file) => ({ ...file, imageMetadata: null }))
    const requestImage = vi.fn((file: BrowserFile, request: ImageRepresentationRequest) => {
      if (request.kind === 'fit_preview') return new Promise<ImageRepresentation>(() => undefined)
      return file.entityId === 'a'
        ? original.promise
        : new Promise<ImageRepresentation>(() => undefined)
    })
    renderWorkspace({ files: withoutMetadata, requestImage })
    fireEvent.click(screen.getByRole('button', { name: '100%' }))

    await act(async () => original.resolve(imageRepresentation('original-a')))
    await waitFor(() => expect(pane('a')).toHaveAttribute('data-scale', '6.25'))
  })

  it('keeps the current transform when an original falls back locally', async () => {
    const failedOriginal = deferred<ImageRepresentation>()
    const status = vi.fn()
    const requestImage = vi.fn((file: BrowserFile, request: ImageRepresentationRequest) => {
      if (request.kind === 'fit_preview') {
        return Promise.resolve(imageRepresentation(`proxy-${file.entityId}`))
      }
      return file.entityId === 'a'
        ? failedOriginal.promise
        : new Promise<ImageRepresentation>(() => undefined)
    })
    renderWorkspace({ requestImage, onStatus: status })
    await screen.findByRole('img', { name: 'a.jpg' })
    fireEvent.click(screen.getByRole('button', { name: '放大当前对比' }))
    expect(pane('a')).toHaveAttribute('data-scale', '1.25')

    await act(async () => failedOriginal.reject({ code: 'image_budget_exceeded' }))

    await waitFor(() =>
      expect(status).toHaveBeenCalledWith('原图超出安全预览限制，已继续使用适窗代理。'),
    )
    expect(pane('a')).toHaveAttribute('data-scale', '1.25')
  })

  it('cancels a removed queued pane before it reaches the native image lane', async () => {
    const queuedFiles = portraitFiles(4)
    const originals = new Map(
      queuedFiles.map((file) => [file.entityId, deferred<ImageRepresentation>()]),
    )
    const started: string[] = []
    const requestImage = vi.fn((file: BrowserFile, request: ImageRepresentationRequest) => {
      if (request.kind === 'fit_preview') {
        return Promise.resolve(imageRepresentation(`proxy-${file.entityId}`))
      }
      started.push(file.entityId)
      return defined(originals.get(file.entityId), `Expected original for ${file.entityId}`).promise
    })
    renderWorkspace({ files: queuedFiles, requestImage })
    await waitFor(() => expect(started).toHaveLength(2))
    await act(async () =>
      originals.get('portrait-0')?.resolve(imageRepresentation('original-portrait-0')),
    )
    await waitFor(() =>
      expect([...new Set(started)]).toEqual(['portrait-0', 'portrait-1', 'portrait-2']),
    )

    fireEvent.click(screen.getByRole('button', { name: '移除 portrait-3.jpg' }))
    await act(async () =>
      originals.get('portrait-1')?.resolve(imageRepresentation('original-portrait-1')),
    )
    await act(async () =>
      originals.get('portrait-2')?.resolve(imageRepresentation('original-portrait-2')),
    )

    expect(started).not.toContain('portrait-3')
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
    const requestImage = vi.fn((file: BrowserFile, request: ImageRepresentationRequest) => {
      if (request.kind === 'original100_percent') {
        return new Promise<ImageRepresentation>(() => undefined)
      }
      return file.entityId === 'landscape-1'
        ? Promise.reject(new Error('corrupt image'))
        : Promise.resolve(imageRepresentation(`proxy-${file.entityId}`))
    })
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
    const requestImage = vi.fn((file: BrowserFile, request: ImageRepresentationRequest) =>
      request.kind === 'original100_percent'
        ? new Promise<ImageRepresentation>(() => undefined)
        : Promise.resolve({
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
    await waitFor(() =>
      expect(
        requestImage.mock.calls.filter(([, request]) => request.kind === 'fit_preview'),
      ).toHaveLength(4),
    )
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
    await waitFor(() =>
      expect(
        requestImage.mock.calls.filter(([, request]) => request.kind === 'fit_preview'),
      ).toHaveLength(8),
    )
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
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((next, fail) => {
    resolve = next
    reject = fail
  })
  return { promise, resolve, reject }
}
