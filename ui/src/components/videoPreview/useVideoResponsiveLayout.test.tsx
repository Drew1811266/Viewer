import { act, renderHook } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { useVideoResponsiveLayout, VIDEO_COMPACT_QUERY } from './useVideoResponsiveLayout'

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('useVideoResponsiveLayout', () => {
  it('tracks the compact player boundary when the viewport changes', () => {
    let matches = false
    const listeners = new Set<(event: MediaQueryListEvent) => void>()
    const query = {
      get matches() {
        return matches
      },
      media: VIDEO_COMPACT_QUERY,
      onchange: null,
      addEventListener: (_type: string, listener: (event: MediaQueryListEvent) => void) => {
        listeners.add(listener)
      },
      removeEventListener: (_type: string, listener: (event: MediaQueryListEvent) => void) => {
        listeners.delete(listener)
      },
      addListener: vi.fn(),
      removeListener: vi.fn(),
      dispatchEvent: vi.fn(),
    } as MediaQueryList
    vi.stubGlobal(
      'matchMedia',
      vi.fn((requested: string) => {
        expect(requested).toBe('(max-width: 1099px)')
        return query
      }),
    )

    const { result, unmount } = renderHook(() => useVideoResponsiveLayout())
    expect(result.current).toEqual({ compact: false })

    act(() => {
      matches = true
      const event = { matches: true, media: VIDEO_COMPACT_QUERY } as MediaQueryListEvent
      for (const listener of listeners) listener(event)
    })
    expect(result.current).toEqual({ compact: true })

    unmount()
    expect(listeners.size).toBe(0)
  })
})
