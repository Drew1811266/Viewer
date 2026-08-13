import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
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
  it.each([
    ['unsupported', '不支持此视频的容器或编码'],
    ['damaged', '视频已损坏或无法读取'],
    ['missing', '视频文件已移动或删除'],
    ['engine_initialization', '视频引擎无法启动'],
    ['decode_fallback_failed', '硬件与软件解码均失败'],
    ['render_surface', '视频显示区域无法创建'],
  ])('maps %s to a concise retryable state', async (code, message) => {
    const harness = videoBridgeHarness()
    const onClose = vi.fn()
    render(
      <VideoPreview
        file={video('a.mp4')}
        files={[video('a.mp4'), video('b.mp4')]}
        bridge={harness.bridge}
        onClose={onClose}
      />,
    )
    await waitFor(() => expect(harness.open).toHaveBeenCalledOnce())

    harness.emit({ type: 'failed', generation: 1, error: { code, retryable: true } })

    const alert = await screen.findByRole('alert')
    expect(alert).toHaveTextContent(message)
    expect(alert).not.toHaveTextContent('/Users/private/Videos/a.mp4')
    expect(screen.getByRole('button', { name: '重试' })).toBeEnabled()
    expect(screen.getByRole('button', { name: '完成' })).toBeEnabled()
    expect(screen.getByRole('navigation', { name: '视频导航' })).toBeVisible()

    fireEvent.keyDown(window, { key: 'Escape' })
    expect(onClose).toHaveBeenCalledOnce()
  })

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

    fireEvent.click(screen.getByRole('button', { name: '完成' }))
    expect(onClose).toHaveBeenCalledOnce()
  })

  it('retries an indexed terminal failure by opening the same entity', async () => {
    const harness = videoBridgeHarness()
    const authoritativeMedia = session(1).media
    harness.open.mockResolvedValue({
      generation: 1,
      sessionId: 'session-1',
      media: {
        durationUs: null,
        displayWidth: null,
        displayHeight: null,
        rotationDegrees: 0,
      },
    })
    const damaged = video('damaged.mp4')
    damaged.videoMetadata = {
      ...damaged.videoMetadata,
      durationUs: null,
      displayWidth: null,
      displayHeight: null,
      videoCodec: null,
      audioCodec: null,
      probeStatus: 'failed',
      failureKind: 'damaged',
      coverUrl: null,
    }
    render(<VideoPreview file={damaged} files={[damaged]} bridge={harness.bridge} />)

    expect(await screen.findByRole('alert')).toHaveTextContent('视频已损坏或无法读取')
    expect(harness.open).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: '重试' }))

    await waitFor(() =>
      expect(harness.open).toHaveBeenCalledWith({
        attemptId: expect.any(String),
        entityId: damaged.entityId,
        surfaceRect: { x: 100, y: 50, width: 800, height: 600 },
      }),
    )
    expect(screen.getByRole('status', { name: '正在加载视频' })).toBeVisible()
    await act(async () => {
      harness.emit({ type: 'firstFrameReady', generation: 1 })
      await Promise.resolve()
    })
    expect(screen.getByRole('status', { name: '正在加载视频' })).toBeVisible()
    expect(harness.setRect).not.toHaveBeenCalled()
    await act(async () => {
      harness.emit({ type: 'prepared', generation: 1, media: authoritativeMedia })
      await Promise.resolve()
    })

    await waitFor(() =>
      expect(harness.setRect).toHaveBeenCalledWith({
        generation: 1,
        x: 100,
        y: 125,
        width: 800,
        height: 450,
      }),
    )
    await waitFor(() =>
      expect(screen.queryByRole('status', { name: '正在加载视频' })).not.toBeInTheDocument(),
    )
  })

  it('cancels an unresolved indexed retry when Done unmounts the preview', async () => {
    let finishOpen: (session: VideoSession) => void = () => undefined
    const harness = videoBridgeHarness()
    harness.open.mockImplementation(
      () =>
        new Promise<VideoSession>((resolve) => {
          finishOpen = resolve
        }),
    )
    const damaged = video('damaged.mp4')
    damaged.videoMetadata = {
      ...damaged.videoMetadata,
      displayWidth: null,
      displayHeight: null,
      probeStatus: 'failed',
      failureKind: 'damaged',
    }
    const onClose = vi.fn()
    const preview = render(
      <VideoPreview file={damaged} files={[damaged]} bridge={harness.bridge} onClose={onClose} />,
    )

    fireEvent.click(await screen.findByRole('button', { name: '重试' }))
    await waitFor(() => expect(harness.open).toHaveBeenCalledOnce())
    const attemptId = harness.open.mock.calls[0]?.[0].attemptId
    fireEvent.click(screen.getByRole('button', { name: '完成' }))
    preview.unmount()

    expect(onClose).toHaveBeenCalledOnce()
    await waitFor(() => expect(harness.cancelOpen).toHaveBeenCalledWith({ attemptId }))
    await act(async () => {
      finishOpen(session(1))
      await Promise.resolve()
    })
    expect(harness.close).toHaveBeenCalledWith({ generation: 1 })
  })

  it('waits for the failed generation to close before reopening the same entity', async () => {
    let finishClose: () => void = () => undefined
    const closeFinished = new Promise<void>((resolve) => {
      finishClose = resolve
    })
    const harness = videoBridgeHarness()
    const order: string[] = []
    harness.open.mockImplementation(async ({ entityId }) => {
      order.push(`open:${entityId}`)
      return session(harness.open.mock.calls.length)
    })
    harness.close.mockImplementation(async ({ generation }) => {
      order.push(`close:${generation}:start`)
      await closeFinished
      order.push(`close:${generation}:end`)
    })
    render(<VideoPreview file={video('a.mp4')} files={[video('a.mp4')]} bridge={harness.bridge} />)
    await waitFor(() => expect(harness.open).toHaveBeenCalledOnce())
    harness.emit({
      type: 'failed',
      generation: 1,
      error: { code: 'damaged', retryable: true },
    })

    fireEvent.click(await screen.findByRole('button', { name: '重试' }))

    await waitFor(() => expect(harness.close).toHaveBeenCalledWith({ generation: 1 }))
    expect(harness.open).toHaveBeenCalledOnce()
    finishClose()
    await waitFor(() => expect(harness.open).toHaveBeenCalledTimes(2))
    expect(harness.open).toHaveBeenNthCalledWith(2, {
      attemptId: expect.any(String),
      entityId: 'video-a.mp4',
      surfaceRect: { x: 100, y: 125, width: 800, height: 450 },
    })
    expect(order).toEqual(['open:video-a.mp4', 'close:1:start', 'close:1:end', 'open:video-a.mp4'])
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
  const cancelOpen = vi.fn<ViewerBridge['videoCancelOpen']>().mockResolvedValue(true)
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
    videoCancelOpen: cancelOpen,
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
    cancelOpen,
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
