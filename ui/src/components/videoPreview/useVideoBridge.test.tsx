import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
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
    })
  })

  it('ignores late events from the previous file and closes each opened generation once', async () => {
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

  it('keeps the shell retryable when videoOpen rejects', async () => {
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

  it('binds every playback command to the active generation and normalizes bounded values', async () => {
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
  const { state } = useVideoBridge({ bridge, file, retryKey })
  return (
    <output aria-label="bridge state">
      {state.generation}:{state.surfaceVisible ? 'visible' : 'hidden'}
      {state.phase === 'failed' ? `:failed:${state.error?.code}` : ''}
    </output>
  )
}

function CommandBridgeHarness({ bridge, file }: { bridge: VideoPreviewBridge; file: VideoFile }) {
  const { state, controlState, commands } = useVideoBridge({ bridge, file, retryKey: 0 })
  return (
    <>
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
    videoSetFullscreen: setFullscreen,
    videoRequestCover: vi.fn().mockResolvedValue('viewer-image://localhost/session/cover'),
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
    setFullscreen,
    setMuted,
    setRate,
    setVolume,
    step,
    unlisten,
  }
}
