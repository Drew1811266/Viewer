import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
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
      attemptId: expect.any(String),
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
    const animation = installAnimationFrameHarness()
    const harness = videoBridgeHarness()
    harness.open.mockResolvedValue(session(7))
    render(<BridgeHarness file={video('a.mp4')} bridge={harness.bridge} />)
    const stage = screen.getByTestId('video-stage')

    await waitFor(() => expect(harness.setRect).toHaveBeenCalledTimes(1))
    expect(harness.setRect).toHaveBeenLastCalledWith({
      generation: 7,
      sequence: 1,
      x: 100,
      y: 125,
      width: 800,
      height: 450,
    })

    resize.trigger(stage, { left: 120, top: 80, width: 400, height: 400 })
    animation.flush()

    await waitFor(() => expect(harness.setRect).toHaveBeenCalledTimes(2))
    expect(harness.setRect).toHaveBeenLastCalledWith({
      generation: 7,
      sequence: 2,
      x: 120,
      y: 168,
      width: 400,
      height: 225,
    })
    expect(harness.open).toHaveBeenCalledTimes(1)
  })

  it('coalesces resize bursts to the latest fitted rect and keeps the active video alive', async () => {
    const resize = installResizeObserver()
    const animation = installAnimationFrameHarness()
    const harness = videoBridgeHarness()
    const inFlight = deferred<void>()
    harness.open.mockResolvedValue(session(9))
    harness.setRect
      .mockResolvedValueOnce(undefined)
      .mockReturnValueOnce(inFlight.promise)
      .mockResolvedValue(undefined)
    render(<BridgeHarness file={video('a.mp4')} bridge={harness.bridge} />)
    const stage = screen.getByTestId('video-stage')

    await waitFor(() => expect(harness.setRect).toHaveBeenCalledOnce())

    resize.trigger(stage, { left: 104, top: 60, width: 760, height: 560 })
    resize.trigger(stage, { left: 108, top: 70, width: 680, height: 500 })
    resize.trigger(stage, { left: 112, top: 80, width: 600, height: 440 })

    expect(harness.setRect).toHaveBeenCalledOnce()
    animation.flush()
    await waitFor(() => expect(harness.setRect).toHaveBeenCalledTimes(2))

    resize.trigger(stage, { left: 120, top: 90, width: 520, height: 380 })
    resize.trigger(stage, { left: 128, top: 100, width: 440, height: 320 })
    animation.flush()
    expect(harness.setRect).toHaveBeenCalledTimes(3)

    inFlight.reject(new Error('transient resize mismatch'))
    await waitFor(() => expect(harness.setRect).toHaveBeenCalledTimes(3))
    expect(harness.setRect).toHaveBeenLastCalledWith({
      generation: 9,
      sequence: 3,
      x: 128,
      y: 136,
      width: 440,
      height: 248,
    })
    act(() => harness.emit({ type: 'firstFrameReady', generation: 9 }))
    expect(screen.getByRole('status', { name: 'bridge state' })).toHaveTextContent('9:visible')
    expect(harness.close).not.toHaveBeenCalled()
  })

  it('ignores a transient zero-sized stage while the window is being resized', async () => {
    const resize = installResizeObserver()
    const animation = installAnimationFrameHarness()
    const harness = videoBridgeHarness()
    harness.open.mockResolvedValue(session(10))
    render(<BridgeHarness file={video('a.mp4')} bridge={harness.bridge} />)
    const stage = screen.getByTestId('video-stage')

    await waitFor(() => expect(harness.open).toHaveBeenCalledOnce())
    act(() => harness.emit({ type: 'firstFrameReady', generation: 10 }))
    expect(screen.getByRole('status', { name: 'bridge state' })).toHaveTextContent('10:visible')
    resize.trigger(stage, { left: 0, top: 0, width: 0, height: 0 })
    animation.flush()
    await act(async () => Promise.resolve())

    expect(harness.open).toHaveBeenCalledOnce()
    expect(harness.close).not.toHaveBeenCalled()
    expect(screen.getByRole('status', { name: 'bridge state' })).toHaveTextContent('10:visible')
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

  it('does not terminate playback when a transient dynamic geometry update is rejected', async () => {
    installResizeObserver()
    const harness = videoBridgeHarness()
    harness.open.mockResolvedValue(session(7))
    harness.setRect.mockRejectedValue(new Error('surface unavailable'))
    render(<BridgeHarness file={video('a.mp4')} bridge={harness.bridge} />)

    await waitFor(() => expect(harness.setRect).toHaveBeenCalledOnce())
    act(() => harness.emit({ type: 'firstFrameReady', generation: 7 }))
    expect(screen.getByRole('status', { name: 'bridge state' })).toHaveTextContent('7:visible')
    expect(harness.close).not.toHaveBeenCalled()
    expect(harness.unlisten).not.toHaveBeenCalled()
  })

  it('binds every playback command to the active generation and normalizes bounded values', async () => {
    installResizeObserver()
    const harness = videoBridgeHarness()
    harness.open.mockResolvedValue(session(8))
    render(<CommandBridgeHarness file={video('a.mp4')} bridge={harness.bridge} />)
    await waitFor(() =>
      expect(screen.getByRole('status', { name: 'command state' })).toHaveTextContent('8'),
    )

    for (const name of [
      'play',
      'pause',
      'backward',
      'preview seek',
      'seek',
      'volume',
      'muted',
      'rate',
      'fullscreen',
      'thumbnail',
    ]) {
      fireEvent.click(screen.getByRole('button', { name }))
    }

    await waitFor(() =>
      expect(harness.setFullscreen).toHaveBeenCalledWith({ generation: 8, fullscreen: true }),
    )
    expect(harness.play).toHaveBeenCalledWith({ generation: 8 })
    expect(harness.pause).toHaveBeenCalledWith({ generation: 8 })
    expect(harness.step).toHaveBeenCalledWith({ generation: 8, direction: 'backward' })
    expect(harness.seek.mock.calls).toEqual([
      [{ generation: 8, requestId: 1, timeUs: 12_000_000, intent: 'preview' }],
      [{ generation: 8, requestId: 2, timeUs: 12_000_000, intent: 'commit' }],
    ])
    expect(harness.setVolume).toHaveBeenCalledWith({ generation: 8, volumePercent: 100 })
    expect(harness.setMuted).toHaveBeenCalledWith({ generation: 8, muted: true })
    expect(harness.setRate).toHaveBeenCalledWith({ generation: 8, rate: 'one_and_half' })
    expect(harness.requestThumbnail).toHaveBeenCalledWith({
      generation: 8,
      requestId: 'thumbnail-request',
      timeUs: 0,
    })
  })

  it('publishes only active-generation settings, fullscreen, and timeline artifacts', async () => {
    installResizeObserver()
    const harness = videoBridgeHarness()
    harness.open.mockResolvedValue(session(4))
    render(<CommandBridgeHarness file={video('a.mp4')} bridge={harness.bridge} />)
    await waitFor(() =>
      expect(screen.getByRole('status', { name: 'command state' })).toHaveTextContent('4'),
    )

    act(() => {
      harness.emit({
        type: 'settingsChanged',
        generation: 3,
        volumePercent: 20,
        muted: true,
        rate: 2,
      })
      harness.emit({ type: 'fullscreenChanged', generation: 3, fullscreen: true })
      harness.emit({
        type: 'timelineThumbnailReady',
        generation: 3,
        requestId: 'stale',
        bucketUs: 500_000,
        artifactUrl: 'viewer-image://localhost/stale',
      })
    })
    expect(screen.getByRole('status', { name: 'control state' })).toHaveTextContent(
      '100:false:1:false:none',
    )

    act(() => {
      harness.emit({
        type: 'settingsChanged',
        generation: 4,
        volumePercent: 64,
        muted: true,
        rate: 1.25,
      })
      harness.emit({ type: 'fullscreenChanged', generation: 4, fullscreen: true })
      harness.emit({
        type: 'timelineThumbnailReady',
        generation: 4,
        requestId: 'ready',
        bucketUs: 1_000_000,
        artifactUrl: 'viewer-image://localhost/ready',
      })
    })
    expect(screen.getByRole('status', { name: 'control state' })).toHaveTextContent(
      '64:true:1.25:true:ready',
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

function CommandBridgeHarness({ bridge, file }: { bridge: VideoPreviewBridge; file: VideoFile }) {
  const stage = useRef<HTMLDivElement>(null)
  const { state, controlState, commands } = useVideoBridge({ bridge, file, retryKey: 0, stage })
  return (
    <>
      <div ref={stage} />
      <output aria-label="command state">{state.generation}</output>
      <output aria-label="control state">
        {controlState.volumePercent}:{String(controlState.muted)}:{controlState.rate}:
        {String(controlState.fullscreen)}:{controlState.timelineThumbnail?.requestId ?? 'none'}
      </output>
      <button type="button" aria-label="play" onClick={() => void commands.play()} />
      <button type="button" aria-label="pause" onClick={() => void commands.pause()} />
      <button type="button" aria-label="backward" onClick={() => void commands.step('backward')} />
      <button
        type="button"
        aria-label="preview seek"
        onClick={() => void commands.previewSeek(99_000_000)}
      />
      <button type="button" aria-label="seek" onClick={() => void commands.seek(99_000_000)} />
      <button type="button" aria-label="volume" onClick={() => void commands.setVolume(160)} />
      <button type="button" aria-label="muted" onClick={() => void commands.setMuted(true)} />
      <button type="button" aria-label="rate" onClick={() => void commands.setRate(1.5)} />
      <button
        type="button"
        aria-label="fullscreen"
        onClick={() => void commands.setFullscreen(true)}
      />
      <button
        type="button"
        aria-label="thumbnail"
        onClick={() =>
          void commands.requestThumbnail({ requestId: 'thumbnail-request', timeUs: -10 })
        }
      />
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
  const play = vi.fn<ViewerBridge['videoPlay']>().mockResolvedValue(undefined)
  const pause = vi.fn<ViewerBridge['videoPause']>().mockResolvedValue(undefined)
  const seek = vi.fn<ViewerBridge['videoSeek']>().mockResolvedValue(undefined)
  const step = vi.fn<ViewerBridge['videoStep']>().mockResolvedValue(undefined)
  const setVolume = vi.fn<ViewerBridge['videoSetVolume']>().mockResolvedValue(undefined)
  const setMuted = vi.fn<ViewerBridge['videoSetMuted']>().mockResolvedValue(undefined)
  const setRate = vi.fn<ViewerBridge['videoSetRate']>().mockResolvedValue(undefined)
  const setFullscreen = vi.fn<ViewerBridge['videoSetFullscreen']>().mockResolvedValue(undefined)
  const requestThumbnail = vi
    .fn<ViewerBridge['videoRequestThumbnail']>()
    .mockResolvedValue(undefined)
  const unlisten = vi.fn()
  const listen = vi.fn<ViewerBridge['listenVideo']>(async (listener) => {
    order.push('listen')
    listeners.push(listener)
    return unlisten
  })
  const bridge: VideoPreviewBridge = {
    videoOpen: open,
    videoCancelOpen: vi.fn().mockResolvedValue(true),
    videoClose: close,
    videoPlay: play,
    videoPause: pause,
    videoSeek: seek,
    videoStep: step,
    videoSetVolume: setVolume,
    videoSetMuted: setMuted,
    videoSetRate: setRate,
    videoSetSurfaceRect: setRect,
    videoSetFullscreen: setFullscreen,
    videoRequestThumbnail: requestThumbnail,
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
    pause,
    play,
    requestThumbnail,
    seek,
    setRect,
    setFullscreen,
    setMuted,
    setRate,
    setVolume,
    step,
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

function installAnimationFrameHarness() {
  let nextId = 1
  const callbacks = new Map<number, FrameRequestCallback>()
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
    const id = nextId++
    callbacks.set(id, callback)
    return id
  })
  vi.stubGlobal('cancelAnimationFrame', (id: number) => {
    callbacks.delete(id)
  })
  return {
    flush() {
      const pending = [...callbacks.entries()]
      callbacks.clear()
      act(() => {
        for (const [, callback] of pending) callback(performance.now())
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
