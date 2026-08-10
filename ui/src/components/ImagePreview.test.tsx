import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { Profiler } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { BrowserFile, ImageRepresentation, ImageRepresentationRequest } from '../api/types'
import { defined } from '../defined'
import ImagePreview from './ImagePreview'

const MAGNIFIER = { shape: 'circle', magnification: 2, area: 'small' } as const
const POINTER_CLIENT_POINT = { current: null }
const nativeGetBoundingClientRect = HTMLElement.prototype.getBoundingClientRect
const previewResizeCallbacks = new Map<Element, ResizeObserverCallback>()
let initialPreviewStage = { width: 640, height: 480 }

beforeEach(() => {
  previewResizeCallbacks.clear()
  initialPreviewStage = { width: 640, height: 480 }
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (
    this: HTMLElement,
  ) {
    if (this.classList.contains('image-preview-stage')) {
      return stageRect(initialPreviewStage.width, initialPreviewStage.height)
    }
    return nativeGetBoundingClientRect.call(this)
  })
  class Observer {
    private node: Element | null = null
    private readonly callback: ResizeObserverCallback

    constructor(callback: ResizeObserverCallback) {
      this.callback = callback
    }

    observe(node: Element) {
      this.node = node
      previewResizeCallbacks.set(node, this.callback)
    }

    disconnect() {
      if (this.node !== null) previewResizeCallbacks.delete(this.node)
      this.node = null
    }
  }
  vi.stubGlobal('ResizeObserver', Observer)
})

afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

function image(index: number): BrowserFile {
  return {
    entityId: `image-${index}`,
    relativePath: `id-1/${index}.jpg`,
    name: `${index}.jpg`,
    kind: 'jpeg',
    size: index * 100,
    modifiedNs: String(index),
    marker: { reviewState: null, favorite: false },
    imageMetadata: null,
    imageUrl: null,
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, resolve, reject }
}

function loaded(cacheKey: string, width: number, height: number): ImageRepresentation {
  return {
    cacheKey,
    url: `viewer-image://localhost/session/${cacheKey}`,
    width,
    height,
    backend: 'image_io',
  }
}

function visibleImageSize(image: HTMLElement) {
  const scale = Number(image.getAttribute('style')?.match(/scale\(([^)]+)\)/)?.[1])
  return {
    width: Number(image.getAttribute('width')) * scale,
    height: Number(image.getAttribute('height')) * scale,
  }
}

function publishPreviewStage(stage: Element, width: number, height: number) {
  act(() => {
    previewResizeCallbacks.get(stage)?.(
      [{ target: stage, contentRect: { width, height } } as ResizeObserverEntry],
      {} as ResizeObserver,
    )
  })
}

function stageRect(width: number, height: number): DOMRect {
  return {
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    right: width,
    bottom: height,
    width,
    height,
    toJSON: () => undefined,
  }
}

describe('ImagePreview', () => {
  it.each([
    [1024, 720],
    [720, 450],
  ])('keeps all primary preview controls mounted at %d×%d CSS pixels', (width, height) => {
    vi.stubGlobal('innerWidth', width)
    vi.stubGlobal('innerHeight', height)
    const target = image(1)
    render(
      <ImagePreview
        file={target}
        files={[target]}
        magnifier={MAGNIFIER}
        pointerClientPoint={POINTER_CLIENT_POINT}
        requestImage={vi.fn(() => new Promise<never>(() => undefined))}
        onNavigate={vi.fn()}
        onClose={vi.fn()}
      />,
    )
    expect(screen.getByRole('toolbar', { name: '图片预览工具' })).toBeVisible()
    expect(screen.getByRole('button', { name: '适应窗口' })).toBeVisible()
    expect(screen.queryByRole('button', { name: '按 100% 显示' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '关闭预览' })).toBeVisible()
  })

  it('keeps the large rounded magnifier inside the clipped preview stage at 720×450', () => {
    vi.stubGlobal('innerWidth', 720)
    vi.stubGlobal('innerHeight', 450)
    const target = image(1)
    render(
      <ImagePreview
        file={target}
        files={[target]}
        magnifier={{ shape: 'rounded_rectangle', magnification: 2, area: 'large' }}
        pointerClientPoint={POINTER_CLIENT_POINT}
        requestImage={vi.fn(() => new Promise<never>(() => undefined))}
        onNavigate={vi.fn()}
        onClose={vi.fn()}
      />,
    )

    const lens = screen.getByTestId('image-magnifier')
    const stage = document.querySelector('.image-preview-stage')
    expect(lens).toHaveStyle({ width: '420px', height: '280px' })
    expect(stage).toContainElement(lens)
  })

  it('groups image controls in the toolbar and floats navigation over the stage', () => {
    const front = {
      ...image(1),
      relativePath: 'id-1/front.jpg',
      name: 'front.jpg',
      imageMetadata: { width: 4_000, height: 3_000 },
    }

    render(
      <ImagePreview
        file={front}
        files={[front]}
        magnifier={MAGNIFIER}
        pointerClientPoint={POINTER_CLIENT_POINT}
        requestImage={vi.fn(() => new Promise<never>(() => undefined))}
        onNavigate={vi.fn()}
        onClose={vi.fn()}
      />,
    )

    const dialog = screen.getByRole('dialog', { name: /front\.jpg/ })
    const toolbar = within(dialog).getByRole('toolbar', { name: '图片预览工具' })
    const leading = toolbar.querySelector('.viewer-toolbar__leading')
    const actions = toolbar.querySelector('.viewer-toolbar__actions')
    expect(toolbar).toHaveClass('viewer-toolbar')
    expect(leading).toHaveTextContent('front.jpg')
    expect(leading).toHaveTextContent('4000 × 3000 px · 100 B')
    expect(within(dialog).getByRole('toolbar', { name: '图片显示控制' })).toHaveClass(
      'viewer-segmented-control',
    )
    for (const name of ['缩小', '放大', '顺时针旋转']) {
      const iconButton = within(dialog).getByRole('button', { name })
      expect(iconButton).toHaveClass('viewer-icon-button')
      expect(iconButton.querySelector('.viewer-icon')).toHaveAttribute('aria-hidden', 'true')
    }
    const complete = within(actions as HTMLElement).getByRole('button', { name: '关闭预览' })
    expect(complete).toHaveClass('viewer-button', 'preview-complete-action')
    expect(complete).toHaveTextContent('完成')
    const navigation = within(dialog).getByRole('navigation', { name: '图片导航' })
    expect(navigation).toHaveClass('preview-navigation-float')
    expect(within(navigation).getByRole('button', { name: '上一张' })).toHaveClass(
      'viewer-icon-button',
    )
    expect(within(navigation).getByRole('button', { name: '下一张' })).toHaveClass(
      'viewer-icon-button',
    )
    expect(within(dialog).queryByText('front.jpg')?.closest('footer')).toBeNull()
  })

  it('keeps the formal toolbar mounted while loading and contains failure feedback in the stage', async () => {
    const target = image(1)
    const pending = render(
      <ImagePreview
        file={target}
        files={[target]}
        magnifier={MAGNIFIER}
        pointerClientPoint={POINTER_CLIENT_POINT}
        requestImage={vi.fn(() => new Promise<never>(() => undefined))}
        onNavigate={vi.fn()}
        onClose={vi.fn()}
      />,
    )

    const loadingStage = pending.container.querySelector('.image-preview-stage')
    expect(screen.getByRole('toolbar', { name: '图片预览工具' })).toHaveClass('viewer-toolbar')
    expect(within(loadingStage as HTMLElement).getByRole('status')).toHaveClass(
      'viewer-local-feedback',
    )
    pending.unmount()

    render(
      <ImagePreview
        file={target}
        files={[target]}
        magnifier={MAGNIFIER}
        pointerClientPoint={POINTER_CLIENT_POINT}
        requestImage={vi.fn(() => Promise.reject(new Error('decode failed')))}
        onNavigate={vi.fn()}
        onClose={vi.fn()}
      />,
    )

    const alert = await screen.findByRole('alert')
    expect(alert).toHaveClass('viewer-local-feedback')
    expect(alert.closest('.image-preview-stage')).not.toBeNull()
    expect(screen.getByRole('toolbar', { name: '图片预览工具' })).toHaveClass('viewer-toolbar')
  })

  it('renders an unsupported current image without issuing image requests', () => {
    const unsupported = {
      ...image(1),
      entityId: 'raw',
      relativePath: 'id-1/raw.cr2',
      name: 'raw.cr2',
      kind: 'unsupported_image' as const,
    }
    const request = vi.fn(() => new Promise<never>(() => undefined))

    render(
      <ImagePreview
        file={unsupported}
        files={[unsupported]}
        magnifier={MAGNIFIER}
        pointerClientPoint={POINTER_CLIENT_POINT}
        requestImage={request}
        onNavigate={vi.fn()}
        onClose={vi.fn()}
      />,
    )

    expect(screen.getByLabelText('raw.cr2 .CR2 暂不支持预览')).toBeVisible()
    expect(screen.getByRole('button', { name: '适应窗口' })).toBeDisabled()
    expect(screen.queryByRole('button', { name: '按 100% 显示' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '缩小' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '放大' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '顺时针旋转' })).toBeDisabled()
    expect(request).not.toHaveBeenCalled()
  })

  it('progressively replaces the fit proxy with original pixels without changing display geometry', async () => {
    const fit = deferred<ImageRepresentation>()
    const original = deferred<ImageRepresentation>()
    const target = {
      ...image(1),
      imageMetadata: { width: 6000, height: 4000 },
    }
    const request = vi.fn((_file: BrowserFile, representation: ImageRepresentationRequest) =>
      representation.kind === 'original100_percent' ? original.promise : fit.promise,
    )
    const onDimensions = vi.fn()
    render(
      <ImagePreview
        file={target}
        files={[target]}
        magnifier={MAGNIFIER}
        pointerClientPoint={POINTER_CLIENT_POINT}
        requestImage={request}
        onNavigate={vi.fn()}
        onClose={vi.fn()}
        onDimensions={onDimensions}
      />,
    )

    await waitFor(() => {
      expect(request.mock.calls.map(([, representation]) => representation.kind).sort()).toEqual([
        'fit_preview',
        'original100_percent',
      ])
    })

    await act(async () => fit.resolve(loaded('fit', 900, 600)))
    const preview = await screen.findByRole('img', { name: '1.jpg' })
    expect(preview).toHaveAttribute('src', expect.stringContaining('/fit'))
    expect(preview).toHaveAttribute('width', '6000')
    expect(preview).toHaveAttribute('height', '4000')
    expect(preview).toHaveAttribute('data-representation', 'fit')
    expect(onDimensions).not.toHaveBeenCalled()

    fireEvent.click(screen.getByRole('button', { name: '放大' }))
    const zoomedTransform = preview.getAttribute('style')
    await act(async () => original.resolve(loaded('original', 6000, 4000)))
    await waitFor(() =>
      expect(preview).toHaveAttribute('src', expect.stringContaining('/original')),
    )
    expect(preview).toHaveAttribute('data-representation', 'original')
    expect(preview.getAttribute('style')).toBe(zoomedTransform)
    expect(onDimensions).toHaveBeenCalledWith('image-1', 6000, 4000)

    fireEvent.click(screen.getByRole('button', { name: '放大镜' }))
    expect(
      request.mock.calls.filter(
        ([, representation]) => representation.kind === 'original100_percent',
      ),
    ).toHaveLength(1)
  })

  it('commits one stable fitted geometry while a small proxy upgrades to the original', async () => {
    const fit = deferred<ImageRepresentation>()
    const original = deferred<ImageRepresentation>()
    const target = image(1)
    const committedSizes: Array<{ width: number; height: number }> = []
    const request = vi.fn((_file: BrowserFile, representation: ImageRepresentationRequest) =>
      representation.kind === 'original100_percent' ? original.promise : fit.promise,
    )
    render(
      <Profiler
        id="stable-progressive-preview"
        onRender={() => {
          const preview = document.querySelector<HTMLElement>('.image-preview-image')
          if (preview) committedSizes.push(visibleImageSize(preview))
        }}
      >
        <ImagePreview
          file={target}
          files={[target]}
          magnifier={MAGNIFIER}
          pointerClientPoint={POINTER_CLIENT_POINT}
          requestImage={request}
          onNavigate={vi.fn()}
          onClose={vi.fn()}
        />
      </Profiler>,
    )

    await act(async () => fit.resolve(loaded('fit', 300, 200)))
    const preview = await screen.findByRole('img', { name: '1.jpg' })
    const fittedSize = visibleImageSize(preview)
    expect(fittedSize).toEqual({ width: 576, height: 384 })
    committedSizes.length = 0

    await act(async () => original.resolve(loaded('original', 6000, 4000)))
    await waitFor(() => expect(preview).toHaveAttribute('data-representation', 'original'))

    expect(committedSizes.length).toBeGreaterThan(0)
    for (const committed of committedSizes) {
      expect(committed.width).toBeCloseTo(fittedSize.width, 6)
      expect(committed.height).toBeCloseTo(fittedSize.height, 6)
    }
  })

  it('waits for a real stage and never commits the 576×384 fallback image', async () => {
    initialPreviewStage = { width: 0, height: 0 }
    const fit = deferred<ImageRepresentation>()
    const committedSizes: Array<{ width: number; height: number }> = []
    const target = image(1)
    const request = vi.fn((_file: BrowserFile, representation: ImageRepresentationRequest) =>
      representation.kind === 'fit_preview'
        ? fit.promise
        : new Promise<ImageRepresentation>(() => undefined),
    )

    const view = render(
      <Profiler
        id="real-stage-preview"
        onRender={() => {
          const preview = document.querySelector<HTMLElement>('.image-preview-image')
          if (preview) committedSizes.push(visibleImageSize(preview))
        }}
      >
        <ImagePreview
          file={target}
          files={[target]}
          magnifier={MAGNIFIER}
          pointerClientPoint={POINTER_CLIENT_POINT}
          requestImage={request}
          onNavigate={vi.fn()}
          onClose={vi.fn()}
        />
      </Profiler>,
    )

    await act(async () => fit.resolve(loaded('fit', 2400, 1600)))
    expect(screen.queryByRole('img', { name: '1.jpg' })).not.toBeInTheDocument()

    const stage = view.container.querySelector('.image-preview-stage') as HTMLElement
    publishPreviewStage(stage, 2048, 1060)
    const preview = await screen.findByRole('img', { name: '1.jpg' })

    const fittedSize = visibleImageSize(preview)
    expect(fittedSize.width).toBeCloseTo(1431, 6)
    expect(fittedSize.height).toBeCloseTo(954, 6)
    expect(committedSizes.length).toBeGreaterThan(0)
    expect(committedSizes).not.toContainEqual({ width: 576, height: 384 })
    expect(committedSizes.every(({ width }) => width > 1000)).toBe(true)
  })

  it('uses fitted display as the only 100% baseline and resets zoom through one control', async () => {
    const files = [image(1), image(2), image(3)]
    const request = vi.fn(
      async (file: BrowserFile, representation: ImageRepresentationRequest) => ({
        cacheKey: `${file.entityId}:${representation.kind}`,
        url: `viewer-image://localhost/session/${file.entityId}-${representation.kind}`,
        width: 1600,
        height: 1200,
        backend: 'image_io' as const,
      }),
    )
    render(
      <ImagePreview
        file={defined(files[1], 'Expected second preview image')}
        files={files}
        magnifier={MAGNIFIER}
        pointerClientPoint={POINTER_CLIENT_POINT}
        requestImage={request}
        onNavigate={vi.fn()}
        onClose={vi.fn()}
      />,
    )

    const preview = await screen.findByRole('img', { name: '2.jpg' })
    expect(screen.getByText('1600 × 1200 px · 200 B')).toBeVisible()
    expect(preview).toHaveAttribute('data-mode', 'fit')
    expect(request.mock.calls.map((call) => call[1].kind)).toContain('original100_percent')
    expect(screen.queryByRole('button', { name: '按 100% 显示' })).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: '放大' }))
    expect(screen.getByText('125%', { selector: '.preview-scale-label' })).toBeVisible()
    expect(screen.getByRole('button', { name: '适应窗口' })).toHaveAttribute(
      'aria-pressed',
      'false',
    )
    fireEvent.click(screen.getByRole('button', { name: '适应窗口' }))
    expect(screen.getByText('100%', { selector: '.preview-scale-label' })).toBeVisible()
    expect(screen.getByRole('button', { name: '适应窗口' })).toHaveAttribute('aria-pressed', 'true')

    fireEvent.click(screen.getByRole('button', { name: '顺时针旋转' }))
    expect(preview.getAttribute('style')).toContain('rotate(90deg) scale(')
    expect(
      request.mock.calls.filter((call) => call[1].kind === 'original100_percent'),
    ).toHaveLength(1)
  })

  it('navigates in stable content order and prefetches no more than three fit images', async () => {
    const files = [image(1), image(2), image(3), image(4)]
    const navigate = vi.fn()
    const request = vi.fn(
      async (file: BrowserFile, _representation: ImageRepresentationRequest) => ({
        cacheKey: file.entityId,
        url: `viewer-image://localhost/session/${file.entityId}`,
        width: 800,
        height: 600,
        backend: 'quick_look' as const,
      }),
    )
    render(
      <ImagePreview
        file={defined(files[1], 'Expected second preview image')}
        files={files}
        magnifier={MAGNIFIER}
        pointerClientPoint={POINTER_CLIENT_POINT}
        requestImage={request}
        onNavigate={navigate}
        onClose={vi.fn()}
      />,
    )
    await screen.findByRole('img', { name: '2.jpg' })

    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'ArrowRight' })

    expect(navigate).toHaveBeenCalledWith(files[2])
    expect(
      request.mock.calls.filter(([, representation]) => representation.kind === 'fit_preview'),
    ).toHaveLength(3)
    expect(
      request.mock.calls.filter(
        ([, representation]) => representation.kind === 'original100_percent',
      ),
    ).toHaveLength(1)
  })

  it('aborts the prior original request when navigation changes the current image', async () => {
    const files = [image(1), image(2)]
    const originals = new Map([
      ['image-1', deferred<ImageRepresentation>()],
      ['image-2', deferred<ImageRepresentation>()],
    ])
    const originalSignals = new Map<string, AbortSignal>()
    const request = vi.fn(
      (file: BrowserFile, representation: ImageRepresentationRequest, signal?: AbortSignal) => {
        if (representation.kind === 'fit_preview')
          return Promise.resolve(loaded(file.entityId, 800, 600))
        if (signal) originalSignals.set(file.entityId, signal)
        return (
          originals.get(file.entityId)?.promise ?? Promise.reject(new Error('missing original'))
        )
      },
    )
    const view = render(
      <ImagePreview
        file={files[0] as BrowserFile}
        files={files}
        magnifier={MAGNIFIER}
        pointerClientPoint={POINTER_CLIENT_POINT}
        requestImage={request}
        onNavigate={vi.fn()}
        onClose={vi.fn()}
      />,
    )
    await waitFor(() => expect(originalSignals.get('image-1')).toBeDefined())

    view.rerender(
      <ImagePreview
        file={files[1] as BrowserFile}
        files={files}
        magnifier={MAGNIFIER}
        pointerClientPoint={POINTER_CLIENT_POINT}
        requestImage={request}
        onNavigate={vi.fn()}
        onClose={vi.fn()}
      />,
    )

    await waitFor(() => expect(originalSignals.get('image-2')).toBeDefined())
    expect(originalSignals.get('image-1')?.aborted).toBe(true)
    expect(originalSignals.get('image-2')?.aborted).toBe(false)
  })

  it('hides a watcher-removed current image without new requests and preserves navigation', async () => {
    const files = [image(1), image(2), image(3)]
    const navigate = vi.fn()
    const request = vi.fn(async (file: BrowserFile) => ({
      cacheKey: file.entityId,
      url: `viewer-image://localhost/session/${file.entityId}`,
      width: 800,
      height: 600,
      backend: 'quick_look' as const,
    }))
    const rendered = render(
      <ImagePreview
        file={defined(files[1], 'Expected second preview image')}
        files={files}
        magnifier={MAGNIFIER}
        pointerClientPoint={POINTER_CLIENT_POINT}
        requestImage={request}
        onNavigate={navigate}
        onClose={vi.fn()}
      />,
    )
    await screen.findByRole('img', { name: '2.jpg' })
    const requestsBeforeRemoval = request.mock.calls.length

    rendered.rerender(
      <ImagePreview
        file={defined(files[1], 'Expected second preview image')}
        files={files}
        magnifier={MAGNIFIER}
        pointerClientPoint={POINTER_CLIENT_POINT}
        unavailableEntityIds={new Set(['image-2'])}
        requestImage={request}
        onNavigate={navigate}
        onClose={vi.fn()}
      />,
    )

    expect(screen.getByLabelText('2.jpg .JPG 文件已不可用')).toBeVisible()
    expect(screen.queryByRole('img', { name: '2.jpg' })).not.toBeInTheDocument()
    expect(request).toHaveBeenCalledTimes(requestsBeforeRemoval)
    expect(screen.getByRole('button', { name: '上一张' })).toBeEnabled()
    expect(screen.getByRole('button', { name: '下一张' })).toBeEnabled()
    fireEvent.click(screen.getByRole('button', { name: '上一张' }))
    fireEvent.click(screen.getByRole('button', { name: '下一张' }))
    expect(navigate.mock.calls.map(([file]) => file.entityId)).toEqual(['image-1', 'image-3'])
  })

  it('toggles the magnifier through the toolbar and strict unmodified Q ownership', async () => {
    const target = image(1)
    const request = vi.fn(async (_file: BrowserFile, request: ImageRepresentationRequest) => ({
      cacheKey: request.kind,
      url: `viewer-image://localhost/session/${request.kind}`,
      width: request.kind === 'original100_percent' ? 1280 : 800,
      height: request.kind === 'original100_percent' ? 960 : 600,
      backend: 'image_io' as const,
    }))
    render(
      <ImagePreview
        file={target}
        files={[target]}
        magnifier={MAGNIFIER}
        pointerClientPoint={POINTER_CLIENT_POINT}
        requestImage={request}
        onNavigate={vi.fn()}
        onClose={vi.fn()}
      />,
    )
    const preview = await screen.findByRole('img', { name: '1.jpg' })
    await waitFor(() => expect(preview).toHaveAttribute('data-representation', 'original'))

    const dialog = screen.getByRole('dialog')
    const button = screen.getByRole('button', { name: '放大镜' })
    expect(button).toHaveAttribute('aria-pressed', 'false')
    expect(button).toHaveAttribute('aria-keyshortcuts', 'Q')
    expect(button).toHaveAttribute('title', '放大镜（Q）')
    await waitFor(() =>
      expect(request).toHaveBeenCalledWith(
        target,
        { kind: 'original100_percent' },
        expect.any(AbortSignal),
      ),
    )

    fireEvent.keyDown(dialog, { key: 'q', metaKey: true })
    fireEvent.keyDown(dialog, { key: 'q', repeat: true })
    fireEvent.keyDown(dialog, { key: 'q', isComposing: true })
    const alreadyHandled = new KeyboardEvent('keydown', {
      key: 'q',
      bubbles: true,
      cancelable: true,
    })
    alreadyHandled.preventDefault()
    dialog.dispatchEvent(alreadyHandled)
    expect(button).toHaveAttribute('aria-pressed', 'false')

    fireEvent.keyDown(dialog, { key: 'q' })
    expect(button).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByText('放大镜已开启')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: '放大' }))
    fireEvent.click(screen.getByRole('button', { name: '适应窗口' }))
    expect(
      request.mock.calls.filter(
        ([, representation]) => representation.kind === 'original100_percent',
      ),
    ).toHaveLength(1)

    fireEvent.keyDown(dialog, { key: 'Q' })
    expect(button).toHaveAttribute('aria-pressed', 'false')
    expect(screen.getByText('放大镜已关闭')).toBeInTheDocument()
    fireEvent.click(button)
    expect(button).toHaveAttribute('aria-pressed', 'true')
  })

  it('shows the pointer-adjacent lens immediately for stationary Q activation', async () => {
    const frames: FrameRequestCallback[] = []
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      frames.push(callback)
      return frames.length
    })
    vi.stubGlobal('cancelAnimationFrame', vi.fn())
    const pointerClientPoint = { current: { x: 320, y: 240 } }
    const target = image(1)
    const request = vi.fn(async (_file: BrowserFile, request: ImageRepresentationRequest) => ({
      cacheKey: request.kind,
      url: `viewer-image://localhost/session/${request.kind}`,
      width: request.kind === 'original100_percent' ? 1280 : 800,
      height: request.kind === 'original100_percent' ? 960 : 600,
      backend: 'image_io' as const,
    }))
    const view = render(
      <ImagePreview
        file={target}
        files={[target]}
        magnifier={MAGNIFIER}
        pointerClientPoint={pointerClientPoint}
        requestImage={request}
        onNavigate={vi.fn()}
        onClose={vi.fn()}
      />,
    )
    const preview = await screen.findByRole('img', { name: '1.jpg' })
    await waitFor(() => expect(preview).toHaveAttribute('data-representation', 'original'))
    const stage = view.container.querySelector('.image-preview-stage') as HTMLElement
    vi.spyOn(stage, 'getBoundingClientRect').mockReturnValue({
      x: 0,
      y: 0,
      left: 0,
      top: 0,
      right: 640,
      bottom: 480,
      width: 640,
      height: 480,
      toJSON: () => undefined,
    })
    const pointerMove = vi.fn()
    stage.addEventListener('pointermove', pointerMove)

    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'q' })
    await waitFor(() => expect(screen.getByTestId('image-magnifier')).toBeInTheDocument())
    act(() => {
      while (frames.length > 0) frames.shift()?.(0)
    })

    const lens = screen.getByTestId('image-magnifier')
    expect(pointerMove).not.toHaveBeenCalled()
    expect(pointerClientPoint.current).toEqual({ x: 320, y: 240 })
    expect(lens).toHaveAttribute('data-visible', 'true')
    expect(lens).toHaveStyle({ '--magnifier-x': '438px', '--magnifier-y': '358px' })
    expect(lens).toHaveStyle({ '--magnifier-scale': '0.9' })

    fireEvent.click(screen.getByRole('button', { name: '放大' }))
    act(() => {
      while (frames.length > 0) frames.shift()?.(0)
    })
    expect(lens).toHaveStyle({ '--magnifier-scale': '1.125' })
    fireEvent.click(screen.getByRole('button', { name: '适应窗口' }))
    act(() => {
      while (frames.length > 0) frames.shift()?.(0)
    })

    pointerClientPoint.current = { x: 2, y: 2 }
    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'q' })
    expect(lens).toBeInTheDocument()
    expect(lens).not.toHaveAttribute('data-visible')
    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'q' })
    act(() => {
      while (frames.length > 0) frames.shift()?.(0)
    })

    expect(screen.getByRole('button', { name: '放大镜' })).toHaveAttribute('aria-pressed', 'true')
    expect(lens).not.toHaveAttribute('data-visible')
  })

  it('hides the lens outside actual image pixels while preserving enabled state across navigation', async () => {
    const files = [image(1), image(2)]
    const request = vi.fn(
      async (file: BrowserFile, representation: ImageRepresentationRequest) => ({
        cacheKey: `${file.entityId}:${representation.kind}`,
        url: `viewer-image://localhost/session/${file.entityId}-${representation.kind}`,
        width: representation.kind === 'original100_percent' ? 6000 : 800,
        height: representation.kind === 'original100_percent' ? 4000 : 600,
        backend: 'image_io' as const,
      }),
    )
    const view = render(
      <ImagePreview
        file={files[0] as BrowserFile}
        files={files}
        magnifier={MAGNIFIER}
        pointerClientPoint={POINTER_CLIENT_POINT}
        requestImage={request}
        onNavigate={vi.fn()}
        onClose={vi.fn()}
      />,
    )
    await screen.findByRole('img', { name: '1.jpg' })
    fireEvent.click(screen.getByRole('button', { name: '放大镜' }))
    await waitFor(() => expect(screen.getByTestId('image-magnifier-source')).toBeInTheDocument())
    const stage = view.container.querySelector('.image-preview-stage') as HTMLElement

    fireEvent.pointerMove(stage, { clientX: 320, clientY: 240, pointerId: 1 })
    expect(stage).toHaveAttribute('data-magnifier-over-image', 'true')
    fireEvent.pointerMove(stage, { clientX: 2, clientY: 2, pointerId: 1 })
    expect(stage).not.toHaveAttribute('data-magnifier-over-image')
    fireEvent.pointerMove(stage, { clientX: 320, clientY: 240, pointerId: 1 })
    expect(stage).toHaveAttribute('data-magnifier-over-image', 'true')
    fireEvent.pointerLeave(stage)
    expect(stage).not.toHaveAttribute('data-magnifier-over-image')
    expect(screen.getByRole('button', { name: '放大镜' })).toHaveAttribute('aria-pressed', 'true')

    view.rerender(
      <ImagePreview
        file={files[1] as BrowserFile}
        files={files}
        magnifier={MAGNIFIER}
        pointerClientPoint={POINTER_CLIENT_POINT}
        requestImage={request}
        onNavigate={vi.fn()}
        onClose={vi.fn()}
      />,
    )
    expect(screen.getByRole('button', { name: '放大镜' })).toHaveAttribute('aria-pressed', 'true')
    await waitFor(() =>
      expect(request).toHaveBeenCalledWith(
        expect.objectContaining({ entityId: 'image-2' }),
        { kind: 'original100_percent' },
        expect.any(AbortSignal),
      ),
    )
  })

  it.each([
    [{ code: 'image_budget_exceeded' }, '原图超出安全预览限制，已继续使用适窗预览。'],
    [new Error('decode failed'), '无法加载原图，已继续使用适窗预览。'],
  ])('keeps the fit preview when automatic original loading fails', async (failure, copy) => {
    const files = [image(1), image(2)]
    const request = vi.fn((file: BrowserFile, representation: ImageRepresentationRequest) =>
      representation.kind === 'original100_percent' && file.entityId === 'image-1'
        ? Promise.reject(failure)
        : Promise.resolve({
            cacheKey: `${file.entityId}:${representation.kind}`,
            url: `viewer-image://localhost/session/${file.entityId}-${representation.kind}`,
            width: representation.kind === 'original100_percent' ? 6000 : 800,
            height: representation.kind === 'original100_percent' ? 4000 : 600,
            backend: 'image_io' as const,
          }),
    )
    const view = render(
      <ImagePreview
        file={files[0] as BrowserFile}
        files={files}
        magnifier={MAGNIFIER}
        pointerClientPoint={POINTER_CLIENT_POINT}
        requestImage={request}
        onNavigate={vi.fn()}
        onClose={vi.fn()}
      />,
    )
    await screen.findByRole('img', { name: '1.jpg' })

    const warning = await screen.findByRole('status')
    expect(warning).toHaveTextContent('正在使用适窗预览')
    expect(warning).toHaveTextContent(copy)
    expect(screen.getByRole('img', { name: '1.jpg' })).toHaveAttribute('data-representation', 'fit')
    expect(screen.getByRole('button', { name: '放大镜' })).toHaveAttribute('aria-pressed', 'false')

    view.rerender(
      <ImagePreview
        file={files[1] as BrowserFile}
        files={files}
        magnifier={MAGNIFIER}
        pointerClientPoint={POINTER_CLIENT_POINT}
        requestImage={request}
        onNavigate={vi.fn()}
        onClose={vi.fn()}
      />,
    )
    await waitFor(() =>
      expect(screen.getByTestId('image-magnifier-source')).toHaveAttribute(
        'src',
        expect.stringContaining('image-2-original100_percent'),
      ),
    )
  })

  it('routes pinch, two-axis pan, and toolbar zoom through one viewport', async () => {
    const frames: FrameRequestCallback[] = []
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      frames.push(callback)
      return frames.length
    })
    const target = image(1)
    const navigate = vi.fn()
    const request = vi.fn(async () => ({
      cacheKey: 'fit',
      url: 'viewer-image://localhost/session/fit',
      width: 800,
      height: 600,
      backend: 'image_io' as const,
    }))
    const view = render(
      <ImagePreview
        file={target}
        files={[target]}
        magnifier={MAGNIFIER}
        pointerClientPoint={POINTER_CLIENT_POINT}
        requestImage={request}
        onNavigate={navigate}
        onClose={vi.fn()}
      />,
    )
    const preview = await screen.findByRole('img', { name: '1.jpg' })
    const stage = view.container.querySelector('.image-preview-stage') as HTMLElement
    vi.spyOn(stage, 'getBoundingClientRect').mockReturnValue({
      x: 0,
      y: 0,
      left: 0,
      top: 0,
      right: 640,
      bottom: 480,
      width: 640,
      height: 480,
      toJSON: () => undefined,
    })

    const pinch = new WheelEvent('wheel', {
      bubbles: true,
      cancelable: true,
      ctrlKey: true,
      clientX: 320,
      clientY: 240,
      deltaY: -200,
    })
    stage.dispatchEvent(pinch)
    expect(pinch.defaultPrevented).toBe(true)
    act(() => frames.shift()?.(0))
    expect(preview).toHaveAttribute('data-mode', 'free')
    expect(screen.getByText('223%')).toBeVisible()

    fireEvent.click(screen.getByRole('button', { name: '放大' }))
    expect(screen.getByText('278%')).toBeVisible()
    const pan = new WheelEvent('wheel', {
      bubbles: true,
      cancelable: true,
      deltaX: 12,
      deltaY: -18,
    })
    stage.dispatchEvent(pan)
    act(() => frames.shift()?.(16))
    expect(preview.getAttribute('style')).toContain('translate(-12px, 18px)')
    expect(navigate).not.toHaveBeenCalled()
    vi.unstubAllGlobals()
  })

  it('updates visible scale immediately but announces only the settled percentage', async () => {
    const target = image(1)
    const request = vi.fn(async () => ({
      cacheKey: 'fit',
      url: 'viewer-image://localhost/session/fit',
      width: 800,
      height: 600,
      backend: 'image_io' as const,
    }))
    render(
      <ImagePreview
        file={target}
        files={[target]}
        magnifier={MAGNIFIER}
        pointerClientPoint={POINTER_CLIENT_POINT}
        requestImage={request}
        onNavigate={vi.fn()}
        onClose={vi.fn()}
      />,
    )
    await screen.findByRole('img', { name: '1.jpg' })
    vi.useFakeTimers()

    try {
      const live = screen.getByTestId('preview-scale-announcement')
      expect(live).toHaveTextContent('缩放比例 100%')

      fireEvent.click(screen.getByRole('button', { name: '放大' }))
      fireEvent.click(screen.getByRole('button', { name: '放大' }))
      expect(screen.getByText('156%', { selector: '.preview-scale-label' })).toBeVisible()
      expect(live).toHaveTextContent('缩放比例 100%')

      act(() => vi.advanceTimersByTime(299))
      expect(live).toHaveTextContent('缩放比例 100%')
      act(() => vi.advanceTimersByTime(1))
      expect(live).toHaveTextContent('缩放比例 156%')
    } finally {
      vi.useRealTimers()
    }
  })
})
