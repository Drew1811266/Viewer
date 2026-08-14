import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { VideoEvent } from '../../api/types'
import VideoTimeline from './VideoTimeline'

const DURATION_US = 10_000_000

beforeEach(() => {
  vi.useFakeTimers()
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue(rect(0, 0, 200, 20))
})

afterEach(() => {
  vi.useRealTimers()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe('VideoTimeline', () => {
  it('clamps the preview and renders only the matching generation, request, and bucket', () => {
    const onRequestThumbnail = vi.fn().mockResolvedValue(undefined)
    const rendered = render(
      <TimelineHarness onRequestThumbnail={onRequestThumbnail} thumbnail={null} />,
    )
    const slider = screen.getByRole('slider', { name: '视频时间轴' })

    fireEvent.pointerMove(slider, { clientX: 198 })
    expect(screen.getByTestId('timeline-thumbnail')).toHaveStyle({ right: '0px' })
    expect(screen.getByTestId('timeline-thumbnail-pending')).toBeVisible()
    act(() => vi.advanceTimersByTime(80))

    const request = onRequestThumbnail.mock.calls[0]?.[0]
    expect(request).toEqual({ requestId: expect.any(String), timeUs: 9_500_000 })

    rendered.rerender(
      <TimelineHarness
        onRequestThumbnail={onRequestThumbnail}
        thumbnail={readyThumbnail({ requestId: 'old' })}
      />,
    )
    expect(screen.queryByRole('img', { name: '00:09.5 预览' })).not.toBeInTheDocument()

    rendered.rerender(
      <TimelineHarness
        onRequestThumbnail={onRequestThumbnail}
        thumbnail={readyThumbnail({ generation: 1, requestId: request.requestId })}
      />,
    )
    expect(screen.queryByRole('img', { name: '00:09.5 预览' })).not.toBeInTheDocument()

    rendered.rerender(
      <TimelineHarness
        onRequestThumbnail={onRequestThumbnail}
        thumbnail={readyThumbnail({ generation: 2, requestId: request.requestId })}
      />,
    )
    expect(screen.getByRole('img', { name: '00:09.5 预览' })).toHaveAttribute(
      'src',
      'viewer-image://localhost/session/timeline',
    )
  })

  it('keeps an accepted matching frame when a stale event arrives afterward', () => {
    const onRequestThumbnail = vi.fn().mockResolvedValue(undefined)
    const rendered = render(
      <TimelineHarness onRequestThumbnail={onRequestThumbnail} thumbnail={null} />,
    )
    const slider = screen.getByRole('slider', { name: '视频时间轴' })
    fireEvent.pointerMove(slider, { clientX: 198 })
    act(() => vi.advanceTimersByTime(80))
    const requestId = onRequestThumbnail.mock.calls[0]?.[0].requestId

    rendered.rerender(
      <TimelineHarness
        onRequestThumbnail={onRequestThumbnail}
        thumbnail={readyThumbnail({ requestId })}
      />,
    )
    expect(screen.getByRole('img', { name: '00:09.5 预览' })).toBeVisible()

    rendered.rerender(
      <TimelineHarness
        onRequestThumbnail={onRequestThumbnail}
        thumbnail={readyThumbnail({ requestId: 'stale', bucketUs: 9_000_000 })}
      />,
    )
    expect(screen.getByRole('img', { name: '00:09.5 预览' })).toBeVisible()
  })

  it('clamps pointer time, debounces thumbnail work for 80 ms, and changes buckets', () => {
    const onRequestThumbnail = vi.fn().mockResolvedValue(undefined)
    render(<TimelineHarness onRequestThumbnail={onRequestThumbnail} thumbnail={null} />)
    const slider = screen.getByRole('slider', { name: '视频时间轴' })

    fireEvent.pointerMove(slider, { clientX: -20 })
    fireEvent.pointerMove(slider, { clientX: 76 })
    act(() => vi.advanceTimersByTime(79))
    expect(onRequestThumbnail).not.toHaveBeenCalled()
    act(() => vi.advanceTimersByTime(1))
    expect(onRequestThumbnail).toHaveBeenCalledOnce()
    expect(onRequestThumbnail).toHaveBeenLastCalledWith({
      requestId: expect.any(String),
      timeUs: 3_500_000,
    })

    fireEvent.pointerMove(slider, { clientX: 240 })
    act(() => vi.advanceTimersByTime(80))
    expect(onRequestThumbnail).toHaveBeenLastCalledWith({
      requestId: expect.any(String),
      timeUs: 10_000_000,
    })
  })

  it('previews only the latest drag position per frame and commits exactly once on release', () => {
    let frame: FrameRequestCallback | null = null
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      frame = callback
      return 19
    })
    vi.stubGlobal('cancelAnimationFrame', vi.fn())
    const onPreviewSeek = vi.fn().mockResolvedValue(undefined)
    const onCommitSeek = vi.fn().mockResolvedValue(undefined)
    const onSeekingChange = vi.fn()
    const rendered = render(
      <TimelineHarness
        onPreviewSeek={onPreviewSeek}
        onCommitSeek={onCommitSeek}
        onSeekingChange={onSeekingChange}
        thumbnail={null}
      />,
    )
    const slider = screen.getByRole('slider', { name: '视频时间轴' })
    Object.defineProperty(slider, 'setPointerCapture', { configurable: true, value: vi.fn() })
    Object.defineProperty(slider, 'releasePointerCapture', { configurable: true, value: vi.fn() })

    fireEvent.pointerDown(slider, { pointerId: 7, clientX: 20 })
    fireEvent.pointerMove(slider, { pointerId: 7, clientX: 40 })
    fireEvent.pointerMove(slider, { pointerId: 7, clientX: 60 })

    expect(slider).toHaveAttribute('aria-valuenow', '3')
    expect(onPreviewSeek).not.toHaveBeenCalled()
    expect(onCommitSeek).not.toHaveBeenCalled()
    act(() => frame?.(0))
    expect(onPreviewSeek.mock.calls).toEqual([[3_000_000]])
    expect(onCommitSeek).not.toHaveBeenCalled()

    fireEvent.pointerUp(slider, { pointerId: 7, clientX: 60 })
    expect(onCommitSeek.mock.calls).toEqual([[3_000_000]])
    expect(onPreviewSeek).toHaveBeenCalledTimes(1)
    expect(onSeekingChange.mock.calls).toEqual([[true], [false]])
    expect(slider).toHaveAttribute('aria-valuenow', '3')

    rendered.rerender(
      <TimelineHarness
        timeUs={3_000_000}
        onPreviewSeek={onPreviewSeek}
        onCommitSeek={onCommitSeek}
        onSeekingChange={onSeekingChange}
        thumbnail={null}
      />,
    )
    rendered.rerender(
      <TimelineHarness
        timeUs={4_000_000}
        onPreviewSeek={onPreviewSeek}
        onCommitSeek={onCommitSeek}
        onSeekingChange={onSeekingChange}
        thumbnail={null}
      />,
    )
    expect(slider).toHaveAttribute('aria-valuenow', '4')
  })

  it('does not carry seek coalescing state into a new generation', () => {
    vi.stubGlobal(
      'requestAnimationFrame',
      vi.fn(() => 23),
    )
    vi.stubGlobal('cancelAnimationFrame', vi.fn())
    const onPreviewSeek = vi.fn().mockResolvedValue(undefined)
    const onCommitSeek = vi.fn().mockResolvedValue(undefined)
    const rendered = render(
      <TimelineHarness
        generation={2}
        onPreviewSeek={onPreviewSeek}
        onCommitSeek={onCommitSeek}
        thumbnail={null}
      />,
    )
    const slider = screen.getByRole('slider', { name: '视频时间轴' })
    Object.defineProperty(slider, 'setPointerCapture', { configurable: true, value: vi.fn() })
    Object.defineProperty(slider, 'releasePointerCapture', { configurable: true, value: vi.fn() })

    fireEvent.pointerDown(slider, { pointerId: 7, clientX: 60 })
    fireEvent.pointerUp(slider, { pointerId: 7, clientX: 60 })
    expect(onCommitSeek).toHaveBeenCalledTimes(1)
    expect(onCommitSeek).toHaveBeenLastCalledWith(3_000_000)

    rendered.rerender(
      <TimelineHarness
        generation={3}
        onPreviewSeek={onPreviewSeek}
        onCommitSeek={onCommitSeek}
        thumbnail={null}
      />,
    )
    fireEvent.pointerDown(slider, { pointerId: 8, clientX: 60 })
    fireEvent.pointerUp(slider, { pointerId: 8, clientX: 60 })
    expect(onCommitSeek).toHaveBeenCalledTimes(2)
    expect(onCommitSeek).toHaveBeenLastCalledWith(3_000_000)
    expect(onPreviewSeek).not.toHaveBeenCalled()
  })

  it('uses commit-only seeking for keyboard interaction', () => {
    const onPreviewSeek = vi.fn().mockResolvedValue(undefined)
    const onCommitSeek = vi.fn().mockResolvedValue(undefined)
    render(
      <TimelineHarness
        timeUs={5_000_000}
        onPreviewSeek={onPreviewSeek}
        onCommitSeek={onCommitSeek}
        thumbnail={null}
      />,
    )
    const slider = screen.getByRole('slider', { name: '视频时间轴' })

    fireEvent.keyDown(slider, { key: 'Home' })
    fireEvent.keyDown(slider, { key: 'End' })
    fireEvent.keyDown(slider, { key: 'ArrowLeft' })

    expect(onPreviewSeek).not.toHaveBeenCalled()
    expect(onCommitSeek.mock.calls).toEqual([[0], [10_000_000], [4_000_000]])
  })

  it('exposes microsecond-derived slider values and formatted current time without a live region', () => {
    render(<TimelineHarness timeUs={1_250_000} thumbnail={null} />)

    const slider = screen.getByRole('slider', { name: '视频时间轴' })
    expect(slider).toHaveAttribute('aria-valuemin', '0')
    expect(slider).toHaveAttribute('aria-valuemax', '10')
    expect(slider).toHaveAttribute('aria-valuenow', '1.25')
    expect(slider).toHaveAttribute('aria-valuetext', '00:01.2 / 00:10')
    expect(screen.queryByRole('status')).not.toBeInTheDocument()
  })
})

function TimelineHarness({
  generation = 2,
  timeUs = 0,
  thumbnail,
  onPreviewSeek = vi.fn().mockResolvedValue(undefined),
  onCommitSeek = vi.fn().mockResolvedValue(undefined),
  onRequestThumbnail = vi.fn().mockResolvedValue(undefined),
  onSeekingChange = vi.fn(),
}: {
  generation?: number
  timeUs?: number
  thumbnail: Extract<VideoEvent, { type: 'timelineThumbnailReady' }> | null
  onPreviewSeek?: (timeUs: number) => Promise<void>
  onCommitSeek?: (timeUs: number) => Promise<void>
  onRequestThumbnail?: (request: { requestId: string; timeUs: number }) => Promise<void>
  onSeekingChange?: (seeking: boolean) => void
}) {
  return (
    <VideoTimeline
      generation={generation}
      durationUs={DURATION_US}
      timeUs={timeUs}
      thumbnail={thumbnail}
      onPreviewSeek={onPreviewSeek}
      onCommitSeek={onCommitSeek}
      onRequestThumbnail={onRequestThumbnail}
      onSeekingChange={onSeekingChange}
      onActivity={() => undefined}
    />
  )
}

function readyThumbnail(
  overrides: Partial<Extract<VideoEvent, { type: 'timelineThumbnailReady' }>>,
): Extract<VideoEvent, { type: 'timelineThumbnailReady' }> {
  return {
    type: 'timelineThumbnailReady',
    generation: 2,
    requestId: 'request',
    bucketUs: 9_500_000,
    artifactUrl: 'viewer-image://localhost/session/timeline',
    ...overrides,
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
