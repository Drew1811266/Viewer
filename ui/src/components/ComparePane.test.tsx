import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type { BrowserFile, ImageRepresentation, ImageRepresentationRequest } from '../api/types'
import { defined } from '../defined'
import type { PaneTransform } from '../state/compareModel'
import ComparePane from './ComparePane'

const file = image('a', 'front.jpg', 'keep', true)
const transform: PaneTransform = {
  scale: 1,
  centerX: 0.5,
  centerY: 0.5,
  rotation: 0,
}

afterEach(() => {
  vi.unstubAllGlobals()
  Object.defineProperty(window, 'devicePixelRatio', { value: 1, configurable: true })
})

describe('ComparePane', () => {
  it('uses a named remove icon and keeps local loading feedback inside the stable stage', () => {
    installResizeObserver()
    const rendered = renderPane({
      requestImage: vi.fn(() => new Promise<ImageRepresentation>(() => undefined)),
    })

    const remove = screen.getByRole('button', { name: '移除 front.jpg' })
    expect(remove).toHaveClass('viewer-icon-button')
    expect(remove.querySelector('.viewer-icon')).toHaveAttribute('aria-hidden', 'true')
    const stage = rendered.container.querySelector<HTMLElement>('.compare-pane-stage')
    expect(withinStage(stage).getByRole('status')).toHaveClass('viewer-local-feedback')
  })

  it('renders an unsupported image placeholder without issuing image requests', () => {
    const resize = installResizeObserver()
    const requestImage = vi.fn(() => new Promise<never>(() => undefined))
    const unsupported = {
      ...file,
      relativePath: 'id/front.cr2',
      name: 'front.cr2',
      kind: 'unsupported_image' as const,
    }

    renderPane({ file: unsupported, requestImage })
    act(() => resize(800, 600))

    expect(screen.getByLabelText('front.cr2 .CR2 暂不支持预览')).toBeVisible()
    expect(screen.getByRole('group', { name: '对比 front.cr2' })).toBeVisible()
    expect(screen.getByRole('button', { name: '移除 front.cr2' })).toBeEnabled()
    expect(requestImage).not.toHaveBeenCalled()
  })

  it('requests a display-scale proxy from the measured viewport', async () => {
    const resize = installResizeObserver()
    Object.defineProperty(window, 'devicePixelRatio', { value: 2, configurable: true })
    const requestImage = vi.fn().mockResolvedValue(representation('proxy-a', 1_600, 1_200))

    renderPane({ requestImage })
    act(() => resize(800, 600))

    await waitFor(() =>
      expect(requestImage).toHaveBeenCalledWith(
        file,
        {
          kind: 'fit_preview',
          maxWidth: 800,
          maxHeight: 600,
          scaleMilli: 2_000,
        },
        expect.any(AbortSignal),
      ),
    )
    expect(await screen.findByRole('img', { name: 'front.jpg' })).toHaveAttribute(
      'src',
      'viewer-image://localhost/proxy-a',
    )
  })

  it('keeps the proxy and reports a bounded-memory downgrade when original fails', async () => {
    const resize = installResizeObserver()
    const requestImage = vi
      .fn()
      .mockResolvedValueOnce(representation('proxy-a', 800, 600))
      .mockRejectedValueOnce({ code: 'image_budget_exceeded' })
    const originalUnavailable = vi.fn()
    const rendered = renderPane({ requestImage, onOriginalUnavailable: originalUnavailable })
    act(() => resize(800, 600))
    await screen.findByRole('img', { name: 'front.jpg' })

    rendered.rerender(
      pane({
        requestImage,
        useOriginal: true,
        onOriginalUnavailable: originalUnavailable,
      }),
    )

    expect(await screen.findByRole('alert')).toHaveTextContent('原图超出安全预览限制')
    expect(screen.getByRole('alert')).toHaveClass('viewer-local-feedback')
    expect(screen.getByRole('alert').closest('.compare-pane-stage')).not.toBeNull()
    expect(screen.getByRole('img', { name: 'front.jpg' })).toHaveAttribute(
      'src',
      'viewer-image://localhost/proxy-a',
    )
    expect(originalUnavailable).toHaveBeenCalledWith('a', 'budget')
  })

  it('requests the original only for 100% mode and reports normalized pointer pan', async () => {
    const resize = installResizeObserver()
    const requestImage = vi
      .fn()
      .mockResolvedValueOnce(representation('proxy-a', 800, 600))
      .mockResolvedValueOnce(representation('original-a', 4_000, 3_000))
    const pan = vi.fn()
    const rendered = renderPane({ requestImage, onPan: pan })
    act(() => resize(800, 600))
    await screen.findByRole('img', { name: 'front.jpg' })

    const stage = defined(
      document.querySelector('.compare-pane-stage'),
      'Expected compare pane stage',
    )
    expect(stage).not.toBeNull()
    fireEvent.pointerDown(stage, { button: 0, pointerId: 1, clientX: 200, clientY: 200 })
    fireEvent.pointerMove(stage, { pointerId: 1, clientX: 280, clientY: 260 })
    expect(pan).toHaveBeenCalledWith('a', -0.1, -0.1)

    rendered.rerender(pane({ requestImage, useOriginal: true, onPan: pan }))
    await waitFor(() =>
      expect(requestImage).toHaveBeenLastCalledWith(
        file,
        {
          kind: 'original100_percent',
        },
        expect.any(AbortSignal),
      ),
    )
    expect(await screen.findByRole('img', { name: 'front.jpg' })).toHaveAttribute(
      'src',
      'viewer-image://localhost/original-a',
    )
  })

  it('does not decode the same pane again when only its marker changes', async () => {
    const resize = installResizeObserver()
    const requestImage = vi.fn().mockResolvedValue(representation('proxy-a', 800, 600))
    const rendered = renderPane({ requestImage })
    act(() => resize(800, 600))
    await screen.findByRole('img', { name: 'front.jpg' })

    rendered.rerender(
      pane({
        requestImage,
        file: { ...file, marker: { reviewState: 'reject', favorite: true } },
      }),
    )
    expect(requestImage).toHaveBeenCalledTimes(1)
  })

  it('restores the retained proxy immediately when an original pane is released', async () => {
    const resize = installResizeObserver()
    const requestImage = vi
      .fn()
      .mockResolvedValueOnce(representation('proxy-a', 800, 600))
      .mockResolvedValueOnce(representation('original-a', 4_000, 3_000))
    const rendered = renderPane({ requestImage })
    act(() => resize(800, 600))
    await screen.findByRole('img', { name: 'front.jpg' })

    rendered.rerender(pane({ requestImage, useOriginal: true }))
    await waitFor(() =>
      expect(screen.getByRole('img', { name: 'front.jpg' })).toHaveAttribute(
        'src',
        'viewer-image://localhost/original-a',
      ),
    )
    rendered.rerender(pane({ requestImage, useOriginal: false }))

    expect(screen.getByRole('img', { name: 'front.jpg' })).toHaveAttribute(
      'src',
      'viewer-image://localhost/proxy-a',
    )
    expect(requestImage).toHaveBeenCalledTimes(2)
  })

  it('maps pointer movement to rendered pixels after zoom and rotation', async () => {
    const resize = installResizeObserver()
    const pan = vi.fn()
    renderPane({
      requestImage: vi.fn().mockResolvedValue(representation('proxy-a', 800, 600)),
      transform: {
        scale: 2,
        centerX: 0.6,
        centerY: 0.4,
        rotation: 90,
      },
      onPan: pan,
    })
    act(() => resize(800, 600))
    const image = await screen.findByRole('img', { name: 'front.jpg' })
    expect(image).toHaveStyle({
      width: '600px',
      height: '450px',
      transform: 'translate(-90px, 120px) rotate(90deg) scale(2)',
    })

    const stage = defined(
      document.querySelector('.compare-pane-stage'),
      'Expected compare pane stage',
    )
    fireEvent.pointerDown(stage, { button: 0, pointerId: 1, clientX: 200, clientY: 200 })
    fireEvent.pointerMove(stage, { pointerId: 1, clientX: 290, clientY: 320 })
    expect(pan).toHaveBeenCalledWith('a', -0.1, -0.1)
  })

  it('swaps viewport constraints for quarter-turn proxies without upscaling them', async () => {
    const resize = installResizeObserver()
    const vertical = {
      ...file,
      imageMetadata: { width: 1_000, height: 4_000 },
    }
    const requestImage = vi.fn().mockResolvedValue(representation('vertical', 200, 800))
    const rendered = renderPane({
      file: vertical,
      requestImage,
      transform: { ...transform, rotation: 90 },
    })
    act(() => resize(1_000, 200))

    const image = await screen.findByRole('img', { name: 'front.jpg' })
    expect(requestImage).toHaveBeenLastCalledWith(
      vertical,
      {
        kind: 'fit_preview',
        maxWidth: 200,
        maxHeight: 1_000,
        scaleMilli: 1_000,
      },
      expect.any(AbortSignal),
    )
    expect(image).toHaveStyle({
      width: '200px',
      height: '800px',
      transform: 'translate(0px, 0px) rotate(90deg) scale(1)',
    })

    rendered.rerender(pane({ file: vertical, requestImage, transform }))
    await waitFor(() =>
      expect(requestImage).toHaveBeenLastCalledWith(
        vertical,
        {
          kind: 'fit_preview',
          maxWidth: 1_000,
          maxHeight: 200,
          scaleMilli: 1_000,
        },
        expect.any(AbortSignal),
      ),
    )
  })

  it('invalidates a proxy when the same entity source revision changes', async () => {
    const resize = installResizeObserver()
    const replacement = deferred<ImageRepresentation>()
    const requestImage = vi
      .fn()
      .mockResolvedValueOnce(representation('old-proxy', 640, 480))
      .mockReturnValueOnce(replacement.promise)
    const rendered = renderPane({ requestImage })
    act(() => resize(640, 480))
    await screen.findByRole('img', { name: 'front.jpg' })

    rendered.rerender(
      pane({
        requestImage,
        file: { ...file, modifiedNs: '2', size: 101 },
      }),
    )
    await waitFor(() => expect(requestImage).toHaveBeenCalledTimes(2))
    expect(screen.queryByRole('img', { name: 'front.jpg' })).not.toBeInTheDocument()
    await act(async () => replacement.resolve(representation('new-proxy', 640, 480)))
    expect(await screen.findByRole('img', { name: 'front.jpg' })).toHaveAttribute(
      'src',
      'viewer-image://localhost/new-proxy',
    )
  })

  it('ignores a stale representation after the pane entity changes', async () => {
    const resize = installResizeObserver()
    const first = deferred<ImageRepresentation>()
    const second = deferred<ImageRepresentation>()
    const requestImage = vi
      .fn()
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise)
    const rendered = renderPane({ requestImage })
    act(() => resize(640, 480))
    await waitFor(() => expect(requestImage).toHaveBeenCalledTimes(1))

    rendered.rerender(pane({ requestImage, file: image('b', 'back.jpg', null, false) }))
    await waitFor(() => expect(requestImage).toHaveBeenCalledTimes(2))
    await act(async () => first.resolve(representation('stale-a', 640, 480)))
    expect(screen.queryByRole('img')).not.toBeInTheDocument()
    await act(async () => second.resolve(representation('proxy-b', 640, 480)))
    expect(await screen.findByRole('img', { name: 'back.jpg' })).toHaveAttribute(
      'src',
      'viewer-image://localhost/proxy-b',
    )
  })

  it('does not surface AbortError rejections for proxy or original requests', async () => {
    const resize = installResizeObserver()
    const originalUnavailable = vi.fn()
    const requestImage = vi.fn(() =>
      Promise.reject(new DOMException('request cancelled', 'AbortError')),
    )
    renderPane({
      requestImage,
      useOriginal: true,
      onOriginalUnavailable: originalUnavailable,
    })
    act(() => resize(640, 480))

    await waitFor(() => expect(requestImage).toHaveBeenCalledTimes(2))
    await act(async () => Promise.resolve())
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(originalUnavailable).not.toHaveBeenCalled()
  })

  it('aborts pending proxy and original requests without reporting cleanup as a load failure', async () => {
    const resize = installResizeObserver()
    const originalUnavailable = vi.fn()
    const signals = new Map<string, AbortSignal>()
    const requestImage = vi.fn(
      (
        _file: BrowserFile,
        request: ImageRepresentationRequest,
        signal?: AbortSignal,
      ): Promise<ImageRepresentation> => {
        if (signal === undefined) throw new Error('Expected an abort signal')
        signals.set(request.kind, signal)
        return new Promise<ImageRepresentation>((_resolve, reject) => {
          signal.addEventListener(
            'abort',
            () => reject(new DOMException('request cancelled', 'AbortError')),
            { once: true },
          )
        })
      },
    )
    const rendered = renderPane({
      requestImage,
      useOriginal: true,
      onOriginalUnavailable: originalUnavailable,
    })
    act(() => resize(640, 480))
    await waitFor(() => expect(requestImage).toHaveBeenCalledTimes(2))

    rendered.unmount()
    await act(async () => Promise.resolve())

    expect(signals.get('fit_preview')?.aborted).toBe(true)
    expect(signals.get('original100_percent')?.aborted).toBe(true)
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(originalUnavailable).not.toHaveBeenCalled()
  })

  it('targets pane-local markers without changing selection and disables writes in read-only mode', () => {
    installResizeObserver()
    const setReview = vi.fn()
    const toggleFavorite = vi.fn()
    const rendered = renderPane({
      requestImage: vi.fn(() => new Promise<ImageRepresentation>(() => undefined)),
      onSetReview: setReview,
      onToggleFavorite: toggleFavorite,
    })

    fireEvent.click(screen.getByRole('button', { name: 'front.jpg 标记为淘汰' }))
    fireEvent.click(screen.getByRole('button', { name: 'front.jpg 切换收藏' }))
    expect(setReview).toHaveBeenCalledWith('a', 'reject')
    expect(toggleFavorite).toHaveBeenCalledWith('a')

    rendered.rerender(
      pane({
        requestImage: vi.fn(() => new Promise<ImageRepresentation>(() => undefined)),
        readOnly: true,
        onSetReview: setReview,
        onToggleFavorite: toggleFavorite,
      }),
    )
    expect(screen.getByRole('button', { name: 'front.jpg 标记为保留' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'front.jpg 切换收藏' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '移除 front.jpg' })).toBeEnabled()
  })
})

function renderPane(overrides: Partial<React.ComponentProps<typeof ComparePane>>) {
  return render(pane(overrides))
}

function withinStage(stage: HTMLElement | null) {
  return within(defined(stage, 'Expected stable compare stage'))
}

function pane(overrides: Partial<React.ComponentProps<typeof ComparePane>>) {
  return (
    <ComparePane
      file={file}
      transform={transform}
      active
      useOriginal={false}
      readOnly={false}
      requestImage={vi.fn()}
      onActivate={vi.fn()}
      onMetrics={vi.fn()}
      onPan={vi.fn()}
      onRemove={vi.fn()}
      onSetReview={vi.fn()}
      onToggleFavorite={vi.fn()}
      onOriginalUnavailable={vi.fn()}
      {...overrides}
    />
  )
}

function image(
  entityId: string,
  name: string,
  reviewState: 'keep' | 'pending' | 'reject' | null,
  favorite: boolean,
): BrowserFile {
  return {
    entityId,
    relativePath: `id/${name}`,
    name,
    kind: 'jpeg',
    size: 100,
    modifiedNs: '1',
    marker: { reviewState, favorite },
    imageMetadata: { width: 4_000, height: 3_000 },
    imageUrl: null,
  }
}

function representation(token: string, width: number, height: number): ImageRepresentation {
  return {
    cacheKey: token,
    url: `viewer-image://localhost/${token}`,
    width,
    height,
    backend: 'image_io',
  }
}

function installResizeObserver() {
  let callback: ResizeObserverCallback | undefined
  class Observer {
    constructor(next: ResizeObserverCallback) {
      callback = next
    }
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  vi.stubGlobal('ResizeObserver', Observer)
  return (width: number, height: number) => {
    callback?.([{ contentRect: { width, height } } as ResizeObserverEntry], {} as ResizeObserver)
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((next) => {
    resolve = next
  })
  return { promise, resolve }
}
