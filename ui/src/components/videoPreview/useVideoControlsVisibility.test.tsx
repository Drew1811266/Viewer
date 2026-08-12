import { act, renderHook } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  useVideoControlsVisibility,
  type VideoControlsVisibilityOptions,
} from './useVideoControlsVisibility'

afterEach(() => {
  vi.useRealTimers()
})

describe('useVideoControlsVisibility', () => {
  it('hides after 2.5 seconds of playing idle and reveals on owned activity', () => {
    vi.useFakeTimers()
    const { result } = renderHook(() =>
      useVideoControlsVisibility({
        playing: true,
        seeking: false,
        adjusting: false,
        focusedWithin: false,
        error: null,
        reducedMotion: false,
      }),
    )

    act(() => vi.advanceTimersByTime(2_499))
    expect(result.current.visible).toBe(true)
    act(() => vi.advanceTimersByTime(1))
    expect(result.current.visible).toBe(false)

    act(() => result.current.reveal())
    expect(result.current.visible).toBe(true)
    act(() => vi.advanceTimersByTime(2_500))
    expect(result.current.visible).toBe(false)
  })

  it('does not hide while paused, seeking, adjusting, focused, or showing an error', () => {
    vi.useFakeTimers()
    const initial: VideoControlsVisibilityOptions = {
      playing: true,
      seeking: false,
      adjusting: false,
      focusedWithin: false,
      error: null,
      reducedMotion: false,
    }
    const { result, rerender } = renderHook((props) => useVideoControlsVisibility(props), {
      initialProps: initial,
    })

    act(() => vi.advanceTimersByTime(2_500))
    expect(result.current.visible).toBe(false)

    rerender({ ...initial, playing: false })
    expect(result.current.visible).toBe(true)
    act(() => vi.advanceTimersByTime(10_000))
    expect(result.current.visible).toBe(true)

    rerender({ ...initial, seeking: true })
    act(() => vi.advanceTimersByTime(10_000))
    expect(result.current.visible).toBe(true)

    rerender({ ...initial, adjusting: true })
    act(() => vi.advanceTimersByTime(10_000))
    expect(result.current.visible).toBe(true)

    rerender({ ...initial, focusedWithin: true })
    act(() => vi.advanceTimersByTime(10_000))
    expect(result.current.visible).toBe(true)

    rerender({ ...initial, error: { code: 'decode_failed', retryable: true } })
    act(() => vi.advanceTimersByTime(10_000))
    expect(result.current.visible).toBe(true)
  })

  it('keeps reduced-motion visibility changes immediate', () => {
    vi.useFakeTimers()
    const { result, rerender } = renderHook((props) => useVideoControlsVisibility(props), {
      initialProps: {
        playing: true,
        seeking: false,
        adjusting: false,
        focusedWithin: false,
        error: null,
        reducedMotion: true,
      },
    })

    act(() => vi.advanceTimersByTime(2_500))
    expect(result.current.visible).toBe(false)
    rerender({
      playing: false,
      seeking: false,
      adjusting: false,
      focusedWithin: false,
      error: null,
      reducedMotion: true,
    })
    expect(result.current.visible).toBe(true)
  })
})
