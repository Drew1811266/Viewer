import { act, render, screen, waitFor } from '@testing-library/react'
import { useRef } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import type {
  VideoEvent,
  VideoFile,
  VideoMedia,
  VideoMetadata,
  VideoSession,
} from '../../api/types'
import type { ViewerBridge } from '../../api/viewer'
import { useVideoBridge, type VideoPreviewBridge } from './useVideoBridge'

const MEDIA: VideoMedia = {
  durationUs: 12_000_000,
  displayWidth: 1_920,
  displayHeight: 1_080,
  rotationDegrees: 0,
}

afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe('useVideoBridge', () => {
  it('subscribes before open and replays same-generation events emitted before open resolves', async () => {
    installResizeObserver()
    const harness = videoBridgeHarness()
    harness.open.mockImplementation(async (request) => {
      harness.order.push(`open:${request.entityId.replace('video-', '')}`)
      harness.emit({ type: 'firstFrameReady', generation: 1 })
      return session(1)
    })

    render(<BridgeHarness file={video('a.mp4')} bridge={harness.bridge} />)

    await waitFor(() =>
      expect(screen.getByRole('status', { name: 'bridge state' })).toHaveTextContent('1:visible'),
    )
    expect(harness.order.slice(0, 2)).toEqual(['listen', 'open:a.mp4'])
    expect(harness.open).toHaveBeenCalledWith({
      entityId: 'video-a.mp4',
      surfaceRect: { x: 100, y: 125, width: 800, height: 450 },
    })
  })

  it('ignores late events from the previous file and closes each opened generation once', async () => {
    installResizeObserver()
    const harness = videoBridgeHarness()
    harness.open
      .mockResolvedValueOnce(session(1))
      .mockResolvedValueOnce(session(2, { rotationDegrees: 90 }))
    const rendered = render(<BridgeHarness file={video('a.mp4')} bridge={harness.bridge} />)
    await waitFor(() => expect(harness.open).toHaveBeenCalledTimes(1))
    const firstListener = harness.listeners[0]

    rendered.rerender(<BridgeHarness file={video('b.mp4')} bridge={harness.bridge} />)
    await waitFor(() => expect(harness.open).toHaveBeenCalledTimes(2))
    firstListener?.({ type: 'firstFrameReady', generation: 1 })
    expect(screen.getByRole('status', { name: 'bridge state' })).toHaveTextContent('2:hidden')
    expect(harness.close).toHaveBeenCalledTimes(1)
    expect(harness.close).toHaveBeenNthCalledWith(1, { generation: 1 })
    expect(harness.unlisten).toHaveBeenCalledTimes(1)

    rendered.unmount()
    await waitFor(() => expect(harness.close).toHaveBeenCalledTimes(2))
    expect(harness.close).toHaveBeenNthCalledWith(2, { generation: 2 })
    expect(harness.unlisten).toHaveBeenCalledTimes(2)
  })

  it('updates fitted surface geometry after resize without reopening media', async () => {
    const resize = installResizeObserver()
    const harness = videoBridgeHarness()
    harness.open.mockResolvedValue(session(7))
    render(<BridgeHarness file={video('a.mp4')} bridge={harness.bridge} />)
    const stage = screen.getByTestId('video-stage')

    await waitFor(() => expect(harness.setRect).toHaveBeenCalledTimes(1))
    expect(harness.setRect).toHaveBeenLastCalledWith({
      generation: 7,
      x: 100,
      y: 125,
      width: 800,
      height: 450,
    })

    resize.trigger(stage, { left: 120, top: 80, width: 400, height: 400 })

    await waitFor(() => expect(harness.setRect).toHaveBeenCalledTimes(2))
    expect(harness.setRect).toHaveBeenLastCalledWith({
      generation: 7,
      x: 120,
      y: 168,
      width: 400,
      height: 225,
    })
    expect(harness.open).toHaveBeenCalledTimes(1)
  })

  it('keeps the shell retryable when videoOpen rejects', async () => {
    installResizeObserver()
    const harness = videoBridgeHarness()
    harness.open.mockRejectedValue({ code: 'decoder_missing', retryable: true })

    render(<BridgeHarness file={video('broken.mp4')} bridge={harness.bridge} />)

    await waitFor(() =>
      expect(screen.getByRole('status', { name: 'bridge state' })).toHaveTextContent(
        '0:hidden:failed:decoder_missing',
      ),
    )
    expect(harness.close).not.toHaveBeenCalled()
    expect(harness.unlisten).toHaveBeenCalledOnce()
  })

  it('fails explicitly when indexed probe metadata is terminal without dimensions', async () => {
    installResizeObserver()
    const harness = videoBridgeHarness()

    render(
      <BridgeHarness
        file={video('damaged.mp4', {
          displayWidth: null,
          displayHeight: null,
          probeStatus: 'failed',
          failureKind: 'damaged',
        })}
        bridge={harness.bridge}
      />,
    )

    await waitFor(() =>
      expect(screen.getByRole('status', { name: 'bridge state' })).toHaveTextContent(
        '0:hidden:failed:damaged',
      ),
    )
    expect(harness.open).not.toHaveBeenCalled()
    expect(harness.bridge.listenVideo).not.toHaveBeenCalled()
  })

  it('fails explicitly when ready indexed metadata has no display dimensions', async () => {
    installResizeObserver()
    const harness = videoBridgeHarness()

    render(
      <BridgeHarness
        file={video('dimensionless.mp4', {
          displayWidth: null,
          displayHeight: null,
          probeStatus: 'ready',
        })}
        bridge={harness.bridge}
      />,
    )

    await waitFor(() =>
      expect(screen.getByRole('status', { name: 'bridge state' })).toHaveTextContent(
        '0:hidden:failed:video_geometry_unavailable',
      ),
    )
    expect(harness.open).not.toHaveBeenCalled()
    expect(harness.bridge.listenVideo).not.toHaveBeenCalled()
  })

  it('starts the lifecycle when pending indexed metadata becomes ready', async () => {
    installResizeObserver()
    const harness = videoBridgeHarness()
    const rendered = render(
      <BridgeHarness
        file={video('pending.mp4', {
          displayWidth: null,
          displayHeight: null,
          probeStatus: 'pending',
        })}
        bridge={harness.bridge}
      />,
    )

    expect(harness.open).not.toHaveBeenCalled()
    rendered.rerender(<BridgeHarness file={video('pending.mp4')} bridge={harness.bridge} />)

    await waitFor(() => expect(harness.open).toHaveBeenCalledOnce())
  })

  it('terminates the active generation when initial surface geometry is rejected', async () => {
    installResizeObserver()
    const harness = videoBridgeHarness()
    harness.open.mockResolvedValue(session(7))
    harness.setRect.mockRejectedValue(new Error('surface unavailable'))
    const rendered = render(<BridgeHarness file={video('a.mp4')} bridge={harness.bridge} />)

    await waitFor(() =>
      expect(screen.getByRole('status', { name: 'bridge state' })).toHaveTextContent(
        '7:hidden:failed:video_surface_failed',
      ),
    )
    expect(harness.close).toHaveBeenCalledOnce()
    expect(harness.close).toHaveBeenCalledWith({ generation: 7 })
    expect(harness.unlisten).toHaveBeenCalledOnce()

    rendered.unmount()
    expect(harness.close).toHaveBeenCalledOnce()
  })

  it('closes a revealed surface on geometry failure and ignores later native state', async () => {
    installResizeObserver()
    const harness = videoBridgeHarness()
    const surface = deferred<void>()
    harness.open.mockResolvedValue(session(8))
    harness.setRect.mockReturnValue(surface.promise)
    render(<BridgeHarness file={video('a.mp4')} bridge={harness.bridge} />)

    await waitFor(() => expect(harness.setRect).toHaveBeenCalledOnce())
    act(() => harness.emit({ type: 'firstFrameReady', generation: 8 }))
    expect(screen.getByRole('status', { name: 'bridge state' })).toHaveTextContent('8:visible')

    surface.reject(new Error('surface lost'))
    await waitFor(() =>
      expect(screen.getByRole('status', { name: 'bridge state' })).toHaveTextContent(
        '8:hidden:failed:video_surface_failed',
      ),
    )
    expect(harness.close).toHaveBeenCalledOnce()
    expect(harness.close).toHaveBeenCalledWith({ generation: 8 })
    expect(harness.unlisten).toHaveBeenCalledOnce()

    harness.emit({ type: 'stateChanged', generation: 8, state: 'playing' })
    expect(screen.getByRole('status', { name: 'bridge state' })).toHaveTextContent(
      '8:hidden:failed:video_surface_failed',
    )
  })
})

function BridgeHarness({
  bridge,
  file,
  retryKey = 0,
}: {
  bridge: VideoPreviewBridge
  file: VideoFile
  retryKey?: number
}) {
  const stage = useRef<HTMLDivElement>(null)
  const { state } = useVideoBridge({ bridge, file, retryKey, stage })
  return (
    <>
      <div ref={stage} data-testid="video-stage" />
      <output aria-label="bridge state">
        {state.generation}:{state.surfaceVisible ? 'visible' : 'hidden'}
        {state.phase === 'failed' ? `:failed:${state.error?.code}` : ''}
      </output>
    </>
  )
}

function video(name: string, metadata: Partial<VideoMetadata> = {}): VideoFile {
  return {
    entityId: `video-${name}`,
    relativePath: `id/${name}`,
    name,
    kind: 'video',
    size: 200,
    modifiedNs: '2',
    marker: { reviewState: null, favorite: false },
    imageMetadata: null,
    imageUrl: null,
    videoMetadata: {
      durationUs: MEDIA.durationUs,
      displayWidth: MEDIA.displayWidth,
      displayHeight: MEDIA.displayHeight,
      rotationDegrees: MEDIA.rotationDegrees,
      frameRateMillihertz: 30_000,
      videoCodec: 'h264',
      audioCodec: 'aac',
      probeStatus: 'ready',
      failureKind: null,
      coverUrl: 'viewer-image://localhost/session/cover',
      ...metadata,
    },
  }
}

function session(generation: number, overrides: Partial<VideoMedia> = {}): VideoSession {
  return {
    generation,
    sessionId: `session-${generation}`,
    media: { ...MEDIA, ...overrides },
  }
}

function videoBridgeHarness() {
  const order: string[] = []
  const listeners: Array<(event: VideoEvent) => void> = []
  const open = vi.fn<ViewerBridge['videoOpen']>((request) => {
    order.push(`open:${request.entityId.replace('video-', '')}`)
    return Promise.resolve(session(1))
  })
  const close = vi.fn<ViewerBridge['videoClose']>().mockResolvedValue(undefined)
  const setRect = vi.fn<ViewerBridge['videoSetSurfaceRect']>().mockResolvedValue(undefined)
  const unlisten = vi.fn()
  const listen = vi.fn<ViewerBridge['listenVideo']>(async (listener) => {
    order.push('listen')
    listeners.push(listener)
    return unlisten
  })
  const bridge: VideoPreviewBridge = {
    videoOpen: open,
    videoClose: close,
    videoSetSurfaceRect: setRect,
    listenVideo: listen,
  }
  return {
    bridge,
    close,
    emit(event: VideoEvent) {
      listeners.at(-1)?.(event)
    },
    listeners,
    open,
    order,
    setRect,
    unlisten,
  }
}

function installResizeObserver() {
  const callbacks = new Map<Element, ResizeObserverCallback>()
  const rectangles = new Map<Element, DOMRectReadOnly>()
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (
    this: HTMLElement,
  ) {
    return (rectangles.get(this) ??
      rect({ left: 100, top: 50, width: 800, height: 600 })) as DOMRect
  })
  class Observer {
    private readonly callback: ResizeObserverCallback

    constructor(callback: ResizeObserverCallback) {
      this.callback = callback
    }
    observe(node: Element) {
      callbacks.set(node, this.callback)
    }
    disconnect() {}
  }
  vi.stubGlobal('ResizeObserver', Observer)
  return {
    trigger(node: Element, next: { left: number; top: number; width: number; height: number }) {
      const bounds = rect(next)
      rectangles.set(node, bounds)
      act(() => {
        callbacks.get(node)?.(
          [{ target: node, contentRect: bounds } as ResizeObserverEntry],
          {} as ResizeObserver,
        )
      })
    },
  }
}

function rect({
  left,
  top,
  width,
  height,
}: {
  left: number
  top: number
  width: number
  height: number
}): DOMRectReadOnly {
  return {
    x: left,
    y: top,
    left,
    top,
    right: left + width,
    bottom: top + height,
    width,
    height,
    toJSON: () => undefined,
  }
}

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, reject, resolve }
}
