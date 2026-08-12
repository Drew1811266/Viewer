import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { VideoEvent, VideoFile, VideoSession } from '../api/types'
import type { ViewerBridge } from '../api/viewer'
import VideoPreview from './VideoPreview'
import type { VideoPreviewBridge } from './videoPreview/useVideoBridge'

beforeEach(() => {
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue(rect(100, 50, 800, 600))
  vi.stubGlobal(
    'ResizeObserver',
    class {
      observe() {}
      disconnect() {}
    },
  )
})

afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe('VideoPreview', () => {
  it('keeps the native surface gated until the active generation first frame', async () => {
    const harness = videoBridgeHarness()
    render(<VideoPreview file={video('a.mp4')} files={[video('a.mp4')]} bridge={harness.bridge} />)

    expect(screen.getByRole('status', { name: '正在加载视频' })).toBeVisible()
    expect(screen.queryByRole('img')).not.toBeInTheDocument()
    await waitFor(() => expect(harness.open).toHaveBeenCalledTimes(1))

    harness.emit({ type: 'firstFrameReady', generation: 1 })

    await waitFor(() =>
      expect(screen.queryByRole('status', { name: '正在加载视频' })).not.toBeInTheDocument(),
    )
    expect(harness.setRect).toHaveBeenCalledTimes(1)
    expect(screen.getByRole('dialog', { name: '视频预览 a.mp4' })).toHaveAttribute(
      'data-surface-visible',
      'true',
    )
    expect(screen.getByTestId('video-preview-stage')).toHaveAttribute(
      'data-surface-visible',
      'true',
    )
  })

  it('keeps the shell and navigation on failure and retries with a new lifecycle', async () => {
    const harness = videoBridgeHarness()
    harness.open.mockResolvedValueOnce(session(3)).mockResolvedValueOnce(session(4))
    const onClose = vi.fn()
    render(
      <VideoPreview
        file={video('a.mp4')}
        files={[video('a.mp4'), video('b.mp4')]}
        bridge={harness.bridge}
        onClose={onClose}
      />,
    )
    await waitFor(() => expect(harness.open).toHaveBeenCalledTimes(1))

    harness.emit({
      type: 'failed',
      generation: 3,
      error: { code: 'decode_failed', retryable: true },
    })

    expect(await screen.findByRole('alert')).toHaveTextContent('无法播放这个视频')
    expect(screen.getByRole('navigation', { name: '视频导航' })).toBeVisible()
    expect(screen.getByRole('button', { name: '重试' })).toBeVisible()
    fireEvent.click(screen.getByRole('button', { name: '重试' }))
    await waitFor(() => expect(harness.open).toHaveBeenCalledTimes(2))
    expect(harness.close).toHaveBeenCalledWith({ generation: 3 })
    expect(screen.getByRole('status', { name: '正在加载视频' })).toBeVisible()

    fireEvent.click(screen.getByRole('button', { name: '关闭预览' }))
    expect(onClose).toHaveBeenCalledOnce()
  })

  it('navigates only within its supplied video neighbors', () => {
    const harness = videoBridgeHarness()
    const first = video('a.mp4')
    const second = video('b.mp4')
    const onNavigate = vi.fn()
    render(
      <VideoPreview
        file={second}
        files={[first, second]}
        bridge={harness.bridge}
        onNavigate={onNavigate}
      />,
    )

    const navigation = screen.getByRole('navigation', { name: '视频导航' })
    expect(navigation).toHaveTextContent('2 / 2')
    expect(within(navigation).getByRole('button', { name: '下一个视频' })).toBeDisabled()
    fireEvent.click(within(navigation).getByRole('button', { name: '上一个视频' }))
    expect(onNavigate).toHaveBeenCalledWith(first)
  })

  it('pauses before stepping when ArrowRight is pressed while playing', async () => {
    const harness = videoBridgeHarness()
    render(<VideoPreview file={video('a.mp4')} files={[video('a.mp4')]} bridge={harness.bridge} />)
    await waitFor(() => expect(harness.open).toHaveBeenCalledOnce())
    harness.emit({ type: 'stateChanged', generation: 1, state: 'playing' })
    await screen.findByRole('button', { name: '暂停' })

    fireEvent.keyDown(window, { key: 'ArrowRight' })

    await waitFor(() => expect(harness.order).toEqual(['videoPause', 'videoStep:forward']))
  })

  it('exits fullscreen first and closes only after fullscreen has ended', async () => {
    const harness = videoBridgeHarness()
    const onClose = vi.fn()
    render(
      <VideoPreview
        file={video('a.mp4')}
        files={[video('a.mp4')]}
        bridge={harness.bridge}
        onClose={onClose}
      />,
    )
    await waitFor(() => expect(harness.open).toHaveBeenCalledOnce())
    harness.emit({ type: 'fullscreenChanged', generation: 1, fullscreen: true })
    await screen.findByRole('button', { name: '退出全屏' })

    fireEvent.keyDown(window, { key: 'Escape' })
    await waitFor(() =>
      expect(harness.setFullscreen).toHaveBeenCalledWith({ generation: 1, fullscreen: false }),
    )
    expect(onClose).not.toHaveBeenCalled()

    harness.emit({ type: 'fullscreenChanged', generation: 1, fullscreen: false })
    await screen.findByRole('button', { name: '进入全屏' })
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(onClose).toHaveBeenCalledOnce()
  })

  it('renders ended playback paused on the final frame without advancing', async () => {
    const harness = videoBridgeHarness()
    const onNavigate = vi.fn()
    render(
      <VideoPreview
        file={video('a.mp4')}
        files={[video('a.mp4'), video('b.mp4')]}
        bridge={harness.bridge}
        onNavigate={onNavigate}
      />,
    )
    await waitFor(() => expect(harness.open).toHaveBeenCalledOnce())
    harness.emit({ type: 'firstFrameReady', generation: 1 })
    harness.emit({
      type: 'progress',
      generation: 1,
      timeUs: 11_000_000,
      durationUs: 12_000_000,
    })
    harness.emit({ type: 'ended', generation: 1 })

    expect(await screen.findByRole('button', { name: '播放' })).toBeVisible()
    expect(screen.getByRole('slider', { name: '视频时间轴' })).toHaveAttribute(
      'aria-valuenow',
      '12',
    )
    expect(onNavigate).not.toHaveBeenCalled()
  })

  it('serializes rapid control intents and preserves every frame step', async () => {
    const harness = videoBridgeHarness()
    render(<VideoPreview file={video('a.mp4')} files={[video('a.mp4')]} bridge={harness.bridge} />)
    await waitFor(() => expect(harness.open).toHaveBeenCalledOnce())
    harness.emit({ type: 'stateChanged', generation: 1, state: 'playing' })
    await screen.findByRole('button', { name: '暂停' })

    fireEvent.keyDown(window, { key: ' ' })
    fireEvent.keyDown(window, { key: ' ' })
    fireEvent.keyDown(window, { key: 'm' })
    fireEvent.keyDown(window, { key: 'm' })
    fireEvent.keyDown(window, { key: 'f' })
    fireEvent.keyDown(window, { key: 'f' })
    fireEvent.keyDown(window, { key: 'ArrowRight' })
    fireEvent.keyDown(window, { key: 'ArrowRight' })

    await waitFor(() =>
      expect(harness.order).toEqual([
        'videoPause',
        'videoPlay',
        'videoMuted:true',
        'videoMuted:false',
        'videoFullscreen:true',
        'videoFullscreen:false',
        'videoPause',
        'videoStep:forward',
        'videoStep:forward',
      ]),
    )
  })
})

function video(name: string): VideoFile {
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
      durationUs: 12_000_000,
      displayWidth: 1_920,
      displayHeight: 1_080,
      rotationDegrees: 0,
      frameRateMillihertz: 30_000,
      videoCodec: 'h264',
      audioCodec: 'aac',
      probeStatus: 'ready',
      failureKind: null,
      coverUrl: 'viewer-image://localhost/session/cover',
    },
  }
}

function session(generation: number): VideoSession {
  return {
    generation,
    sessionId: `session-${generation}`,
    media: {
      durationUs: 12_000_000,
      displayWidth: 1_920,
      displayHeight: 1_080,
      rotationDegrees: 0,
    },
  }
}

function videoBridgeHarness() {
  const listeners: Array<(event: VideoEvent) => void> = []
  const order: string[] = []
  const open = vi.fn<ViewerBridge['videoOpen']>().mockResolvedValue(session(1))
  const close = vi.fn<ViewerBridge['videoClose']>().mockResolvedValue(undefined)
  const setRect = vi.fn<ViewerBridge['videoSetSurfaceRect']>().mockResolvedValue(undefined)
  const play = vi.fn<ViewerBridge['videoPlay']>(async () => {
    order.push('videoPlay')
  })
  const pause = vi.fn<ViewerBridge['videoPause']>(async () => {
    order.push('videoPause')
  })
  const seek = vi.fn<ViewerBridge['videoSeek']>().mockResolvedValue(undefined)
  const step = vi.fn<ViewerBridge['videoStep']>(async ({ direction }) => {
    order.push(`videoStep:${direction}`)
  })
  const setVolume = vi.fn<ViewerBridge['videoSetVolume']>().mockResolvedValue(undefined)
  const setMuted = vi.fn<ViewerBridge['videoSetMuted']>(async ({ muted }) => {
    order.push(`videoMuted:${muted}`)
  })
  const setRate = vi.fn<ViewerBridge['videoSetRate']>().mockResolvedValue(undefined)
  const setFullscreen = vi.fn<ViewerBridge['videoSetFullscreen']>(async ({ fullscreen }) => {
    order.push(`videoFullscreen:${fullscreen}`)
  })
  const requestThumbnail = vi
    .fn<ViewerBridge['videoRequestThumbnail']>()
    .mockResolvedValue(undefined)
  const bridge: VideoPreviewBridge = {
    videoOpen: open,
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
    listenVideo: vi.fn(async (listener) => {
      listeners.push(listener)
      return () => undefined
    }),
  }
  return {
    bridge,
    close,
    emit(event: VideoEvent) {
      listeners.at(-1)?.(event)
    },
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
  }
}

function rect(left: number, top: number, width: number, height: number): DOMRect {
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
