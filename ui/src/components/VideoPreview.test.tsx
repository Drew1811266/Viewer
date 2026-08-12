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
  const open = vi.fn<ViewerBridge['videoOpen']>().mockResolvedValue(session(1))
  const close = vi.fn<ViewerBridge['videoClose']>().mockResolvedValue(undefined)
  const setRect = vi.fn<ViewerBridge['videoSetSurfaceRect']>().mockResolvedValue(undefined)
  const bridge: VideoPreviewBridge = {
    videoOpen: open,
    videoClose: close,
    videoSetSurfaceRect: setRect,
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
    setRect,
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
